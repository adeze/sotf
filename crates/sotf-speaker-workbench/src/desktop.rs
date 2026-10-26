//! Embeds Sonium's passive editor without changing player routing or playback.
use crate::{ActiveCrossoverSettings, Evaluation};
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, NumberInput, TabItem, TabVariant, Tabs, Text,
    theme::ThemeExt,
};
use sonium_speaker_designer::desktop::{Designer, create, response_plot};
use std::sync::Arc;

/// Persistent tool view embedded in the player's existing content area.
pub struct Workbench {
    designer: Entity<Designer>,
    selected_tab: usize,
    settings: ActiveCrossoverSettings,
    preview: Option<Arc<Evaluation>>,
    generation: u64,
    designer_calculation_generation: u64,
    busy: bool,
    message: String,
}

impl Workbench {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let designer = create(cx);
        let designer_calculation_generation = designer.read(cx).calculation_generation();
        cx.observe(&designer, |this, designer, cx| {
            let generation = designer.read(cx).calculation_generation();
            if generation != this.designer_calculation_generation {
                this.designer_calculation_generation = generation;
                this.invalidate(cx);
            }
        })
        .detach();
        Self {
            designer,
            selected_tab: 0,
            settings: ActiveCrossoverSettings::default(),
            preview: None,
            generation: 0,
            designer_calculation_generation,
            busy: false,
            message: "Import driver responses in Passive circuit, then preview the active design."
                .into(),
        }
    }

    fn invalidate(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        self.preview = None;
        self.message = "Design changed. Recalculate the active preview.".into();
        cx.notify();
    }

    fn calculate(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let project = self.designer.read(cx).project().clone();
        let synthetic = project.synthetic;
        let settings = self.settings;
        let generation = self.generation;
        self.preview = None;
        self.busy = true;
        self.message = "Calculating active response with the SOTF crossover plugin…".into();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { settings.evaluate(&project) }).await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if this.generation == generation {
                    match result {
                        Ok(evaluation) => {
                            this.preview = Some(Arc::new(evaluation));
                            this.message = if synthetic {
                                "Synthetic demonstration · LR24 prediction; ideal amplifiers assumed."
                            } else {
                                "LR24 prediction at the project's measurement reference; ideal amplifiers assumed."
                            }.into();
                        }
                        Err(error) => this.message = error.to_string(),
                    }
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    fn copy_parameters(&mut self, cx: &mut Context<Self>) {
        let result = self
            .settings
            .plugin_params()
            .map_err(|error| error.to_string())
            .and_then(|parameters| {
                serde_json::to_string_pretty(&serde_json::json!({
                    "sample_rate_hz": self.settings.sample_rate_hz,
                    "plugin": "crossover",
                    "parameters": parameters
                }))
                .map_err(|error| error.to_string())
            });
        match result {
            Ok(json) => {
                cx.write_to_clipboard(ClipboardItem::new_string(json));
                self.message = "Copied electrical crossover parameters. Driver delay, polarity and routing are separate.".into();
            }
            Err(error) => self.message = error,
        }
        cx.notify();
    }
}

impl Render for Workbench {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let tabs_entity = cx.entity();
        let preview_entity = cx.entity();
        let copy_entity = cx.entity();
        let entity = cx.entity();
        let cutoff = NumberInput::new("active-cutoff")
            .label("Crossover frequency")
            .width(180.0)
            .unit("Hz")
            .value(self.settings.frequency_hz)
            .range(20.0, 20_000.0)
            .step(50.0)
            .decimals(0)
            .on_change(move |value, _, cx| {
                entity.update(cx, |this, cx| {
                    this.settings.frequency_hz = value;
                    this.invalidate(cx);
                })
            });
        let entity = cx.entity();
        let rate = NumberInput::new("active-sample-rate")
            .label("Sample rate")
            .width(180.0)
            .unit("Hz")
            .value(f64::from(self.settings.sample_rate_hz))
            .range(8_000.0, 192_000.0)
            .step(1_000.0)
            .decimals(0)
            .on_change(move |value, _, cx| {
                entity.update(cx, |this, cx| {
                    this.settings.sample_rate_hz = value as u32;
                    this.invalidate(cx);
                })
            });
        let tabs = Tabs::new("speaker-workbench-tabs")
            .tabs(vec![
                TabItem::new("passive", "Passive circuit"),
                TabItem::new("active", "Active crossover"),
            ])
            .selected_index(self.selected_tab)
            .variant(TabVariant::Underline)
            .theme(gpui_ui_kit::TabsTheme {
                text_selected: theme.text_primary,
                ..gpui_ui_kit::TabsTheme::from(theme.as_ref())
            })
            .aria_label("Speaker design mode")
            .on_change(move |index, _, cx| {
                tabs_entity.update(cx, |this, cx| {
                    this.selected_tab = index;
                    cx.notify();
                });
            });
        let content = if self.selected_tab == 0 {
            div()
                .size_full()
                .min_h_0()
                .child(self.designer.clone())
                .into_any_element()
        } else {
            div()
                .id("active-crossover-scroll")
                .size_full()
                .min_w_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_4()
                .p_4()
                .child(Text::section_header("Two-way active crossover").color(theme.text_primary))
                .child(Text::caption("LR24 · low output → woofer; high output → tweeter. Uses the driver responses from Passive circuit.").color(theme.text_secondary))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_end()
                        .gap_4()
                        .child(cutoff)
                        .child(rate)
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap_2()
                                .child(
                                    Button::new("preview-active", if self.busy { "Calculating…" } else { "Preview response" })
                                        .variant(ButtonVariant::Primary)
                                        .size(ButtonSize::Sm)
                                        .disabled(self.busy)
                                        .on_click(move |_, cx| preview_entity.update(cx, |this, cx| this.calculate(cx))),
                                )
                                .child(
                                    Button::new("copy-active", "Copy parameters")
                                        .variant(ButtonVariant::Secondary)
                                        .size(ButtonSize::Sm)
                                        .on_click(move |_, cx| copy_entity.update(cx, |this, cx| this.copy_parameters(cx))),
                                ),
                        ),
                )
                .child(Text::caption(self.message.clone()).color(theme.text_primary))
                .children(self.preview.clone().map(|preview| {
                    response_plot("Active branches and acoustic sum", Some(preview), theme.border.into())
                }))
                .child(Text::caption("Prediction only · ideal amplifiers. Passive component values do not set this cutoff. Copying parameters does not configure routing or driver protection.").color(theme.text_secondary))
                .into_any_element()
        };
        div()
            .size_full()
            .min_w_0()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.text_primary)
            .child(
                div()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(tabs),
            )
            .child(div().flex_1().min_h_0().min_w_0().child(content))
    }
}

#[cfg(test)]
mod tests {
    use super::{Arc, Evaluation, Workbench};
    use gpui::{AppContext, TestAppContext};

    #[gpui::test]
    fn presentation_notifications_preserve_preview_but_active_changes_invalidate(
        cx: &mut TestAppContext,
    ) {
        let workbench = cx.new(Workbench::new);
        let preview = Arc::new(Evaluation::default());
        workbench.update(cx, |workbench, _| {
            workbench.preview = Some(preview.clone());
            workbench.message = "Existing preview".into();
            workbench.generation = 7;
        });
        let designer = cx.read(|cx| workbench.read(cx).designer.clone());
        designer.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        cx.read(|cx| {
            let state = workbench.read(cx);
            assert_eq!(state.generation, 7);
            assert_eq!(state.message, "Existing preview");
            assert!(Arc::ptr_eq(state.preview.as_ref().unwrap(), &preview));
        });
        workbench.update(cx, |workbench, cx| {
            workbench.settings.frequency_hz = 2500.0;
            workbench.invalidate(cx);
        });
        cx.read(|cx| {
            let state = workbench.read(cx);
            assert_eq!(state.generation, 8);
            assert!(state.preview.is_none());
            assert!(state.message.contains("Recalculate"));
        });
    }
}
