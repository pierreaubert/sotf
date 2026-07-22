//! Gain plugin parameter definitions — single source of truth.
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
    ParamSpec::float("Gain", "gain_db", 0.0, -60.0, 20.0, 0.5, "dB", "General")
        .doc("Volume level adjustment"),
    ParamSpec::float(
        "Smoothing",
        "smoothing_ms",
        10.0,
        0.0,
        100.0,
        1.0,
        "ms",
        "General",
    )
    .doc("Transition time for gain changes"),
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[],
    main: &[ControlGroup::new(
        "primary",
        "",
        &[
            ControlSpec::knob_large(0), // gain_db
            ControlSpec::knob(1),       // smoothing_ms
        ],
    )],
    output: &[],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[ColumnConstraint::main(200.0)],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// Gain plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_gain_db")]
    pub gain_db: f64,
    #[serde(default = "d_smoothing_ms")]
    pub smoothing_ms: f64,
}

fn d_gain_db() -> f64 {
    pk(PARAMS, "gain_db").default_f64()
}
fn d_smoothing_ms() -> f64 {
    pk(PARAMS, "smoothing_ms").default_f64()
}

/// Canonical default smoothing time for the gain plugin (milliseconds).
pub fn default_smoothing_ms() -> f32 {
    d_smoothing_ms() as f32
}

impl Default for Params {
    fn default() -> Self {
        Self {
            gain_db: d_gain_db(),
            smoothing_ms: d_smoothing_ms(),
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
    const PLUGIN_TYPE_KEY: &'static str = "gain";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.gain_db),
            1 => Some(self.smoothing_ms),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.gain_db = value,
            1 => self.smoothing_ms = value,
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
        assert_eq!(original.gain_db, restored.gain_db);
        assert_eq!(original.smoothing_ms, restored.smoothing_ms);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.gain_db, pk(PARAMS, "gain_db").default_f64());
        assert_eq!(p.smoothing_ms, pk(PARAMS, "smoothing_ms").default_f64());
    }
}
