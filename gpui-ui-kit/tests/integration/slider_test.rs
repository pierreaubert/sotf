//! Integration test for Slider component

use gpui::{Context, TestAppContext, Window, div, prelude::*};
use gpui_ui_kit::slider::Slider;

struct SliderTestView;

impl Render for SliderTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(Slider::new("test-slider").value(0.5).min(0.0).max(1.0))
    }
}

#[gpui::test]
async fn test_slider_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| SliderTestView);
}
