//! Thin host adapter for the isolated calibration Designer workbench.
use crate::ui::PlayerView;
use gpui::*;
use sotf_calibration_workbench::desktop::Designer;
use sotf_calibration_workbench::{DesignerCommand, DesignerSnapshot};

impl PlayerView {
    /// Mount the isolated workbench through the host's shared state and commands.
    pub fn calibration_designer(&self, cx: &mut Context<Self>) -> Entity<Designer> {
        let snapshot = {
            let state = self.state.read(cx);
            let room = &state.app.measurement_state.room_eq_state;
            let mut snapshot = DesignerSnapshot::from_model(&room.model);
            snapshot.input_identity = room.optimization_input_digest();
            snapshot
        };
        let existing = self
            .state
            .read(cx)
            .app
            .measurement_state
            .room_eq_state
            .calibration_designer
            .clone();
        let designer = existing.unwrap_or_else(|| {
            let designer = cx.new(|cx| Designer::new(snapshot.clone(), cx));
            cx.subscribe(&designer, |view, _, command, cx| {
                view.handle_calibration_designer_command(*command, cx);
            })
            .detach();
            self.state.update(cx, |state, _| {
                state
                    .app
                    .measurement_state
                    .room_eq_state
                    .calibration_designer = Some(designer.clone());
            });
            designer
        });
        designer.update(cx, |designer, cx| designer.update_snapshot(snapshot, cx));
        designer
    }

    fn handle_calibration_designer_command(
        &mut self,
        command: DesignerCommand,
        cx: &mut Context<Self>,
    ) {
        let error = {
            let state = self.state.read(cx);
            let room = &state.app.measurement_state.room_eq_state;
            match command {
                DesignerCommand::RunStudy if room.channel_measurements.is_empty() => {
                    Some("No measurements loaded in the current project")
                }
                DesignerCommand::RunStudy if room.is_optimizing() => {
                    Some("A workbench study is already running")
                }
                DesignerCommand::CancelStudy if !room.is_optimizing() => {
                    Some("No workbench study is running")
                }
                DesignerCommand::MaximumCut(value) | DesignerCommand::MaximumBoost(value)
                    if !value.is_finite() || value < 0.0 =>
                {
                    Some("Boost and cut limits must be finite nonnegative magnitudes")
                }
                _ => None,
            }
        };
        if let Some(error) = error {
            self.state.update(cx, |state, cx| {
                state.app.measurement_state.room_eq_state.error_message = Some(error.into());
                cx.notify();
            });
            return;
        }
        let limits = match command {
            DesignerCommand::UseExplicitSettings => {
                self.state.update(cx, |state, cx| {
                    let room = &mut state.app.measurement_state.room_eq_state;
                    if !room.is_optimizing() {
                        room.wizard_mode = crate::app::types::room_eq::RoomEqWizardMode::Full;
                    }
                    cx.notify();
                });
                return;
            }
            DesignerCommand::OpenData
            | DesignerCommand::OpenChecks
            | DesignerCommand::OpenParameters
            | DesignerCommand::OpenResults => {
                use crate::app::types::RoomEqStep;
                self.state.update(cx, |state, cx| {
                    let room = &mut state.app.measurement_state.room_eq_state;
                    room.step = match command {
                        DesignerCommand::OpenData => RoomEqStep::LoadData,
                        DesignerCommand::OpenChecks => RoomEqStep::Delay,
                        DesignerCommand::OpenResults => RoomEqStep::Review,
                        _ => RoomEqStep::Configure,
                    };
                    if command == DesignerCommand::OpenParameters {
                        room.configure_designer_selected = false;
                    }
                    cx.notify();
                });
                return;
            }
            DesignerCommand::TargetSlope(value) | DesignerCommand::BassShelf(value) => {
                self.state.update(cx, |state, cx| {
                    let room = &mut state.app.measurement_state.room_eq_state;
                    room.error_message = room
                        .model
                        .update_design_target(
                            matches!(command, DesignerCommand::TargetSlope(_)).then_some(value),
                            matches!(command, DesignerCommand::BassShelf(_)).then_some(value),
                        )
                        .err();
                    cx.notify();
                });
                return;
            }
            DesignerCommand::RunStudy => {
                self.start_room_eq_optimization(cx);
                return;
            }
            DesignerCommand::CancelStudy => {
                self.cancel_room_eq_optimization(cx);
                return;
            }
            DesignerCommand::MinimumFrequency(value) => (Some(value), None, None, None),
            DesignerCommand::MaximumFrequency(value) => (None, Some(value), None, None),
            DesignerCommand::MaximumCut(value) => (None, None, Some(-value), None),
            DesignerCommand::MaximumBoost(value) => (None, None, None, Some(value)),
        };
        self.state.update(cx, |state, cx| {
            let room = &mut state.app.measurement_state.room_eq_state;
            room.error_message = room
                .update_design_limits(limits.0, limits.1, limits.2, limits.3)
                .err();
            cx.notify();
        });
        cx.notify();
    }
}
