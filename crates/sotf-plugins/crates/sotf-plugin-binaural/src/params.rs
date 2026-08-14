//! Binaural plugin parameter definitions — single source of truth.
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
use sotf_host::param_specs::{ParamSpec, find_by_key as pk};
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    // Index 0
    ParamSpec::file_path("SOFA File", "sofa_file", "General")
        .setup()
        .doc("HRTF data file (SOFA format)"),
    // Index 1
    ParamSpec::int(
        "Input Channels",
        "input_channels",
        2,
        2,
        16,
        1,
        "ch",
        "General",
    )
    .structural()
    .setup()
    .doc("Number of surround input channels"),
    // Index 2
    ParamSpec::float(
        "Externalization",
        "externalization",
        0.0,
        0.0,
        1.0,
        0.05,
        "",
        "General",
    )
    .doc("Out-of-head perception strength"),
    // Index 3
    ParamSpec::float(
        "Near-field",
        "near_field_strength",
        0.0,
        0.0,
        1.0,
        0.05,
        "",
        "General",
    )
    .structural()
    .doc("Near-field compensation amount"),
    // Index 4
    ParamSpec::choice(
        "Crossfade Mode",
        "crossfade_mode",
        0,
        &["Linear", "Spectral"],
        "Quality",
    )
    .doc("Linear: simple blend (may cause tonal shift). Spectral: magnitude interpolation + phase reconstruction (smoother)"),
    // --- Phase 4E: SOTA additions ---
    // Index 5
    ParamSpec::bool_param("Late Reverb", "late_reverb_enabled", false, "Room")
        .doc("Add FDN-based late reverb tail after early reflections"),
    // Index 6
    ParamSpec::float("Reverb Mix", "late_reverb_mix", 0.3, 0.0, 1.0, 0.05, "", "Room")
        .scaled(100.0)
        .doc("Late reverb wet/dry mix"),
    // Index 7
    ParamSpec::float("Reverb Time", "late_reverb_rt60", 1.0, 0.1, 5.0, 0.1, "s", "Room")
        .doc("RT60 decay time for late reverb"),
    // Index 8
    ParamSpec::float("Reverb Damping", "late_reverb_damping", 0.3, 0.0, 1.0, 0.05, "", "Room")
        .doc("High-frequency damping (0=bright, 1=dark)"),
    // Index 9
    ParamSpec::float("Crossfade", "crossfade_ms", 50.0, 10.0, 500.0, 5.0, "ms", "Quality")
        .doc("HRTF state transition duration"),
    // Indices 10-12
    ParamSpec::float("Head Yaw", "head_yaw_deg", 0.0, -180.0, 180.0, 1.0, "deg", "Tracking"),
    ParamSpec::float("Head Pitch", "head_pitch_deg", 0.0, -180.0, 180.0, 1.0, "deg", "Tracking"),
    ParamSpec::float("Head Roll", "head_roll_deg", 0.0, -180.0, 180.0, 1.0, "deg", "Tracking"),
    // Index 13
    ParamSpec::file_path("HRTF Database", "hrtf_database_dir", "Setup")
        .setup()
        .doc("Directory containing anthropometrically named SOFA files"),
    // Indices 14-15
    ParamSpec::float("Head Width", "head_width_cm", 15.0, 10.0, 25.0, 0.5, "cm", "Tracking")
        .structural(),
    ParamSpec::float("Ear Height", "ear_height_cm", 10.0, 4.0, 16.0, 0.5, "cm", "Tracking")
        .structural(),
    // Legacy no-op fields are intentionally ignored by serde rather than
    // retained in live parameter state.
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::label(1),        // input_channels (read-only)
        ControlSpec::file_picker(13), // hrtf_database_dir
    ],
    main: &[ControlGroup::new(
        "CONTROLS",
        "CONTROLS",
        &[
            ControlSpec::file_picker(0), // sofa_file
            ControlSpec::knob(2),        // externalization
            ControlSpec::knob(3),        // near_field_strength
            ControlSpec::selector(4),    // crossfade_mode
            ControlSpec::knob(9),        // crossfade_ms
        ],
    )],
    output: &[],
    tabs: &[
        TabSpec {
            name: "Reverb",
            controls: &[
                ControlSpec::toggle(5), // late_reverb_enabled
                ControlSpec::knob(6).enabled_when(ParamCondition::bool(5, true)), // mix
                ControlSpec::knob(7).enabled_when(ParamCondition::bool(5, true)), // rt60
                ControlSpec::knob(8).enabled_when(ParamCondition::bool(5, true)), // damping
            ],
        },
        TabSpec {
            name: "Tracking",
            controls: &[
                ControlSpec::knob(10), // head_yaw_deg
                ControlSpec::knob(11), // head_pitch_deg
                ControlSpec::knob(12), // head_roll_deg
                ControlSpec::knob(14), // head_width_cm
                ControlSpec::knob(15), // ear_height_cm
            ],
        },
    ],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(180.0, 0.5),
        ColumnConstraint::main(200.0),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State (PluginParamDef pattern)
// ============================================================================

/// Binaural plugin parameters for PluginParamDef.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    // sofa_file is handled separately (FilePath — skip in param_value/set_param_value)
    #[serde(default = "d_input_channels")]
    pub input_channels: usize,
    #[serde(default = "d_externalization")]
    pub externalization: f64,
    #[serde(default = "d_near_field_strength")]
    pub near_field_strength: f64,
    #[serde(default = "d_crossfade_mode")]
    pub crossfade_mode: usize,
    #[serde(default)]
    pub late_reverb_enabled: bool,
    #[serde(default = "d_late_reverb_mix")]
    pub late_reverb_mix: f64,
    #[serde(default = "d_late_reverb_rt60")]
    pub late_reverb_rt60: f64,
    #[serde(default = "d_late_reverb_damping")]
    pub late_reverb_damping: f64,
    #[serde(default = "d_crossfade_ms")]
    pub crossfade_ms: f64,
    #[serde(default)]
    pub head_yaw_deg: f64,
    #[serde(default)]
    pub head_pitch_deg: f64,
    #[serde(default)]
    pub head_roll_deg: f64,
    #[serde(default = "d_head_width_cm")]
    pub head_width_cm: f64,
    #[serde(default = "d_ear_height_cm")]
    pub ear_height_cm: f64,
}

fn d_crossfade_ms() -> f64 {
    pk(PARAMS, "crossfade_ms").default_f64()
}
fn d_head_width_cm() -> f64 {
    pk(PARAMS, "head_width_cm").default_f64()
}
fn d_ear_height_cm() -> f64 {
    pk(PARAMS, "ear_height_cm").default_f64()
}

fn d_late_reverb_mix() -> f64 {
    pk(PARAMS, "late_reverb_mix").default_f64()
}
fn d_late_reverb_rt60() -> f64 {
    pk(PARAMS, "late_reverb_rt60").default_f64()
}
fn d_late_reverb_damping() -> f64 {
    pk(PARAMS, "late_reverb_damping").default_f64()
}

fn d_input_channels() -> usize {
    pk(PARAMS, "input_channels").default_usize()
}
fn d_externalization() -> f64 {
    pk(PARAMS, "externalization").default_f64()
}
fn d_near_field_strength() -> f64 {
    pk(PARAMS, "near_field_strength").default_f64()
}
fn d_crossfade_mode() -> usize {
    pk(PARAMS, "crossfade_mode").default_usize()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            input_channels: d_input_channels(),
            externalization: d_externalization(),
            near_field_strength: d_near_field_strength(),
            crossfade_mode: d_crossfade_mode(),
            late_reverb_enabled: false,
            late_reverb_mix: d_late_reverb_mix(),
            late_reverb_rt60: d_late_reverb_rt60(),
            late_reverb_damping: d_late_reverb_damping(),
            crossfade_ms: d_crossfade_ms(),
            head_yaw_deg: 0.0,
            head_pitch_deg: 0.0,
            head_roll_deg: 0.0,
            head_width_cm: d_head_width_cm(),
            ear_height_cm: d_ear_height_cm(),
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
    const PLUGIN_TYPE_KEY: &'static str = "binaural";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => None, // sofa_file (FilePath — handled separately)
            1 => Some(self.input_channels as f64),
            2 => Some(self.externalization),
            3 => Some(self.near_field_strength),
            4 => Some(self.crossfade_mode as f64),
            5 => Some(if self.late_reverb_enabled { 1.0 } else { 0.0 }),
            6 => Some(self.late_reverb_mix),
            7 => Some(self.late_reverb_rt60),
            8 => Some(self.late_reverb_damping),
            9 => Some(self.crossfade_ms),
            10 => Some(self.head_yaw_deg),
            11 => Some(self.head_pitch_deg),
            12 => Some(self.head_roll_deg),
            13 => None,
            14 => Some(self.head_width_cm),
            15 => Some(self.ear_height_cm),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => {} // sofa_file (FilePath — handled separately)
            1 => self.input_channels = value as usize,
            2 => self.externalization = value,
            3 => self.near_field_strength = value,
            4 => self.crossfade_mode = value as usize,
            5 => self.late_reverb_enabled = value > 0.5,
            6 => self.late_reverb_mix = value,
            7 => self.late_reverb_rt60 = value,
            8 => self.late_reverb_damping = value,
            9 => self.crossfade_ms = value,
            10 => self.head_yaw_deg = value,
            11 => self.head_pitch_deg = value,
            12 => self.head_roll_deg = value,
            13 => {}
            14 => self.head_width_cm = value,
            15 => self.ear_height_cm = value,
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
        // index 0 is FilePath (sofa_file) — returns None by design
        assert!(p.param_value(0).is_none(), "sofa_file should return None");
        for i in 1..PARAMS.len() {
            if i == 13 {
                continue;
            }
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
        assert_eq!(original.input_channels, restored.input_channels);
        assert_eq!(original.externalization, restored.externalization);
        assert_eq!(original.near_field_strength, restored.near_field_strength);
        assert_eq!(original.crossfade_mode, restored.crossfade_mode);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(
            p.input_channels,
            pk(PARAMS, "input_channels").default_usize()
        );
        assert_eq!(
            p.externalization,
            pk(PARAMS, "externalization").default_f64()
        );
        assert_eq!(
            p.near_field_strength,
            pk(PARAMS, "near_field_strength").default_f64()
        );
        assert_eq!(
            p.crossfade_mode,
            pk(PARAMS, "crossfade_mode").default_usize()
        );
    }
}
