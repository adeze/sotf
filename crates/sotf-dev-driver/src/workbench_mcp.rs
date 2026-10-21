//! Stdio MCP adapter for the live app's narrow authenticated workbench service.
//! Does not own project state, generate filters or activate a chat/model provider.

use clap::Parser;
use rmcp::{
    ErrorData, ServerHandler, ServiceExt,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
};
use serde::Deserialize;
use sotf_dev_api::{
    RUN_ID_HEADER, RunId,
    workbench::{DesignerPatch, WorkbenchRequest},
};
use std::{net::SocketAddr, time::Duration};

const MAX_RESPONSE_BYTES: usize = 512 * 1024;

#[derive(Parser)]
#[command(about = "Expose current SOTF workbench state and offline operations over stdio MCP")]
struct Args {
    /// Address printed by the app's --workbench-api listener; loopback only.
    #[arg(long)]
    endpoint: SocketAddr,
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let run_id = RunId::parse(
        std::env::var("SOTF_WORKBENCH_RUN_ID")
            .map_err(|_| anyhow::anyhow!("SOTF_WORKBENCH_RUN_ID is required"))?,
    )?;
    WorkbenchMcp::new(args.endpoint, run_id)?
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
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
        assert!(
            serde_json::from_value::<UpdateInput>(serde_json::json!({
                "expected_revision":"v1", "patch":{"unknown":3}
            }))
            .is_err()
        );
        let tools = WorkbenchMcp::tool_router().list_all();
        assert_eq!(tools.len(), 4);
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
                    _ => unreachable!(),
                };
                HttpResponse::json(status, value.to_string())
            },
        )
        .unwrap();
        let service = WorkbenchMcp::new(backend.endpoint(), run_id).unwrap();
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let server = tokio::spawn(async move {
            service
                .serve(server_io)
                .await
                .unwrap()
                .waiting()
                .await
                .unwrap()
        });
        let client = ().serve(client_io).await.unwrap();
        assert_eq!(client.list_all_tools().await.unwrap().len(), 4);
        let inspected = client
            .call_tool(CallToolRequestParams::new("inspect_workbench"))
            .await
            .unwrap();
        assert_eq!(inspected.structured_content.unwrap()["chat_enabled"], false);
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
        client.cancel().await.unwrap();
        server.await.unwrap();
        backend.shutdown();
    }
}
