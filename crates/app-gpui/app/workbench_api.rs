//! Explicitly enabled offline capability service over the live UI state.
use crate::{app::types::RoomEqState, ui::PlayerView};
use anyhow::{Result, anyhow};
use gpui::{AnyWindowHandle, App, AsyncApp, Context, Task};
use serde_json::{Value, json};
use sotf_dev_api::server::{ServerConfig, start_server};
#[cfg(feature = "calibration-designer")]
use sotf_dev_api::workbench::DesignerPanel;
use sotf_dev_api::workbench::WorkbenchRequest;
use sotf_dev_api::{Capabilities, HttpResponse, Method, RunId};
use std::{
    net::SocketAddr,
    sync::mpsc,
    time::{Duration, Instant},
};

type Reply = mpsc::SyncSender<std::result::Result<Value, String>>;

/// No service starts unless both the CLI opt-in and valid local run ID exist.
pub fn start(cx: &mut App, port: u16, window: AnyWindowHandle) -> Result<()> {
    let run_id = RunId::parse(std::env::var("SOTF_WORKBENCH_RUN_ID")?)?;
    start_for_testing(cx, port, window, run_id)?.task.detach();
    Ok(())
}

/// An owned service: dropping it cancels the GPUI task and closes its listener.
#[doc(hidden)]
pub struct WorkbenchApiHandle {
    endpoint: SocketAddr,
    task: Task<()>,
}

impl WorkbenchApiHandle {
    pub fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }
}

/// Start the production service with an explicit run ID and owned lifetime.
#[doc(hidden)]
pub fn start_for_testing(
    cx: &mut App,
    port: u16,
    window: AnyWindowHandle,
    run_id: RunId,
) -> Result<WorkbenchApiHandle> {
    let mut capabilities = Capabilities::new("workbench", "sotf-desktop");
    capabilities.limits.command_body_bytes = 8192;
    capabilities.limits.command_queue = 8;
    let mut config = ServerConfig::loopback(run_id, capabilities);
    config.bind.set_port(port);
    let (tx, rx) = mpsc::sync_channel::<(WorkbenchRequest, Reply, Instant)>(8);
    let server = start_server(
        config,
        move |request: sotf_dev_api::HttpRequest, _context| {
            if request.method != Method::Post || request.path != "/workbench" {
                return HttpResponse::json(
                    404,
                    json!({"ok":false,"error":"Unknown workbench operation"}).to_string(),
                );
            }
            let request: WorkbenchRequest = match serde_json::from_slice(&request.body) {
                Ok(request) => request,
                Err(error) => {
                    return HttpResponse::json(
                        400,
                        json!({"ok":false,"error":error.to_string()}).to_string(),
                    );
                }
            };
            let (reply, result) = mpsc::sync_channel(1);
            if tx
                .try_send((request, reply, Instant::now() + Duration::from_secs(4)))
                .is_err()
            {
                return HttpResponse::json(
                    503,
                    json!({"ok":false,"error":"Workbench queue unavailable"}).to_string(),
                );
            }
            match result.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(value)) => HttpResponse::json(200, json!({"ok":true,"value":value}).to_string()),
            Ok(Err(error)) => HttpResponse::json(400, json!({"ok":false,"error":error}).to_string()),
            Err(_) => HttpResponse::json(503, json!({"ok":false,"error":"Workbench reply timeout; inspect state before retrying"}).to_string()),
        }
        },
    )?;
    log::info!(
        "offline workbench API listening on http://{}",
        server.endpoint()
    );
    let endpoint = server.endpoint();
    let task = cx.spawn(async move |cx: &mut AsyncApp| {
        // Own the listener and command queue for precisely this application lifetime.
        let _server = server;
        loop {
            while let Ok((request, reply, deadline)) = rx.try_recv() {
                if Instant::now() >= deadline {
                    let _ = reply.send(Err("Workbench command expired before execution".into()));
                    continue;
                }
                // A timed-out caller must not cause a late mutation.
                let result = cx
                    .update(|cx| {
                        window.update(cx, |root, _, cx| {
                            let entity = root
                                .downcast::<PlayerView>()
                                .map_err(|_| anyhow!("Window does not contain the workbench"))?;
                            entity.update(cx, |view, cx| execute(view, request, cx))
                        })?
                    })
                    .map_err(|error| format!("{error:#}"));
                let _ = reply.send(result);
            }
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
        }
    });
    Ok(WorkbenchApiHandle { endpoint, task })
}

fn revision(room: &RoomEqState) -> Result<String> {
    Ok(sotf_dev_api::canonical_json_hash(
        &room.optimization_input_snapshot(),
    )?)
}

#[cfg(feature = "calibration-designer")]
fn designer_step(panel: DesignerPanel) -> sotf_calibration_workbench::DesignerStep {
    use sotf_calibration_workbench::DesignerStep;
    match panel {
        DesignerPanel::Data => DesignerStep::Data,
        DesignerPanel::Checks => DesignerStep::Checks,
        DesignerPanel::Goals => DesignerStep::Goals,
        DesignerPanel::Constraints => DesignerStep::Constraints,
        DesignerPanel::Advice => DesignerStep::Advice,
        DesignerPanel::Study => DesignerStep::Study,
    }
}

#[cfg(feature = "calibration-designer")]
fn open_designer(
    view: &mut PlayerView,
    panel: DesignerPanel,
    cx: &mut Context<PlayerView>,
) -> Result<()> {
    anyhow::ensure!(
        view.state
            .read(cx)
            .app
            .is_screen_available(&crate::Screen::RoomEq),
        "Room EQ workspace is unavailable in this application profile"
    );
    view.state.update(cx, |state, cx| {
        state.app.set_screen(crate::Screen::RoomEq, "workbench-mcp");
        let room = &mut state.app.measurement_state.room_eq_state;
        room.step = crate::RoomEqStep::Configure;
        room.configure_designer_selected = true;
        cx.notify();
    });
    view.calibration_designer(cx).update(cx, |designer, cx| {
        designer.select_step(designer_step(panel), cx);
    });
    Ok(())
}

fn presentation(room: &RoomEqState, cx: &gpui::App) -> Value {
    #[cfg(feature = "calibration-designer")]
    {
        use sotf_calibration_workbench::{DesignerStep, measurement_preview::MeasurementPreviews};
        let previews = MeasurementPreviews::from_model(&room.model);
        let designer = room
            .calibration_designer
            .as_ref()
            .map(|entity| entity.read(cx));
        let selected = designer
            .and_then(|d| d.selected_measurement_index())
            .filter(|_| !previews.rows.is_empty() && previews.magnitude_range_db.is_some())
            .map(|index| index.min(previews.rows.len() - 1));
        let panel = designer.map(|d| match d.selected_step() {
            DesignerStep::Data => DesignerPanel::Data,
            DesignerStep::Checks => DesignerPanel::Checks,
            DesignerStep::Goals => DesignerPanel::Goals,
            DesignerStep::Constraints => DesignerPanel::Constraints,
            DesignerStep::Advice => DesignerPanel::Advice,
            DesignerStep::Study => DesignerPanel::Study,
        });
        json!({
            "designer_available": true,
            "designer_selected": room.configure_designer_selected,
            "room_eq_step": format!("{:?}", room.step),
            "selected_panel": panel,
            "panels": ["data", "checks", "goals", "constraints", "advice", "study"],
            "selected_measurement": selected,
            "measurements": previews.rows.iter().enumerate().map(|(index, row)| json!({
                "index": index, "label": row.label, "response_available": row.points.len() >= 2,
                "display_sample_count": row.points.len()
            })).collect::<Vec<_>>(),
            "omitted_measurements": previews.omitted_rows,
            "selected_response": selected.and_then(|i| previews.rows.get(i)).map(|row| json!({
                "label": row.label, "samples_hz_db": row.points,
                "projection": "source extrema in 64 logarithmic bins; display only; no smoothing",
                "frequency_range_hz": [20.0, 20000.0],
                "magnitude_range_db": previews.magnitude_range_db
            }))
        })
    }
    #[cfg(not(feature = "calibration-designer"))]
    {
        let _ = (room, cx);
        json!({"designer_available":false,"panels":[],"measurements":[],
            "selected_panel":null,"selected_measurement":null,"selected_response":null,
            "unavailable_reason":"Build with the calibration-designer feature"})
    }
}

fn execute(
    view: &mut PlayerView,
    request: WorkbenchRequest,
    cx: &mut Context<PlayerView>,
) -> Result<Value> {
    if let Some(expected) = match &request {
        WorkbenchRequest::Inspect {} => None,
        WorkbenchRequest::UpdateDesigner {
            expected_revision, ..
        }
        | WorkbenchRequest::SelectDesignerPanel {
            expected_revision, ..
        }
        | WorkbenchRequest::SelectMeasurement {
            expected_revision, ..
        }
        | WorkbenchRequest::RunStudy { expected_revision }
        | WorkbenchRequest::CancelStudy { expected_revision } => Some(expected_revision),
    } {
        let state = view.state.read(cx);
        if expected != &revision(&state.app.measurement_state.room_eq_state)? {
            return Err(anyhow!(
                "Stale workbench revision; inspect the current project before retrying"
            ));
        }
    }
    match request {
        WorkbenchRequest::Inspect {} => {}
        WorkbenchRequest::SelectDesignerPanel { panel, .. } => {
            #[cfg(feature = "calibration-designer")]
            open_designer(view, panel, cx)?;
            #[cfg(not(feature = "calibration-designer"))]
            {
                let _ = panel;
                return Err(anyhow!(
                    "Calibration Designer unavailable; build with calibration-designer"
                ));
            }
        }
        WorkbenchRequest::SelectMeasurement { index, .. } => {
            #[cfg(feature = "calibration-designer")]
            {
                let previews = {
                    let state = view.state.read(cx);
                    sotf_calibration_workbench::measurement_preview::MeasurementPreviews::from_model(
                        &state.app.measurement_state.room_eq_state.model,
                    )
                };
                if previews.magnitude_range_db.is_none() || index >= previews.rows.len() {
                    return Err(anyhow!(
                        "Measurement preview index unavailable; inspect current measurements"
                    ));
                }
                open_designer(view, DesignerPanel::Data, cx)?;
                view.calibration_designer(cx).update(cx, |designer, cx| {
                    designer.select_measurement(index, cx);
                });
            }
            #[cfg(not(feature = "calibration-designer"))]
            {
                let _ = index;
                return Err(anyhow!(
                    "Calibration Designer unavailable; build with calibration-designer"
                ));
            }
        }
        WorkbenchRequest::UpdateDesigner { patch, .. } => {
            if patch.max_boost_db.is_some_and(|v| v < 0.0)
                || patch.max_cut_db.is_some_and(|v| v < 0.0)
            {
                return Err(anyhow!(
                    "Boost and cut limits must be nonnegative magnitudes"
                ));
            }
            view.state.update(cx, |state, cx| {
                let room = &mut state.app.measurement_state.room_eq_state;
                room.update_design_limits(
                    patch.min_freq,
                    patch.max_freq,
                    patch.max_cut_db.map(|v| -v),
                    patch.max_boost_db,
                )
                .map_err(|error| anyhow!(error))?;
                cx.notify();
                Ok::<_, anyhow::Error>(())
            })?;
        }
        WorkbenchRequest::RunStudy { .. } => {
            let room = &view.state.read(cx).app.measurement_state.room_eq_state;
            if room.channel_measurements.is_empty() {
                return Err(anyhow!("No measurements loaded in the current project"));
            }
            if room.is_optimizing() {
                return Err(anyhow!("A workbench study is already running"));
            }
            view.start_room_eq_optimization(cx);
            let room = &view.state.read(cx).app.measurement_state.room_eq_state;
            if !room.is_optimizing() {
                return Err(anyhow!(
                    "{}",
                    room.error_message
                        .as_deref()
                        .unwrap_or("Study did not start")
                ));
            }
        }
        WorkbenchRequest::CancelStudy { .. } => {
            if !view
                .state
                .read(cx)
                .app
                .measurement_state
                .room_eq_state
                .is_optimizing()
            {
                return Err(anyhow!("No workbench study is running"));
            }
            view.cancel_room_eq_optimization(cx);
        }
    }
    cx.notify();
    let state = view.state.read(cx);
    let room = &state.app.measurement_state.room_eq_state;
    let recording = &state.app.measurement_state.recording_state;
    let selected_recording_result = recording
        .channel_recordings
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            recording
                .plot_selected_channel
                .is_none_or(|selected| selected == *index)
        })
        .find_map(|(_, channel)| channel.result.as_ref());
    let recording_waterfall_availability = selected_recording_result
        .map(PlayerView::recording_waterfall_availability)
        .unwrap_or_else(|| Err("No selected recording result".to_owned()));
    let recording_spectrogram_available = selected_recording_result.is_some_and(|result| {
        result.sample_rate_hz.is_some_and(|rate| rate > 0)
            && result.spectrogram_db.as_ref().is_some_and(|grid| {
                grid.first().is_some_and(|row| {
                    !row.is_empty()
                        && grid.iter().all(|other| {
                            other.len() == row.len() && other.iter().all(|value| value.is_finite())
                        })
                })
            })
    });
    Ok(json!({
        "revision": revision(room)?,
        "project": {
            "config_directory": sotf_audio_player::config::get_app_config_dir(),
            "data_source": format!("{:?}", room.data_source),
            "screen": format!("{:?}", state.app.ui_state.current_screen),
            "measurement_count": room.channel_measurements.len(),
            "channels": room.channel_measurements.iter().map(|m| &m.channel_name).collect::<Vec<_>>()
        },
        "config": room.optimizer_config,
        "designer": {
            "min_freq": room.optimizer_config.min_freq,
            "max_freq": room.optimizer_config.max_freq,
            "max_boost_db": room.optimizer_config.max_db,
            "max_cut_db": -room.optimizer_config.min_db,
            "wizard_mode": format!("{:?}", room.wizard_mode)
        },
        "presentation": presentation(room, cx),
        "recording_presentation": {
            "selected_channel": recording.plot_selected_channel,
            "waterfall_source_eligible": recording_waterfall_availability.is_ok(),
            "waterfall_unavailable_reason": recording_waterfall_availability.err(),
            "waterfall_grid_inspected": false,
            "waterfall_controls_available": false
        },
        "capabilities": {
            "commands": if cfg!(feature = "calibration-designer") {
                vec!["inspect", "select_designer_panel", "select_measurement", "update_designer", "run_study", "cancel_study"]
            } else { vec!["inspect", "update_designer", "run_study", "cancel_study"] },
            "display_selection_changes_filters": false,
            "waterfall": {
                "data_present": null,
                "designer_control_available": false,
                "unavailable_reason": "Designer results do not expose waterfall grids; inspect recording_presentation for source eligibility"
            },
            "recording_spectrogram": {
                "selected_result_available": recording_spectrogram_available,
                "designer_control_available": false,
                "unavailable_reason": "Recording spectrogram exists separately; Designer selection is not connected"
            }
        },
        "study": {
            "status": format!("{:?}", room.optimization_status),
            "progress": room.overall_progress,
            "iteration": room.current_iteration,
            "message": room.status_message,
            "error": room.error_message,
            "result_count": room.channel_results.len(),
            "results_current": room.result_is_current(),
            "artifact_directory": room.artifact_dir
        },
        "chat_enabled": false,
        "offline_only": true
    }))
}
