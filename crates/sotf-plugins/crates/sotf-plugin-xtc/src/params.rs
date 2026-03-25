//! XTC plugin parameter definitions — single source of truth.
//!
//! This file owns:
//! - Parameter specs (PARAMS array)
//! - UI layout (LAYOUT)
//! - Serializable state (Params struct with serde defaults)
//! - Index↔field mapping (PluginParamDef impl)
//!
//! Adding a parameter: add to PARAMS, add field to Params, add match arms.
//! Nothing else needs to change.

use serde::{Deserialize, Serialize};
use sotf_host::param_specs::{find_by_key as pk, ParamSpec};
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    // Geometry
    ParamSpec::float(
        "Distance",
        "distance_m",
        2.0,
        0.5,
        10.0,
        0.05,
        "m",
        "Geometry",
    )
    .doc("Listener-to-speaker distance"),
    ParamSpec::float(
        "Speaker Angle",
        "speaker_angle_deg",
        30.0,
        10.0,
        90.0,
        0.5,
        "\u{00b0}",
        "Geometry",
    )
    .doc("Half-angle between speakers"),
    ParamSpec::float(
        "Head Radius",
        "head_radius_m",
        0.0875,
        0.05,
        0.12,
        0.001,
        "m",
        "Geometry",
    )
    .scaled(100.0)
    .doc("Acoustic head radius"),
    // Head Tracking
    ParamSpec::float(
        "Head Offset X",
        "head_offset_x",
        0.0,
        -0.5,
        0.5,
        0.01,
        "m",
        "Head Tracking",
    )
    .doc("Lateral head position offset"),
    ParamSpec::float(
        "Head Offset Z",
        "head_offset_z",
        0.0,
        -0.5,
        0.5,
        0.01,
        "m",
        "Head Tracking",
    )
    .doc("Forward/back head position"),
    ParamSpec::float(
        "Head Yaw",
        "head_yaw_deg",
        0.0,
        -90.0,
        90.0,
        1.0,
        "\u{00b0}",
        "Head Tracking",
    )
    .doc("Head rotation angle"),
    ParamSpec::float(
        "Head Tracking Smooth",
        "head_tracking_smooth_s",
        0.1,
        0.0,
        1.0,
        0.01,
        "s",
        "Head Tracking",
    )
    .doc("Tracking data smoothing time"),
    // Beta
    ParamSpec::float(
        "Beta Base",
        "beta_base",
        0.001,
        0.0001,
        0.1,
        0.001,
        "",
        "Beta",
    )
    .scaled(1000.0)
    .doc("Regularization base level"),
    ParamSpec::float(
        "Beta Low Boost",
        "beta_low_freq_boost",
        10.0,
        0.0,
        30.0,
        0.5,
        "",
        "Beta",
    )
    .doc("Extra regularization at low freq"),
    ParamSpec::float(
        "Beta High Boost",
        "beta_high_freq_boost",
        10.0,
        0.0,
        30.0,
        0.5,
        "",
        "Beta",
    )
    .doc("Extra regularization at high freq"),
    // Shadow
    ParamSpec::float(
        "Shadow Cutoff",
        "head_shadow_cutoff_hz",
        4000.0,
        1000.0,
        10000.0,
        50.0,
        "Hz",
        "Shadow",
    )
    .doc("Head shadow filter onset freq"),
    ParamSpec::float(
        "Shadow Slope",
        "head_shadow_slope_db_per_octave",
        6.0,
        0.0,
        12.0,
        0.5,
        "dB/oct",
        "Shadow",
    )
    .doc("Head shadow attenuation rate"),
    // Filter
    ParamSpec::float(
        "Max Gain",
        "max_gain_db",
        12.0,
        3.0,
        30.0,
        1.0,
        "dB",
        "Filter",
    )
    .doc("Maximum XTC filter boost"),
    // Advanced
    ParamSpec::bool_param("Spectral Norm", "spectral_normalization", true, "Advanced")
        .doc("Normalize filter energy"),
    ParamSpec::bool_param("Pinna Model", "pinna_model_enabled", false, "Advanced")
        .doc("Include pinna diffraction model"),
    // Room
    ParamSpec::bool_param(
        "Room Reflections",
        "room_reflections_enabled",
        false,
        "Room",
    )
    .doc("Include first-order reflections"),
    ParamSpec::float(
        "Room Width",
        "room_width_m",
        4.0,
        2.0,
        10.0,
        0.1,
        "m",
        "Room",
    )
    .doc("Listening room width"),
    ParamSpec::float(
        "Room Depth",
        "room_depth_m",
        5.0,
        2.0,
        15.0,
        0.1,
        "m",
        "Room",
    )
    .doc("Listening room depth"),
    ParamSpec::float(
        "Wall Absorption",
        "wall_absorption",
        0.3,
        0.0,
        1.0,
        0.05,
        "",
        "Room",
    )
    .doc("Wall absorption coefficient"),
    ParamSpec::float(
        "Reflection Beta",
        "reflection_beta_boost",
        3.0,
        1.0,
        10.0,
        0.1,
        "",
        "Room",
    )
    .doc("Reflection path regularization"),
    // Diagnostic
    ParamSpec::bool_param(
        "Bypass XTC Filters",
        "bypass_xtc_filters",
        false,
        "Diagnostic",
    )
    .diagnostic()
    .doc("Skip crosstalk cancellation"),
    ParamSpec::bool_param(
        "Bypass Spectral Norm",
        "bypass_spectral_normalization",
        false,
        "Diagnostic",
    )
    .diagnostic()
    .doc("Skip spectral normalization"),
    ParamSpec::bool_param(
        "Bypass Neumann",
        "bypass_neumann_refinement",
        false,
        "Diagnostic",
    )
    .diagnostic()
    .doc("Skip Neumann KH refinement"),
    // Auto Gain
    ParamSpec::bool_param("Auto Gain", "auto_gain_enabled", true, "Auto Gain")
        .output()
        .doc("Auto-normalize output level"),
    ParamSpec::float(
        "AG Max",
        "auto_gain_max_db",
        12.0,
        0.0,
        24.0,
        1.0,
        "dB",
        "Auto Gain",
    )
    .output()
    .doc("Maximum auto gain correction"),
    ParamSpec::float(
        "AG Smoothing",
        "auto_gain_smoothing_ms",
        100.0,
        10.0,
        500.0,
        5.0,
        "ms",
        "Auto Gain",
    )
    .output()
    .doc("Auto gain transition time"),
    // Phase 4D: SOTA addition
    ParamSpec::choice("Head Model", "head_model", 0, HEAD_MODELS, "Geometry")
        .setup()
        .doc("Head diffraction model: Woodworth (classic) or Brown-Duda (rigid sphere, more accurate above 1.5kHz)"),
];

pub const HEAD_MODELS: &[&str] = &["Woodworth", "Brown-Duda"];

// ============================================================================
// UI Layout
// ============================================================================

/// XTC: idx 0=distance, 1=speaker_angle, 2=head_radius,
/// 3=head_offset_x, 4=head_offset_z, 5=head_yaw, 6=head_tracking_smooth,
/// 7=beta_base, 8=beta_low_boost, 9=beta_high_boost,
/// 10=shadow_cutoff, 11=shadow_slope, 12=max_gain,
/// 13=spectral_norm, 14=pinna_model,
/// 15=room_reflections, 16=room_width, 17=room_depth, 18=wall_absorption, 19=reflection_beta,
/// 20=bypass_xtc, 21=bypass_spectral_norm, 22=bypass_neumann,
/// 23=auto_gain, 24=ag_max, 25=ag_smoothing
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::knob(0), // distance_m
        ControlSpec::knob(1), // speaker_angle_deg
        ControlSpec::knob(2), // head_radius_m
    ],
    main: &[
        ControlGroup {
            title: "BETA",
            controls: &[
                ControlSpec::knob(7), // beta_base
                ControlSpec::knob(8), // beta_low_boost
                ControlSpec::knob(9), // beta_high_boost
            ],
        },
        ControlGroup {
            title: "SHADOW",
            controls: &[
                ControlSpec::knob(10), // shadow_cutoff
                ControlSpec::knob(11), // shadow_slope
                ControlSpec::knob(12), // max_gain
            ],
        },
        ControlGroup {
            title: "ADVANCED",
            controls: &[
                ControlSpec::toggle(13), // spectral_norm
                ControlSpec::toggle(14), // pinna_model
            ],
        },
        ControlGroup {
            title: "ROOM",
            controls: &[
                ControlSpec::toggle(15), // room_reflections
                ControlSpec::knob(16),   // room_width
                ControlSpec::knob(17),   // room_depth
                ControlSpec::knob(18),   // wall_absorption
                ControlSpec::knob(19),   // reflection_beta
            ],
        },
    ],
    output: &[
        ControlSpec::toggle(20), // bypass_xtc (diagnostic)
        ControlSpec::toggle(21), // bypass_spectral_norm (diagnostic)
        ControlSpec::toggle(22), // bypass_neumann (diagnostic)
        ControlSpec::toggle(23), // auto_gain
        ControlSpec::knob(24),   // ag_max
        ControlSpec::knob(25),   // ag_smoothing
    ],
    tabs: &[TabSpec {
        name: "Head Tracking",
        controls: &[
            ControlSpec::knob(3), // head_offset_x
            ControlSpec::knob(4), // head_offset_z
            ControlSpec::knob(5), // head_yaw
            ControlSpec::knob(6), // head_tracking_smooth
        ],
    }],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(120.0, 0.5),
        ColumnConstraint::main(400.0),
        ColumnConstraint::output(130.0, 0.6),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// XTC plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_distance_m")]
    pub distance_m: f64,
    #[serde(default = "d_speaker_angle_deg")]
    pub speaker_angle_deg: f64,
    #[serde(default = "d_head_radius_m")]
    pub head_radius_m: f64,
    #[serde(default = "d_head_offset_x")]
    pub head_offset_x: f64,
    #[serde(default = "d_head_offset_z")]
    pub head_offset_z: f64,
    #[serde(default = "d_head_yaw_deg")]
    pub head_yaw_deg: f64,
    #[serde(default = "d_head_tracking_smooth_s")]
    pub head_tracking_smooth_s: f64,
    #[serde(default = "d_beta_base")]
    pub beta_base: f64,
    #[serde(default = "d_beta_low_freq_boost")]
    pub beta_low_freq_boost: f64,
    #[serde(default = "d_beta_high_freq_boost")]
    pub beta_high_freq_boost: f64,
    #[serde(default = "d_head_shadow_cutoff_hz")]
    pub head_shadow_cutoff_hz: f64,
    #[serde(default = "d_head_shadow_slope_db_per_octave")]
    pub head_shadow_slope_db_per_octave: f64,
    #[serde(default = "d_max_gain_db")]
    pub max_gain_db: f64,
    #[serde(default = "d_spectral_normalization")]
    pub spectral_normalization: bool,
    #[serde(default = "d_pinna_model_enabled")]
    pub pinna_model_enabled: bool,
    #[serde(default = "d_room_reflections_enabled")]
    pub room_reflections_enabled: bool,
    #[serde(default = "d_room_width_m")]
    pub room_width_m: f64,
    #[serde(default = "d_room_depth_m")]
    pub room_depth_m: f64,
    #[serde(default = "d_wall_absorption")]
    pub wall_absorption: f64,
    #[serde(default = "d_reflection_beta_boost")]
    pub reflection_beta_boost: f64,
    #[serde(default = "d_bypass_xtc_filters")]
    pub bypass_xtc_filters: bool,
    #[serde(default = "d_bypass_spectral_normalization")]
    pub bypass_spectral_normalization: bool,
    #[serde(default = "d_bypass_neumann_refinement")]
    pub bypass_neumann_refinement: bool,
    #[serde(default = "d_auto_gain_enabled")]
    pub auto_gain_enabled: bool,
    #[serde(default = "d_auto_gain_max_db")]
    pub auto_gain_max_db: f64,
    #[serde(default = "d_auto_gain_smoothing_ms")]
    pub auto_gain_smoothing_ms: f64,
    #[serde(default)]
    pub head_model: f64,
}

fn d_distance_m() -> f64 {
    pk(PARAMS, "distance_m").default_f64()
}
fn d_speaker_angle_deg() -> f64 {
    pk(PARAMS, "speaker_angle_deg").default_f64()
}
fn d_head_radius_m() -> f64 {
    pk(PARAMS, "head_radius_m").default_f64()
}
fn d_head_offset_x() -> f64 {
    pk(PARAMS, "head_offset_x").default_f64()
}
fn d_head_offset_z() -> f64 {
    pk(PARAMS, "head_offset_z").default_f64()
}
fn d_head_yaw_deg() -> f64 {
    pk(PARAMS, "head_yaw_deg").default_f64()
}
fn d_head_tracking_smooth_s() -> f64 {
    pk(PARAMS, "head_tracking_smooth_s").default_f64()
}
fn d_beta_base() -> f64 {
    pk(PARAMS, "beta_base").default_f64()
}
fn d_beta_low_freq_boost() -> f64 {
    pk(PARAMS, "beta_low_freq_boost").default_f64()
}
fn d_beta_high_freq_boost() -> f64 {
    pk(PARAMS, "beta_high_freq_boost").default_f64()
}
fn d_head_shadow_cutoff_hz() -> f64 {
    pk(PARAMS, "head_shadow_cutoff_hz").default_f64()
}
fn d_head_shadow_slope_db_per_octave() -> f64 {
    pk(PARAMS, "head_shadow_slope_db_per_octave").default_f64()
}
fn d_max_gain_db() -> f64 {
    pk(PARAMS, "max_gain_db").default_f64()
}
fn d_spectral_normalization() -> bool {
    pk(PARAMS, "spectral_normalization").default_bool()
}
fn d_pinna_model_enabled() -> bool {
    pk(PARAMS, "pinna_model_enabled").default_bool()
}
fn d_room_reflections_enabled() -> bool {
    pk(PARAMS, "room_reflections_enabled").default_bool()
}
fn d_room_width_m() -> f64 {
    pk(PARAMS, "room_width_m").default_f64()
}
fn d_room_depth_m() -> f64 {
    pk(PARAMS, "room_depth_m").default_f64()
}
fn d_wall_absorption() -> f64 {
    pk(PARAMS, "wall_absorption").default_f64()
}
fn d_reflection_beta_boost() -> f64 {
    pk(PARAMS, "reflection_beta_boost").default_f64()
}
fn d_bypass_xtc_filters() -> bool {
    pk(PARAMS, "bypass_xtc_filters").default_bool()
}
fn d_bypass_spectral_normalization() -> bool {
    pk(PARAMS, "bypass_spectral_normalization").default_bool()
}
fn d_bypass_neumann_refinement() -> bool {
    pk(PARAMS, "bypass_neumann_refinement").default_bool()
}
fn d_auto_gain_enabled() -> bool {
    pk(PARAMS, "auto_gain_enabled").default_bool()
}
fn d_auto_gain_max_db() -> f64 {
    pk(PARAMS, "auto_gain_max_db").default_f64()
}
fn d_auto_gain_smoothing_ms() -> f64 {
    pk(PARAMS, "auto_gain_smoothing_ms").default_f64()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            distance_m: d_distance_m(),
            speaker_angle_deg: d_speaker_angle_deg(),
            head_radius_m: d_head_radius_m(),
            head_offset_x: d_head_offset_x(),
            head_offset_z: d_head_offset_z(),
            head_yaw_deg: d_head_yaw_deg(),
            head_tracking_smooth_s: d_head_tracking_smooth_s(),
            beta_base: d_beta_base(),
            beta_low_freq_boost: d_beta_low_freq_boost(),
            beta_high_freq_boost: d_beta_high_freq_boost(),
            head_shadow_cutoff_hz: d_head_shadow_cutoff_hz(),
            head_shadow_slope_db_per_octave: d_head_shadow_slope_db_per_octave(),
            max_gain_db: d_max_gain_db(),
            spectral_normalization: d_spectral_normalization(),
            pinna_model_enabled: d_pinna_model_enabled(),
            room_reflections_enabled: d_room_reflections_enabled(),
            room_width_m: d_room_width_m(),
            room_depth_m: d_room_depth_m(),
            wall_absorption: d_wall_absorption(),
            reflection_beta_boost: d_reflection_beta_boost(),
            bypass_xtc_filters: d_bypass_xtc_filters(),
            bypass_spectral_normalization: d_bypass_spectral_normalization(),
            bypass_neumann_refinement: d_bypass_neumann_refinement(),
            auto_gain_enabled: d_auto_gain_enabled(),
            auto_gain_max_db: d_auto_gain_max_db(),
            auto_gain_smoothing_ms: d_auto_gain_smoothing_ms(),
            head_model: 0.0,
        }
    }
}

// ============================================================================
// PluginParamDef implementation
// ============================================================================

impl PluginParamDef for Params {
    const PARAMS: &'static [ParamSpec] = PARAMS;
    const LAYOUT: Option<&'static PluginLayout> = Some(&LAYOUT);
    const VERSION: u32 = 1;
    const PLUGIN_TYPE_KEY: &'static str = "xtc";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.distance_m),
            1 => Some(self.speaker_angle_deg),
            2 => Some(self.head_radius_m),
            3 => Some(self.head_offset_x),
            4 => Some(self.head_offset_z),
            5 => Some(self.head_yaw_deg),
            6 => Some(self.head_tracking_smooth_s),
            7 => Some(self.beta_base),
            8 => Some(self.beta_low_freq_boost),
            9 => Some(self.beta_high_freq_boost),
            10 => Some(self.head_shadow_cutoff_hz),
            11 => Some(self.head_shadow_slope_db_per_octave),
            12 => Some(self.max_gain_db),
            13 => Some(if self.spectral_normalization { 1.0 } else { 0.0 }),
            14 => Some(if self.pinna_model_enabled { 1.0 } else { 0.0 }),
            15 => Some(if self.room_reflections_enabled {
                1.0
            } else {
                0.0
            }),
            16 => Some(self.room_width_m),
            17 => Some(self.room_depth_m),
            18 => Some(self.wall_absorption),
            19 => Some(self.reflection_beta_boost),
            20 => Some(if self.bypass_xtc_filters { 1.0 } else { 0.0 }),
            21 => Some(if self.bypass_spectral_normalization {
                1.0
            } else {
                0.0
            }),
            22 => Some(if self.bypass_neumann_refinement {
                1.0
            } else {
                0.0
            }),
            23 => Some(if self.auto_gain_enabled { 1.0 } else { 0.0 }),
            24 => Some(self.auto_gain_max_db),
            25 => Some(self.auto_gain_smoothing_ms),
            26 => Some(self.head_model),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.distance_m = value,
            1 => self.speaker_angle_deg = value,
            2 => self.head_radius_m = value,
            3 => self.head_offset_x = value,
            4 => self.head_offset_z = value,
            5 => self.head_yaw_deg = value,
            6 => self.head_tracking_smooth_s = value,
            7 => self.beta_base = value,
            8 => self.beta_low_freq_boost = value,
            9 => self.beta_high_freq_boost = value,
            10 => self.head_shadow_cutoff_hz = value,
            11 => self.head_shadow_slope_db_per_octave = value,
            12 => self.max_gain_db = value,
            13 => self.spectral_normalization = value > 0.5,
            14 => self.pinna_model_enabled = value > 0.5,
            15 => self.room_reflections_enabled = value > 0.5,
            16 => self.room_width_m = value,
            17 => self.room_depth_m = value,
            18 => self.wall_absorption = value,
            19 => self.reflection_beta_boost = value,
            20 => self.bypass_xtc_filters = value > 0.5,
            21 => self.bypass_spectral_normalization = value > 0.5,
            22 => self.bypass_neumann_refinement = value > 0.5,
            23 => self.auto_gain_enabled = value > 0.5,
            24 => self.auto_gain_max_db = value,
            25 => self.auto_gain_smoothing_ms = value,
            26 => self.head_model = value,
            _ => {}
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_index_coverage() {
        let p = Params::default();
        for i in 0..PARAMS.len() {
            assert!(
                p.param_value(i).is_some(),
                "param_value({}) returned None",
                i
            );
        }
        assert!(
            p.param_value(PARAMS.len()).is_none(),
            "param_value beyond PARAMS.len() should return None"
        );
    }

    #[test]
    fn roundtrip_serde() {
        let original = Params::default();
        let json = serde_json::to_value(&original).unwrap();
        let restored: Params = serde_json::from_value(json).unwrap();
        assert_eq!(original.distance_m, restored.distance_m);
        assert_eq!(original.speaker_angle_deg, restored.speaker_angle_deg);
        assert_eq!(original.head_radius_m, restored.head_radius_m);
        assert_eq!(original.head_offset_x, restored.head_offset_x);
        assert_eq!(original.head_offset_z, restored.head_offset_z);
        assert_eq!(original.head_yaw_deg, restored.head_yaw_deg);
        assert_eq!(
            original.head_tracking_smooth_s,
            restored.head_tracking_smooth_s
        );
        assert_eq!(original.beta_base, restored.beta_base);
        assert_eq!(original.beta_low_freq_boost, restored.beta_low_freq_boost);
        assert_eq!(
            original.beta_high_freq_boost,
            restored.beta_high_freq_boost
        );
        assert_eq!(
            original.head_shadow_cutoff_hz,
            restored.head_shadow_cutoff_hz
        );
        assert_eq!(
            original.head_shadow_slope_db_per_octave,
            restored.head_shadow_slope_db_per_octave
        );
        assert_eq!(original.max_gain_db, restored.max_gain_db);
        assert_eq!(
            original.spectral_normalization,
            restored.spectral_normalization
        );
        assert_eq!(original.pinna_model_enabled, restored.pinna_model_enabled);
        assert_eq!(
            original.room_reflections_enabled,
            restored.room_reflections_enabled
        );
        assert_eq!(original.room_width_m, restored.room_width_m);
        assert_eq!(original.room_depth_m, restored.room_depth_m);
        assert_eq!(original.wall_absorption, restored.wall_absorption);
        assert_eq!(
            original.reflection_beta_boost,
            restored.reflection_beta_boost
        );
        assert_eq!(original.bypass_xtc_filters, restored.bypass_xtc_filters);
        assert_eq!(
            original.bypass_spectral_normalization,
            restored.bypass_spectral_normalization
        );
        assert_eq!(
            original.bypass_neumann_refinement,
            restored.bypass_neumann_refinement
        );
        assert_eq!(original.auto_gain_enabled, restored.auto_gain_enabled);
        assert_eq!(original.auto_gain_max_db, restored.auto_gain_max_db);
        assert_eq!(
            original.auto_gain_smoothing_ms,
            restored.auto_gain_smoothing_ms
        );
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.distance_m, pk(PARAMS, "distance_m").default_f64());
        assert_eq!(
            p.speaker_angle_deg,
            pk(PARAMS, "speaker_angle_deg").default_f64()
        );
        assert_eq!(p.head_radius_m, pk(PARAMS, "head_radius_m").default_f64());
        assert_eq!(p.head_offset_x, pk(PARAMS, "head_offset_x").default_f64());
        assert_eq!(p.head_offset_z, pk(PARAMS, "head_offset_z").default_f64());
        assert_eq!(p.head_yaw_deg, pk(PARAMS, "head_yaw_deg").default_f64());
        assert_eq!(
            p.head_tracking_smooth_s,
            pk(PARAMS, "head_tracking_smooth_s").default_f64()
        );
        assert_eq!(p.beta_base, pk(PARAMS, "beta_base").default_f64());
        assert_eq!(
            p.beta_low_freq_boost,
            pk(PARAMS, "beta_low_freq_boost").default_f64()
        );
        assert_eq!(
            p.beta_high_freq_boost,
            pk(PARAMS, "beta_high_freq_boost").default_f64()
        );
        assert_eq!(
            p.head_shadow_cutoff_hz,
            pk(PARAMS, "head_shadow_cutoff_hz").default_f64()
        );
        assert_eq!(
            p.head_shadow_slope_db_per_octave,
            pk(PARAMS, "head_shadow_slope_db_per_octave").default_f64()
        );
        assert_eq!(p.max_gain_db, pk(PARAMS, "max_gain_db").default_f64());
        assert_eq!(
            p.spectral_normalization,
            pk(PARAMS, "spectral_normalization").default_bool()
        );
        assert_eq!(
            p.pinna_model_enabled,
            pk(PARAMS, "pinna_model_enabled").default_bool()
        );
        assert_eq!(
            p.room_reflections_enabled,
            pk(PARAMS, "room_reflections_enabled").default_bool()
        );
        assert_eq!(p.room_width_m, pk(PARAMS, "room_width_m").default_f64());
        assert_eq!(p.room_depth_m, pk(PARAMS, "room_depth_m").default_f64());
        assert_eq!(
            p.wall_absorption,
            pk(PARAMS, "wall_absorption").default_f64()
        );
        assert_eq!(
            p.reflection_beta_boost,
            pk(PARAMS, "reflection_beta_boost").default_f64()
        );
        assert_eq!(
            p.bypass_xtc_filters,
            pk(PARAMS, "bypass_xtc_filters").default_bool()
        );
        assert_eq!(
            p.bypass_spectral_normalization,
            pk(PARAMS, "bypass_spectral_normalization").default_bool()
        );
        assert_eq!(
            p.bypass_neumann_refinement,
            pk(PARAMS, "bypass_neumann_refinement").default_bool()
        );
        assert_eq!(
            p.auto_gain_enabled,
            pk(PARAMS, "auto_gain_enabled").default_bool()
        );
        assert_eq!(
            p.auto_gain_max_db,
            pk(PARAMS, "auto_gain_max_db").default_f64()
        );
        assert_eq!(
            p.auto_gain_smoothing_ms,
            pk(PARAMS, "auto_gain_smoothing_ms").default_f64()
        );
    }
}
