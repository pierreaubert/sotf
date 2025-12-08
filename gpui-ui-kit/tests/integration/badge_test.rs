//! Integration test for Badge component

use gpui::{div, prelude::*, TestAppContext, Window, Context};
use gpui_ui_kit::badge::Badge;

struct BadgeTestView;

impl Render for BadgeTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            Badge::new("New")
        )
    }
}

#[gpui::test]
async fn test_badge_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| BadgeTestView);
}
