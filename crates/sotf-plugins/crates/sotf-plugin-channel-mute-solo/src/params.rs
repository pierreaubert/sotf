//! Channel Mute/Solo plugin parameter definitions — single source of truth.
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

pub const PARAMS: &[ParamSpec] = &[
    ParamSpec::bool_param("Enabled", "enabled", true, "General").doc("Master enable for mute/solo"),
    ParamSpec::float(
        "Dim Gain",
        "dim_gain_db",
        -20.0,
        -60.0,
        0.0,
        1.0,
        "dB",
        "General",
    )
    .doc("Attenuation level when dimmed"),
    ParamSpec::float(
        "Fade Time",
        "fade_ms",
        5.0,
        0.0,
        100.0,
        1.0,
        "ms",
        "General",
    )
    .doc("Mute/solo crossfade duration"),
];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[ControlSpec::toggle(0)], // enabled
    main: &[ControlGroup::new(
        "primary",
        "",
        &[
            ControlSpec::knob(1), // dim_gain_db
            ControlSpec::knob(2), // fade_ms
        ],
    )],
    output: &[],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[
        ColumnConstraint::config(120.0, 0.5),
        ColumnConstraint::main(200.0),
    ],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// Channel Mute/Solo plugin parameters.
///
/// All serde defaults are derived from PARAMS — adding a field here with
/// the correct default function is enough to support old presets that
/// don't have the new field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    #[serde(default = "d_enabled")]
    pub enabled: bool,
    #[serde(default = "d_dim_gain_db")]
    pub dim_gain_db: f32,
    #[serde(default = "d_fade_ms")]
    pub fade_ms: f32,
}

fn d_enabled() -> bool {
    pk(PARAMS, "enabled").default_f64() != 0.0
}
fn d_dim_gain_db() -> f32 {
    pk(PARAMS, "dim_gain_db").default_f64() as f32
}
fn d_fade_ms() -> f32 {
    pk(PARAMS, "fade_ms").default_f64() as f32
}

impl Default for Params {
    fn default() -> Self {
        Self {
            enabled: d_enabled(),
            dim_gain_db: d_dim_gain_db(),
            fade_ms: d_fade_ms(),
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
    const PLUGIN_TYPE_KEY: &'static str = "channel_mute_solo";

    fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(if self.enabled { 1.0 } else { 0.0 }),
            1 => Some(self.dim_gain_db as f64),
            2 => Some(self.fade_ms as f64),
            _ => None,
        }
    }

    fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.enabled = value != 0.0,
            1 => self.dim_gain_db = value as f32,
            2 => self.fade_ms = value as f32,
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
        assert_eq!(original.enabled, restored.enabled);
        assert_eq!(original.dim_gain_db, restored.dim_gain_db);
        assert_eq!(original.fade_ms, restored.fade_ms);
        assert_eq!(
            original.param_value(1),
            Some(original.dim_gain_db as f64),
            "param_value must expose dim_gain_db via f64 without losing trait compatibility"
        );
        assert_eq!(
            original.param_value(2),
            Some(original.fade_ms as f64),
            "param_value must expose fade_ms via f64 without losing trait compatibility"
        );
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        let p: Params = serde_json::from_str("{}").unwrap();
        assert!(p.enabled);
        assert_eq!(
            p.dim_gain_db,
            pk(PARAMS, "dim_gain_db").default_f64() as f32
        );
        assert_eq!(p.fade_ms, pk(PARAMS, "fade_ms").default_f64() as f32);
    }
}
