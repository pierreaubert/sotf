//! Denoiser Plugin UI Component
//!
//! Spectral noise reduction with MCRA estimation:
//! - Reduction (dB) - Amount of noise reduction
//! - Floor (dB) - Minimum output level
//! - Smoothing - Spectral smoothing factor
//! - Attack/Release timing
//! - Low latency mode toggle
//! - Decision-Directed SNR estimation
//! - Psychoacoustic masking
//! - Noise profile capture

use super::common::{
    render_knob, render_section_title, render_toggle_button, render_vertical_slider_sized,
};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::param_specs::denoiser::*;

/// State for rendering the Denoiser plugin
pub struct DenoiserRenderState {
    pub reduction_db: f64,
    pub floor_db: f64,
    pub smoothing: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
    pub low_latency: bool,
    pub polyphonic_detection: bool,
    pub dd_enabled: bool,
    pub dd_alpha: f64,
    pub psychoacoustic_masking: bool,
    pub use_captured_profile: bool,
    pub is_editing: bool,
    pub selected_param: usize,
}

// Layout constants
const SLIDER_HEIGHT: f32 = 200.0;

/// Render the Denoiser plugin
pub fn render_denoiser_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: DenoiserRenderState,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        // Main section - columns side by side
        .child(
            div()
                .flex()
                .gap_6()
                // Column 1: Reduction and Floor sliders
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("NOISE REDUCTION", theme))
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(render_vertical_slider_sized(
                                    entity.clone(),
                                    plugin_idx,
                                    "Reduction",
                                    state.reduction_db,
                                    REDUCTION_DB_MIN as f64,
                                    REDUCTION_DB_MAX as f64,
                                    "dB",
                                    0,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('r'),
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                ))
                                .child(render_vertical_slider_sized(
                                    entity.clone(),
                                    plugin_idx,
                                    "Floor",
                                    state.floor_db,
                                    FLOOR_DB_MIN as f64,
                                    FLOOR_DB_MAX as f64,
                                    "dB",
                                    1,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('f'),
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                )),
                        ),
                )
                // Column 2: Attack and Release sliders
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("TIMING", theme))
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(render_vertical_slider_sized(
                                    entity.clone(),
                                    plugin_idx,
                                    "Attack",
                                    state.attack_ms,
                                    ATTACK_MS_MIN as f64,
                                    ATTACK_MS_MAX as f64,
                                    "ms",
                                    3,
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
                                    RELEASE_MS_MIN as f64,
                                    RELEASE_MS_MAX as f64,
                                    "ms",
                                    4,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('e'),
                                    Some(SLIDER_HEIGHT),
                                    theme,
                                )),
                        ),
                )
                // Column 3: Smoothing knob, toggles, and DD controls
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(render_section_title("PROCESSING", theme))
                                // Smoothing knob
                                .child(render_knob(
                                    entity.clone(),
                                    plugin_idx,
                                    "Smoothing",
                                    state.smoothing * 100.0,
                                    SMOOTHING_MIN as f64 * 100.0,
                                    SMOOTHING_MAX as f64 * 100.0,
                                    "%",
                                    2,
                                    state.selected_param,
                                    state.is_editing,
                                    Some('s'),
                                    theme,
                                ))
                                // DD Alpha knob (only visible when DD enabled)
                                .when(state.dd_enabled, |d| {
                                    d.child(render_knob(
                                        entity.clone(),
                                        plugin_idx,
                                        "DD Alpha",
                                        state.dd_alpha * 1000.0,
                                        DD_ALPHA_MIN as f64 * 1000.0,
                                        DD_ALPHA_MAX as f64 * 1000.0,
                                        "",
                                        8,
                                        state.selected_param,
                                        state.is_editing,
                                        None,
                                        theme,
                                    ))
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                // Low Latency toggle
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .w_full()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.text_muted)
                                                .child("Low Latency"),
                                        )
                                        .child(render_toggle_button(
                                            entity.clone(),
                                            plugin_idx,
                                            state.low_latency,
                                            5,
                                            state.selected_param,
                                            state.is_editing,
                                            theme,
                                        )),
                                )
                                // Polyphonic Detection toggle
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .w_full()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.text_muted)
                                                .child("Polyphonic"),
                                        )
                                        .child(render_toggle_button(
                                            entity.clone(),
                                            plugin_idx,
                                            state.polyphonic_detection,
                                            6,
                                            state.selected_param,
                                            state.is_editing,
                                            theme,
                                        )),
                                )
                                // DD SNR toggle
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .w_full()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.text_muted)
                                                .child("DD SNR"),
                                        )
                                        .child(render_toggle_button(
                                            entity.clone(),
                                            plugin_idx,
                                            state.dd_enabled,
                                            7,
                                            state.selected_param,
                                            state.is_editing,
                                            theme,
                                        )),
                                )
                                // Psychoacoustic Masking toggle
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .w_full()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.text_muted)
                                                .child("Masking"),
                                        )
                                        .child(render_toggle_button(
                                            entity.clone(),
                                            plugin_idx,
                                            state.psychoacoustic_masking,
                                            9,
                                            state.selected_param,
                                            state.is_editing,
                                            theme,
                                        )),
                                ),
                        ),
                )
                // Column 4: Noise Profile
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("NOISE PROFILE", theme))
                        // Learn Noise button (trigger)
                        .child(render_toggle_button(
                            entity.clone(),
                            plugin_idx,
                            false, // Trigger — always shows as off
                            10,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(div().text_xs().text_color(theme.text_muted).child("Learn"))
                        // Use Captured Profile toggle
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .w_full()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child("Use Profile"),
                                )
                                .child(render_toggle_button(
                                    entity.clone(),
                                    plugin_idx,
                                    state.use_captured_profile,
                                    11,
                                    state.selected_param,
                                    state.is_editing,
                                    theme,
                                )),
                        )
                        // Clear Profile button (trigger)
                        .child(render_toggle_button(
                            entity.clone(),
                            plugin_idx,
                            false, // Trigger — always shows as off
                            12,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(div().text_xs().text_color(theme.text_muted).child("Clear")),
                ),
        )
}
