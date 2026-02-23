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
    pub crack_sensitivity: f64,
    pub mcra_alpha_s: f64,
    pub mcra_alpha_p: f64,
    pub mcra_l: usize,
    pub mcra_delta: f64,
    pub transparency: f64,
    pub dd_enabled: bool,
    pub dd_alpha: f64,
    pub psychoacoustic_masking: bool,
    pub learn_noise: bool,
    pub use_captured_profile: bool,
    pub clear_profile: bool,
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
                .items_start()
                // Column 1: Reduction and Floor sliders
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("REDUCTION", theme))
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
                // Column 3: Detection and Processing
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("DETECTION", theme))
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
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Crack Sens",
                            state.crack_sensitivity,
                            1.0,
                            100.0,
                            "",
                            7,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Transparency",
                            state.transparency * 100.0,
                            TRANSPARENCY_MIN as f64 * 100.0,
                            TRANSPARENCY_MAX as f64 * 100.0,
                            "%",
                            12,
                            state.selected_param,
                            state.is_editing,
                            Some('t'),
                            theme,
                        )),
                )
                // Column 4: MCRA Advanced
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("ADVANCED", theme))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "MCRA S",
                            state.mcra_alpha_s,
                            0.5,
                            0.99,
                            "",
                            8,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "MCRA P",
                            state.mcra_alpha_p,
                            0.1,
                            0.99,
                            "",
                            9,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "MCRA Window",
                            state.mcra_l as f64,
                            10.0,
                            200.0,
                            "fr",
                            10,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "MCRA Delta",
                            state.mcra_delta,
                            1.0,
                            20.0,
                            "",
                            11,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                )
                // Column 5: Toggles
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("MODES", theme))
                        .child(render_toggle_button(
                            entity.clone(),
                            plugin_idx,
                            state.low_latency,
                            5,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(div().text_xs().text_color(theme.text_muted).child("Low Lat"))
                        .child(render_toggle_button(
                            entity.clone(),
                            plugin_idx,
                            state.polyphonic_detection,
                            6,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(div().text_xs().text_color(theme.text_muted).child("Polyphonic"))
                        .child(render_toggle_button(
                            entity.clone(),
                            plugin_idx,
                            state.dd_enabled,
                            13,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(div().text_xs().text_color(theme.text_muted).child("DD SNR"))
                        .child(render_toggle_button(
                            entity.clone(),
                            plugin_idx,
                            state.psychoacoustic_masking,
                            15,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(div().text_xs().text_color(theme.text_muted).child("Masking")),
                )
                // Column 6: Profile
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("PROFILE", theme))
                        .child(render_toggle_button(
                            entity.clone(),
                            plugin_idx,
                            state.learn_noise,
                            16,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(div().text_xs().text_color(theme.text_muted).child("Learn"))
                        .child(render_toggle_button(
                            entity.clone(),
                            plugin_idx,
                            state.use_captured_profile,
                            17,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(div().text_xs().text_color(theme.text_muted).child("Use Prof"))
                        .child(render_toggle_button(
                            entity.clone(),
                            plugin_idx,
                            state.clear_profile,
                            18,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(div().text_xs().text_color(theme.text_muted).child("Clear")),
                ),
        )
}
