//! Integration test for Card component

use gpui::{Context, TestAppContext, Window, div, prelude::*};
use gpui_ui_kit::card::Card;

struct CardTestView;

impl Render for CardTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(Card::new().content("Card content"))
    }
}

#[gpui::test]
async fn test_card_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| CardTestView);
}
