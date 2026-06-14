use super::types::PaletteItemType;
use crate::components::design::Ds;
use gpui::prelude::*;
use gpui::*;

/// Drag data for palette items
#[derive(Clone)]
pub struct PaletteDragData {
    pub item_type: PaletteItemType,
    pub label: String,
    pub color: Rgba,
    pub text_on_accent: Rgba,
}

impl Render for PaletteDragData {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        div()
            .px(d.pad_x)
            .py(d.pad_y)
            .bg(self.color)
            .rounded(d.r_md)
            .text_size(d.text_sm)
            .text_color(self.text_on_accent)
            .shadow_lg()
            .child(self.label.clone())
    }
}
