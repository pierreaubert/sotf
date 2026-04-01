//! HAL Input plugin parameter definitions — single source of truth.
//!
//! This file owns:
//! - Parameter specs (PARAMS array)
//! - UI layout (LAYOUT)
//! - Serializable state (Params struct with serde defaults)
//! - Index-to-field mapping (PluginParamDef impl)
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

pub const PARAMS: &[ParamSpec] = &[ParamSpec::int(
    "Input Channels",
    "input_channels",
    2,
    1,
    16,
    1,
    "ch",
    "General",
)
.structural()
.doc("Number of HAL input channels")];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[ControlSpec::knob(0)], // input_channels
    main: &[],
    output: &[],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[ColumnConstraint::config(120.0, 0.5)],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// HAL Input plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_input_channels")]
    pub input_channels: f64,
}

fn d_input_channels() -> f64 {
    pk(PARAMS, "input_channels").default_f64()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            input_channels: d_input_channels(),
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
    const PLUGIN_TYPE_KEY: &'static str = "hal_input";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.input_channels),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        if index == 0 {
            self.input_channels = value;
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
        assert_eq!(original.input_channels, restored.input_channels);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.input_channels, pk(PARAMS, "input_channels").default_f64());
    }
}
