//! Stepped presentation; the host owns validation, data and execution.
use crate::{DesignerCommand, DesignerSnapshot, DesignerStep, stage_description};
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, EmptyState, MetadataEntry, MetadataList, NumberInput,
    Progress, TabItem, TabVariant, Tabs, TabsTheme, Text, ThemeExt,
};

type NumericField = (
    &'static str,
    &'static str,
    &'static str,
    f64,
    (f64, f64),
    fn(f64) -> DesignerCommand,
);

pub struct Designer {
    snapshot: DesignerSnapshot,
    selected: DesignerStep,
    reviewed: bool,
    brief_copied: bool,
    selected_measurement: usize,
}
impl EventEmitter<DesignerCommand> for Designer {}
impl Designer {
    pub fn new(snapshot: DesignerSnapshot, _: &mut Context<Self>) -> Self {
        Self {
            snapshot,
            selected: DesignerStep::Data,
            reviewed: false,
            brief_copied: false,
            selected_measurement: 0,
        }
    }
    pub fn update_snapshot(&mut self, snapshot: DesignerSnapshot, cx: &mut Context<Self>) {
        if self.snapshot != snapshot {
            self.snapshot = snapshot;
            self.reviewed = false;
            self.brief_copied = false;
            cx.notify();
        }
    }
    pub fn select_step(&mut self, step: DesignerStep, cx: &mut Context<Self>) {
        self.selected = step;
        cx.notify();
    }
    pub fn selected_step(&self) -> DesignerStep {
        self.selected
    }
    /// Matches the visible selection; no selection exists when no plot can be shown.
    pub fn selected_measurement_index(&self) -> Option<usize> {
        self.snapshot.measurement_previews.magnitude_range_db?;
        (!self.snapshot.measurement_previews.rows.is_empty()).then(|| {
            self.selected_measurement
                .min(self.snapshot.measurement_previews.rows.len() - 1)
        })
    }
    /// Presentation selection never edits the draft or clears study review.
    pub fn select_measurement(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.snapshot.measurement_previews.rows.len() {
            self.selected_measurement = index;
            cx.notify();
        }
    }

    fn measurement_list(&self, cx: &Context<Self>) -> AnyElement {
        let previews = &self.snapshot.measurement_previews;
        let theme = cx.theme();
        let mut list = div().flex().flex_col().gap_2().min_w_0();
        let Some(domain) = previews.magnitude_range_db else {
            return list
                .child(Text::body(
                    "No plottable response in the visible takes (20 Hz–20 kHz).",
                ))
                .children((previews.omitted_rows > 0).then(|| {
                    Text::caption(format!(
                        "{} more takes in measurement data",
                        previews.omitted_rows
                    ))
                }))
                .into_any_element();
        };
        list = list.child(Text::caption(format!(
            "20 Hz–20 kHz · common {:.0} to {:.0} dB scale",
            domain.0, domain.1
        )));
        let selected = self.selected_measurement_index().unwrap_or(0);
        for (index, row) in previews.rows.iter().enumerate() {
            let entity = cx.entity();
            let mut item = div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .min_w_0()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(if index == selected {
                    theme.accent_muted
                } else {
                    theme.surface
                });
            item = item.child(
                Button::new(("designer-measurement", index), row.label.clone())
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |_, cx| {
                        entity.update(cx, |view, cx| view.select_measurement(index, cx))
                    }),
            );
            item = if row.points.len() >= 2 {
                item.child(
                    div()
                        .flex_1()
                        .min_w(rems(8.0))
                        .h(rems(3.0))
                        .child(crate::measurement_preview::plot(&row.points, domain, cx)),
                )
            } else {
                item.child(Text::caption("Response unavailable"))
            };
            list = list.child(item);
        }
        if previews.omitted_rows > 0 {
            list = list.child(Text::caption(format!(
                "{} more takes in measurement data",
                previews.omitted_rows
            )));
        }
        if let Some(row) = previews.rows.get(selected) {
            list = list.child(Text::section_header(row.label.clone()));
            if row.points.len() >= 2 {
                list = list
                    .child(Text::caption(format!(
                        "Magnitude (dB) · upper {:.0}, lower {:.0}",
                        domain.1, domain.0
                    )))
                    .child(
                        div()
                            .w_full()
                            .h(rems(12.0))
                            .child(crate::measurement_preview::plot(&row.points, domain, cx)),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .children(["20 Hz", "200 Hz", "2 kHz", "20 kHz"].map(Text::caption)),
                    );
            } else {
                list = list.child(Text::body("No plottable response for this take."));
            }
        }
        list.into_any_element()
    }
    fn action(
        &self,
        id: &'static str,
        label: &'static str,
        command: DesignerCommand,
        disabled: bool,
        cx: &Context<Self>,
    ) -> Button {
        let entity = cx.entity();
        Button::new(id, label)
            .size(ButtonSize::Sm)
            .variant(ButtonVariant::Secondary)
            .disabled(disabled)
            .on_click(move |_, cx| entity.update(cx, |_, cx| cx.emit(command)))
    }
    fn fields(&self, goals: bool, cx: &Context<Self>) -> AnyElement {
        let mut fields = div().flex().flex_wrap().gap_4();
        let values: Vec<NumericField> = if goals {
            vec![
                (
                    "designer-target-slope",
                    "Target slope",
                    "dB/oct",
                    self.snapshot.target_slope,
                    (-3.0, 3.0),
                    DesignerCommand::TargetSlope,
                ),
                (
                    "designer-bass-shelf",
                    "Bass preference",
                    "dB",
                    self.snapshot.bass_shelf_db,
                    (0.0, 12.0),
                    DesignerCommand::BassShelf,
                ),
            ]
        } else {
            vec![
                (
                    "designer-min-frequency",
                    "Minimum frequency",
                    "Hz",
                    self.snapshot.minimum_frequency_hz,
                    (20.0, 20000.0),
                    DesignerCommand::MinimumFrequency,
                ),
                (
                    "designer-max-frequency",
                    "Maximum frequency",
                    "Hz",
                    self.snapshot.maximum_frequency_hz,
                    (20.0, 20000.0),
                    DesignerCommand::MaximumFrequency,
                ),
                (
                    "designer-max-cut",
                    "Maximum cut",
                    "dB",
                    self.snapshot.maximum_cut_db,
                    (0.0, 25.0),
                    DesignerCommand::MaximumCut,
                ),
                (
                    "designer-max-boost",
                    "Maximum boost",
                    "dB",
                    self.snapshot.maximum_boost_db,
                    (0.0, 25.0),
                    DesignerCommand::MaximumBoost,
                ),
            ]
        };
        for (id, label, unit, value, range, command) in values {
            let entity = cx.entity();
            fields = fields.child(
                NumberInput::new(SharedString::from(id.to_owned()))
                    .label(SharedString::from(label.to_owned()))
                    .unit(SharedString::from(unit.to_owned()))
                    .value(value)
                    .range(range.0, range.1)
                    .width(180.0)
                    .step(if unit == "Hz" { 10.0 } else { 0.1 })
                    .decimals(if unit == "Hz" { 0 } else { 1 })
                    .disabled(self.snapshot.running)
                    .scroll_requires_alt(true)
                    .on_change(move |v, _, cx| entity.update(cx, |_, cx| cx.emit(command(v)))),
            );
        }
        fields.into_any_element()
    }
    fn panel(&self, cx: &Context<Self>) -> AnyElement {
        let s = &self.snapshot;
        let mut panel = div().flex().flex_col().gap_4().min_w_0();
        match self.selected {
            DesignerStep::Data => {
                let manage = self.action(
                    "designer-open-data",
                    "Manage measurement data",
                    DesignerCommand::OpenData,
                    s.running,
                    cx,
                );
                panel = panel.child(Text::section_header("Measurement data"));
                if s.has_measurements {
                    panel = panel
                        .child(Text::body(format!(
                            "{} measurement channels loaded",
                            s.channel_names.len()
                        )))
                        .child(manage)
                        .child(self.measurement_list(cx))
                        .child(crate::room_layout::render_channel_layout(
                            &s.channel_names,
                            cx,
                        ));
                } else {
                    panel = panel.child(
                        EmptyState::new("No measurement data")
                            .description(
                                "Import saved responses or choose the existing Recording workflow.",
                            )
                            .action(manage),
                    );
                }
            }
            DesignerStep::Checks => {
                panel = panel.child(Text::section_header("Check alignment and coverage"))
                    .child(Text::body(if s.has_phase { "Phase data is present. Review timing and measurement compatibility before choosing phase-dependent processing." } else { "No phase data detected. Magnitude-only data cannot establish timing or phase alignment." }))
                    .child(Text::body("Existing analysis and delay detection remain the source of preliminary calculations; opening this panel does not run them."))
                    .child(self.action("designer-open-checks", "Review delays and alignment", DesignerCommand::OpenChecks, !s.has_measurements || s.running, cx));
            }
            DesignerStep::Goals => {
                panel = panel.child(Text::section_header("Set the tonal goal"))
                    .child(Text::body(format!("Current target: {} · shaping {}", s.target_shape, if s.target_enabled { "enabled" } else { "disabled" })))
                    .child(Text::body("Slope selects a custom target and enables shaping. Bass preference adds a shelf; correction limits still apply."))
                    .child(self.fields(true, cx))
                    .child(Text::caption("Bass weight trades headroom. Tonal controls do not predict soundstage; spatial goals need geometry and listening evidence."))
                    .child(self.action("designer-target-options", "All target and objective options", DesignerCommand::OpenParameters, s.running, cx));
            }
            DesignerStep::Constraints => {
                panel = panel.child(Text::section_header("Bound the correction"))
                    .child(self.fields(false, cx))
                    .child(Text::body("Boost uses headroom. Narrower correction preserves more of the original response. Controls lock during calculation."))
                    .child(self.action("designer-all-parameters", "All processing parameters", DesignerCommand::OpenParameters, s.running, cx));
            }
            DesignerStep::Advice => {
                let entity = cx.entity();
                panel = panel.child(Text::section_header("Discuss the choices"))
                    .child(Text::body("In-app chat is disabled. Copy a draft brief for external discussion. Supported MCP commands share the current app state."))
                    .child(Button::new("designer-copy-brief", if self.brief_copied { "Brief copied" } else { "Copy draft brief" })
                        .size(ButtonSize::Sm).variant(ButtonVariant::Secondary).on_click(move |_, cx| entity.update(cx, |view, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(view.snapshot.advice_brief()));
                            view.brief_copied = true; cx.notify();
                        })))
                    .child(Text::body("Apply agreed changes in Goals or Constraints, then review again."));
            }
            DesignerStep::Study => {
                let entity = cx.entity();
                panel = panel.child(Text::section_header("Review and calculate"))
                    .children((!s.explicit_configuration).then(|| Text::body("Simple preset mode is active. Adopt the displayed configuration explicitly before review; otherwise study startup can resolve different settings.")))
                    .children((!s.explicit_configuration).then(|| self.action("designer-explicit-draft", "Use displayed settings", DesignerCommand::UseExplicitSettings, s.running, cx)))
                    .child(MetadataList::new("designer-draft-summary").aria_label("Calibration draft summary")
                        .entries(vec![
                            MetadataEntry::new("Measurement channels", s.channel_names.len().to_string()),
                            MetadataEntry::new("Correction band", format!("{:.0}–{:.0} Hz", s.minimum_frequency_hz, s.maximum_frequency_hz)),
                            MetadataEntry::new("Maximum cut / boost", format!("{:.1} / {:.1} dB", s.maximum_cut_db, s.maximum_boost_db)),
                        ]))
                    .child(Text::body(format!("Target {} · shaping {} · shelf {:.1} dB", s.target_shape, s.target_enabled, s.bass_shelf_db)))
                    .child(Button::new("designer-review-inputs", if self.reviewed { "Draft reviewed" } else { "Mark draft reviewed" })
                        .size(ButtonSize::Sm).variant(ButtonVariant::Secondary).disabled(!s.can_run())
                        .on_click(move |_, cx| entity.update(cx, |view, cx| { view.reviewed = true; cx.notify(); })))
                    .child(self.action("designer-run-study", "Run offline study", DesignerCommand::RunStudy, !self.reviewed || !s.can_run(), cx))
                    .child(self.action("designer-cancel-study", "Cancel study", DesignerCommand::CancelStudy, !s.running, cx))
                    .children(s.reported_progress.map(|value| Progress::new(value).show_label(true).aria_label("Reported offline study progress")))
                    .child(self.action("designer-open-results", "Review results and exports", DesignerCommand::OpenResults, s.running || s.result_count == 0, cx))
                    .child(Text::caption("Edits clear review. Results remain bound to their input revision. This study does not install filters."));
                for stage in &s.stages {
                    if stage.status != "Not observed in a study" {
                        panel = panel.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(Text::label(format!(
                                    "{} · {}",
                                    stage.step.label(),
                                    stage.status
                                )))
                                .child(Text::caption(stage_description(stage.step)))
                                .child(Text::caption(stage.summary.clone())),
                        );
                    }
                }
            }
        }
        panel.into_any_element()
    }
}
impl Render for Designer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let selected = DesignerStep::ALL
            .iter()
            .position(|s| *s == self.selected)
            .unwrap_or(0);
        let entity = cx.entity();
        let tabs = Tabs::new("designer-decision-steps")
            .tabs(
                DesignerStep::ALL
                    .iter()
                    .enumerate()
                    .map(|(i, step)| {
                        TabItem::new(format!("step-{i}"), format!("{} {}", i + 1, step.label()))
                    })
                    .collect(),
            )
            .selected_index(selected)
            .variant(TabVariant::Underline)
            .theme(TabsTheme {
                text_selected: theme.text_primary,
                ..TabsTheme::from(theme.as_ref())
            })
            .aria_label("Calibration design steps")
            .on_change(move |index, _, cx| {
                entity.update(cx, |view, cx| {
                    if let Some(step) = DesignerStep::ALL.get(index) {
                        view.select_step(*step, cx);
                    }
                })
            });
        let mut navigation = div().flex().flex_wrap().gap_2();
        for (id, label, next, disabled) in [
            (
                "designer-previous-step",
                "Previous phase",
                selected.saturating_sub(1),
                selected == 0,
            ),
            (
                "designer-next-step",
                "Next phase",
                (selected + 1).min(5),
                selected == 5,
            ),
        ] {
            let entity = cx.entity();
            navigation = navigation.child(
                Button::new(id, label)
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .disabled(disabled)
                    .on_click(move |_, cx| {
                        entity.update(cx, |view, cx| {
                            view.select_step(DesignerStep::ALL[next], cx);
                        })
                    }),
            );
        }
        div()
            .id("calibration-designer-plugin")
            .flex()
            .flex_col()
            .gap_4()
            .min_w_0()
            .text_color(theme.text_primary)
            .child(Text::section_header("Calibration Designer"))
            .child(tabs)
            .child(self.panel(cx))
            .children(
                self.snapshot
                    .error
                    .clone()
                    .map(|error| Text::body(error).color(theme.error)),
            )
            .child(navigation)
    }
}
#[cfg(test)]
mod tests {
    use super::{Designer, DesignerSnapshot, DesignerStep};
    use gpui::{AppContext, TestAppContext};
    use sotf_audio_player::ui_models::room_eq::RoomEqScreenModel;
    #[gpui::test]
    fn snapshot_changes_preserve_step_and_invalidate_review(cx: &mut TestAppContext) {
        let mut model = RoomEqScreenModel::default();
        let designer = cx.new(|cx| Designer::new(DesignerSnapshot::from_model(&model), cx));
        designer.update(cx, |view, _| {
            view.selected = DesignerStep::Goals;
            view.reviewed = true;
        });
        model.optimizer_config.min_freq = 55.0;
        designer.update(cx, |view, cx| {
            view.update_snapshot(DesignerSnapshot::from_model(&model), cx)
        });
        cx.read(|cx| {
            let view = designer.read(cx);
            assert_eq!(view.selected, DesignerStep::Goals);
            assert!(!view.reviewed);
            assert_eq!(view.snapshot.minimum_frequency_hz, 55.0);
            assert!(!view.snapshot.can_run());
        });
        designer.update(cx, |view, cx| {
            view.reviewed = true;
            let mut next = view.snapshot.clone();
            next.input_identity = "different-measurement-or-delay".into();
            view.update_snapshot(next, cx);
            assert!(!view.reviewed);
        });
        cx.read(|cx| {
            let view = designer.read(cx);
            assert_eq!(view.selected, DesignerStep::Goals);
        });
    }

    #[gpui::test]
    fn response_selection_changes_only_display_state(cx: &mut TestAppContext) {
        let mut snapshot = DesignerSnapshot::from_model(&RoomEqScreenModel::default());
        snapshot.measurement_previews.rows = ["L", "R"]
            .map(|label| crate::measurement_preview::ResponsePreview {
                label: label.into(),
                points: vec![(20.0, -5.0), (20000.0, 0.0)],
            })
            .into();
        snapshot.measurement_previews.magnitude_range_db = Some((-10.0, 5.0));
        let original = snapshot.clone();
        let designer = cx.new(|cx| Designer::new(snapshot, cx));
        designer.update(cx, |view, cx| {
            view.reviewed = true;
            view.select_measurement(1, cx);
            assert_eq!(view.selected_measurement, 1);
            assert_eq!(view.selected_measurement_index(), Some(1));
            assert!(view.reviewed);
            assert_eq!(view.snapshot, original);
            view.select_measurement(9, cx);
            assert_eq!(view.selected_measurement, 1);
            view.snapshot.measurement_previews = Default::default();
            assert_eq!(view.selected_measurement_index(), None);
        });
    }
}
