//! Gate Plugin UI Component
//!
//! Noise gate with:
//! - Threshold visualization with gate status
//! - Rotary knob controls
//! - Gain reduction meter

use super::common::{render_edit_hints, render_knob, render_section_header, render_toggle};
use super::level_meters::render_gr_meter;
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Divider, HStack, StackAlign, StackSpacing, Text, TextSize, TextWeight, VStack};
use sotf_audio_player::param_specs::gate::*;

/// State for rendering the Gate plugin
pub struct GateRenderState {
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
    pub mix: f64,
    pub link_channels: bool,
    pub sidechain_hpf_hz: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render the Gate plugin
pub fn render_gate_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: GateRenderState,
    theme: &Theme,
) -> impl IntoElement {
    // Simulated values (in real implementation, these would come from the audio engine)
    let simulated_input_db = -30.0; // Simulated input level
    let gate_open = simulated_input_db > state.threshold_db;
    let simulated_gr = if gate_open {
        0.0
    } else {
        (state.threshold_db - simulated_input_db).min(0.0) * state.ratio
    };

    // Normalize threshold for visual display (-60 to 0 dB range)
    let threshold_normalized = ((state.threshold_db + 60.0) / 60.0).clamp(0.0, 1.0) as f32;
    let input_normalized = ((simulated_input_db + 60.0) / 60.0).clamp(0.0, 1.0) as f32;

    // Cache theme colors for closures
    let gate_color = if gate_open {
        theme.success
    } else {
        theme.error
    };
    let gate_glow = if gate_open {
        Theme::opacity_20pct(theme.success)
    } else {
        Theme::opacity_20pct(theme.error)
    };
    let input_bar_color = gate_color;
    let gr_color = if simulated_gr.abs() > 1.0 {
        theme.error
    } else {
        theme.success
    };
    let border_color = theme.border;

    VStack::new()
        .spacing(StackSpacing::Lg)
        // Main section - Knobs and Threshold Display
        .child(
            HStack::new()
                .spacing(StackSpacing::Lg)
                .align(StackAlign::Start)
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
                                .child(
                                    Text::new("Input Level")
                                        .size(TextSize::Xs)
                                        .color(theme.text_muted),
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
                                        .child(
                                            div()
                                                .h_full()
                                                .w(relative(input_normalized))
                                                .bg(input_bar_color),
                                        )
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
                                        .child(
                                            Text::new("-60 dB")
                                                .size(TextSize::Xs)
                                                .color(theme.text_muted),
                                        )
                                        .child(gpui_ui_kit::Spacer::new())
                                        .child(
                                            Text::new("0 dB")
                                                .size(TextSize::Xs)
                                                .color(theme.text_muted),
                                        ),
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
                )
                // Parameters section with knobs
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(render_section_header("GATE SETTINGS", theme))
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Md)
                                .wrap(true)
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Threshold",
                                    state.threshold_db,
                                    THRESHOLD_MIN as f64,
                                    THRESHOLD_MAX as f64,
                                    "dB",
                                    0,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('t'),
                                    theme,
                                ))
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Ratio",
                                    state.ratio,
                                    RATIO_MIN as f64,
                                    RATIO_MAX as f64,
                                    ":1",
                                    1,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('r'),
                                    theme,
                                ))
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Attack",
                                    state.attack_ms,
                                    ATTACK_MIN as f64,
                                    ATTACK_MAX as f64,
                                    "ms",
                                    2,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('a'),
                                    theme,
                                ))
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Release",
                                    state.release_ms,
                                    RELEASE_MIN as f64,
                                    RELEASE_MAX as f64,
                                    "ms",
                                    3,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('e'),
                                    theme,
                                ))
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Mix",
                                    state.mix,
                                    MIX_MIN as f64,
                                    MIX_MAX as f64,
                                    "%",
                                    4,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('m'),
                                    theme,
                                ))
                                .build()
                                .justify_center(),
                        )
                        // Options row
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Lg)
                                // Link channels toggle
                                .child(render_toggle(
                                    entity.clone(),
                                    plugin_idx,
                                    "Link Channels",
                                    state.link_channels,
                                    5,
                                    state.selected_param,
                                    state.is_editing,
                                    theme,
                                ))
                                .build()
                                .mt_4()
                                .justify_center(),
                        )
                        .child(
                            // Sidechain HPF display
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .align(StackAlign::Center)
                                .child(
                                    Text::new("Sidechain HPF")
                                        .size(TextSize::Xs)
                                        .color(theme.text_muted),
                                )
                                .child(
                                    Text::new(format!("{:.0} Hz", state.sidechain_hpf_hz))
                                        .size(TextSize::Sm)
                                        .weight(TextWeight::Bold)
                                        .color(theme.text_primary),
                                )
                                .build()
                                .p_2()
                                .rounded_lg()
                                .bg(theme.background)
                                .mt_2(),
                        )
                        .build()
                        .flex_1()
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
                        .child(
                            Text::new("THRESHOLD")
                                .size(TextSize::Xs)
                                .color(theme.text_muted),
                        )
                        .child(
                            div()
                                .text_3xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.warning)
                                .child(format!("{:.1} dB", state.threshold_db)),
                        ),
                )
                .child(
                    Divider::vertical()
                        .color(border_color)
                        .build_simple()
                        .h(px(40.0)),
                )
                .child(
                    VStack::new()
                        .align(StackAlign::Center)
                        .child(
                            Text::new("REDUCTION")
                                .size(TextSize::Xs)
                                .color(theme.text_muted),
                        )
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
}
