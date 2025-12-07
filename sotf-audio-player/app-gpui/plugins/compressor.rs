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
use gpui_ui_kit::{HStack, StackAlign, StackSpacing, Text, TextSize, TextWeight, VStack};

/// Render the Compressor plugin
#[allow(clippy::too_many_arguments)]
pub fn render_compressor_plugin(
    plugin_idx: usize,
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

    VStack::new()
        .spacing(StackSpacing::Lg)
        // Main section - Sliders and Transfer Curve side by side
        .child(
            HStack::new()
                .spacing(StackSpacing::Lg)
                // Parameters section with vertical sliders
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(render_section_header("DYNAMICS CONTROL", theme))
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Sm)
                                .wrap(true)
                                .child(render_vertical_slider(
                                    plugin_idx, "Threshold", threshold_db, -60.0, 0.0, "dB",
                                    0, selected_param, is_editing, Some('t'), theme,
                                ))
                                .child(render_vertical_slider(
                                    plugin_idx, "Ratio", ratio, 1.0, 20.0, ":1",
                                    1, selected_param, is_editing, Some('r'), theme,
                                ))
                                .child(render_vertical_slider(
                                    plugin_idx, "Attack", attack_ms, 0.1, 100.0, "ms",
                                    2, selected_param, is_editing, Some('a'), theme,
                                ))
                                .child(render_vertical_slider(
                                    plugin_idx, "Release", release_ms, 10.0, 1000.0, "ms",
                                    3, selected_param, is_editing, Some('e'), theme,
                                ))
                                .child(render_vertical_slider(
                                    plugin_idx, "Knee", knee_db, 0.0, 12.0, "dB",
                                    4, selected_param, is_editing, Some('k'), theme,
                                ))
                                .child(render_vertical_slider(
                                    plugin_idx, "Makeup", makeup_gain_db, 0.0, 24.0, "dB",
                                    5, selected_param, is_editing, Some('m'), theme,
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
                // Transfer curve and options
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Md)
                        .align(StackAlign::Center)
                        .child(render_transfer_curve(threshold_db, ratio, knee_db, false, theme))
                        // Mix slider
                        .child(render_vertical_slider(
                            plugin_idx, "Mix", mix, 0.0, 1.0, "%",
                            6, selected_param, is_editing, Some('x'), theme,
                        ))
                        .build()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .p_4(),
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
            HStack::new()
                .spacing(StackSpacing::Lg)
                // Auto makeup toggle
                .child(div().flex_1().child(render_toggle(
                    plugin_idx, "Auto Makeup", auto_makeup, 7, selected_param, is_editing, theme,
                )))
                // Link channels toggle
                .child(div().flex_1().child(render_toggle(
                    plugin_idx, "Link Channels", link_channels, 8, selected_param, is_editing, theme,
                )))
                // Sidechain HPF
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Xs)
                        .align(StackAlign::Center)
                        .child(Text::new("Sidechain HPF").size(TextSize::Xs).color(theme.text_muted))
                        .child(Text::new(format!("{:.0} Hz", sidechain_hpf_hz))
                            .size(TextSize::Sm)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary))
                        .build()
                        .flex_1()
                        .p_3()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border),
                ),
        )
        // Keyboard hints
        .child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .wrap(true)
                .child(Text::new("[T]hreshold").size(TextSize::Xs).color(theme.text_secondary))
                .child(Text::new("[R]atio").size(TextSize::Xs).color(theme.text_secondary))
                .child(Text::new("[A]ttack").size(TextSize::Xs).color(theme.text_secondary))
                .child(Text::new("R[e]lease").size(TextSize::Xs).color(theme.text_secondary))
                .child(Text::new("[K]nee").size(TextSize::Xs).color(theme.text_secondary))
                .child(Text::new("[M]akeup").size(TextSize::Xs).color(theme.text_secondary))
                .child(Text::new("Mi[x]").size(TextSize::Xs).color(theme.text_secondary))
                .build()
                .p_3()
                .rounded_lg()
                .bg(theme.accent_muted)
                .border_1()
                .border_color(theme.accent),
        )
        .when(is_editing, |d| d.child(render_edit_hints(theme)))
}
