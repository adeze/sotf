//! Embeds Sonium's passive editor without changing player routing or playback.
use crate::{ActiveCrossoverSettings, Evaluation};
use gpui::*;
use gpui_ui_kit::{Button, NumberInput, Text, theme::ThemeExt};
use sonium_speaker_designer::desktop::{Designer, create, response_plot};
use std::sync::Arc;

/// Persistent tool view embedded in the player's existing content area.
pub struct Workbench {
    designer: Entity<Designer>,
    settings: ActiveCrossoverSettings,
    preview: Option<Arc<Evaluation>>,
    generation: u64,
    busy: bool,
    message: String,
}

impl Workbench {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let designer = create(cx);
        cx.observe(&designer, |this, _, cx| this.invalidate(cx))
            .detach();
        Self {
            designer,
            settings: ActiveCrossoverSettings::default(),
            preview: None,
            generation: 0,
            busy: false,
            message: "Import driver responses above, then preview the separate active design."
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
        let preview_entity = cx.entity();
        let copy_entity = cx.entity();
        let entity = cx.entity();
        let cutoff = NumberInput::new("active-cutoff")
            .label("LR24 crossover")
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
        div().size_full().flex().flex_col().bg(theme.background).text_color(theme.text_primary)
            .child(div().flex_1().min_h_0().child(self.designer.clone()))
            .child(div().flex_shrink_0().p_4().border_t_1().border_color(theme.border).flex().flex_col().gap_2()
                .child(Text::section_header("Active crossover · SOTF").color(theme.text_primary))
                .child(Text::caption("Separate from the passive circuit above. Low output → woofer; high output → tweeter. Preview only.").color(theme.text_secondary))
                .child(div().flex().gap_4().items_end().child(cutoff).child(rate)
                    .child(Button::new("preview-active", if self.busy { "Calculating…" } else { "Preview active" })
                        .on_click(move |_, cx| preview_entity.update(cx, |this, cx| this.calculate(cx))))
                    .child(Button::new("copy-active", "Copy crossover parameters")
                        .on_click(move |_, cx| copy_entity.update(cx, |this, cx| this.copy_parameters(cx)))))
                .child(Text::caption(self.message.clone()).color(theme.text_primary))
                .children(self.preview.clone().map(|preview| response_plot("Active branches and acoustic sum", Some(preview), theme.border.into()))))
    }
}
