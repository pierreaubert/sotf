//! AB Compare plugin parameter definitions — single source of truth.
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
// Choice Label Constants
// ============================================================================

pub const MIX_MODE_LABELS: &[&str] = &["Pot", "Binary"];
pub const SELECTED_PATH_LABELS: &[&str] = &["A", "B"];
pub const LOUDNESS_TYPE_LABELS: &[&str] = &["Momentary", "ShortTerm"];

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::float("Mix (A/B)", "mix", 0.0, -1.0, 1.0, 0.05, "", "Mix")
        .scaled(100.0)
        .doc("Crossfade between path A and B"),
    ParamSpec::choice("Mix Mode", "mix_mode", 0, MIX_MODE_LABELS, "Mix")
        .doc("Smooth pot or instant switch"),
    ParamSpec::choice(
        "Selected Path",
        "selected_path",
        0,
        SELECTED_PATH_LABELS,
        "Mix",
    )
    .doc("Active path in binary mode"),
    ParamSpec::bool_labeled("Bypass", "bypass", false, "Yes", "No", "Mix")
        .doc("Bypass both paths (dry signal)"),
    ParamSpec::bool_param("Auto Gain", "auto_gain_enabled", true, "Auto Gain")
        .output()
        .doc("Loudness-match A and B paths"),
    ParamSpec::choice(
        "Loudness Type",
        "loudness_type",
        0,
        LOUDNESS_TYPE_LABELS,
        "Auto Gain",
    )
    .output()
    .doc("Loudness measurement window"),
    ParamSpec::float(
        "Max Auto Gain",
        "max_auto_gain_db",
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
        "Gain Smoothing",
        "gain_smoothing_ms",
        100.0,
        1.0,
        500.0,
        5.0,
        "ms",
        "Auto Gain",
    )
    .output()
    .doc("Auto gain transition time"),
    ParamSpec::float(
        "Mix Transition",
        "mix_transition_ms",
        50.0,
        1.0,
        500.0,
        5.0,
        "ms",
        "Mix",
    )
    .doc("A/B crossfade duration"),
    ParamSpec::file_path("Path A Config", "path_a_config", "Configuration")
        .doc("Plugin chain config for path A"),
    ParamSpec::file_path("Path B Config", "path_b_config", "Configuration")
        .doc("Plugin chain config for path B"),
    ParamSpec::bool_param("Phase Invert A", "phase_invert_a", false, "Phase")
        .doc("Invert polarity of path A"),
    ParamSpec::bool_param("Phase Invert B", "phase_invert_b", false, "Phase")
        .doc("Invert polarity of path B"),
    ParamSpec::bool_param("Difference Mode", "difference_mode", false, "Mix")
        .doc("Output A minus B difference"),
];

// ============================================================================
// UI Layout
// ============================================================================

/// AB Compare: idx 0=mix, 1=mix_mode, 2=selected_path, 3=bypass,
/// 4=auto_gain, 5=loudness_type, 6=max_auto_gain, 7=gain_smoothing,
/// 8=mix_transition, 9=path_a_config, 10=path_b_config,
/// 11=phase_invert_a, 12=phase_invert_b, 13=difference_mode
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[],
    main: &[
        ControlGroup {
            title: "A/B MIX",
            controls: &[
                ControlSpec::slider(0),                         // mix (A/B)
                ControlSpec::button_set(1, &["Pot", "Binary"]), // mix_mode
                ControlSpec::button_set(2, &["A", "B"]),        // selected_path
                ControlSpec::toggle(3),                         // bypass
                ControlSpec::knob(8),                           // mix_transition_ms
            ],
        },
        ControlGroup {
            title: "AUTO GAIN",
            controls: &[
                ControlSpec::toggle(4),   // auto_gain
                ControlSpec::selector(5), // loudness_type
                ControlSpec::knob(6),     // max_auto_gain
                ControlSpec::knob(7),     // gain_smoothing
            ],
        },
    ],
    output: &[],
    tabs: &[TabSpec {
        name: "Paths",
        controls: &[
            ControlSpec::file_picker(9),  // path_a_config
            ControlSpec::file_picker(10), // path_b_config
        ],
    }],
    visualizations: &[],
    column_constraints: &[ColumnConstraint::main(350.0)],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// AB Compare plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
///
/// Choice params (mix_mode, selected_path, loudness_type) are stored as
/// usize indices. FilePath params (path_a_config, path_b_config) are
/// skipped in param_value/set_param_value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_mix")]
    pub mix: f64,
    #[serde(default = "d_mix_mode")]
    pub mix_mode: usize,
    #[serde(default = "d_selected_path")]
    pub selected_path: usize,
    #[serde(default = "d_bypass")]
    pub bypass: bool,
    #[serde(default = "d_auto_gain_enabled")]
    pub auto_gain_enabled: bool,
    #[serde(default = "d_loudness_type")]
    pub loudness_type: usize,
    #[serde(default = "d_max_auto_gain_db")]
    pub max_auto_gain_db: f64,
    #[serde(default = "d_gain_smoothing_ms")]
    pub gain_smoothing_ms: f64,
    #[serde(default = "d_mix_transition_ms")]
    pub mix_transition_ms: f64,
    // path_a_config: FilePath — handled separately
    // path_b_config: FilePath — handled separately
    #[serde(default = "d_phase_invert_a")]
    pub phase_invert_a: bool,
    #[serde(default = "d_phase_invert_b")]
    pub phase_invert_b: bool,
    #[serde(default = "d_difference_mode")]
    pub difference_mode: bool,
}

fn d_mix() -> f64 {
    pk(PARAMS, "mix").default_f64()
}
fn d_mix_mode() -> usize {
    pk(PARAMS, "mix_mode").default_usize()
}
fn d_selected_path() -> usize {
    pk(PARAMS, "selected_path").default_usize()
}
fn d_bypass() -> bool {
    pk(PARAMS, "bypass").default_bool()
}
fn d_auto_gain_enabled() -> bool {
    pk(PARAMS, "auto_gain_enabled").default_bool()
}
fn d_loudness_type() -> usize {
    pk(PARAMS, "loudness_type").default_usize()
}
fn d_max_auto_gain_db() -> f64 {
    pk(PARAMS, "max_auto_gain_db").default_f64()
}
fn d_gain_smoothing_ms() -> f64 {
    pk(PARAMS, "gain_smoothing_ms").default_f64()
}
fn d_mix_transition_ms() -> f64 {
    pk(PARAMS, "mix_transition_ms").default_f64()
}
fn d_phase_invert_a() -> bool {
    pk(PARAMS, "phase_invert_a").default_bool()
}
fn d_phase_invert_b() -> bool {
    pk(PARAMS, "phase_invert_b").default_bool()
}
fn d_difference_mode() -> bool {
    pk(PARAMS, "difference_mode").default_bool()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            mix: d_mix(),
            mix_mode: d_mix_mode(),
            selected_path: d_selected_path(),
            bypass: d_bypass(),
            auto_gain_enabled: d_auto_gain_enabled(),
            loudness_type: d_loudness_type(),
            max_auto_gain_db: d_max_auto_gain_db(),
            gain_smoothing_ms: d_gain_smoothing_ms(),
            mix_transition_ms: d_mix_transition_ms(),
            phase_invert_a: d_phase_invert_a(),
            phase_invert_b: d_phase_invert_b(),
            difference_mode: d_difference_mode(),
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
    const PLUGIN_TYPE_KEY: &'static str = "ab_compare";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.mix),
            1 => Some(self.mix_mode as f64),
            2 => Some(self.selected_path as f64),
            3 => Some(if self.bypass { 1.0 } else { 0.0 }),
            4 => Some(if self.auto_gain_enabled { 1.0 } else { 0.0 }),
            5 => Some(self.loudness_type as f64),
            6 => Some(self.max_auto_gain_db),
            7 => Some(self.gain_smoothing_ms),
            8 => Some(self.mix_transition_ms),
            9 => None,  // path_a_config (FilePath — handled separately)
            10 => None, // path_b_config (FilePath — handled separately)
            11 => Some(if self.phase_invert_a { 1.0 } else { 0.0 }),
            12 => Some(if self.phase_invert_b { 1.0 } else { 0.0 }),
            13 => Some(if self.difference_mode { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.mix = value,
            1 => self.mix_mode = value as usize,
            2 => self.selected_path = value as usize,
            3 => self.bypass = value > 0.5,
            4 => self.auto_gain_enabled = value > 0.5,
            5 => self.loudness_type = value as usize,
            6 => self.max_auto_gain_db = value,
            7 => self.gain_smoothing_ms = value,
            8 => self.mix_transition_ms = value,
            9 => {}  // path_a_config (FilePath — handled separately)
            10 => {} // path_b_config (FilePath — handled separately)
            11 => self.phase_invert_a = value > 0.5,
            12 => self.phase_invert_b = value > 0.5,
            13 => self.difference_mode = value > 0.5,
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
            // indices 9, 10 are FilePath — return None by design
            if i == 9 || i == 10 {
                assert!(
                    p.param_value(i).is_none(),
                    "param_value({}) should return None for FilePath",
                    i
                );
            } else {
                assert!(
                    p.param_value(i).is_some(),
                    "param_value({}) returned None",
                    i
                );
            }
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
        assert_eq!(original.mix, restored.mix);
        assert_eq!(original.mix_mode, restored.mix_mode);
        assert_eq!(original.selected_path, restored.selected_path);
        assert_eq!(original.bypass, restored.bypass);
        assert_eq!(original.auto_gain_enabled, restored.auto_gain_enabled);
        assert_eq!(original.loudness_type, restored.loudness_type);
        assert_eq!(original.max_auto_gain_db, restored.max_auto_gain_db);
        assert_eq!(original.gain_smoothing_ms, restored.gain_smoothing_ms);
        assert_eq!(original.mix_transition_ms, restored.mix_transition_ms);
        assert_eq!(original.phase_invert_a, restored.phase_invert_a);
        assert_eq!(original.phase_invert_b, restored.phase_invert_b);
        assert_eq!(original.difference_mode, restored.difference_mode);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64());
        assert_eq!(p.mix_mode, pk(PARAMS, "mix_mode").default_usize());
        assert_eq!(p.selected_path, pk(PARAMS, "selected_path").default_usize());
        assert_eq!(p.bypass, pk(PARAMS, "bypass").default_bool());
        assert_eq!(
            p.auto_gain_enabled,
            pk(PARAMS, "auto_gain_enabled").default_bool()
        );
        assert_eq!(p.loudness_type, pk(PARAMS, "loudness_type").default_usize());
        assert_eq!(
            p.max_auto_gain_db,
            pk(PARAMS, "max_auto_gain_db").default_f64()
        );
        assert_eq!(
            p.gain_smoothing_ms,
            pk(PARAMS, "gain_smoothing_ms").default_f64()
        );
        assert_eq!(
            p.mix_transition_ms,
            pk(PARAMS, "mix_transition_ms").default_f64()
        );
        assert_eq!(
            p.phase_invert_a,
            pk(PARAMS, "phase_invert_a").default_bool()
        );
        assert_eq!(
            p.phase_invert_b,
            pk(PARAMS, "phase_invert_b").default_bool()
        );
        assert_eq!(
            p.difference_mode,
            pk(PARAMS, "difference_mode").default_bool()
        );
    }
}
