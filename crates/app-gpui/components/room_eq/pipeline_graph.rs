//! Read-only projection of RoomEQ engine stages, not a second workflow executor.
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::workflow::{
    Position, WorkflowCanvas, WorkflowGraph, WorkflowNodeData, WorkflowTheme,
};
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, Text, ThemeExt};
use sotf_audio_player::autoeq::{PipelineStepId, PipelineStepStatus};

/// Order comes from the engine; conditional stages remain visible when a run skips them.
pub fn room_eq_pipeline_graph() -> WorkflowGraph {
    let mut graph = WorkflowGraph::new();
    let mut previous = None;
    for (index, step) in PipelineStepId::ALL.iter().enumerate() {
        let row = index / 3;
        let column = if row % 2 == 0 {
            index % 3
        } else {
            2 - index % 3
        };
        let node = WorkflowNodeData::new(
            format!("{:02} {}", index + 1, step.label()),
            Position::new(16.0 + column as f32 * 206.0, 16.0 + row as f32 * 82.0),
        )
        .with_ports(
            usize::from(index > 0),
            usize::from(index + 1 < PipelineStepId::ALL.len()),
        )
        .with_size(180.0, 64.0)
        .with_max_ports(Some(1), Some(1))
        .with_user_data(serde_json::json!({"stage_index": index}));
        let id = graph.add_node(node);
        if let Some(previous) = previous {
            graph
                .add_connection(previous, 0, id, 0)
                .expect("ordered pipeline has valid edges");
        }
        previous = Some(id);
    }
    graph
}

fn stage_description(step: PipelineStepId) -> &'static str {
    match step {
        PipelineStepId::ConfigPreparation => {
            "Prepare effective configuration and measurement inputs. Simple presets are resolved when the study starts."
        }
        PipelineStepId::Validation => {
            "Validate the prepared request before optimization. Missing or incompatible inputs remain errors."
        }
        PipelineStepId::TopologyRouteSelection => {
            "Choose the topology-specific or generic channel workflow from the prepared system configuration."
        }
        PipelineStepId::TopologyWorkflowExecution => {
            "Run the selected topology workflow. This is conditional; the generic route is an alternative."
        }
        PipelineStepId::GenericChannelOptimization => {
            "Fit channel correction using the selected objective, filter model, and optimizer limits."
        }
        PipelineStepId::FirGeneration => {
            "Realize full FIR coefficients when the chosen processing mode requires them."
        }
        PipelineStepId::MixedPhaseFirGeneration => {
            "Realize the short mixed-phase FIR stage when that mode is selected."
        }
        PipelineStepId::PhaseCorrection => {
            "Apply standalone phase correction when supported and requested by the effective configuration."
        }
        PipelineStepId::TimeAlignment => {
            "Align channels using arrival information available to this Rust workflow."
        }
        PipelineStepId::SpectralAlignment => {
            "Align broad spectral balance when enabled in the effective configuration."
        }
        PipelineStepId::InterChannelTimbreMatching => {
            "Apply inter-channel timbre matching when requested by the selected workflow."
        }
        PipelineStepId::HeightChannelAlignment => {
            "Align overhead channels with role-appropriate bed references when available."
        }
        PipelineStepId::PhaseAlignment => {
            "Optimize subwoofer/main phase alignment when enabled and supported by input data."
        }
        PipelineStepId::GroupDelayOptimization => {
            "Run optional group-delay optimization when enabled in the effective configuration."
        }
        PipelineStepId::ImpulseResponseComputation => {
            "Compute pre/post impulse responses from optimization results."
        }
        PipelineStepId::ChannelMatching => {
            "Analyze and optionally correct inter-channel response deviation."
        }
        PipelineStepId::MetadataRefresh => {
            "Refresh result metadata and derived reports after processing."
        }
        PipelineStepId::SanityCheck => {
            "Check final result invariants before returning the completed study."
        }
    }
}

impl PlayerView {
    pub(super) fn render_room_eq_pipeline(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let existing = self
            .state
            .read(cx)
            .app
            .measurement_state
            .room_eq_state
            .pipeline_canvas
            .clone();
        let canvas = existing.unwrap_or_else(|| {
            let canvas = cx.new(|cx| {
                let mut canvas = WorkflowCanvas::with_graph(room_eq_pipeline_graph(), cx);
                canvas.set_read_only(true);
                canvas
            });
            cx.observe(&canvas, |_, _, cx| cx.notify()).detach();
            self.state.update(cx, |state, _| {
                state.app.measurement_state.room_eq_state.pipeline_canvas = Some(canvas.clone());
            });
            canvas
        });
        let mut graph_theme = WorkflowTheme::from_theme(&cx.theme());
        let state = self.state.read(cx);
        let room = &state.app.measurement_state.room_eq_state;
        let theme = &state.app.ui_state.theme;
        let d = Ds::from_cx(cx);
        graph_theme.canvas_background = theme.background_secondary;
        graph_theme.node_background = theme.surface;
        graph_theme.node_header = theme.background_secondary;
        graph_theme.node_text = theme.text_primary;
        graph_theme.node_border = theme.border;
        graph_theme.node_border_selected = theme.accent;
        graph_theme.connection_color = theme.text_muted;
        let selected = {
            let canvas = canvas.read(cx);
            canvas
                .selection()
                .selected_nodes
                .iter()
                .next()
                .and_then(|id| canvas.graph().nodes.get(id))
                .and_then(|node| node.user_data["stage_index"].as_u64())
                .map(|index| index as usize)
                .unwrap_or_else(|| {
                    room.current_pipeline_step
                        .and_then(|step| {
                            PipelineStepId::ALL
                                .iter()
                                .position(|candidate| *candidate == step)
                        })
                        .unwrap_or(0)
                })
        };
        let step = PipelineStepId::ALL[selected];
        let stage_status = match room.step_history.get(&step) {
            Some(PipelineStepStatus::Started | PipelineStepStatus::InProgress)
                if room.is_optimizing() =>
            {
                "Running"
            }
            Some(PipelineStepStatus::Started | PipelineStepStatus::InProgress) => {
                "Stopped before completion"
            }
            Some(PipelineStepStatus::Completed) => "Completed in last study",
            Some(PipelineStepStatus::Skipped) => "Skipped in last study",
            None => "Not observed in a study",
        };
        let config = &room.optimizer_config;
        let draft_summary = match step {
            PipelineStepId::GenericChannelOptimization => format!(
                "Draft: {} · {} filters · {:.0}–{:.0} Hz · {:.1} to +{:.1} dB",
                config.algorithm,
                config.num_filters,
                config.min_freq,
                config.max_freq,
                config.min_db,
                config.max_db
            ),
            PipelineStepId::FirGeneration => format!(
                "Draft: {} · {} FIR taps · {} Hz",
                config.mode.as_str(),
                config.fir.taps,
                config.sample_rate
            ),
            PipelineStepId::MixedPhaseFirGeneration => format!(
                "Draft: {} · maximum FIR length {:.1} ms · {} Hz",
                config.mode.as_str(),
                config.mixed_phase.max_fir_length_ms,
                config.sample_rate
            ),
            PipelineStepId::PhaseAlignment => format!(
                "Draft: phase alignment {} · {:.0}–{:.0} Hz",
                if config.phase_alignment.enabled {
                    "enabled"
                } else {
                    "disabled"
                },
                config.phase_alignment.min_freq,
                config.phase_alignment.max_freq
            ),
            PipelineStepId::ChannelMatching => format!(
                "Draft: channel matching {} · threshold {:.1} dB",
                if config.channel_matching.enabled {
                    "enabled"
                } else {
                    "disabled"
                },
                config.channel_matching.threshold_db
            ),
            _ => format!(
                "{} measurement channels · draft mode {}",
                room.channel_measurements.len(),
                config.mode.as_str()
            ),
        };
        canvas.update(cx, |canvas, _| canvas.set_theme(graph_theme));
        let mut navigation = div().flex().flex_wrap().gap(d.gap);
        for (id, label, index, disabled) in [
            (
                "pipeline-previous",
                "Previous stage",
                selected.saturating_sub(1),
                selected == 0,
            ),
            (
                "pipeline-next",
                "Next stage",
                (selected + 1).min(PipelineStepId::ALL.len() - 1),
                selected + 1 == PipelineStepId::ALL.len(),
            ),
        ] {
            let canvas = canvas.clone();
            navigation = navigation.child(
                Button::new(id, label)
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .disabled(disabled)
                    .on_click(move |_, cx| {
                        canvas.update(cx, |canvas, cx| {
                            let id = canvas
                                .graph()
                                .nodes
                                .values()
                                .find(|node| {
                                    node.user_data["stage_index"].as_u64() == Some(index as u64)
                                })
                                .map(|node| node.id);
                            if let Some(id) = id {
                                canvas.select_node(id, false);
                                cx.notify();
                            }
                        });
                    }),
            );
        }
        div().id("room-eq-pipeline").flex().flex_col().gap(d.gap)
            .child(Text::section_header("Calibration pipeline"))
            .child(Text::caption("Select a stage to inspect it. Connections show engine order; conditional stages may be skipped. Edit parameters in the configuration below."))
            .child(div().h(rems(32.0)).w_full().min_w_0().rounded(d.r_md).overflow_hidden().child(canvas))
            .child(navigation)
            .child(Text::section_header(format!("{:02} · {}", selected + 1, step.label())))
            .child(Text::label(stage_status))
            .child(Text::body(stage_description(step)))
            .child(Text::caption(draft_summary))
    }
}
