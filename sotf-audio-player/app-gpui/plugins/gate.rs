//! Gate Plugin UI Component
//!
//! Noise gate with:
//! - Threshold visualization with gate status
//! - Vertical slider controls
//! - Gain reduction meter

use super::common::{
    render_edit_hints, render_gr_meter, render_section_header, render_toggle,
    render_vertical_slider,
};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

/// Render the Gate plugin
#[allow(clippy::too_many_arguments)]
pub fn render_gate_plugin(
    plugin_idx: usize,
    threshold_db: f64,
    ratio: f64,
    attack_ms: f64,
    release_ms: f64,
    mix: f64,
    link_channels: bool,
    sidechain_hpf_hz: f64,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
) -> impl IntoElement {
    // Simulated values (in real implementation, these would come from the audio engine)
    let simulated_input_db = -30.0; // Simulated input level
    let gate_open = simulated_input_db > threshold_db;
    let simulated_gr = if gate_open {
        0.0
    } else {
        (threshold_db - simulated_input_db).min(0.0) * ratio
    };

    // Normalize threshold for visual display (-60 to 0 dB range)
    let threshold_normalized = ((threshold_db + 60.0) / 60.0).clamp(0.0, 1.0) as f32;
    let input_normalized = ((simulated_input_db + 60.0) / 60.0).clamp(0.0, 1.0) as f32;

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Main section - Sliders and Threshold Display
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
                        .child(render_section_header("GATE SETTINGS", theme))
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .justify_center()
                                .child(render_vertical_slider(
                                    plugin_idx,
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
                                    plugin_idx,
                                    "Ratio",
                                    ratio,
                                    1.0,
                                    10.0,
                                    ":1",
                                    1,
                                    selected_param,
                                    is_editing,
                                    Some('r'),
                                    theme,
                                ))
                                .child(render_vertical_slider(
                                    plugin_idx,
                                    "Attack",
                                    attack_ms,
                                    0.1,
                                    50.0,
                                    "ms",
                                    2,
                                    selected_param,
                                    is_editing,
                                    Some('a'),
                                    theme,
                                ))
                                .child(render_vertical_slider(
                                    plugin_idx,
                                    "Release",
                                    release_ms,
                                    10.0,
                                    500.0,
                                    "ms",
                                    3,
                                    selected_param,
                                    is_editing,
                                    Some('e'),
                                    theme,
                                ))
                                .child(render_vertical_slider(
                                    plugin_idx,
                                    "Mix",
                                    mix,
                                    0.0,
                                    1.0,
                                    "%",
                                    4,
                                    selected_param,
                                    is_editing,
                                    Some('m'),
                                    theme,
                                )),
                        ),
                )
                // Gate threshold visualization
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
                        .items_center()
                        .child(render_section_header("GATE STATUS", theme))
                        // Large gate open/closed indicator
                        .child(
                            div()
                                .w(px(80.0))
                                .h(px(80.0))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(if gate_open {
                                    rgba(0x22c55e33) // Green glow
                                } else {
                                    rgba(0xef444433) // Red glow
                                })
                                .border_4()
                                .border_color(if gate_open {
                                    theme.success
                                } else {
                                    theme.error
                                })
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(if gate_open {
                                            theme.success
                                        } else {
                                            theme.error
                                        })
                                        .child(if gate_open { "OPEN" } else { "CLOSED" }),
                                ),
                        )
                        // Threshold meter
                        .child(
                            div()
                                .w_full()
                                .mt_4()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .mb_1()
                                        .child("Input Level"),
                                )
                                .child(
                                    div()
                                        .h(px(12.0))
                                        .w_full()
                                        .bg(theme.background)
                                        .rounded_md()
                                        .border_1()
                                        .border_color(theme.border)
                                        .relative()
                                        .overflow_hidden()
                                        // Input level bar
                                        .child(div().h_full().w(relative(input_normalized)).bg(
                                            if gate_open {
                                                theme.success
                                            } else {
                                                theme.error
                                            },
                                        ))
                                        // Threshold marker
                                        .child(
                                            div()
                                                .absolute()
                                                .left(relative(threshold_normalized))
                                                .top_0()
                                                .bottom_0()
                                                .w(px(2.0))
                                                .bg(theme.warning),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .mt_1()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child("-60 dB")
                                        .child("0 dB"),
                                ),
                        ),
                ),
        )
        // Large threshold display
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .gap_6()
                .p_4()
                .rounded_xl()
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.border)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("THRESHOLD"),
                        )
                        .child(
                            div()
                                .text_3xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.warning)
                                .child(format!("{:.1} dB", threshold_db)),
                        ),
                )
                .child(div().w(px(1.0)).h(px(40.0)).bg(theme.border))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("REDUCTION"),
                        )
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if simulated_gr.abs() > 1.0 {
                                    theme.error
                                } else {
                                    theme.success
                                })
                                .child(format!("{:.1} dB", simulated_gr)),
                        ),
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
                .child(render_gr_meter(simulated_gr, -40.0, theme)),
        )
        // Options row
        .child(
            div().flex().gap_4().children([
                // Link channels toggle
                div().flex_1().child(render_toggle(
                    plugin_idx,
                    "Link Channels",
                    link_channels,
                    5,
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
                .child("[M]ix"),
        )
        .when(is_editing, |d| d.child(render_edit_hints(theme)))
}
