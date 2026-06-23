//! De-Esser plugin parameter definitions — single source of truth.
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
// Mode Constants
// ============================================================================

pub const MODES: &[&str] = &["Wideband", "Split-Band"];

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    // Detection
    ParamSpec::float(
        "Frequency",
        "frequency",
        7000.0,
        2000.0,
        16000.0,
        100.0,
        "Hz",
        "Detection",
    )
    .doc("Center frequency for sibilance detection"),
    ParamSpec::float("Q", "q", 1.5, 0.5, 5.0, 0.1, "", "Detection")
        .doc("Bandwidth of detection filter"),
    // Dynamics
    ParamSpec::float(
        "Threshold",
        "threshold",
        -20.0,
        -60.0,
        0.0,
        0.5,
        "dB",
        "Dynamics",
    )
    .doc("Sibilance detection threshold"),
    ParamSpec::float("Ratio", "ratio", 4.0, 1.0, 20.0, 0.1, ":1", "Dynamics")
        .doc("Compression ratio for sibilance"),
    ParamSpec::float("Attack", "attack", 0.5, 0.1, 10.0, 0.1, "ms", "Dynamics").doc("Attack time"),
    ParamSpec::float(
        "Release", "release", 20.0, 5.0, 200.0, 1.0, "ms", "Dynamics",
    )
    .doc("Release time"),
    // Mode
    ParamSpec::choice("Mode", "mode", 1, MODES, "Mode")
        .setup()
        .doc("Wideband reduces full signal; Split-band only reduces HF"),
    // Output
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "%", "Output")
        .scaled(100.0)
        .output()
        .doc("Dry/wet mix"),
];

// ============================================================================
// UI Layout
// ============================================================================

/// De-Esser: idx 0=frequency, 1=q, 2=threshold, 3=ratio, 4=attack, 5=release, 6=mode, 7=mix
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::selector(6), // mode
    ],
    main: &[
        ControlGroup {
            title: "DETECTION",
            controls: &[
                ControlSpec::slider(0), // frequency
                ControlSpec::slider(1), // q
            ],
        },
        ControlGroup {
            title: "DYNAMICS",
            controls: &[
                ControlSpec::slider(2), // threshold
                ControlSpec::slider(3), // ratio
                ControlSpec::slider(4), // attack
                ControlSpec::slider(5), // release
            ],
        },
    ],
    output: &[
        ControlSpec::meter(-30.0, 0.0), // GR meter
        ControlSpec::knob(7),           // mix
    ],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(100.0, 0.5),
        ColumnConstraint::main(300.0),
        ColumnConstraint::output(120.0, 0.6),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// De-Esser plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_frequency")]
    pub frequency: f64,
    #[serde(default = "d_q")]
    pub q: f64,
    #[serde(default = "d_threshold")]
    pub threshold: f64,
    #[serde(default = "d_ratio")]
    pub ratio: f64,
    #[serde(default = "d_attack")]
    pub attack: f64,
    #[serde(default = "d_release")]
    pub release: f64,
    #[serde(default = "d_mode")]
    pub mode: String,
    #[serde(default = "d_mix")]
    pub mix: f64,
}

fn d_frequency() -> f64 {
    pk(PARAMS, "frequency").default_f64()
}
fn d_q() -> f64 {
    pk(PARAMS, "q").default_f64()
}
fn d_threshold() -> f64 {
    pk(PARAMS, "threshold").default_f64()
}
fn d_ratio() -> f64 {
    pk(PARAMS, "ratio").default_f64()
}
fn d_attack() -> f64 {
    pk(PARAMS, "attack").default_f64()
}
fn d_release() -> f64 {
    pk(PARAMS, "release").default_f64()
}
fn d_mode() -> String {
    MODES[1].to_string()
}
fn d_mix() -> f64 {
    pk(PARAMS, "mix").default_f64()
}

/// Public default helpers used by `DeEsserPluginParams` so its serde defaults
/// come from the same `PARAMS` array used by `PluginParamDef`.
pub fn default_frequency() -> f32 {
    d_frequency() as f32
}
pub fn default_q() -> f32 {
    d_q() as f32
}
pub fn default_threshold() -> f32 {
    d_threshold() as f32
}
pub fn default_ratio() -> f32 {
    d_ratio() as f32
}
pub fn default_attack_ms() -> f32 {
    d_attack() as f32
}
pub fn default_release_ms() -> f32 {
    d_release() as f32
}
pub fn default_mode() -> String {
    d_mode()
}
pub fn default_mix() -> f32 {
    d_mix() as f32
}

impl Default for Params {
    fn default() -> Self {
        Self {
            frequency: d_frequency(),
            q: d_q(),
            threshold: d_threshold(),
            ratio: d_ratio(),
            attack: d_attack(),
            release: d_release(),
            mode: d_mode(),
            mix: d_mix(),
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
    const PLUGIN_TYPE_KEY: &'static str = "de_esser";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.frequency),
            1 => Some(self.q),
            2 => Some(self.threshold),
            3 => Some(self.ratio),
            4 => Some(self.attack),
            5 => Some(self.release),
            6 => Some(
                MODES
                    .iter()
                    .position(|&m| m.eq_ignore_ascii_case(&self.mode))
                    .unwrap_or(1) as f64,
            ),
            7 => Some(self.mix),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.frequency = value,
            1 => self.q = value,
            2 => self.threshold = value,
            3 => self.ratio = value,
            4 => self.attack = value,
            5 => self.release = value,
            6 => {
                let idx = value as usize;
                if let Some(&label) = MODES.get(idx) {
                    self.mode = label.to_string();
                }
            }
            7 => self.mix = value,
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
        assert_eq!(original.frequency, restored.frequency);
        assert_eq!(original.q, restored.q);
        assert_eq!(original.threshold, restored.threshold);
        assert_eq!(original.ratio, restored.ratio);
        assert_eq!(original.attack, restored.attack);
        assert_eq!(original.release, restored.release);
        assert_eq!(original.mode, restored.mode);
        assert_eq!(original.mix, restored.mix);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.frequency, pk(PARAMS, "frequency").default_f64());
        assert_eq!(p.q, pk(PARAMS, "q").default_f64());
        assert_eq!(p.threshold, pk(PARAMS, "threshold").default_f64());
        assert_eq!(p.ratio, pk(PARAMS, "ratio").default_f64());
        assert_eq!(p.attack, pk(PARAMS, "attack").default_f64());
        assert_eq!(p.release, pk(PARAMS, "release").default_f64());
        assert_eq!(p.mode, MODES[1]);
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64());
    }
}
