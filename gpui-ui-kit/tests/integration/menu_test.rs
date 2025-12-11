//! Integration test for Menu component

use gpui::{Context, TestAppContext, Window, div, prelude::*};
use gpui_ui_kit::menu::{Menu, MenuItem};

struct MenuTestView;

impl Render for MenuTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(Menu::new(vec![
            MenuItem::new("item1", "Menu Item 1"),
            MenuItem::new("item2", "Menu Item 2"),
        ]))
    }
}

#[gpui::test]
async fn test_menu_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| MenuTestView);
}
