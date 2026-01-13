//! Fletcher-Munson Loudness Compensation Plugin UI

use super::common::{ParamSectionStyle, render_knob, render_section_title, render_toggle};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

/// State for rendering the Fletcher-Munson plugin
pub struct FletcherMunsonRenderState {
    pub playback_volume_db: f64,
    pub reference_level_db: f64,
    // Band 1 (sub-bass)
    pub band1_freq: f64,
    pub band1_q: f64,
    pub band1_max_gain: f64,
    pub band1_slope: f64,
    // Band 2 (mid-bass)
    pub band2_freq: f64,
    pub band2_q: f64,
    pub band2_max_gain: f64,
    pub band2_slope: f64,
    // Band 3 (presence)
    pub band3_freq: f64,
    pub band3_q: f64,
    pub band3_max_gain: f64,
    pub band3_slope: f64,
    // Band 4 (air/brilliance)
    pub band4_freq: f64,
    pub band4_q: f64,
    pub band4_max_gain: f64,
    pub band4_slope: f64,
    // Smoothing
    pub smoothing_ms: f64,
    // Auto-gain
    pub auto_gain_enabled: bool,
    pub auto_gain_max_db: f64,
    pub auto_gain_smoothing_ms: f64,
    pub auto_gain_loudness_type: i32,
    // UI state
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Calculate the current gain for a band based on volume delta
fn calculate_band_gain(slope: f64, max_gain: f64, volume_delta_db: f64) -> f64 {
    if volume_delta_db <= 0.0 {
        0.0
    } else {
        (slope * volume_delta_db).min(max_gain)
    }
}

/// Render the Fletcher-Munson plugin
/// Uses 4 peak bands with volume-dependent gains based on ISO 226 equal-loudness contours
pub fn render_fletcher_munson_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: FletcherMunsonRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let volume_delta_db = state.reference_level_db - state.playback_volume_db;

    // Calculate current gains for each band
    let band1_current = calculate_band_gain(state.band1_slope, state.band1_max_gain, volume_delta_db);
    let band2_current = calculate_band_gain(state.band2_slope, state.band2_max_gain, volume_delta_db);
    let band3_current = calculate_band_gain(state.band3_slope, state.band3_max_gain, volume_delta_db);
    let band4_current = calculate_band_gain(state.band4_slope, state.band4_max_gain, volume_delta_db);

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Description
        .child(
            div()
                .text_sm()
                .text_color(theme.text_muted)
                .child("4-band parametric EQ with gains that adjust based on playback volume, following ISO 226 equal-loudness contours."),
        )
        // Global parameters row
        .child(
            div()
                .flex()
                .gap_4()
                .items_center()
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Reference",
                    state.reference_level_db,
                    -40.0,
                    0.0,
                    "dB",
                    1, // Corrected index (was 0)
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Smooth",
                    state.smoothing_ms,
                    1.0,
                    200.0,
                    "ms",
                    3, // Corrected index (was 1)
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                // Show current volume info
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("Volume"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text_primary)
                                .child(format!("{:.1} dB", state.playback_volume_db)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("Delta"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if volume_delta_db > 0.0 {
                                    theme.warning
                                } else {
                                    theme.success
                                })
                                .child(format!("{:+.1} dB", volume_delta_db)),
                        ),
                ),
        )
        // Auto-gain section
        .child(
            div()
                .flex()
                .gap_4()
                .items_center()
                .child(render_section_title("AUTO GAIN", theme))
                .child(render_toggle(
                    entity.clone(),
                    plugin_idx,
                    "Enabled",
                    state.auto_gain_enabled,
                    4, // Auto-gain enabled index
                    state.selected_param,
                    state.is_editing,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Max Gain",
                    state.auto_gain_max_db,
                    0.0,
                    24.0,
                    "dB",
                    5, // Corrected index (was 21)
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "AG Smooth",
                    state.auto_gain_smoothing_ms,
                    10.0,
                    500.0,
                    "ms",
                    6, // Corrected index (was 22)
                    state.selected_param,
                    state.is_editing,
                    None,
                    theme,
                ))
                .child(render_toggle(
                    entity.clone(),
                    plugin_idx,
                    if state.auto_gain_loudness_type == 0 {
                        "Momentary"
                    } else {
                        "ShortTerm"
                    },
                    state.auto_gain_loudness_type == 1,
                    7, // Auto-gain type index
                    state.selected_param,
                    state.is_editing,
                    theme,
                )),
        )
        // Bands - 2x2 grid
        .child(
            div()
                .flex()
                .flex_col()
                .gap_4()
                // Row 1: Band 1 (Sub-bass) and Band 2 (Mid-bass)
                .child(
                    div()
                        .flex()
                        .gap_6()
                        .child(render_band_section(
                            entity.clone(),
                            plugin_idx,
                            "BAND 1 - SUB-BASS",
                            state.band1_freq,
                            state.band1_q,
                            state.band1_max_gain,
                            state.band1_slope,
                            band1_current,
                            8, // Corrected index (was 4/2)
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_band_section(
                            entity.clone(),
                            plugin_idx,
                            "BAND 2 - MID-BASS",
                            state.band2_freq,
                            state.band2_q,
                            state.band2_max_gain,
                            state.band2_slope,
                            band2_current,
                            12, // Corrected index (was 8/6)
                            state.selected_param,
                            state.is_editing,
                            theme,
                        )),
                )
                // Row 2: Band 3 (Presence) and Band 4 (Air)
                .child(
                    div()
                        .flex()
                        .gap_6()
                        .child(render_band_section(
                            entity.clone(),
                            plugin_idx,
                            "BAND 3 - PRESENCE",
                            state.band3_freq,
                            state.band3_q,
                            state.band3_max_gain,
                            state.band3_slope,
                            band3_current,
                            16, // Corrected index (was 12/10)
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_band_section(
                            entity,
                            plugin_idx,
                            "BAND 4 - AIR",
                            state.band4_freq,
                            state.band4_q,
                            state.band4_max_gain,
                            state.band4_slope,
                            band4_current,
                            20, // Corrected index (was 16/14)
                            state.selected_param,
                            state.is_editing,
                            theme,
                        )),
                ),
        )
        // .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}

/// Render a single band section with freq, Q, max gain, slope, and current gain display
fn render_band_section(
    entity: Entity<AppState>,
    plugin_idx: usize,
    title: &str,
    freq: f64,
    q: f64,
    max_gain: f64,
    slope: f64,
    current_gain: f64,
    param_offset: usize,
    selected_param: usize,
    is_editing: bool,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .param_section_base(theme)
        .child(render_section_title(title, theme))
        .child(
            div()
                .flex()
                .gap_3()
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Freq",
                    freq,
                    20.0,
                    20000.0,
                    "Hz",
                    param_offset,
                    selected_param,
                    is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Q",
                    q,
                    0.1,
                    10.0,
                    "",
                    param_offset + 1,
                    selected_param,
                    is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    entity.clone(),
                    plugin_idx,
                    "Max",
                    max_gain,
                    0.0,
                    24.0,
                    "dB",
                    param_offset + 2,
                    selected_param,
                    is_editing,
                    None,
                    theme,
                ))
                .child(render_knob(
                    entity,
                    plugin_idx,
                    "Slope",
                    slope,
                    0.0,
                    1.0,
                    "",
                    param_offset + 3,
                    selected_param,
                    is_editing,
                    None,
                    theme,
                ))
                // Current gain indicator
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child("Current"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if current_gain > 0.0 {
                                    theme.success
                                } else {
                                    theme.text_primary
                                })
                                .child(format!("{:+.1} dB", current_gain)),
                        ),
                ),
        )
}
