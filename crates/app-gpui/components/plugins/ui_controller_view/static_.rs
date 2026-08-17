// intentional-file: fixed pixel values here are graph and plugin control geometry.
use super::consts::BUTTON_SIZE;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

pub(super) fn static_knob(theme: &Theme, accent: Rgba, scale: f32) -> impl IntoElement {
    let scale = scale.clamp(0.75, 1.5);
    div()
        .w(px(40.0 * scale))
        .h(px(40.0 * scale))
        .rounded_full()
        .bg(theme.background_secondary)
        .border_2()
        .border_color(accent)
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(px(18.0 * scale))
                .h(px(18.0 * scale))
                .rounded_full()
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
