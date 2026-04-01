//! State serialization for plugin presets (save/load).
//!
//! Converts a plugin's current parameter state to/from a JSON blob,
//! suitable for AU `fullState` and VST3 state chunks.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::Plugin;

/// Save all plugin parameters to a JSON byte vector.
///
/// Returns a JSON object mapping parameter ID → value.
pub fn save_state(plugin: &dyn Plugin) -> Vec<u8> {
    let params = plugin.parameters();
    let mut map = serde_json::Map::new();

    for param in &params {
        if let Some(value) = plugin.get_parameter(&param.id) {
            let json_val = match value {
                ParameterValue::Float(f) => serde_json::Value::from(f),
                ParameterValue::Int(i) => serde_json::Value::from(i),
                ParameterValue::Bool(b) => serde_json::Value::from(b),
                ParameterValue::String(s) => serde_json::Value::from(s),
            };
            map.insert(param.id.0.clone(), json_val);
        }
    }

    serde_json::to_vec(&serde_json::Value::Object(map)).unwrap_or_default()
}

/// Load plugin parameters from a JSON byte slice.
///
/// The data should be a JSON object mapping parameter ID → value,
/// as produced by `save_state`.
pub fn load_state(plugin: &mut dyn Plugin, data: &[u8]) -> Result<(), String> {
    let map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(data).map_err(|e| format!("Failed to parse state: {e}"))?;

    for (key, json_val) in &map {
        let value = match json_val {
            serde_json::Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    // Check if this is an integer parameter
                    if f == f.floor() && f.abs() < i32::MAX as f64 {
                        // Try float first — the plugin's set_parameter will accept it
                        ParameterValue::Float(f as f32)
                    } else {
                        ParameterValue::Float(f as f32)
                    }
                } else {
                    continue;
                }
            }
            serde_json::Value::Bool(b) => ParameterValue::Bool(*b),
            serde_json::Value::String(s) => ParameterValue::String(s.clone()),
            _ => continue,
        };

        let id = ParameterId(key.clone());
        // Ignore errors for unknown parameters (forward compatibility)
        let _ = plugin.set_parameter(id, value);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factory::create_plugin;

    #[test]
    fn test_save_load_roundtrip() {
        let mut plugin = create_plugin("Gain", 2, 48000, "{}").unwrap();
        plugin.initialize(48000).unwrap();

        // Set a parameter
        let id = ParameterId("gain_db".to_string());
        plugin
            .set_parameter(id.clone(), ParameterValue::Float(3.0))
            .unwrap();

        // Save state
        let state = save_state(&*plugin);
        assert!(!state.is_empty());

        // Create a fresh plugin and load state
        let mut plugin2 = create_plugin("Gain", 2, 48000, "{}").unwrap();
        plugin2.initialize(48000).unwrap();
        load_state(&mut *plugin2, &state).unwrap();

        // Verify parameter was restored
        let value = plugin2.get_parameter(&id);
        match value {
            Some(ParameterValue::Float(f)) => {
                assert!((f - 3.0).abs() < 0.01, "Expected gain_db=3.0, got {f}");
            }
            other => panic!("Expected Float(3.0), got {other:?}"),
        }
    }

    #[test]
    fn test_load_invalid_json() {
        let mut plugin = create_plugin("Gain", 2, 48000, "{}").unwrap();
        let result = load_state(&mut *plugin, b"not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_unknown_params_ignored() {
        let mut plugin = create_plugin("Gain", 2, 48000, "{}").unwrap();
        let data = br#"{"unknown_param": 42.0, "gain_db": 5.0}"#;
        let result = load_state(&mut *plugin, data);
        assert!(result.is_ok());
    }
}
