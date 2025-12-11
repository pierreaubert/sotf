//! Integration test for Toggle component

use gpui::{Context, TestAppContext, Window, div, prelude::*};
use gpui_ui_kit::toggle::Toggle;

struct ToggleTestView;

impl Render for ToggleTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            Toggle::new("test-toggle")
                .label("Test Toggle")
                .checked(true),
        )
    }
}

#[gpui::test]
async fn test_toggle_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| ToggleTestView);
}
