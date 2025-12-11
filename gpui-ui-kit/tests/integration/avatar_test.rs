//! Integration test for Avatar component

use gpui::{Context, TestAppContext, Window, div, prelude::*};
use gpui_ui_kit::avatar::Avatar;

struct AvatarTestView;

impl Render for AvatarTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(Avatar::new().name("Test User"))
    }
}

#[gpui::test]
async fn test_avatar_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| AvatarTestView);
}
