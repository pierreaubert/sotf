//! Integration test for Tooltip component

use gpui::{Context, TestAppContext, Window, div, prelude::*};
use gpui_ui_kit::tooltip::Tooltip;

struct TooltipTestView;

impl Render for TooltipTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(Tooltip::new("Tooltip text"))
    }
}

#[gpui::test]
async fn test_tooltip_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| TooltipTestView);
}
