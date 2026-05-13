//! HAL Output plugin parameter definitions — single source of truth.
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
use sotf_host::param_specs::ParamSpec;
use sotf_host::plugin_layout::*;
use sotf_host::plugin_params::PluginParamDef;

// ============================================================================
// Parameter Specifications
// ============================================================================

// Channel count is fixed at plugin construction time and is not a runtime-settable
// parameter — the plugin sink has no adjustable parameters exposed to the host.
pub const PARAMS: &[ParamSpec] = &[];

// ============================================================================
// UI Layout
// ============================================================================

pub const LAYOUT: PluginLayout = PluginLayout {
    config: &[],
    main: &[],
    output: &[],
    tabs: &[],
    visualizations: &[],
    column_constraints: &[],
    dynamic_sections: &[],
};

// ============================================================================
// Serializable Parameter State
// ============================================================================

/// HAL Output plugin parameters.
///
/// This plugin has no runtime-settable parameters — the channel count is
/// fixed at construction time. The struct exists to satisfy the
/// `PluginParamDef` interface required for UI registration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Params {}

// ============================================================================
// PluginParamDef implementation
// ============================================================================

impl PluginParamDef for Params {
    const PARAMS: &'static [ParamSpec] = PARAMS;
    const LAYOUT: Option<&'static PluginLayout> = Some(&LAYOUT);
    const VERSION: u32 = 1;
    const PLUGIN_TYPE_KEY: &'static str = "hal_output";

    fn param_value(&self, _index: usize) -> Option<f64> {
        None
    }

    fn set_param_value(&mut self, _index: usize, _value: f64) {}
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_is_empty() {
        // No runtime-settable parameters — PARAMS must remain empty.
        assert_eq!(PARAMS.len(), 0, "PARAMS should have no entries");
    }

    #[test]
    fn param_value_always_none() {
        // Querying any index must return None (no parameters defined).
        let p = Params::default();
        assert!(p.param_value(0).is_none());
        assert!(p.param_value(usize::MAX).is_none());
    }

    #[test]
    fn roundtrip_serde() {
        // Empty struct serialises to `{}` and deserialises back without error.
        let original = Params::default();
        let json = serde_json::to_value(&original).unwrap();
        let _restored: Params = serde_json::from_value(json).unwrap();
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        // Old presets with unknown fields (e.g. `output_channels`) must not fail.
        let _p: Params = serde_json::from_str("{}").unwrap();
    }

    #[test]
    fn deserialize_old_preset_with_output_channels_ignores_unknown_field() {
        // Old presets that serialised `output_channels` must be accepted silently
        // via serde's default deny_unknown_fields absence.
        let result: Result<Params, _> = serde_json::from_str(r#"{"output_channels": 4}"#);
        // serde by default ignores unknown fields, so this should succeed.
        assert!(
            result.is_ok(),
            "Old preset with output_channels should deserialise without error"
        );
    }
}
