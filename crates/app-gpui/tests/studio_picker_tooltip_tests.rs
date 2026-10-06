//! Regression coverage for a plugin picker button's description tooltip.

use gpui::{
    AppContext, Context, InteractiveElement, IntoElement, Modifiers, ParentElement, Render,
    StatefulInteractiveElement, Styled, TestAppContext, VisualTestContext, Window, div, px,
};
use gpui_ui_kit::Button;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

struct DescriptionTooltip;

impl Render for DescriptionTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child("Reduce dynamic range above a threshold.")
    }
}

struct PickerTooltipView {
    shown: Rc<Cell<bool>>,
}

impl Render for PickerTooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let shown = self.shown.clone();
        div().size_full().child(
            div()
                .id("add-plugin-menu")
                .h(px(300.0))
                .overflow_y_scroll()
                .child(
                    div().flex().child(
                        div()
                            .id("rack-add-tooltip-Compressor")
                            .debug_selector(|| "picker-compressor".to_owned())
                            .tooltip(move |_, cx| {
                                shown.set(true);
                                cx.new(|_| DescriptionTooltip).into()
                            })
                            .child(
                                Button::new("rack-add-compressor", "Compressor")
                                    .custom_icon_left(div().child("C"))
                                    .on_click(|_, _| {}),
                            ),
                    ),
                ),
        )
    }
}

#[gpui::test]
async fn plugin_description_appears_after_button_hover(cx: &mut TestAppContext) {
    let shown = Rc::new(Cell::new(false));
    let state = shown.clone();
    let window = cx.add_window(move |_, _| PickerTooltipView { shown: state });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds("picker-compressor")
        .expect("picker button must render");
    cx.simulate_mouse_move(bounds.center(), None, Modifiers::default());
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(
        shown.get(),
        "hover must create the plugin description tooltip"
    );
}
