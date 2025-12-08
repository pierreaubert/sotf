//! Integration test for Spinner component

use gpui::{div, prelude::*, TestAppContext, Window, Context};
use gpui_ui_kit::spinner::Spinner;

struct SpinnerTestView;

impl Render for SpinnerTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(Spinner::new())
    }
}

#[gpui::test]
async fn test_spinner_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| SpinnerTestView);
}
