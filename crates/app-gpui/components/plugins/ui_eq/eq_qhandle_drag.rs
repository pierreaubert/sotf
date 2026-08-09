use gpui::prelude::*;
use gpui::*;

/// Drag data for Q handle manipulation
#[derive(Clone)]
pub(crate) struct EqQHandleDrag {
    pub(super) band_idx: usize,
    pub(super) plugin_idx: usize,
    #[allow(dead_code)]
    pub(super) is_right_handle: bool, // true = right handle (increase Q), false = left handle (decrease Q)
    pub(super) start_x: f32,
    pub(super) start_q: f64,
    pub(super) color: u32,
    pub(super) border_color: Rgba,
    pub(super) radius: f32,
}

impl Render for EqQHandleDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let rgba_color = gpui::rgba(self.color * 256 + 0xFF);
        div()
            .w(px(self.radius * 2.0))
            .h(px(self.radius * 2.0))
            .rounded_full()
            .bg(rgba_color)
            .border_1()
            .border_color(self.border_color)
            .shadow_md()
    }
}
