// ============================================================================
// Plugin Factory - Creates plugin instances from JSON configs
// ============================================================================

use sotf_plugins::{EqPlugin, EqPluginParams, Plugin};

/// Create a plugin instance from plugin type and JSON config string
pub fn create_plugin(
    plugin_type: &str,
    config_json: &str,
    input_channels: usize,
    output_channels: usize,
) -> Result<Box<dyn Plugin>, String> {
    match plugin_type {
        "EQ" => create_eq_plugin(config_json, input_channels, output_channels),
        _ => Err(format!("Unknown plugin type: {}", plugin_type)),
    }
}

fn create_eq_plugin(
    config_json: &str,
    input_channels: usize,
    output_channels: usize,
) -> Result<Box<dyn Plugin>, String> {
    // EQ must be in-place (same input/output channels)
    if input_channels != output_channels {
        return Err(format!(
            "EQ plugin requires same input/output channels, got {}/{}",
            input_channels, output_channels
        ));
    }

    // Parse EQ parameters
    let params: EqPluginParams = serde_json::from_str(config_json)
        .map_err(|e| format!("Failed to parse EQ parameters: {}", e))?;

    // Create EQ plugin with default sample rate (will be updated in initialize())
    let plugin = EqPlugin::from_params(input_channels, 48000, params)?;

    Ok(Box::new(plugin))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_eq_plugin() {
        let config_json = r#"{
            "filters": [
                {"filter_type": "peak", "freq": 1000.0, "q": 1.0, "db_gain": 3.0}
            ]
        }"#;

        let plugin = create_plugin("EQ", config_json, 2, 2).unwrap();

        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 2);
    }

    #[test]
    fn test_eq_channel_mismatch() {
        let config_json = r#"{"filters": []}"#;

        // EQ requires input_channels == output_channels
        let result = create_plugin("EQ", config_json, 2, 5);
        assert!(result.is_err());
    }
}
