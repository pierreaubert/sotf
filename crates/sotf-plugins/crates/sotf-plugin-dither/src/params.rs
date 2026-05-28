//! Dither plugin parameter definitions — single source of truth.
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
    ParamSpec::choice("Bit Depth", "bit_depth", 0, &["16", "20", "24"], "Dither")
        .doc("Target bit depth for quantization"),
    ParamSpec::bool_labeled(
        "Noise Shaping",
        "noise_shaping",
        true,
        "On",
        "Off",
        "Dither",
    )
    .doc("F-weighted noise shaping (Wannamaker 1992)"),
    ParamSpec::choice(
        "Dither Type",
        "dither_type",
        0,
        &["TPDF", "None (round)", "Truncate"],
        "Dither",
    )
    .doc("TPDF with rounding, round-only passthrough, or truncated quantization"),
];

// ============================================================================
// UI Layout
// ============================================================================

/// Dither: idx 0=bit_depth, 1=noise_shaping, 2=dither_type
pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[ControlSpec::toggle(1)], // noise_shaping
    main: &[ControlGroup {
        title: "DITHER",
        controls: &[
            ControlSpec::selector(0), // bit_depth
            ControlSpec::selector(2), // dither_type
        ],
    }],
    output: &[],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(80.0, 0.4),
        ColumnConstraint::main(200.0),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// Dither plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_bit_depth")]
    pub bit_depth: usize,
    #[serde(default = "d_noise_shaping")]
    pub noise_shaping: bool,
    #[serde(default = "d_dither_type")]
    pub dither_type: usize,
}

fn d_bit_depth() -> usize {
    pk(PARAMS, "bit_depth").default_usize()
}
fn d_noise_shaping() -> bool {
    pk(PARAMS, "noise_shaping").default_bool()
}
fn d_dither_type() -> usize {
    pk(PARAMS, "dither_type").default_usize()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            bit_depth: d_bit_depth(),
            noise_shaping: d_noise_shaping(),
            dither_type: d_dither_type(),
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
    const PLUGIN_TYPE_KEY: &'static str = "dither";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.bit_depth as f64),
            1 => Some(if self.noise_shaping { 1.0 } else { 0.0 }),
            2 => Some(self.dither_type as f64),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.bit_depth = value as usize,
            1 => self.noise_shaping = value > 0.5,
            2 => self.dither_type = value as usize,
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
        assert_eq!(original.bit_depth, restored.bit_depth);
        assert_eq!(original.noise_shaping, restored.noise_shaping);
        assert_eq!(original.dither_type, restored.dither_type);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.bit_depth, pk(PARAMS, "bit_depth").default_usize());
        assert_eq!(p.noise_shaping, pk(PARAMS, "noise_shaping").default_bool());
        assert_eq!(p.dither_type, pk(PARAMS, "dither_type").default_usize());
    }
}
