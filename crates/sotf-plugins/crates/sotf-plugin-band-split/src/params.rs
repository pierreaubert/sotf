//! Band Split plugin parameter definitions — single source of truth.
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

pub const CROSSOVER_TYPES: &[&str] = &["LR24", "LR48"];

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::float(
        "Frequency",
        "frequency",
        300.0,
        20.0,
        20000.0,
        10.0,
        "Hz",
        "General",
    )
    .structural()
    .doc("Crossover split frequency"),
    ParamSpec::choice("Type", "crossover_type", 0, CROSSOVER_TYPES, "General")
        .structural()
        .doc("Filter slope (24 or 48 dB/oct)"),
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[],
    main: &[ControlGroup {
        title: "CROSSOVER",
        controls: &[
            ControlSpec::knob(0),                          // frequency
            ControlSpec::button_set(1, &["LR24", "LR48"]), // type
        ],
    }],
    output: &[],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[ColumnConstraint::main(200.0)],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// Band Split plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_frequency")]
    pub frequency: f64,
    #[serde(rename = "type", default = "d_crossover_type")]
    pub crossover_type: String,
}

fn d_frequency() -> f64 {
    pk(PARAMS, "frequency").default_f64()
}
fn d_crossover_type() -> String {
    CROSSOVER_TYPES[0].to_string()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            frequency: d_frequency(),
            crossover_type: d_crossover_type(),
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
    const PLUGIN_TYPE_KEY: &'static str = "band_split";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.frequency),
            1 => {
                let idx = CROSSOVER_TYPES
                    .iter()
                    .position(|&t| t == self.crossover_type)
                    .unwrap_or(0);
                Some(idx as f64)
            }
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.frequency = value,
            1 => {
                let idx = (value as usize).min(CROSSOVER_TYPES.len() - 1);
                self.crossover_type = CROSSOVER_TYPES[idx].to_string();
            }
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
        assert_eq!(original.crossover_type, restored.crossover_type);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.frequency, pk(PARAMS, "frequency").default_f64());
        assert_eq!(p.crossover_type, CROSSOVER_TYPES[0]);
    }
}
