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
/// This plugin has no runtime-settable parameters. Channel count is persisted
/// here so presets can reconstruct the sink layout, but remains construction-only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Params {
    /// Construction-only input layout. This is serialized with presets but is
    /// deliberately absent from `PARAMS`, so hosts cannot automate it.
    #[serde(default = "default_channels", alias = "output_channels")]
    pub channels: usize,
}

const fn default_channels() -> usize {
    2
}

impl Default for Params {
    fn default() -> Self {
        Self {
            channels: default_channels(),
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
        let original = Params { channels: 6 };
        let json = serde_json::to_value(&original).unwrap();
        let restored: Params = serde_json::from_value(json).unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn deserialize_empty_json_uses_defaults() {
        // Old presets with unknown fields (e.g. `output_channels`) must not fail.
        let p: Params = serde_json::from_str("{}").unwrap();
        assert_eq!(p.channels, 2);
    }

    #[test]
    fn deserialize_old_preset_migrates_output_channels() {
        let params: Params = serde_json::from_str(r#"{"output_channels": 4}"#).unwrap();
        assert_eq!(params.channels, 4);
        assert_eq!(
            serde_json::to_value(params).unwrap(),
            serde_json::json!({"channels": 4})
        );
    }
}
