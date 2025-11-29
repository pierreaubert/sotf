//! Compressor Plugin UI Component
//!
//! Professional compressor visualization with:
//! - Transfer curve display
//! - Gain reduction meter
//! - Vertical slider controls with keyboard shortcuts

use super::common::{
    render_edit_hints, render_gr_meter, render_section_header, render_toggle,
    render_transfer_curve, render_vertical_slider,
};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

/// Render the Compressor plugin
#[allow(clippy::too_many_arguments)]
pub fn render_compressor_plugin(
    threshold_db: f64,
    ratio: f64,
    attack_ms: f64,
    release_ms: f64,
    knee_db: f64,
    makeup_gain_db: f64,
    mix: f64,
    auto_makeup: bool,
    link_channels: bool,
    sidechain_hpf_hz: f64,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
) -> impl IntoElement {
    // Simulated gain reduction (in real implementation, this would come from the audio engine)
    let simulated_gr = if threshold_db < -10.0 {
        (threshold_db + 10.0) * 0.5
    } else {
        0.0
    };

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Main section - Sliders and Transfer Curve side by side
        .child(
            div()
                .flex()
                .gap_4()
                // Parameters section with vertical sliders
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .p_4()
                        .child(render_section_header("DYNAMICS CONTROL", theme))
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .justify_center()
                                .flex_wrap()
                                .child(render_vertical_slider(
                                    "Threshold",
                                    threshold_db,
                                    -60.0,
                                    0.0,
                                    "dB",
                                    0,
                                    selected_param,
                                    is_editing,
                                    Some('t'),
                                    theme,
                                ))
                                .child(render_vertical_slider(
                                    "Ratio",
                                    ratio,
                                    1.0,
                                    20.0,
                                    ":1",
                                    1,
                                    selected_param,
                                    is_editing,
                                    Some('r'),
                                    theme,
                                ))
                                .child(render_vertical_slider(
                                    "Attack",
                                    attack_ms,
                                    0.1,
                                    100.0,
                                    "ms",
                                    2,
                                    selected_param,
                                    is_editing,
                                    Some('a'),
                                    theme,
                                ))
                                .child(render_vertical_slider(
                                    "Release",
                                    release_ms,
                                    10.0,
                                    1000.0,
                                    "ms",
                                    3,
                                    selected_param,
                                    is_editing,
                                    Some('e'),
                                    theme,
                                ))
                                .child(render_vertical_slider(
                                    "Knee",
                                    knee_db,
                                    0.0,
                                    12.0,
                                    "dB",
                                    4,
                                    selected_param,
                                    is_editing,
                                    Some('k'),
                                    theme,
                                ))
                                .child(render_vertical_slider(
                                    "Makeup",
                                    makeup_gain_db,
                                    0.0,
                                    24.0,
                                    "dB",
                                    5,
                                    selected_param,
                                    is_editing,
                                    Some('m'),
                                    theme,
                                )),
                        ),
                )
                // Transfer curve and options
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .p_4()
                        .items_center()
                        .child(render_transfer_curve(
                            threshold_db,
                            ratio,
                            knee_db,
                            false, // Not a limiter
                            theme,
                        ))
                        // Mix slider
                        .child(div().w_full().child(render_vertical_slider(
                            "Mix",
                            mix,
                            0.0,
                            1.0,
                            "%",
                            6,
                            selected_param,
                            is_editing,
                            Some('x'),
                            theme,
                        ))),
                ),
        )
        // Gain reduction meter
        .child(
            div()
                .rounded_xl()
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.border)
                .p_4()
                .child(render_gr_meter(simulated_gr, -30.0, theme)),
        )
        // Options row
        .child(
            div().flex().gap_4().children([
                // Auto makeup toggle
                div().flex_1().child(render_toggle(
                    "Auto Makeup",
                    auto_makeup,
                    7,
                    selected_param,
                    is_editing,
                    theme,
                )),
                // Link channels toggle
                div().flex_1().child(render_toggle(
                    "Link Channels",
                    link_channels,
                    8,
                    selected_param,
                    is_editing,
                    theme,
                )),
                // Sidechain HPF
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .p_3()
                    .rounded_xl()
                    .bg(theme.background_secondary)
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child("Sidechain HPF"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .child(format!("{:.0} Hz", sidechain_hpf_hz)),
                    ),
            ]),
        )
        // Keyboard hints
        .child(
            div()
                .p_3()
                .rounded_lg()
                .bg(theme.accent_muted)
                .border_1()
                .border_color(theme.accent)
                .flex()
                .flex_wrap()
                .gap_3()
                .text_xs()
                .text_color(theme.text_secondary)
                .child("[T]hreshold")
                .child("[R]atio")
                .child("[A]ttack")
                .child("R[e]lease")
                .child("[K]nee")
                .child("[M]akeup")
                .child("Mi[x]"),
        )
        .when(is_editing, |d| d.child(render_edit_hints(theme)))
}
