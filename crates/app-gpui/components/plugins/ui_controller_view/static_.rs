// intentional-file: fixed pixel values here are graph and plugin control geometry.
use super::consts::BUTTON_SIZE;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::{Rgba, div, px};

/// Mark an unassigned rotary hardware control with a noninteractive slider.
pub(super) fn static_slider(theme: &Theme, accent: Rgba, scale: f32) -> impl IntoElement {
    let scale = scale.clamp(0.01, 1.5);
    div()
        .relative()
        .w(px(60.0 * scale))
        .h(px(4.0 * scale))
        .rounded(px(2.0 * scale))
        .bg(theme.background_secondary)
        .child(
            div()
                .absolute()
                .left(px(26.0 * scale))
                .top(px(-4.0 * scale))
                .w(px(8.0 * scale))
                .h(px(12.0 * scale))
                .rounded(px(2.0 * scale))
                .bg(accent),
        )
}

pub(super) fn static_fader(theme: &Theme, scale: f32) -> impl IntoElement {
    let scale = scale.clamp(0.75, 1.5);
    div()
        .relative()
        .w(px(14.0 * scale))
        .h(px(110.0 * scale))
        .rounded(px(2.0 * scale))
        .bg(theme.background_secondary)
        .border_1()
        .border_color(theme.border)
}

pub(super) fn static_button(theme: &Theme, accent: Rgba, scale: f32) -> impl IntoElement {
    let scale = scale.clamp(0.75, 1.5);
    div()
        .w(px(BUTTON_SIZE * scale))
        .h(px(BUTTON_SIZE * scale))
        .rounded(px(4.0 * scale))
        .bg(theme.surface)
        .border_1()
        .border_color(accent)
}
