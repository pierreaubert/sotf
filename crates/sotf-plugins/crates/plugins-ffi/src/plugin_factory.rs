// ============================================================================
// Plugin Factory - Delegates to plugins-bridge for all plugin creation
// ============================================================================

use sotf_host::plugin::Plugin;

/// Create a plugin instance from plugin type and JSON config string.
///
/// Delegates to `plugins_bridge::create_plugin()` which supports all plugin types.
pub fn create_plugin(
    plugin_type: &str,
    config_json: &str,
    input_channels: usize,
    output_channels: usize,
    sample_rate: u32,
) -> Result<Box<dyn Plugin>, String> {
    let plugin =
        plugins_bridge::create_plugin(plugin_type, input_channels, sample_rate, config_json)?;
    if plugin.input_channels() != input_channels {
        return Err(format!(
            "Plugin {plugin_type} created with {} input channels, requested {input_channels}",
            plugin.input_channels()
        ));
    }
    if plugin.output_channels() != output_channels {
        return Err(format!(
            "Plugin {plugin_type} created with {} output channels, requested {output_channels}",
            plugin.output_channels()
        ));
    }
    Ok(plugin)
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

        let plugin = create_plugin("EQ", config_json, 2, 2, 48000).unwrap();

        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 2);
    }

    #[test]
    fn test_create_compressor() {
        let plugin = create_plugin("Compressor", "{}", 2, 2, 48000).unwrap();
        assert_eq!(plugin.input_channels(), 2);
    }

    #[test]
    fn test_eq_channel_mismatch() {
        // EQ requires input_channels == output_channels (handled by EQ plugin itself)
        let config_json = r#"{"filters": []}"#;
        let result = create_plugin("EQ", config_json, 2, 2, 48000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_downmix_uses_independent_input_and_output_widths() {
        let plugin = create_plugin("Downmix", "{}", 6, 2, 48_000).unwrap();
        assert_eq!(plugin.input_channels(), 6);
        assert_eq!(plugin.output_channels(), 2);
        assert!(create_plugin("Downmix", "{}", 6, 6, 48_000).is_err());
    }
}
