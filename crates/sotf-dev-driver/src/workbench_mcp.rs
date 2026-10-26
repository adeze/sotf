//! Streamable HTTP MCP adapter for the live app's authenticated workbench service.
//! Does not own project state, generate filters or activate a chat/model provider.

use axum::{
    Router,
    extract::{Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::Response,
};
use clap::Parser;
use rmcp::{
    ErrorData, ServerHandler, ServiceExt,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde::Deserialize;
use sotf_dev_api::{
    RUN_ID_HEADER, RunId,
    workbench::{DesignerPanel, DesignerPatch, WorkbenchRequest},
};
use std::{net::SocketAddr, time::Duration};

const MAX_RESPONSE_BYTES: usize = 512 * 1024;

#[derive(Parser)]
#[command(
    about = "Expose current SOTF workbench state and offline operations over Streamable HTTP MCP"
)]
struct Args {
    /// Address printed by the app's --workbench-api listener; loopback only.
    #[arg(long)]
    endpoint: SocketAddr,
    /// Loopback MCP listener; connect clients to http://ADDRESS/mcp.
    #[arg(long, default_value = "127.0.0.1:9411")]
    listen: SocketAddr,
    /// Compatibility transport for hosts that launch a child process.
    #[arg(long)]
    stdio: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RevisionInput {
    /// Exact current revision returned by inspect_workbench.
    expected_revision: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct UpdateInput {
    /// Exact current revision returned by inspect_workbench.
    expected_revision: String,
    patch: DesignerPatch,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct PanelInput {
    expected_revision: String,
    panel: DesignerPanel,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct MeasurementInput {
    expected_revision: String,
    /// Zero-based index from presentation.measurements in inspect_workbench.
    #[schemars(range(max = 127))]
    index: usize,
}

#[derive(Clone)]
struct WorkbenchMcp {
    client: reqwest::Client,
    endpoint: String,
    run_id: RunId,
}

#[tool_router]
impl WorkbenchMcp {
    fn new(endpoint: SocketAddr, run_id: RunId) -> anyhow::Result<Self> {
        anyhow::ensure!(
            endpoint.ip().is_loopback() && endpoint.port() != 0,
            "workbench endpoint must be loopback with a nonzero port"
        );
        Ok(Self {
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(10))
                .build()?,
            endpoint: format!("http://{endpoint}/workbench"),
            run_id,
        })
    }

    async fn request(&self, request: WorkbenchRequest) -> Result<CallToolResult, ErrorData> {
        let mut response = self
            .client
            .post(&self.endpoint)
            .header(RUN_ID_HEADER, self.run_id.as_str())
            .json(&request)
            .send()
            .await
            .map_err(|_| ErrorData::internal_error("workbench connection failed", None))?;
        if !response.status().is_success() && response.status() != reqwest::StatusCode::BAD_REQUEST
        {
            return Err(ErrorData::internal_error(
                format!("workbench HTTP status {}", response.status().as_u16()),
                None,
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| ErrorData::internal_error("workbench response read failed", None))?
        {
            if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err(ErrorData::internal_error(
                    "workbench response exceeds limit",
                    None,
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let envelope: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| ErrorData::internal_error("invalid workbench response", None))?;
        match envelope.get("ok").and_then(serde_json::Value::as_bool) {
            Some(true) => envelope
                .get("value")
                .cloned()
                .map(CallToolResult::structured)
                .ok_or_else(|| ErrorData::internal_error("missing workbench snapshot", None)),
            Some(false) => Ok(CallToolResult::error(vec![ContentBlock::text(
                envelope
                    .get("error")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("workbench command failed")
                    .to_owned(),
            )])),
            None => Err(ErrorData::internal_error(
                "invalid workbench envelope",
                None,
            )),
        }
    }

    #[tool(
        description = "Inspect the running app's current project, Room EQ Designer parameters, revision and offline study status. No independent or synthetic project state.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn inspect_workbench(&self) -> Result<CallToolResult, ErrorData> {
        self.request(WorkbenchRequest::Inspect {}).await
    }

    #[tool(
        description = "Open an actual calibration Designer panel in the running app using its UI selection handler. Requires current input revision and the calibration-designer feature. Changes presentation only; Advice does not enable chat.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn select_designer_panel(
        &self,
        Parameters(input): Parameters<PanelInput>,
    ) -> Result<CallToolResult, ErrorData> {
        self.request(WorkbenchRequest::SelectDesignerPanel {
            expected_revision: input.expected_revision,
            panel: input.panel,
        })
        .await
    }

    #[tool(
        description = "Select an existing measurement preview from inspect_workbench in the embedded Designer Data panel. Uses the same selection as the UI and returns its display samples; no measurement acquisition, smoothing or filter mutation.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn select_measurement(
        &self,
        Parameters(input): Parameters<MeasurementInput>,
    ) -> Result<CallToolResult, ErrorData> {
        self.request(WorkbenchRequest::SelectMeasurement {
            expected_revision: input.expected_revision,
            index: input.index,
        })
        .await
    }

    #[tool(
        description = "Atomically change supported Room EQ draft parameters through the UI's validated command. Requires current revision; does not apply DSP, play audio, save files or transfer to hardware.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn update_designer_parameters(
        &self,
        Parameters(input): Parameters<UpdateInput>,
    ) -> Result<CallToolResult, ErrorData> {
        self.request(WorkbenchRequest::UpdateDesigner {
            expected_revision: input.expected_revision,
            patch: input.patch,
        })
        .await
    }

    #[tool(
        description = "Start the same offline Room EQ calculation as the UI using the current measurements and Designer configuration. Requires current revision and usable inputs; inspect_workbench polls progress/results. Does not install filters or operate hardware.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn run_offline_study(
        &self,
        Parameters(input): Parameters<RevisionInput>,
    ) -> Result<CallToolResult, ErrorData> {
        self.request(WorkbenchRequest::RunStudy {
            expected_revision: input.expected_revision,
        })
        .await
    }

    #[tool(
        description = "Request cancellation of the current offline Room EQ study using the UI's cancellation path. Requires current revision; inspect status until the backend acknowledges cancellation.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn cancel_offline_study(
        &self,
        Parameters(input): Parameters<RevisionInput>,
    ) -> Result<CallToolResult, ErrorData> {
        self.request(WorkbenchRequest::CancelStudy {
            expected_revision: input.expected_revision,
        })
        .await
    }
}

#[tool_handler]
impl ServerHandler for WorkbenchMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions("Operate the current SOTF project through shared UI commands. Inspect before editing; use returned revisions and re-inspect after stale-state errors. Results are offline predictions, not installed calibration or acoustic proof. Chat and model providers remain disabled.")
    }
}

fn validate_listener(address: SocketAddr) -> anyhow::Result<()> {
    anyhow::ensure!(address.ip().is_loopback(), "MCP listener must be loopback");
    Ok(())
}

async fn authenticate(
    State(run_id): State<RunId>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let mut authorization = request.headers().get_all(AUTHORIZATION).iter();
    let token = authorization
        .next()
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if authorization.next().is_some() || !token.is_some_and(|token| run_id.authenticate(token)) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(request).await)
}

fn http_router(service: WorkbenchMcp, address: SocketAddr) -> Router {
    let credential = service.run_id.clone();
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_max_request_body_bytes(16 * 1024)
        .with_allowed_hosts([address.to_string(), format!("localhost:{}", address.port())])
        .with_allowed_origins([
            format!("http://{address}"),
            format!("http://localhost:{}", address.port()),
        ])
        .enforce_origin_validation();
    let transport = StreamableHttpService::new(
        move || Ok(service.clone()),
        LocalSessionManager::default().into(),
        config,
    );
    Router::new()
        .nest_service("/mcp", transport)
        .layer(middleware::from_fn_with_state(credential, authenticate))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let run_id = RunId::parse(
        std::env::var("SOTF_WORKBENCH_RUN_ID")
            .map_err(|_| anyhow::anyhow!("SOTF_WORKBENCH_RUN_ID is required"))?,
    )?;
    let service = WorkbenchMcp::new(args.endpoint, run_id)?;
    if args.stdio {
        service
            .serve(rmcp::transport::stdio())
            .await?
            .waiting()
            .await?;
    } else {
        validate_listener(args.listen)?;
        let listener = tokio::net::TcpListener::bind(args.listen).await?;
        let address = listener.local_addr()?;
        eprintln!("SOTF workbench MCP: http://{address}/mcp (bearer authentication required)");
        axum::serve(listener, http_router(service, address))
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn endpoint_and_input_schema_are_narrow() {
        let run_id = RunId::parse("workbench-test-0123456789").unwrap();
        for address in ["192.0.2.1:9000", "127.0.0.1:0"] {
            assert!(WorkbenchMcp::new(address.parse().unwrap(), run_id.clone()).is_err());
        }
        assert!(validate_listener("0.0.0.0:9411".parse().unwrap()).is_err());
        assert!(validate_listener("127.0.0.1:0".parse().unwrap()).is_ok());
        assert!(
            serde_json::from_value::<UpdateInput>(serde_json::json!({
                "expected_revision":"v1", "patch":{"unknown":3}
            }))
            .is_err()
        );
        let tools = WorkbenchMcp::tool_router().list_all();
        assert_eq!(tools.len(), 6);
        assert!(tools.iter().any(|tool| tool.name == "inspect_workbench"));
        assert!(
            !tools
                .iter()
                .any(|tool| tool.name.contains("transfer") || tool.name.contains("action"))
        );
    }

    #[tokio::test]
    async fn mcp_discovery_and_calls_reach_authenticated_shared_commands() {
        use rmcp::model::CallToolRequestParams;
        use rmcp::transport::{
            StreamableHttpClientTransport,
            streamable_http_client::StreamableHttpClientTransportConfig,
        };
        use sotf_dev_api::{
            Capabilities, HttpResponse,
            server::{ServerConfig, start_server},
        };

        let run_id = RunId::parse("workbench-mcp-test-0123456789").unwrap();
        let backend = start_server(
            ServerConfig::loopback(run_id.clone(), Capabilities::new("test", "workbench")),
            |request: sotf_dev_api::HttpRequest, _context| {
                assert_eq!(request.path, "/workbench");
                let command: WorkbenchRequest = serde_json::from_slice(&request.body).unwrap();
                let (status, value) = match command {
                    WorkbenchRequest::Inspect {} => (
                        200,
                        serde_json::json!({
                            "ok":true,"value":{"revision":"r1","chat_enabled":false}
                        }),
                    ),
                    WorkbenchRequest::UpdateDesigner {
                        expected_revision,
                        patch,
                    } => {
                        assert_eq!(expected_revision, "r1");
                        assert_eq!(patch.max_cut_db, Some(6.0));
                        (
                            400,
                            serde_json::json!({"ok":false,"error":"Stale workbench revision"}),
                        )
                    }
                    WorkbenchRequest::RunStudy { expected_revision } => {
                        assert_eq!(expected_revision, "r1");
                        (
                            200,
                            serde_json::json!({"ok":true,"value":{"status":"Running"}}),
                        )
                    }
                    WorkbenchRequest::CancelStudy { expected_revision } => {
                        assert_eq!(expected_revision, "r1");
                        (
                            200,
                            serde_json::json!({"ok":true,"value":{"status":"Cancelled"}}),
                        )
                    }
                    WorkbenchRequest::SelectDesignerPanel {
                        expected_revision,
                        panel,
                    } => {
                        assert_eq!(expected_revision, "r1");
                        assert_eq!(panel, DesignerPanel::Goals);
                        (
                            200,
                            serde_json::json!({"ok":true,"value":{"panel":"goals"}}),
                        )
                    }
                    WorkbenchRequest::SelectMeasurement {
                        expected_revision,
                        index,
                    } => {
                        assert_eq!(expected_revision, "r1");
                        assert_eq!(index, 1);
                        (
                            200,
                            serde_json::json!({"ok":true,"value":{"selected_measurement":1}}),
                        )
                    }
                };
                HttpResponse::json(status, value.to_string())
            },
        )
        .unwrap();
        let service = WorkbenchMcp::new(backend.endpoint(), run_id.clone()).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, http_router(service, address))
                .await
                .unwrap();
        });
        let transport = StreamableHttpClientTransport::with_client(
            reqwest::Client::builder().no_proxy().build().unwrap(),
            StreamableHttpClientTransportConfig::with_uri(format!("http://{address}/mcp"))
                .auth_header(run_id.as_str()),
        );
        let client = ().serve(transport).await.unwrap();
        assert_eq!(client.list_all_tools().await.unwrap().len(), 6);
        let inspected = client
            .call_tool(CallToolRequestParams::new("inspect_workbench"))
            .await
            .unwrap();
        assert_eq!(inspected.structured_content.unwrap()["chat_enabled"], false);
        for (tool, arguments, key, expected) in [
            (
                "select_designer_panel",
                serde_json::json!({"expected_revision":"r1","panel":"goals"}),
                "panel",
                serde_json::json!("goals"),
            ),
            (
                "select_measurement",
                serde_json::json!({"expected_revision":"r1","index":1}),
                "selected_measurement",
                serde_json::json!(1),
            ),
        ] {
            let result = client
                .call_tool(
                    CallToolRequestParams::new(tool)
                        .with_arguments(arguments.as_object().unwrap().clone()),
                )
                .await
                .unwrap();
            assert_eq!(result.structured_content.unwrap()[key], expected);
        }
        let update = client
            .call_tool(
                CallToolRequestParams::new("update_designer_parameters").with_arguments(
                    serde_json::json!({"expected_revision":"r1","patch":{"max_cut_db":6}})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
            )
            .await
            .unwrap();
        assert_eq!(update.is_error, Some(true));
        assert!(
            serde_json::to_string(&update.content)
                .unwrap()
                .contains("Stale workbench revision")
        );
        for (tool, status) in [
            ("run_offline_study", "Running"),
            ("cancel_offline_study", "Cancelled"),
        ] {
            let result = client
                .call_tool(
                    CallToolRequestParams::new(tool).with_arguments(
                        serde_json::json!({"expected_revision":"r1"})
                            .as_object()
                            .unwrap()
                            .clone(),
                    ),
                )
                .await
                .unwrap();
            assert_eq!(result.structured_content.unwrap()["status"], status);
        }
        client.cancel().await.unwrap();
        server.abort();
        let _ = server.await;
        backend.shutdown();
    }

    #[tokio::test]
    async fn http_rejects_unauthenticated_foreign_and_oversized_requests() {
        let run_id = RunId::parse("workbench-http-test-0123456789").unwrap();
        let service = WorkbenchMcp::new("127.0.0.1:9".parse().unwrap(), run_id.clone()).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, http_router(service, address))
                .await
                .unwrap();
        });
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let url = format!("http://{address}/mcp");
        for token in [None, Some("wrong-token")] {
            let mut request = client.post(&url);
            if let Some(token) = token {
                request = request.bearer_auth(token);
            }
            assert_eq!(
                request.send().await.unwrap().status(),
                StatusCode::UNAUTHORIZED
            );
        }
        for (name, value) in [
            ("Origin", "https://foreign.invalid"),
            ("Host", "foreign.invalid"),
        ] {
            assert_eq!(
                client
                    .post(&url)
                    .bearer_auth(run_id.as_str())
                    .header(name, value)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                StatusCode::FORBIDDEN
            );
        }
        assert_eq!(
            client
                .post(&url)
                .bearer_auth(run_id.as_str())
                .header("Authorization", "Bearer duplicate")
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            client
                .post(&url)
                .bearer_auth(run_id.as_str())
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .body("x".repeat(16 * 1024 + 1))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        server.abort();
        let _ = server.await;
    }
}
