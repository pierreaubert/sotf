//! Loudness Plugin UI Components

use super::common::{ParamSectionStyle, render_knob, render_section_title, render_toggle};
use super::level_meters::render_lufs_with_true_peak;
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::param_specs::loudness_compensation::*;

/// State for rendering the Loudness Compensation plugin
pub struct LoudnessCompensationRenderState {
    pub low_freq: f64,
    pub low_gain: f64,
    pub high_freq: f64,
    pub high_gain: f64,
    pub auto_gain_enabled: bool,
    pub auto_gain_max_db: f64,
    pub auto_gain_smoothing_ms: f64,
    pub auto_gain_current_db: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render the Loudness Compensation plugin
/// Uses two shelving filters (low-shelf and high-shelf) for Fletcher-Munson compensation
pub fn render_loudness_compensation_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: LoudnessCompensationRenderState,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        // Description
        .child(
            div()
                .text_sm()
                .text_color(theme.text_muted)
                .child("Boosts bass and treble at low listening volumes to compensate for the ear's reduced sensitivity at those frequencies."),
        )
        // Low Shelf and High Shelf sections side by side
        .child(
            div()
                .flex()
                .gap_6()
                // Low Shelf section
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("LOW SHELF", theme))
                        .child(
                            div()
                                .flex()
                                .gap_4()
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Frequency",
                                    state.low_freq,
                                    LOW_FREQ_MIN as f64,
                                    LOW_FREQ_MAX as f64,
                                    "Hz",
                                    0,
                                    state.selected_param,
                                    state.is_editing,
                                    None,
                                    theme,
                                ))
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Gain",
                                    state.low_gain,
                                    LOW_GAIN_MIN as f64,
                                    LOW_GAIN_MAX as f64,
                                    "dB",
                                    1,
                                    state.selected_param,
                                    state.is_editing,
                                    None,
                                    theme,
                                )),
                        ),
                )
                // High Shelf section
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("HIGH SHELF", theme))
                        .child(
                            div()
                                .flex()
                                .gap_4()
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Frequency",
                                    state.high_freq,
                                    HIGH_FREQ_MIN as f64,
                                    HIGH_FREQ_MAX as f64,
                                    "Hz",
                                    2,
                                    state.selected_param,
                                    state.is_editing,
                                    None,
                                    theme,
                                ))
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Gain",
                                    state.high_gain,
                                    HIGH_GAIN_MIN as f64,
                                    HIGH_GAIN_MAX as f64,
                                    "dB",
                                    3,
                                    state.selected_param,
                                    state.is_editing,
                                    None,
                                    theme,
                                )),
                        ),
                ),
        )
        // Auto Gain section
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_section_title("AUTO GAIN", theme))
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .items_center()
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Enabled",
                            state.auto_gain_enabled,
                            4,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Max",
                            state.auto_gain_max_db,
                            0.0,
                            24.0,
                            "dB",
                            5,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Smooth",
                            state.auto_gain_smoothing_ms,
                            1.0,
                            1000.0,
                            "ms",
                            6,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        // Show current auto-gain value when enabled
                        .when(state.auto_gain_enabled, |d| {
                            d.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
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
                                            .text_color(if state.auto_gain_current_db > 0.0 {
                                                theme.success
                                            } else if state.auto_gain_current_db < 0.0 {
                                                theme.warning
                                            } else {
                                                theme.text_primary
                                            })
                                            .child(format!("{:+.1} dB", state.auto_gain_current_db)),
                                    ),
                            )
                        }),
                ),
        )
    // .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}

/// Render the Loudness Monitor plugin (analyzer)
pub fn render_loudness_monitor_plugin(
    loudness: Option<sotf_audio_player::LoudnessData>,
    _plugin_idx: usize,
    _is_editing: bool,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        // Dynamic LUFS/Peak/Width display from queue view
        .child(render_lufs_with_true_peak(loudness.as_ref(), theme))
        // Info section
        .child(div().flex().flex_col().gap_2().param_section_base(theme))
}
