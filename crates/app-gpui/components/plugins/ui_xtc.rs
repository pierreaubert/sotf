//! XTC (Crosstalk Cancellation) Plugin UI Component
//!
//! Crosstalk cancellation for speaker playback:
//! - Distance to speakers
//! - Speaker angle
//! - Head radius modeling
//! - Beta (cancellation strength) with frequency-dependent boosts
//! - Head shadow modeling

use super::common::{render_knob, render_section_title, render_toggle};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

// XTC parameter ranges (matching defaults in plugins.rs)
const DISTANCE_MIN: f64 = 0.5;
const DISTANCE_MAX: f64 = 5.0;
const ANGLE_MIN: f64 = 10.0;
const ANGLE_MAX: f64 = 60.0;
const HEAD_RADIUS_MIN: f64 = 0.05;
const HEAD_RADIUS_MAX: f64 = 0.15;
const BETA_BASE_MIN: f64 = 0.0001;
const BETA_BASE_MAX: f64 = 0.1;
const BETA_BOOST_MIN: f64 = 1.0;
const BETA_BOOST_MAX: f64 = 100.0;
const HEAD_SHADOW_CUTOFF_MIN: f64 = 1000.0;
const HEAD_SHADOW_CUTOFF_MAX: f64 = 10000.0;
const HEAD_SHADOW_SLOPE_MIN: f64 = 0.0;
const HEAD_SHADOW_SLOPE_MAX: f64 = 12.0;

/// State for rendering the XTC plugin
pub struct XtcRenderState {
    pub distance_m: f64,
    pub speaker_angle_deg: f64,
    pub head_radius_m: f64,
    pub head_offset_x: f64,
    pub head_offset_z: f64,
    pub head_yaw_deg: f64,
    pub beta_base: f64,
    pub beta_low_freq_boost: f64,
    pub beta_high_freq_boost: f64,
    pub head_shadow_cutoff_hz: f64,
    pub head_shadow_slope_db_per_octave: f64,
    pub max_gain_db: f64,
    pub spectral_normalization: bool,
    pub pinna_model_enabled: bool,
    pub room_reflections_enabled: bool,
    pub room_width_m: f64,
    pub room_depth_m: f64,
    pub wall_absorption: f64,
    pub reflection_beta_boost: f64,
    pub bypass_xtc_filters: bool,
    pub bypass_spectral_normalization: bool,
    pub bypass_neumann_refinement: bool,
    pub auto_gain_enabled: bool,
    pub auto_gain_max_db: f64,
    pub auto_gain_smoothing_ms: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render the XTC plugin
pub fn render_xtc_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: XtcRenderState,
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
                // Column 1: Physical setup
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("SETUP", theme))
                        // Distance knob
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Distance",
                            state.distance_m,
                            DISTANCE_MIN,
                            DISTANCE_MAX,
                            "m",
                            0,
                            state.selected_param,
                            state.is_editing,
                            Some('d'),
                            theme,
                        ))
                        // Speaker angle knob
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Angle",
                            state.speaker_angle_deg,
                            ANGLE_MIN,
                            ANGLE_MAX,
                            "°",
                            1,
                            state.selected_param,
                            state.is_editing,
                            Some('a'),
                            theme,
                        ))
                        // Head radius knob
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Head Radius",
                            state.head_radius_m * 100.0,
                            HEAD_RADIUS_MIN * 100.0,
                            HEAD_RADIUS_MAX * 100.0,
                            "cm",
                            2,
                            state.selected_param,
                            state.is_editing,
                            Some('r'),
                            theme,
                        )),
                )
                // Column 2: Head Tracking
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("TRACKING", theme))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Offset X",
                            state.head_offset_x,
                            -0.5,
                            0.5,
                            "m",
                            3,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Offset Z",
                            state.head_offset_z,
                            -0.5,
                            0.5,
                            "m",
                            4,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Yaw",
                            state.head_yaw_deg,
                            -90.0,
                            90.0,
                            "°",
                            5,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                )
                // Column 3: Cancellation strength (Beta)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("CANCELLATION", theme))
                        // Beta base knob (scaled for display)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Beta Base",
                            state.beta_base * 1000.0,
                            BETA_BASE_MIN * 1000.0,
                            BETA_BASE_MAX * 1000.0,
                            "×10⁻³",
                            6,
                            state.selected_param,
                            state.is_editing,
                            Some('b'),
                            theme,
                        ))
                        // Low frequency boost knob
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "LF Boost",
                            state.beta_low_freq_boost,
                            BETA_BOOST_MIN,
                            BETA_BOOST_MAX,
                            "×",
                            7,
                            state.selected_param,
                            state.is_editing,
                            Some('l'),
                            theme,
                        ))
                        // High frequency boost knob
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "HF Boost",
                            state.beta_high_freq_boost,
                            BETA_BOOST_MIN,
                            BETA_BOOST_MAX,
                            "×",
                            8,
                            state.selected_param,
                            state.is_editing,
                            Some('h'),
                            theme,
                        )),
                )
                // Column 4: Head shadow modeling
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("HEAD SHADOW", theme))
                        // Cutoff frequency knob
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Cutoff",
                            state.head_shadow_cutoff_hz,
                            HEAD_SHADOW_CUTOFF_MIN,
                            HEAD_SHADOW_CUTOFF_MAX,
                            "Hz",
                            9,
                            state.selected_param,
                            state.is_editing,
                            Some('c'),
                            theme,
                        ))
                        // Slope knob
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Slope",
                            state.head_shadow_slope_db_per_octave,
                            HEAD_SHADOW_SLOPE_MIN,
                            HEAD_SHADOW_SLOPE_MAX,
                            "dB/oct",
                            10,
                            state.selected_param,
                            state.is_editing,
                            Some('s'),
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Max Gain",
                            state.max_gain_db,
                            3.0,
                            30.0,
                            "dB",
                            11,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                )
                // Column 5: Advanced & Room
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("ADVANCED / ROOM", theme))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Spectral Norm",
                            state.spectral_normalization,
                            12,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Pinna Model",
                            state.pinna_model_enabled,
                            13,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Room Refl",
                            state.room_reflections_enabled,
                            14,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Reflection Beta",
                            state.reflection_beta_boost,
                            1.0,
                            10.0,
                            "×",
                            18,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                )
                // Column 6: Auto Gain
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("AUTO GAIN", theme))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Auto Gain",
                            state.auto_gain_enabled,
                            22,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "AG Max",
                            state.auto_gain_max_db,
                            0.0,
                            24.0,
                            "dB",
                            23,
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
                            24,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                )
                // Column 7: Diagnostic bypasses
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("DIAGNOSTIC", theme))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Bypass Filters",
                            state.bypass_xtc_filters,
                            19,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Bypass Spec Norm",
                            state.bypass_spectral_normalization,
                            20,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Bypass Neumann",
                            state.bypass_neumann_refinement,
                            21,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        )),
                ),
        )
    // .when(state.is_editing, |d| d.child(render_edit_hints(theme)))
}
