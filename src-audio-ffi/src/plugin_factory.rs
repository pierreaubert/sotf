// ============================================================================
// Plugin Factory - Creates plugin instances from JSON configs
// ============================================================================

use serde_json::Value;
use sotf_audio::plugins::{Plugin, PluginConfig};
use sotf_audio::plugins::plugin_eq::{EqPlugin, EqPluginParams};

/// Create a PluginConfig from plugin type and JSON config string
pub fn create_plugin_config(plugin_type: &str, config_json: &str) -> Result<PluginConfig, String> {
    // Parse JSON
    let params: Value = serde_json::from_str(config_json)
        .map_err(|e| format!("JSON parse error: {}", e))?;

    // Build PluginConfig
    Ok(PluginConfig {
        plugin_type: plugin_type.to_string(),
        parameters: params,
    })
}

/// Create a plugin instance from PluginConfig
pub fn create_plugin(
    config: &PluginConfig,
    input_channels: usize,
    output_channels: usize,
) -> Result<Box<dyn Plugin>, String> {
    match config.plugin_type.as_str() {
        "EQ" => create_eq_plugin(config, input_channels, output_channels),
        _ => Err(format!("Unknown plugin type: {}", config.plugin_type)),
    }
}

fn create_eq_plugin(
    config: &PluginConfig,
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
    let params: EqPluginParams = serde_json::from_value(config.parameters.clone())
        .map_err(|e| format!("Failed to parse EQ parameters: {}", e))?;

    // Create EQ plugin with default sample rate (will be updated in initialize())
    let plugin = EqPlugin::from_params(input_channels, 48000, params)?;

    Ok(Box::new(plugin))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_eq_config() {
        let config_json = r#"{
            "filters": [
                {"filter_type": "peak", "freq": 1000.0, "q": 1.0, "db_gain": 3.0}
            ]
        }"#;

        let config = create_plugin_config("EQ", config_json).unwrap();
        assert_eq!(config.plugin_type, "EQ");
    }

    #[test]
    fn test_create_eq_plugin() {
        let config_json = r#"{
            "filters": [
                {"filter_type": "peak", "freq": 1000.0, "q": 1.0, "db_gain": 3.0}
            ]
        }"#;

        let config = create_plugin_config("EQ", config_json).unwrap();
        let plugin = create_plugin(&config, 2, 2).unwrap();

        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 2);
    }

    #[test]
    fn test_eq_channel_mismatch() {
        let config_json = r#"{"filters": []}"#;
        let config = create_plugin_config("EQ", config_json).unwrap();

        // EQ requires input_channels == output_channels
        let result = create_plugin(&config, 2, 5);
        assert!(result.is_err());
    }
}
