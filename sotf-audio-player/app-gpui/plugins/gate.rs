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
use gpui_ui_kit::{Divider, HStack, StackAlign, StackSpacing, Text, TextSize, TextWeight, VStack};

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

    // Cache theme colors for closures
    let gate_color = if gate_open { theme.success } else { theme.error };
    let gate_glow = if gate_open { rgba(0x22c55e33) } else { rgba(0xef444433) };
    let input_bar_color = gate_color;
    let gr_color = if simulated_gr.abs() > 1.0 { theme.error } else { theme.success };
    let border_color = theme.border;

    VStack::new()
        .spacing(StackSpacing::Lg)
        // Main section - Sliders and Threshold Display
        .child(
            HStack::new()
                .spacing(StackSpacing::Lg)
                // Parameters section with vertical sliders
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(render_section_header("GATE SETTINGS", theme))
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Sm)
                                .child(render_vertical_slider(
                                    plugin_idx, "Threshold", threshold_db, -60.0, 0.0, "dB",
                                    0, selected_param, is_editing, Some('t'), theme,
                                ))
                                .child(render_vertical_slider(
                                    plugin_idx, "Ratio", ratio, 1.0, 10.0, ":1",
                                    1, selected_param, is_editing, Some('r'), theme,
                                ))
                                .child(render_vertical_slider(
                                    plugin_idx, "Attack", attack_ms, 0.1, 50.0, "ms",
                                    2, selected_param, is_editing, Some('a'), theme,
                                ))
                                .child(render_vertical_slider(
                                    plugin_idx, "Release", release_ms, 10.0, 500.0, "ms",
                                    3, selected_param, is_editing, Some('e'), theme,
                                ))
                                .child(render_vertical_slider(
                                    plugin_idx, "Mix", mix, 0.0, 1.0, "%",
                                    4, selected_param, is_editing, Some('m'), theme,
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
                // Gate threshold visualization
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
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
                                .bg(gate_glow)
                                .border_4()
                                .border_color(gate_color)
                                .child(
                                    Text::new(if gate_open { "OPEN" } else { "CLOSED" })
                                        .size(TextSize::Sm)
                                        .weight(TextWeight::Bold)
                                        .color(gate_color),
                                ),
                        )
                        // Threshold meter
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(Text::new("Input Level").size(TextSize::Xs).color(theme.text_muted))
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
                                        .child(div().h_full().w(relative(input_normalized)).bg(input_bar_color))
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
                                    HStack::new()
                                        .spacing(StackSpacing::None)
                                        .child(Text::new("-60 dB").size(TextSize::Xs).color(theme.text_muted))
                                        .child(gpui_ui_kit::Spacer::new())
                                        .child(Text::new("0 dB").size(TextSize::Xs).color(theme.text_muted)),
                                )
                                .build()
                                .w_full()
                                .mt_4(),
                        )
                        .build()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .p_4(),
                ),
        )
        // Large threshold display
        .child(
            HStack::new()
                .spacing(StackSpacing::Xl)
                .child(
                    VStack::new()
                        .align(StackAlign::Center)
                        .child(Text::new("THRESHOLD").size(TextSize::Xs).color(theme.text_muted))
                        .child(
                            div()
                                .text_3xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.warning)
                                .child(format!("{:.1} dB", threshold_db)),
                        ),
                )
                .child(Divider::vertical().color(border_color).build_simple().h(px(40.0)))
                .child(
                    VStack::new()
                        .align(StackAlign::Center)
                        .child(Text::new("REDUCTION").size(TextSize::Xs).color(theme.text_muted))
                        .child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(gr_color)
                                .child(format!("{:.1} dB", simulated_gr)),
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
                .child(render_gr_meter(simulated_gr, -40.0, theme)),
        )
        // Options row
        .child(
            HStack::new()
                .spacing(StackSpacing::Lg)
                // Link channels toggle
                .child(div().flex_1().child(render_toggle(
                    plugin_idx, "Link Channels", link_channels, 5, selected_param, is_editing, theme,
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
                .child(Text::new("[M]ix").size(TextSize::Xs).color(theme.text_secondary))
                .build()
                .p_3()
                .rounded_lg()
                .bg(theme.accent_muted)
                .border_1()
                .border_color(theme.accent),
        )
        .when(is_editing, |d| d.child(render_edit_hints(theme)))
}
