//! Integration test for Tabs component

use gpui::{div, prelude::*, TestAppContext, Window, Context};
use gpui_ui_kit::tabs::{Tabs, TabItem};

struct TabsTestView;

impl Render for TabsTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            Tabs::new()
                .tabs(vec![
                    TabItem::new("tab1", "Tab 1"),
                    TabItem::new("tab2", "Tab 2"),
                ])
                .selected_index(0)
        )
    }
}

#[gpui::test]
async fn test_tabs_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| TabsTestView);
}
