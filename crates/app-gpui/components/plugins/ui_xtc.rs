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
use sotf_plugins::param_specs::{find_by_key as pk, xtc::PARAMS as XT};

/// State for rendering the XTC plugin
pub struct XtcRenderState {
    pub distance_m: f64,
    pub speaker_angle_deg: f64,
    pub head_radius_m: f64,
    pub head_offset_x: f64,
    pub head_offset_z: f64,
    pub head_yaw_deg: f64,
    pub head_tracking_smooth_s: f64,
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

// PARAMS index constants matching xtc::PARAMS order
const I_DISTANCE: usize = 0;
const I_ANGLE: usize = 1;
const I_HEAD_RADIUS: usize = 2;
const I_OFFSET_X: usize = 3;
const I_OFFSET_Z: usize = 4;
const I_YAW: usize = 5;
const I_TRACK_SMOOTH: usize = 6;
const I_BETA_BASE: usize = 7;
const I_BETA_LF: usize = 8;
const I_BETA_HF: usize = 9;
const I_SHADOW_CUTOFF: usize = 10;
const I_SHADOW_SLOPE: usize = 11;
const I_MAX_GAIN: usize = 12;
const I_SPECTRAL_NORM: usize = 13;
const I_PINNA: usize = 14;
const I_ROOM_ENABLED: usize = 15;
const I_ROOM_WIDTH: usize = 16;
const I_ROOM_DEPTH: usize = 17;
const I_WALL_ABSORPTION: usize = 18;
const I_REFL_BETA: usize = 19;
const I_BYPASS_XTC: usize = 20;
const I_BYPASS_SPECTRAL: usize = 21;
const I_BYPASS_NEUMANN: usize = 22;
const I_AG_ENABLED: usize = 23;
const I_AG_MAX: usize = 24;
const I_AG_SMOOTH: usize = 25;

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
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Distance",
                            state.distance_m,
                            pk(XT, "distance_m").min_f64(),
                            pk(XT, "distance_m").max_f64(),
                            "m",
                            I_DISTANCE,
                            state.selected_param,
                            state.is_editing,
                            Some('d'),
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Angle",
                            state.speaker_angle_deg,
                            pk(XT, "speaker_angle_deg").min_f64(),
                            pk(XT, "speaker_angle_deg").max_f64(),
                            "\u{00b0}",
                            I_ANGLE,
                            state.selected_param,
                            state.is_editing,
                            Some('a'),
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Head Radius",
                            state.head_radius_m * 100.0,
                            pk(XT, "head_radius_m").min_f64() * 100.0,
                            pk(XT, "head_radius_m").max_f64() * 100.0,
                            "cm",
                            I_HEAD_RADIUS,
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
                            pk(XT, "head_offset_x").min_f64(),
                            pk(XT, "head_offset_x").max_f64(),
                            "m",
                            I_OFFSET_X,
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
                            pk(XT, "head_offset_z").min_f64(),
                            pk(XT, "head_offset_z").max_f64(),
                            "m",
                            I_OFFSET_Z,
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
                            pk(XT, "head_yaw_deg").min_f64(),
                            pk(XT, "head_yaw_deg").max_f64(),
                            "\u{00b0}",
                            I_YAW,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Smoothing",
                            state.head_tracking_smooth_s,
                            pk(XT, "head_tracking_smooth_s").min_f64(),
                            pk(XT, "head_tracking_smooth_s").max_f64(),
                            "s",
                            I_TRACK_SMOOTH,
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
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Beta Base",
                            state.beta_base * 1000.0,
                            pk(XT, "beta_base").min_f64() * 1000.0,
                            pk(XT, "beta_base").max_f64() * 1000.0,
                            "\u{00d7}10\u{207b}\u{00b3}",
                            I_BETA_BASE,
                            state.selected_param,
                            state.is_editing,
                            Some('b'),
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "LF Boost",
                            state.beta_low_freq_boost,
                            pk(XT, "beta_low_freq_boost").min_f64(),
                            pk(XT, "beta_low_freq_boost").max_f64(),
                            "\u{00d7}",
                            I_BETA_LF,
                            state.selected_param,
                            state.is_editing,
                            Some('l'),
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "HF Boost",
                            state.beta_high_freq_boost,
                            pk(XT, "beta_high_freq_boost").min_f64(),
                            pk(XT, "beta_high_freq_boost").max_f64(),
                            "\u{00d7}",
                            I_BETA_HF,
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
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Cutoff",
                            state.head_shadow_cutoff_hz,
                            pk(XT, "head_shadow_cutoff_hz").min_f64(),
                            pk(XT, "head_shadow_cutoff_hz").max_f64(),
                            "Hz",
                            I_SHADOW_CUTOFF,
                            state.selected_param,
                            state.is_editing,
                            Some('c'),
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Slope",
                            state.head_shadow_slope_db_per_octave,
                            pk(XT, "head_shadow_slope_db_per_octave").min_f64(),
                            pk(XT, "head_shadow_slope_db_per_octave").max_f64(),
                            "dB/oct",
                            I_SHADOW_SLOPE,
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
                            pk(XT, "max_gain_db").min_f64(),
                            pk(XT, "max_gain_db").max_f64(),
                            "dB",
                            I_MAX_GAIN,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                )
                // Column 5: Advanced
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("ADVANCED", theme))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Spectral Norm",
                            state.spectral_normalization,
                            I_SPECTRAL_NORM,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Pinna Model",
                            state.pinna_model_enabled,
                            I_PINNA,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        )),
                )
                // Column 6: Room
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("ROOM", theme))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Room Reflections",
                            state.room_reflections_enabled,
                            I_ROOM_ENABLED,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Room Width",
                            state.room_width_m,
                            pk(XT, "room_width_m").min_f64(),
                            pk(XT, "room_width_m").max_f64(),
                            "m",
                            I_ROOM_WIDTH,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Room Depth",
                            state.room_depth_m,
                            pk(XT, "room_depth_m").min_f64(),
                            pk(XT, "room_depth_m").max_f64(),
                            "m",
                            I_ROOM_DEPTH,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Absorption",
                            state.wall_absorption,
                            pk(XT, "wall_absorption").min_f64(),
                            pk(XT, "wall_absorption").max_f64(),
                            "",
                            I_WALL_ABSORPTION,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Reflection Beta",
                            state.reflection_beta_boost,
                            pk(XT, "reflection_beta_boost").min_f64(),
                            pk(XT, "reflection_beta_boost").max_f64(),
                            "\u{00d7}",
                            I_REFL_BETA,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                )
                // Column 7: Diagnostic bypasses (before Auto Gain)
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
                            I_BYPASS_XTC,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Bypass Spec Norm",
                            state.bypass_spectral_normalization,
                            I_BYPASS_SPECTRAL,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Bypass Neumann",
                            state.bypass_neumann_refinement,
                            I_BYPASS_NEUMANN,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        )),
                )
                // Column 8: Auto Gain
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
                            I_AG_ENABLED,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "AG Max",
                            state.auto_gain_max_db,
                            pk(XT, "auto_gain_max_db").min_f64(),
                            pk(XT, "auto_gain_max_db").max_f64(),
                            "dB",
                            I_AG_MAX,
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
                            pk(XT, "auto_gain_smoothing_ms").min_f64(),
                            pk(XT, "auto_gain_smoothing_ms").max_f64(),
                            "ms",
                            I_AG_SMOOTH,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                ),
        )
}
