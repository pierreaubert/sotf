//! Compressor Plugin UI Component
//!
//! Professional compressor visualization with:
//! - Transfer curve display
//! - Gain reduction meter
//! - Vertical sliders for main dynamics controls
//! - Rotary knobs for secondary parameters

use super::common::{
    render_edit_hints, render_knob, render_section_title, render_toggle,
    render_transfer_curve_sized, render_vertical_slider_sized,
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

// Column layout constants
const METER_WIDTH: f32 = 180.0; // Width for transfer curve and GR meter
const SLIDER_HEIGHT: f32 = 200.0; // Height for vertical sliders

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
        // Main section - columns side by side
        .child(
            div()
                .flex()
                .gap_6()
                // Column 1: Vertical sliders for main dynamics controls
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("DYNAMICS", theme))
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(render_vertical_slider_sized(
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
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                ))
                                .child(render_vertical_slider_sized(
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
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                ))
                                .child(render_vertical_slider_sized(
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
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                ))
                                .child(render_vertical_slider_sized(
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
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                ))
                                .child(render_vertical_slider_sized(
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
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                )),
                        ),
                )
                // Column 2: Link channels, Auto makeup, Makeup knob - with vertical distribution
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .justify_between() // Distribute elements vertically
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(render_section_title("GAIN", theme))
                                .child(render_toggle(
                                    entity.clone(),
                                    plugin_idx,
                                    "Link Channels",
                                    state.link_channels,
                                    8,
                                    state.selected_param,
                                    state.is_editing,
                                    theme,
                                ))
                                .child(render_toggle(
                                    entity.clone(),
                                    plugin_idx,
                                    "Auto Makeup",
                                    state.auto_makeup,
                                    7,
                                    state.selected_param,
                                    state.is_editing,
                                    theme,
                                )),
                        )
                        // Makeup gain knob at the bottom
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
                // Column 3: Mix and Sidechain HPF knobs - with vertical distribution
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .justify_between() // Distribute elements vertically
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(render_section_title("OUTPUT", theme))
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Mix",
                                    state.mix * 100.0,
                                    MIX_MIN as f64 * 100.0,
                                    MIX_MAX as f64 * 100.0,
                                    "%",
                                    6,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('x'),
                                    theme,
                                )),
                        )
                        // SC HPF knob at the bottom
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
                // Column 4: Transfer curve and Gain reduction meter - aligned width
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .w(px(METER_WIDTH))
                        .gap_2()
                        .child(render_section_title("METER", theme))
                        // Transfer curve section - grows to fill space
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1() // Grow to fill available space
                                .child(render_transfer_curve_sized(
                                    state.threshold_db,
                                    state.ratio,
                                    state.knee_db,
                                    false,
                                    METER_WIDTH,
                                    theme,
                                )),
                        )
                        // Gain reduction meter at the bottom
                        .child(render_gr_meter(meter_value, -30.0, theme)),
                ),
        )
        .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}
