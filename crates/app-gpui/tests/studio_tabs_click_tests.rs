//! Regression coverage for Studio tab interaction after independent hover redraws.

use gpui::{
    Context, InteractiveElement, IntoElement, Modifiers, MouseButton, ParentElement, Render,
    Styled, TestAppContext, VisualTestContext, Window, div,
};
use gpui_ui_kit::{TabItem, TabVariant, Tabs};
use std::cell::Cell;
use std::rc::Rc;

struct StudioTabsView {
    variant: TabVariant,
    selected: Rc<Cell<usize>>,
}

impl Render for StudioTabsView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected.clone();
        div().size_full().child(
            Tabs::new("studio-tab-regression")
                .variant(self.variant)
                .tabs(vec![
                    TabItem::new("processing", "Processing"),
                    TabItem::new("measure", "Measure").icon_with_color(|_| {
                        div()
                            .debug_selector(|| "studio-measure-icon".to_owned())
                            .child("M")
                            .into_any_element()
                    }),
                ])
                .on_change(move |index, _, _| selected.set(index)),
        )
    }
}

async fn click_after_hover(variant: TabVariant, cx: &mut TestAppContext) {
    let selected = Rc::new(Cell::new(0));
    let state = selected.clone();
    let window = cx.add_window(move |_, _| StudioTabsView {
        variant,
        selected: state,
    });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds("studio-measure-icon")
        .expect("tab icon must render");
    cx.simulate_mouse_move(bounds.center(), None, Modifiers::default());
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds("studio-measure-icon")
        .expect("hover must preserve tab");
    cx.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    assert_eq!(selected.get(), 1, "hovered tab must remain clickable");
}

#[gpui::test]
async fn studio_categories_click_after_hover(cx: &mut TestAppContext) {
    click_after_hover(TabVariant::Enclosed, cx).await;
}

#[gpui::test]
async fn studio_subtools_click_after_hover(cx: &mut TestAppContext) {
    click_after_hover(TabVariant::Underline, cx).await;
}
