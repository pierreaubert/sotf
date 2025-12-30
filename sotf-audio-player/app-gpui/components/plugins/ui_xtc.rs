//! XTC (Crosstalk Cancellation) Plugin UI Component
//!
//! Crosstalk cancellation for speaker playback:
//! - Distance to speakers
//! - Speaker angle
//! - Head radius modeling
//! - Beta (cancellation strength) with frequency-dependent boosts
//! - Head shadow modeling

use super::common::{ParamSectionStyle, render_knob, render_section_header};
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
    pub beta_base: f64,
    pub beta_low_freq_boost: f64,
    pub beta_high_freq_boost: f64,
    pub head_shadow_cutoff_hz: f64,
    pub head_shadow_slope_db_per_octave: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

// Fixed height for all columns to ensure consistent layout
const COLUMN_HEIGHT: f32 = 380.0;

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
        // Main section - Three columns side by side
        .child(
            div()
                .flex()
                .gap_4()
                .items_start()
                // Column 1: Physical setup
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .h(px(COLUMN_HEIGHT))
                        .param_section_style_lg(theme)
                        .child(render_section_header("SETUP", theme))
                        // Distance knob
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Distance",
                            state.distance_m,
                            DISTANCE_MIN,
                            DISTANCE_MAX,
                            "m",
                            0, // distance_m param index
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
                            1, // speaker_angle_deg param index
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
                            state.head_radius_m * 100.0, // Convert to cm for display
                            HEAD_RADIUS_MIN * 100.0,
                            HEAD_RADIUS_MAX * 100.0,
                            "cm",
                            2, // head_radius_m param index
                            state.selected_param,
                            state.is_editing,
                            Some('r'),
                            theme,
                        )),
                )
                // Column 2: Cancellation strength (Beta)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .h(px(COLUMN_HEIGHT))
                        .param_section_style_lg(theme)
                        .child(render_section_header("CANCELLATION", theme))
                        // Beta base knob (scaled for display)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Beta Base",
                            state.beta_base * 1000.0, // Scale up for display
                            BETA_BASE_MIN * 1000.0,
                            BETA_BASE_MAX * 1000.0,
                            "×10⁻³",
                            3, // beta_base param index
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
                            4, // beta_low_freq_boost param index
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
                            5, // beta_high_freq_boost param index
                            state.selected_param,
                            state.is_editing,
                            Some('h'),
                            theme,
                        )),
                )
                // Column 3: Head shadow modeling
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .h(px(COLUMN_HEIGHT))
                        .param_section_style_lg(theme)
                        .child(render_section_header("HEAD SHADOW", theme))
                        // Cutoff frequency knob
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Cutoff",
                            state.head_shadow_cutoff_hz,
                            HEAD_SHADOW_CUTOFF_MIN,
                            HEAD_SHADOW_CUTOFF_MAX,
                            "Hz",
                            6, // head_shadow_cutoff_hz param index
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
                            7, // head_shadow_slope_db_per_octave param index
                            state.selected_param,
                            state.is_editing,
                            Some('s'),
                            theme,
                        )),
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
