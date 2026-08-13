//! AEC plugin parameter definitions — single source of truth.
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
use sotf_host::param_specs::{ParamSpec, find_by_key as pk};
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::float(
        "Echo Tail",
        "echo_tail_ms",
        200.0,
        50.0,
        500.0,
        10.0,
        "ms",
        "AEC",
    )
    .structural()
    .doc("Max echo path length to cancel"),
    ParamSpec::float("Step Size", "step_size", 0.5, 0.1, 0.9, 0.05, "", "AEC")
        .structural()
        .doc("Adaptive filter convergence rate"),
    ParamSpec::bool_param("Post-Filter", "post_filter_enabled", true, "AEC")
        .output()
        .doc("Apply residual echo suppression"),
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
            ControlSpec::slider(0), // echo_tail_ms
            ControlSpec::slider(1), // step_size
        ],
    )],
    output: &[ControlSpec::toggle(2)], // post_filter_enabled
    tabs: &[],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::main(200.0),
        ColumnConstraint::output(120.0, 0.6),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// AEC plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_echo_tail_ms")]
    pub echo_tail_ms: f64,
    #[serde(default = "d_step_size")]
    pub step_size: f64,
    #[serde(default = "d_post_filter_enabled")]
    pub post_filter_enabled: bool,
}

fn d_echo_tail_ms() -> f64 {
    pk(PARAMS, "echo_tail_ms").default_f64()
}
fn d_step_size() -> f64 {
    pk(PARAMS, "step_size").default_f64()
}
fn d_post_filter_enabled() -> bool {
    pk(PARAMS, "post_filter_enabled").default_bool()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            echo_tail_ms: d_echo_tail_ms(),
            step_size: d_step_size(),
            post_filter_enabled: d_post_filter_enabled(),
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
    const PLUGIN_TYPE_KEY: &'static str = "aec";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.echo_tail_ms),
            1 => Some(self.step_size),
            2 => Some(if self.post_filter_enabled { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.echo_tail_ms = value,
            1 => self.step_size = value,
            2 => self.post_filter_enabled = value > 0.5,
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
        assert_eq!(original.echo_tail_ms, restored.echo_tail_ms);
        assert_eq!(original.step_size, restored.step_size);
        assert_eq!(original.post_filter_enabled, restored.post_filter_enabled);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.echo_tail_ms, pk(PARAMS, "echo_tail_ms").default_f64());
        assert_eq!(p.step_size, pk(PARAMS, "step_size").default_f64());
        assert_eq!(
            p.post_filter_enabled,
            pk(PARAMS, "post_filter_enabled").default_bool()
        );
    }
}
