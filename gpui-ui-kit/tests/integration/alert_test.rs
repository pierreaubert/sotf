//! Integration test for Alert component

use gpui::{div, prelude::*, TestAppContext, Window, Context};
use gpui_ui_kit::alert::{Alert, AlertVariant};

struct AlertTestView;

impl Render for AlertTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            Alert::new("test-alert", "This is a test message")
                .variant(AlertVariant::Info)
        )
    }
}

#[gpui::test]
async fn test_alert_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| AlertTestView);
}
