// ============================================================================
// Parameter Mapping System
// ============================================================================
//
// Maps plugin-specific parameters to a generic parameter system for AU hosts.
// For EQ plugins, this creates parameters for each filter band (frequency, Q, gain).

use std::ffi::CString;
use std::os::raw::c_char;
use sotf_audio::plugins::{Plugin, ParameterId, ParameterValue};

/// Parameter information exposed to AU host
#[repr(C)]
#[derive(Debug, Clone)]
pub struct ParameterInfo {
    /// Unique parameter ID (e.g., "band0_freq")
    pub id: *const c_char,
    /// Human-readable name (e.g., "Band 1 Frequency")
    pub name: *const c_char,
    /// Unit string (e.g., "Hz", "dB", "")
    pub unit: *const c_char,
    /// Minimum value
    pub min_value: f64,
    /// Maximum value
    pub max_value: f64,
    /// Default value
    pub default_value: f64,
    /// Number of steps (0 = continuous)
    pub steps: u32,
}

/// Parameter mapping for a plugin
pub struct ParameterMap {
    /// Parameter metadata
    parameters: Vec<ParameterMetadata>,
}

struct ParameterMetadata {
    id: String,
    name: String,
    unit: String,
    min_value: f64,
    max_value: f64,
    default_value: f64,
    steps: u32,
    /// Conversion to plugin-specific parameter
    to_plugin: fn(&str, f64) -> (ParameterId, ParameterValue),
    /// Conversion from plugin-specific parameter
    from_plugin: fn(&dyn Plugin, &str) -> Option<f64>,
}

impl ParameterMap {
    /// Create parameter map from plugin
    pub fn from_plugin(plugin: &dyn Plugin, plugin_type: &str) -> Self {
        match plugin_type {
            "EQ" => Self::from_eq_plugin(plugin),
            _ => Self::empty(),
        }
    }

    /// Create an empty parameter map
    pub fn empty() -> Self {
        Self {
            parameters: Vec::new(),
        }
    }

    /// Create parameter map for EQ plugin
    fn from_eq_plugin(plugin: &dyn Plugin) -> Self {
        // For now, create a fixed set of parameters for a 10-band EQ
        // In the future, this could be dynamic based on the actual number of filters
        let mut parameters = Vec::new();

        // Create parameters for up to 10 bands
        for band in 0..10 {
            // Frequency parameter
            parameters.push(ParameterMetadata {
                id: format!("band{}_freq", band),
                name: format!("Band {} Frequency", band + 1),
                unit: "Hz".to_string(),
                min_value: 20.0,
                max_value: 20000.0,
                default_value: 1000.0,
                steps: 0, // Continuous
                to_plugin: move |_id, value| {
                    (
                        ParameterId::String(format!("band{}_freq", band)),
                        ParameterValue::Float(value),
                    )
                },
                from_plugin: move |p, _id| {
                    p.get_parameter(&ParameterId::String(format!("band{}_freq", band)))
                        .and_then(|v| match v {
                            ParameterValue::Float(f) => Some(f),
                            _ => None,
                        })
                },
            });

            // Q parameter
            parameters.push(ParameterMetadata {
                id: format!("band{}_q", band),
                name: format!("Band {} Q", band + 1),
                unit: "".to_string(),
                min_value: 0.1,
                max_value: 10.0,
                default_value: 1.0,
                steps: 0,
                to_plugin: move |_id, value| {
                    (
                        ParameterId::String(format!("band{}_q", band)),
                        ParameterValue::Float(value),
                    )
                },
                from_plugin: move |p, _id| {
                    p.get_parameter(&ParameterId::String(format!("band{}_q", band)))
                        .and_then(|v| match v {
                            ParameterValue::Float(f) => Some(f),
                            _ => None,
                        })
                },
            });

            // Gain parameter
            parameters.push(ParameterMetadata {
                id: format!("band{}_gain", band),
                name: format!("Band {} Gain", band + 1),
                unit: "dB".to_string(),
                min_value: -12.0,
                max_value: 12.0,
                default_value: 0.0,
                steps: 0,
                to_plugin: move |_id, value| {
                    (
                        ParameterId::String(format!("band{}_gain", band)),
                        ParameterValue::Float(value),
                    )
                },
                from_plugin: move |p, _id| {
                    p.get_parameter(&ParameterId::String(format!("band{}_gain", band)))
                        .and_then(|v| match v {
                            ParameterValue::Float(f) => Some(f),
                            _ => None,
                        })
                },
            });
        }

        Self { parameters }
    }

    /// Get the number of parameters
    pub fn count(&self) -> usize {
        self.parameters.len()
    }

    /// Get parameter info by index
    pub fn get_info(&self, index: usize) -> Option<ParameterInfo> {
        self.parameters.get(index).map(|param| {
            // Note: These CStrings are leaked intentionally for FFI safety
            // They will be cleaned up when the plugin is destroyed
            let id = CString::new(param.id.clone()).unwrap().into_raw();
            let name = CString::new(param.name.clone()).unwrap().into_raw();
            let unit = CString::new(param.unit.clone()).unwrap().into_raw();

            ParameterInfo {
                id,
                name,
                unit,
                min_value: param.min_value,
                max_value: param.max_value,
                default_value: param.default_value,
                steps: param.steps,
            }
        })
    }

    /// Find parameter by ID
    fn find_param(&self, param_id: &str) -> Option<&ParameterMetadata> {
        self.parameters.iter().find(|p| p.id == param_id)
    }

    /// Set parameter value (normalized 0.0-1.0)
    pub fn set_normalized(
        &self,
        plugin: &mut dyn Plugin,
        param_id: &str,
        normalized_value: f64,
    ) -> Result<(), String> {
        let param = self
            .find_param(param_id)
            .ok_or_else(|| format!("Unknown parameter: {}", param_id))?;

        // Denormalize value
        let value = param.min_value + normalized_value * (param.max_value - param.min_value);

        // Convert to plugin parameter
        let (plugin_param_id, plugin_param_value) = (param.to_plugin)(param_id, value);

        // Set on plugin
        plugin.set_parameter(plugin_param_id, plugin_param_value)
    }

    /// Get parameter value (normalized 0.0-1.0)
    pub fn get_normalized(&self, plugin: &dyn Plugin, param_id: &str) -> Option<f64> {
        let param = self.find_param(param_id)?;

        // Get from plugin
        let value = (param.from_plugin)(plugin, param_id)?;

        // Normalize
        let normalized = (value - param.min_value) / (param.max_value - param.min_value);
        Some(normalized.clamp(0.0, 1.0))
    }

    /// Set parameter value (raw, not normalized)
    pub fn set_raw(
        &self,
        plugin: &mut dyn Plugin,
        param_id: &str,
        value: f64,
    ) -> Result<(), String> {
        let param = self
            .find_param(param_id)
            .ok_or_else(|| format!("Unknown parameter: {}", param_id))?;

        // Clamp to range
        let clamped_value = value.clamp(param.min_value, param.max_value);

        // Convert to plugin parameter
        let (plugin_param_id, plugin_param_value) = (param.to_plugin)(param_id, clamped_value);

        // Set on plugin
        plugin.set_parameter(plugin_param_id, plugin_param_value)
    }

    /// Get parameter value (raw, not normalized)
    pub fn get_raw(&self, plugin: &dyn Plugin, param_id: &str) -> Option<f64> {
        let param = self.find_param(param_id)?;
        (param.from_plugin)(plugin, param_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_map_creation() {
        // Create a mock EQ plugin
        use sotf_audio::plugins::plugin_eq::EqPlugin;
        let plugin = EqPlugin::new(2, vec![]);

        let param_map = ParameterMap::from_eq_plugin(&plugin);

        // Should have 30 parameters (10 bands * 3 params each)
        assert_eq!(param_map.count(), 30);

        // Check first parameter
        let info = param_map.get_info(0).unwrap();
        assert!(!info.id.is_null());
        assert!(!info.name.is_null());
    }

    #[test]
    fn test_normalize_denormalize() {
        // Frequency range: 20-20000 Hz
        // Normalized 0.0 -> 20 Hz
        // Normalized 1.0 -> 20000 Hz
        // Normalized 0.5 -> 10010 Hz

        let min = 20.0;
        let max = 20000.0;

        let denormalized = min + 0.5 * (max - min);
        assert_eq!(denormalized, 10010.0);

        let normalized = (10010.0 - min) / (max - min);
        assert_eq!(normalized, 0.5);
    }
}
