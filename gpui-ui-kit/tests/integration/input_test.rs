//! Integration test for Input component

use gpui::{Context, TestAppContext, Window, div, prelude::*};
use gpui_ui_kit::input::Input;

struct InputTestView;

impl Render for InputTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            Input::new("test-input")
                .placeholder("Enter text...")
                .value("Hello"),
        )
    }
}

#[gpui::test]
async fn test_input_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| InputTestView);
}
