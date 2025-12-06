//! Limiter Plugin UI Component
//!
//! Brick-wall limiter with:
//! - Transfer curve display
//! - Gain reduction meter
//! - Peak meter with ceiling indicator

use super::common::{
    render_edit_hints, render_gr_meter, render_peak_meter, render_section_header,
    render_transfer_curve, render_vertical_slider,
};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

/// Render the Limiter plugin
pub fn render_limiter_plugin(
    plugin_idx: usize,
    threshold_db: f64,
    release_ms: f64,
    mix: f64,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
) -> impl IntoElement {
    // Simulated values (in real implementation, these would come from the audio engine)
    let simulated_gr = if threshold_db < -1.0 {
        (threshold_db + 1.0) * 2.0
    } else {
        0.0
    };
    let simulated_peak = threshold_db - 3.0; // Simulated peak level below ceiling

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Main section - Sliders, Transfer Curve and Peak Meter
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
                        .child(render_section_header("LIMITER SETTINGS", theme))
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .justify_center()
                                .child(render_vertical_slider(
                                    plugin_idx,
                                    "Ceiling",
                                    threshold_db,
                                    -12.0,
                                    0.0,
                                    "dB",
                                    0,
                                    selected_param,
                                    is_editing,
                                    Some('c'),
                                    theme,
                                ))
                                .child(render_vertical_slider(
                                    plugin_idx,
                                    "Release",
                                    release_ms,
                                    10.0,
                                    1000.0,
                                    "ms",
                                    1,
                                    selected_param,
                                    is_editing,
                                    Some('r'),
                                    theme,
                                ))
                                .child(render_vertical_slider(
                                    plugin_idx,
                                    "Mix",
                                    mix,
                                    0.0,
                                    1.0,
                                    "%",
                                    2,
                                    selected_param,
                                    is_editing,
                                    Some('m'),
                                    theme,
                                )),
                        ),
                )
                // Transfer curve
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
                        .child(render_transfer_curve(
                            threshold_db,
                            f64::INFINITY, // Infinite ratio for limiter
                            0.0,           // No knee
                            true,          // Is limiter
                            theme,
                        )),
                )
                // Peak meter
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
                        .child(render_peak_meter(simulated_peak, threshold_db, theme)),
                ),
        )
        // Large ceiling display
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
                                .child("CEILING"),
                        )
                        .child(
                            div()
                                .text_3xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.warning)
                                .child(format!("{:.2} dB", threshold_db)),
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
                                .child("TRUE PEAK"),
                        )
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if simulated_peak > threshold_db {
                                    theme.error
                                } else {
                                    theme.success
                                })
                                .child(format!("{:.1} dB", simulated_peak)),
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
                .child(render_gr_meter(simulated_gr, -20.0, theme)),
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
                .gap_4()
                .text_xs()
                .text_color(theme.text_secondary)
                .child("[C]eiling")
                .child("[R]elease")
                .child("[M]ix")
                .child("1-3: Quick select"),
        )
        .when(is_editing, |d| d.child(render_edit_hints(theme)))
}
