//! Integration test for Select component

use gpui::{Context, TestAppContext, Window, div, prelude::*};
use gpui_ui_kit::select::{Select, SelectOption};

struct SelectTestView;

impl Render for SelectTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            Select::new("test-select")
                .placeholder("Choose option")
                .options(vec![
                    SelectOption::new("1", "Option 1"),
                    SelectOption::new("2", "Option 2"),
                ]),
        )
    }
}

#[gpui::test]
async fn test_select_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| SelectTestView);
}
