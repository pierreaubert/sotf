use crate::components::design::Ds;
use gpui::prelude::*;
use gpui::*;

/// Drag data for EQ control point manipulation (frequency/gain)
#[derive(Clone)]
pub(crate) struct EqControlPointDrag {
    pub(super) band_idx: usize,
    pub(super) plugin_idx: usize,
    pub(super) color: u32,
    pub(super) border_color: Rgba,
    pub(super) radius: f32,
    #[allow(dead_code)]
    pub(super) start_freq: f64,
    #[allow(dead_code)]
    pub(super) start_gain: f64,
    #[allow(dead_code)]
    pub(super) start_x: f32,
    #[allow(dead_code)]
    pub(super) start_y: f32,
}

impl Render for EqControlPointDrag {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let rgba_color = gpui::rgba(self.color * 256 + 0xFF);
        div()
            .w(px(self.radius * 3.0))
            .h(px(self.radius * 3.0))
            .rounded_full()
            .bg(rgba_color)
            .border(d.half_grid)
            .border_color(self.border_color)
            .shadow_lg()
    }
}
