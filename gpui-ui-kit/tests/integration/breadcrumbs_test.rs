//! Integration test for Breadcrumbs component

use gpui::{div, prelude::*, TestAppContext, Window, Context};
use gpui_ui_kit::breadcrumbs::{Breadcrumbs, BreadcrumbItem};

struct BreadcrumbsTestView;

impl Render for BreadcrumbsTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            Breadcrumbs::new()
                .items(vec![
                    BreadcrumbItem::new("home", "Home"),
                    BreadcrumbItem::new("docs", "Docs"),
                    BreadcrumbItem::new("api", "API"),
                ])
        )
    }
}

#[gpui::test]
async fn test_breadcrumbs_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| BreadcrumbsTestView);
}
