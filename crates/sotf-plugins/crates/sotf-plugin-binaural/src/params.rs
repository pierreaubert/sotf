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
use sotf_host::param_specs::{find_by_key as pk, ParamSpec};
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

// ============================================================================
// Parameter Specifications
// ============================================================================

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::file_path("SOFA File", "sofa_file", "General")
        .setup()
        .doc("HRTF data file (SOFA format)"),
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
    ParamSpec::bool_param("Optimization", "enable_optimization", true, "General")
        .structural()
        .setup()
        .doc("Enable HRIR filter optimization"),
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
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::file_picker(0), // sofa_file
        ControlSpec::label(1),       // input_channels (read-only)
        ControlSpec::toggle(2),      // enable_optimization
    ],
    main: &[ControlGroup {
        title: "CONTROLS",
        controls: &[
            ControlSpec::knob(3), // externalization
            ControlSpec::knob(4), // near_field_strength
        ],
    }],
    output: &[],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(180.0, 0.5),
        ColumnConstraint::main(200.0),
    ],
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
    #[serde(default = "d_enable_optimization")]
    pub enable_optimization: bool,
    #[serde(default = "d_externalization")]
    pub externalization: f64,
    #[serde(default = "d_near_field_strength")]
    pub near_field_strength: f64,
}

fn d_input_channels() -> usize {
    pk(PARAMS, "input_channels").default_usize()
}
fn d_enable_optimization() -> bool {
    pk(PARAMS, "enable_optimization").default_bool()
}
fn d_externalization() -> f64 {
    pk(PARAMS, "externalization").default_f64()
}
fn d_near_field_strength() -> f64 {
    pk(PARAMS, "near_field_strength").default_f64()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            input_channels: d_input_channels(),
            enable_optimization: d_enable_optimization(),
            externalization: d_externalization(),
            near_field_strength: d_near_field_strength(),
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
            2 => Some(if self.enable_optimization { 1.0 } else { 0.0 }),
            3 => Some(self.externalization),
            4 => Some(self.near_field_strength),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => {} // sofa_file (FilePath — handled separately)
            1 => self.input_channels = value as usize,
            2 => self.enable_optimization = value > 0.5,
            3 => self.externalization = value,
            4 => self.near_field_strength = value,
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
        assert_eq!(
            original.enable_optimization,
            restored.enable_optimization
        );
        assert_eq!(original.externalization, restored.externalization);
        assert_eq!(original.near_field_strength, restored.near_field_strength);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(
            p.input_channels,
            pk(PARAMS, "input_channels").default_usize()
        );
        assert_eq!(
            p.enable_optimization,
            pk(PARAMS, "enable_optimization").default_bool()
        );
        assert_eq!(
            p.externalization,
            pk(PARAMS, "externalization").default_f64()
        );
        assert_eq!(
            p.near_field_strength,
            pk(PARAMS, "near_field_strength").default_f64()
        );
    }
}
