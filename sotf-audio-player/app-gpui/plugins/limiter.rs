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
use gpui_ui_kit::{Divider, HStack, StackAlign, StackSpacing, Text, TextSize, VStack};

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

    // Cache theme colors for closures
    let peak_color = if simulated_peak > threshold_db { theme.error } else { theme.success };
    let border_color = theme.border;

    VStack::new()
        .spacing(StackSpacing::Lg)
        // Main section - Sliders, Transfer Curve and Peak Meter
        .child(
            HStack::new()
                .spacing(StackSpacing::Lg)
                // Parameters section with vertical sliders
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(render_section_header("LIMITER SETTINGS", theme))
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Md)
                                .child(render_vertical_slider(
                                    plugin_idx, "Ceiling", threshold_db, -12.0, 0.0, "dB",
                                    0, selected_param, is_editing, Some('c'), theme,
                                ))
                                .child(render_vertical_slider(
                                    plugin_idx, "Release", release_ms, 10.0, 1000.0, "ms",
                                    1, selected_param, is_editing, Some('r'), theme,
                                ))
                                .child(render_vertical_slider(
                                    plugin_idx, "Mix", mix, 0.0, 1.0, "%",
                                    2, selected_param, is_editing, Some('m'), theme,
                                ))
                                .build()
                                .justify_center(),
                        )
                        .build()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .p_4(),
                )
                // Transfer curve
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(render_transfer_curve(threshold_db, f64::INFINITY, 0.0, true, theme))
                        .build()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .p_4(),
                )
                // Peak meter
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(render_peak_meter(simulated_peak, threshold_db, theme))
                        .build()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .p_4(),
                ),
        )
        // Large ceiling display
        .child(
            HStack::new()
                .spacing(StackSpacing::Xl)
                .child(
                    VStack::new()
                        .align(StackAlign::Center)
                        .child(Text::new("CEILING").size(TextSize::Xs).color(theme.text_muted))
                        .child(
                            div()
                                .text_3xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.warning)
                                .child(format!("{:.2} dB", threshold_db)),
                        ),
                )
                .child(Divider::vertical().color(border_color).build_simple().h(px(40.0)))
                .child(
                    VStack::new()
                        .align(StackAlign::Center)
                        .child(Text::new("TRUE PEAK").size(TextSize::Xs).color(theme.text_muted))
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(peak_color)
                                .child(format!("{:.1} dB", simulated_peak)),
                        ),
                )
                .build()
                .justify_center()
                .p_4()
                .rounded_xl()
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.border),
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
            HStack::new()
                .spacing(StackSpacing::Lg)
                .child(Text::new("[C]eiling").size(TextSize::Xs).color(theme.text_secondary))
                .child(Text::new("[R]elease").size(TextSize::Xs).color(theme.text_secondary))
                .child(Text::new("[M]ix").size(TextSize::Xs).color(theme.text_secondary))
                .child(Text::new("1-3: Quick select").size(TextSize::Xs).color(theme.text_secondary))
                .build()
                .p_3()
                .rounded_lg()
                .bg(theme.accent_muted)
                .border_1()
                .border_color(theme.accent),
        )
        .when(is_editing, |d| d.child(render_edit_hints(theme)))
}
