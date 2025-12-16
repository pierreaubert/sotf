//! Compressor Plugin UI Component
//!
//! Professional compressor visualization with:
//! - Transfer curve display
//! - Gain reduction meter
//! - Vertical sliders for main dynamics controls
//! - Rotary knobs for secondary parameters

use super::common::{
    render_edit_hints, render_knob, render_section_header, render_toggle, render_transfer_curve,
    render_vertical_slider, ParamSectionStyle,
};
use super::level_meters::render_gr_meter;
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::param_specs::compressor::*;
use sotf_plugins::CompressorData;

/// State for rendering the Compressor plugin
pub struct CompressorRenderState<'a> {
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
    pub knee_db: f64,
    pub makeup_gain_db: f64,
    pub mix: f64,
    pub auto_makeup: bool,
    pub link_channels: bool,
    pub sidechain_hpf_hz: f64,
    pub is_editing: bool,
    pub selected_param: usize,
    pub data: Option<&'a CompressorData>,
}

// Sidechain HPF UI range (40-160Hz as per user request)
const SIDECHAIN_HPF_UI_MIN: f64 = 40.0;
const SIDECHAIN_HPF_UI_MAX: f64 = 160.0;

// Fixed height for all columns to ensure consistent layout
// Height sized to fit OUTPUT column with two stacked knobs
const COLUMN_HEIGHT: f32 = 380.0;

/// Render the Compressor plugin
pub fn render_compressor_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: CompressorRenderState,
    theme: &Theme,
) -> impl IntoElement {
    // Get max gain reduction from all channels
    let gr_db = if let Some(data) = state.data {
        // Find maximum reduction (since GR is positive dB value, we want the max)
        data.gain_reduction_db
            .iter()
            .cloned()
            .fold(0.0_f32, f32::max) as f64
    } else {
        0.0
    };

    // Since gain_reduction_db is stored as the attenuation amount (e.g. 6.0 for -6dB),
    // we want to display it as a negative value for the meter
    let meter_value = -gr_db;

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Main section - Four columns side by side, all same height
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
                                ))
                                .child(render_vertical_slider(
                                    entity.clone(),
                                    plugin_idx,
                                    "Knee",
                                    state.knee_db,
                                    KNEE_MIN as f64,
                                    KNEE_MAX as f64,
                                    "dB",
                                    4,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('k'),
                                    theme,
                                )),
                        ),
                )
                // Column 2: Link channels, Auto makeup, Makeup knob
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .h(px(COLUMN_HEIGHT))
                        .param_section_style_lg(theme)
                        .child(render_section_header("GAIN", theme))
                        // Toggles
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(render_toggle(
                                    entity.clone(),
                                    plugin_idx,
                                    "Link Channels",
                                    state.link_channels,
                                    8, // param index for link_channels
                                    state.selected_param,
                                    state.is_editing,
                                    theme,
                                ))
                                .child(render_toggle(
                                    entity.clone(),
                                    plugin_idx,
                                    "Auto Makeup",
                                    state.auto_makeup,
                                    7, // param index for auto_makeup
                                    state.selected_param,
                                    state.is_editing,
                                    theme,
                                )),
                        )
                        // Spacer to push knob down
                        .child(div().flex_1())
                        // Makeup gain knob (direct child, no wrapper)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Makeup",
                            state.makeup_gain_db,
                            MAKEUP_GAIN_MIN as f64,
                            MAKEUP_GAIN_MAX as f64,
                            "dB",
                            5,
                            state.selected_param,
                            state.is_editing,
                            Some('m'),
                            theme,
                        )),
                )
                // Column 3: Mix and Sidechain HPF knobs
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .h(px(COLUMN_HEIGHT))
                        .param_section_style_lg(theme)
                        .child(render_section_header("OUTPUT", theme))
                        // Spacer to push knobs down
                        .child(div().flex_1())
                        // Mix knob (direct child)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Mix",
                            state.mix * 100.0, // Convert 0-1 to 0-100%
                            MIX_MIN as f64 * 100.0,
                            MIX_MAX as f64 * 100.0,
                            "%",
                            6,
                            state.selected_param,
                            state.is_editing,
                            Some('x'),
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
                            9,
                            state.selected_param,
                            state.is_editing,
                            Some('s'),
                            theme,
                        )),
                )
                // Column 4: Transfer curve (top) and Gain reduction meter (bottom)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .h(px(COLUMN_HEIGHT))
                        .param_section_style_lg(theme)
                        .child(render_section_header("METER", theme))
                        // Transfer curve
                        .child(
                            div()
                                .flex()
                                .justify_center()
                                .child(render_transfer_curve(
                                    state.threshold_db,
                                    state.ratio,
                                    state.knee_db,
                                    false,
                                    theme,
                                )),
                        )
                        // Gain reduction meter
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.text_secondary)
                                        .child("Gain Reduction"),
                                )
                                .child(render_gr_meter(meter_value, -30.0, theme)),
                        ),
                ),
        )
        .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}
