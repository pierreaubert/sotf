//! Loudness Plugin UI Components

use super::common::{render_edit_hints, render_knob, render_section_header};
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
        // Description section
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
                .child(render_section_header("FLETCHER-MUNSON COMPENSATION", theme))
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .child("Boosts bass and treble at low listening volumes to compensate for the ear's reduced sensitivity at those frequencies."),
                ),
        )
        // Low Shelf section
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .rounded_xl()
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.border)
                .p_3()
                .child(render_section_header("LOW SHELF", theme))
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .justify_center()
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
                .rounded_xl()
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.border)
                .p_3()
                .child(render_section_header("HIGH SHELF", theme))
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .justify_center()
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
        )
        .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
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
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .rounded_xl()
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.border)
        )
}
