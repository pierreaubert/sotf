//! Integration test for Accordion component

use gpui::{div, prelude::*, TestAppContext, Window, Context};
use gpui_ui_kit::accordion::{Accordion, AccordionItem};

struct AccordionTestView;

impl Render for AccordionTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            Accordion::new()
                .items(vec![
                    AccordionItem::new("item1", "Section 1")
                        .content("Content 1"),
                    AccordionItem::new("item2", "Section 2")
                        .content("Content 2"),
                ])
        )
    }
}

#[gpui::test]
async fn test_accordion_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| AccordionTestView);
}
