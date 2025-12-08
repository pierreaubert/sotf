//! Integration test for Text components

use gpui::{div, prelude::*, TestAppContext, Window, Context};
use gpui_ui_kit::text::{Text, Heading};

struct TextTestView;

impl Render for TextTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child(Heading::h1("Heading 1"))
            .child(Text::new("Body text"))
    }
}

#[gpui::test]
async fn test_text_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| TextTestView);
}
