//! Gate Plugin UI Component
//!
//! Noise gate with:
//! - Threshold visualization with gate status
//! - Vertical sliders and rotary knob controls
//! - Gain reduction meter

use super::common::{
    render_knob, render_section_header, render_toggle_button, render_vertical_slider,
    ParamSectionStyle,
};
use super::level_meters::render_gr_meter;
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::param_specs::gate::*;
use sotf_plugins::GateData;

/// State for rendering the Gate plugin
pub struct GateRenderState<'a> {
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
    pub mix: f64,
    pub link_channels: bool,
    pub sidechain_hpf_hz: f64,
    pub is_editing: bool,
    pub selected_param: usize,
    pub data: Option<&'a GateData>,
}

// Sidechain HPF UI range (40-160Hz)
const SIDECHAIN_HPF_UI_MIN: f64 = 40.0;
const SIDECHAIN_HPF_UI_MAX: f64 = 160.0;

// Fixed height for all columns to ensure consistent layout
// Height sized to fit columns with stacked knobs
const COLUMN_HEIGHT: f32 = 380.0;

/// Render the Gate plugin
pub fn render_gate_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: GateRenderState,
    theme: &Theme,
) -> impl IntoElement {
    // Get metering data
    let (input_db, is_open, attenuation_db) = if let Some(data) = state.data {
        let max_input = data
            .input_levels_db
            .iter()
            .cloned()
            .fold(-100.0_f32, f32::max) as f64;
        let max_attenuation = data
            .attenuation_db
            .iter()
            .cloned()
            .fold(0.0_f32, f32::max) as f64;
        (max_input, data.is_open, max_attenuation)
    } else {
        (-100.0, false, 0.0)
    };

    // Normalize threshold for visual display (-80 to 0 dB range)
    let threshold_normalized = ((state.threshold_db + 80.0) / 80.0).clamp(0.0, 1.0) as f32;
    let input_normalized = ((input_db + 80.0) / 80.0).clamp(0.0, 1.0) as f32;

    // Cache theme colors
    let gate_color = if is_open {
        theme.success
    } else {
        theme.error
    };
    let gate_glow = if is_open {
        Theme::opacity_20pct(theme.success)
    } else {
        Theme::opacity_20pct(theme.error)
    };

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Main section - Three columns side by side, all same height
        .child(
            div()
                .flex()
                .gap_4()
                .items_start()
                // Column 1: Vertical sliders for main dynamics controls
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .h(px(COLUMN_HEIGHT))
                        .param_section_style_lg(theme)
                        .child(render_section_header("DYNAMICS", theme))
                        .child(
                            div()
                                .flex()
                                .flex_1()
                                .gap_2()
                                .child(render_vertical_slider(
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
                                .child(render_vertical_slider(
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
                                .child(render_vertical_slider(
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
                                .child(render_vertical_slider(
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
                                )),
                        ),
                )
                // Column 2: OUTPUT header with Link Channels on right, then knobs at bottom
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .h(px(COLUMN_HEIGHT))
                        .param_section_style_lg(theme)
                        // Header row with OUTPUT and Link Channels
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .w_full()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.text_secondary)
                                        .child("OUTPUT"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child("Link Ch."),
                                ),
                        )
                        // Toggle button below header
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .mt_1()
                                .child(render_toggle_button(
                                    entity.clone(),
                                    plugin_idx,
                                    state.link_channels,
                                    5,
                                    state.selected_param,
                                    state.is_editing,
                                    theme,
                                )),
                        )
                        // Spacer to push knobs to bottom
                        .child(div().flex_1())
                        // Mix knob (direct child)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Mix",
                            state.mix * 100.0,
                            MIX_MIN as f64 * 100.0,
                            MIX_MAX as f64 * 100.0,
                            "%",
                            4,
                            state.selected_param,
                            state.is_editing,
                            Some('m'),
                            theme,
                        ))
                        // SC HPF knob (direct child)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "SC HPF",
                            state.sidechain_hpf_hz,
                            SIDECHAIN_HPF_UI_MIN,
                            SIDECHAIN_HPF_UI_MAX,
                            "Hz",
                            6,
                            state.selected_param,
                            state.is_editing,
                            Some('s'),
                            theme,
                        )),
                )
                // Column 3: Gate status, Input level meter, GR meter (spaced out)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .h(px(COLUMN_HEIGHT))
                        .param_section_style_lg(theme)
                        .child(render_section_header("METER", theme))
                        // Gate status indicator
                        .child(
                            div()
                                .flex()
                                .justify_center()
                                .mt_2()
                                .child(
                                    div()
                                        .w(px(60.0))
                                        .h(px(60.0))
                                        .rounded_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(gate_glow)
                                        .border_3()
                                        .border_color(gate_color)
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(gate_color)
                                                .child(if is_open { "OPEN" } else { "CLOSED" }),
                                        ),
                                ),
                        )
                        // Spacer
                        .child(div().flex_1())
                        // Input level meter with threshold marker
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .w_full()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
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
                                        .child(
                                            div()
                                                .h_full()
                                                .w(relative(input_normalized))
                                                .bg(gate_color),
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
                                    div()
                                        .flex()
                                        .justify_between()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child("-80")
                                        .child("0 dB"),
                                ),
                        )
                        // Spacer
                        .child(div().h(px(16.0)))
                        // Gain reduction meter (no label, aligned at bottom)
                        .child(render_gr_meter(-attenuation_db, -40.0, theme)),
                ),
        )
        .when(state.is_editing, |d| {
            d.child(
                div()
                    .mt_4()
                    .p_3()
                    .rounded_lg()
                    .bg(theme.background_secondary)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .gap_4()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child("↑/↓: Select")
                    .child("←/→: Adjust")
                    .child("[/]: Large step")
                    .child("Enter: Done"),
            )
        })
}
