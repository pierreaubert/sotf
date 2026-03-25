//! Transient Shaper plugin parameter definitions — single source of truth.
//!
//! This file owns:
//! - Parameter specs (PARAMS array)
//! - UI layout (LAYOUT)
//! - Serializable state (Params struct with serde defaults)
//! - Index<->field mapping (PluginParamDef impl)
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
    ParamSpec::float("Attack", "attack", 0.0, -100.0, 100.0, 1.0, "%", "Shape")
        .setup()
        .doc("Transient emphasis (-100% to +100%)"),
    ParamSpec::float("Sustain", "sustain", 0.0, -100.0, 100.0, 1.0, "%", "Shape")
        .setup()
        .doc("Sustain emphasis (-100% to +100%)"),
    ParamSpec::float(
        "Sensitivity",
        "sensitivity",
        0.0,
        -12.0,
        12.0,
        0.1,
        "dB",
        "Detection",
    )
    .setup()
    .doc("Detection sensitivity offset"),
    ParamSpec::float("Output", "output_gain", 0.0, -12.0, 12.0, 0.1, "dB", "Output")
        .output()
        .doc("Output gain compensation"),
    ParamSpec::float("Mix", "mix", 1.0, 0.0, 1.0, 0.01, "%", "Output")
        .scaled(100.0)
        .output()
        .doc("Dry/wet mix"),
];

// ============================================================================
// UI Layout
// ============================================================================

/// TransientShaper: idx 0=attack, 1=sustain, 2=sensitivity, 3=output_gain, 4=mix
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::knob(2), // sensitivity
    ],
    main: &[ControlGroup {
        title: "SHAPE",
        controls: &[
            ControlSpec::slider(0), // attack
            ControlSpec::slider(1), // sustain
        ],
    }],
    output: &[
        ControlSpec::knob(3), // output_gain
        ControlSpec::knob(4), // mix
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

/// Transient Shaper plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_attack")]
    pub attack: f64,
    #[serde(default = "d_sustain")]
    pub sustain: f64,
    #[serde(default = "d_sensitivity")]
    pub sensitivity: f64,
    #[serde(default = "d_output_gain")]
    pub output_gain: f64,
    #[serde(default = "d_mix")]
    pub mix: f64,
}

fn d_attack() -> f64 {
    pk(PARAMS, "attack").default_f64()
}
fn d_sustain() -> f64 {
    pk(PARAMS, "sustain").default_f64()
}
fn d_sensitivity() -> f64 {
    pk(PARAMS, "sensitivity").default_f64()
}
fn d_output_gain() -> f64 {
    pk(PARAMS, "output_gain").default_f64()
}
fn d_mix() -> f64 {
    pk(PARAMS, "mix").default_f64()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            attack: d_attack(),
            sustain: d_sustain(),
            sensitivity: d_sensitivity(),
            output_gain: d_output_gain(),
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
    const PLUGIN_TYPE_KEY: &'static str = "transient_shaper";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.attack),
            1 => Some(self.sustain),
            2 => Some(self.sensitivity),
            3 => Some(self.output_gain),
            4 => Some(self.mix),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.attack = value,
            1 => self.sustain = value,
            2 => self.sensitivity = value,
            3 => self.output_gain = value,
            4 => self.mix = value,
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
        assert_eq!(original.attack, restored.attack);
        assert_eq!(original.sustain, restored.sustain);
        assert_eq!(original.sensitivity, restored.sensitivity);
        assert_eq!(original.output_gain, restored.output_gain);
        assert_eq!(original.mix, restored.mix);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.attack, pk(PARAMS, "attack").default_f64());
        assert_eq!(p.sustain, pk(PARAMS, "sustain").default_f64());
        assert_eq!(p.sensitivity, pk(PARAMS, "sensitivity").default_f64());
        assert_eq!(p.output_gain, pk(PARAMS, "output_gain").default_f64());
        assert_eq!(p.mix, pk(PARAMS, "mix").default_f64());
    }
}
