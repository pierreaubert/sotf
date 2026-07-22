//! Ambisonics Decoder plugin parameter definitions — single source of truth.
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

pub const TARGET_LAYOUTS: &[&str] = &[
    "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4", "9.1.4", "9.1.6",
];

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::int("Order", "order", 1, 1, 3, 1, "", "Ambisonics")
        .structural()
        .doc("Ambisonics order (1-3)"),
    ParamSpec::choice(
        "Target Layout",
        "target_layout",
        0,
        TARGET_LAYOUTS,
        "Ambisonics",
    )
    .structural()
    .doc("Target speaker layout for decode"),
    ParamSpec::bool_param("Max-rE", "max_re_weighting", true, "Ambisonics")
        .structural()
        .doc("Apply max-rE energy optimization"),
    ParamSpec::bool_param("Dual-Band", "dual_band", false, "Ambisonics")
        .structural()
        .doc("Separate LF/HF decode weights"),
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[
        ControlSpec::knob(0),     // order
        ControlSpec::selector(1), // target_layout
    ],
    main: &[ControlGroup::new(
        "primary",
        "",
        &[ControlSpec::toggle(2), ControlSpec::toggle(3)], // max_re_weighting, dual_band
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

/// Ambisonics Decoder plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_order")]
    pub order: usize,
    #[serde(default = "d_target_layout")]
    pub target_layout: String,
    #[serde(default = "d_max_re_weighting")]
    pub max_re_weighting: bool,
    #[serde(default = "d_dual_band")]
    pub dual_band: bool,
}

fn d_order() -> usize {
    pk(PARAMS, "order").default_usize()
}
fn d_target_layout() -> String {
    TARGET_LAYOUTS[0].to_string()
}
fn d_max_re_weighting() -> bool {
    pk(PARAMS, "max_re_weighting").default_bool()
}
fn d_dual_band() -> bool {
    pk(PARAMS, "dual_band").default_bool()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            order: d_order(),
            target_layout: d_target_layout(),
            max_re_weighting: d_max_re_weighting(),
            dual_band: d_dual_band(),
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
    const PLUGIN_TYPE_KEY: &'static str = "ambisonics";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.order as f64),
            1 => {
                let idx = TARGET_LAYOUTS
                    .iter()
                    .position(|&t| t == self.target_layout)
                    .unwrap_or(0);
                Some(idx as f64)
            }
            2 => Some(if self.max_re_weighting { 1.0 } else { 0.0 }),
            3 => Some(if self.dual_band { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.order = value as usize,
            1 => {
                let idx = (value as usize).min(TARGET_LAYOUTS.len() - 1);
                self.target_layout = TARGET_LAYOUTS[idx].to_string();
            }
            2 => self.max_re_weighting = value > 0.5,
            3 => self.dual_band = value > 0.5,
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
        assert_eq!(original.order, restored.order);
        assert_eq!(original.target_layout, restored.target_layout);
        assert_eq!(original.max_re_weighting, restored.max_re_weighting);
        assert_eq!(original.dual_band, restored.dual_band);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.order, pk(PARAMS, "order").default_usize());
        assert_eq!(p.target_layout, TARGET_LAYOUTS[0]);
        assert_eq!(
            p.max_re_weighting,
            pk(PARAMS, "max_re_weighting").default_bool()
        );
        assert_eq!(p.dual_band, pk(PARAMS, "dual_band").default_bool());
    }
}
