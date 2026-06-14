use super::error::ConfigError;

/// Validate plugin configurations before applying
pub(in crate::engine::manager_thread) fn validate_plugin_configs(
    configs: &[super::super::PluginConfig],
) -> Result<(), ConfigError> {
    log::debug!(
        "[Manager Thread] Starting validation of {} plugins",
        configs.len()
    );

    for (i, config) in configs.iter().enumerate() {
        log::debug!(
            "[Manager Thread] Validating plugin {}: type='{}'",
            i,
            config.plugin_type
        );

        let plugin_type_lower = config.plugin_type.to_lowercase();
        if !sotf_plugins::is_supported_plugin_type(&plugin_type_lower) {
            log::error!(
                "[Manager Thread] Validation failed: Unknown plugin type '{}'",
                config.plugin_type
            );
            return Err(ConfigError::ValidationError {
                plugin_index: i,
                reason: format!("Unknown plugin type '{}'", config.plugin_type),
            });
        }

        // Validate that parameters exist
        if config.parameters.is_null() {
            log::error!(
                "[Manager Thread] Validation failed: Plugin '{}' missing parameters",
                config.plugin_type
            );
            return Err(ConfigError::ValidationError {
                plugin_index: i,
                reason: format!("Plugin '{}' missing parameters", config.plugin_type),
            });
        }

        if let Err(reason) =
            sotf_plugins::validate_plugin_security_config(&plugin_type_lower, &config.parameters)
        {
            log::error!(
                "[Manager Thread] Security validation failed for plugin '{}': {}",
                config.plugin_type,
                reason
            );
            return Err(ConfigError::ValidationError {
                plugin_index: i,
                reason,
            });
        }

        // Type-specific validation (case-insensitive)
        match plugin_type_lower.as_str() {
            "eq" => {
                // Validate EQ filter structure
                if let Some(filters) = config.parameters.get("filters") {
                    if !filters.is_array() {
                        log::error!(
                            "[Manager Thread] EQ validation failed: 'filters' must be an array"
                        );
                        return Err(ConfigError::ValidationError {
                            plugin_index: i,
                            reason: "Invalid 'filters' parameter (must be array)".to_string(),
                        });
                    }
                    log::debug!(
                        "[Manager Thread] EQ validated with {} filters",
                        filters.as_array().unwrap().len()
                    );
                }
            }
            "gain" => {
                // Validate gain_db exists
                if let Some(gain) = config.parameters.get("gain_db") {
                    if !gain.is_number() {
                        log::error!(
                            "[Manager Thread] Gain validation failed: 'gain_db' must be a number"
                        );
                        return Err(ConfigError::ValidationError {
                            plugin_index: i,
                            reason: "'gain_db' must be a number".to_string(),
                        });
                    }
                } else {
                    log::error!("[Manager Thread] Gain validation failed: Missing 'gain_db'");
                    return Err(ConfigError::ValidationError {
                        plugin_index: i,
                        reason: "Missing 'gain_db' parameter".to_string(),
                    });
                }
            }
            "upmixer" => {
                // Validate upmixer mode
                if let Some(mode) = config.parameters.get("mode")
                    && !mode.is_string()
                {
                    log::error!(
                        "[Manager Thread] Upmixer validation failed: 'mode' must be a string"
                    );
                    return Err(ConfigError::ValidationError {
                        plugin_index: i,
                        reason: "Invalid 'mode' parameter (must be string)".to_string(),
                    });
                }
            }
            _ => {
                // Basic validation for other types
            }
        }
    }

    log::debug!(
        "[Manager Thread] All {} plugins validated successfully",
        configs.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_types::PluginConfig;

    fn config_with_params(plugin_type: &str, params: serde_json::Value) -> PluginConfig {
        PluginConfig {
            plugin_type: plugin_type.to_string(),
            parameters: params,
        }
    }

    #[test]
    fn empty_config_validates() {
        assert!(validate_plugin_configs(&[]).is_ok());
    }

    #[test]
    fn unknown_plugin_type_fails() {
        let configs = vec![config_with_params(
            "not_a_real_plugin",
            serde_json::json!({"gain_db": 0.0}),
        )];
        let err = validate_plugin_configs(&configs).unwrap_err();
        assert!(format!("{err}").contains("Unknown plugin type"));
    }

    #[test]
    fn null_parameters_fails() {
        let configs = vec![PluginConfig {
            plugin_type: "gain".to_string(),
            parameters: serde_json::Value::Null,
        }];
        let err = validate_plugin_configs(&configs).unwrap_err();
        assert!(format!("{err}").contains("missing parameters"));
    }

    #[test]
    fn gain_without_gain_db_fails() {
        let configs = vec![config_with_params("gain", serde_json::json!({}))];
        let err = validate_plugin_configs(&configs).unwrap_err();
        assert!(format!("{err}").contains("gain_db"));
    }

    #[test]
    fn gain_with_invalid_gain_db_type_fails() {
        let configs = vec![config_with_params(
            "gain",
            serde_json::json!({"gain_db": "loud"}),
        )];
        let err = validate_plugin_configs(&configs).unwrap_err();
        assert!(format!("{err}").contains("gain_db"));
    }

    #[test]
    fn valid_gain_config_passes() {
        let configs = vec![config_with_params(
            "gain",
            serde_json::json!({"gain_db": -3.0}),
        )];
        assert!(validate_plugin_configs(&configs).is_ok());
    }

    #[test]
    fn eq_filters_must_be_array() {
        let configs = vec![config_with_params(
            "eq",
            serde_json::json!({"filters": "not an array"}),
        )];
        let err = validate_plugin_configs(&configs).unwrap_err();
        assert!(format!("{err}").contains("filters"));
    }

    #[test]
    fn valid_eq_config_passes() {
        let configs = vec![config_with_params(
            "eq",
            serde_json::json!({
                "filters": [
                    {"filter_type": "peak", "frequency": 1000.0, "q": 1.0, "gain_db": 2.0}
                ]
            }),
        )];
        assert!(validate_plugin_configs(&configs).is_ok());
    }

    #[test]
    fn upmixer_mode_must_be_string() {
        let configs = vec![config_with_params(
            "upmixer",
            serde_json::json!({"mode": 42}),
        )];
        let err = validate_plugin_configs(&configs).unwrap_err();
        assert!(format!("{err}").contains("mode"));
    }

    #[test]
    fn valid_upmixer_config_passes() {
        let configs = vec![config_with_params(
            "upmixer",
            serde_json::json!({"mode": "5_1"}),
        )];
        assert!(validate_plugin_configs(&configs).is_ok());
    }

    #[test]
    fn multiple_valid_plugins_pass() {
        let configs = vec![
            config_with_params("gain", serde_json::json!({"gain_db": 0.0})),
            config_with_params("eq", serde_json::json!({"filters": []})),
        ];
        assert!(validate_plugin_configs(&configs).is_ok());
    }

    #[test]
    fn first_error_is_reported() {
        let configs = vec![
            config_with_params("gain", serde_json::json!({})),
            config_with_params("eq", serde_json::json!({"filters": "bad"})),
        ];
        let err = validate_plugin_configs(&configs).unwrap_err();
        assert!(format!("{err}").contains("Plugin 0 validation failed"));
    }
}

pub(super) fn validate_gapless_source_compatible(
    source: &crate::decoder::AudioSource,
    expected_channels: usize,
) -> Result<(), String> {
    if !matches!(source, crate::decoder::AudioSource::File(_)) {
        log::debug!(
            "[Manager Thread] Skipping queued source channel validation for non-file source: {}",
            source.display_name()
        );
        return Ok(());
    }

    let decoder = crate::decoder::create_decoder_from_source(source).map_err(|e| {
        format!(
            "Failed to inspect queued source '{}': {:?}",
            source.display_name(),
            e
        )
    })?;
    let channels = decoder.spec().channels as usize;

    if channels != expected_channels {
        return Err(format!(
            "Queued source channel mismatch for '{}': expected {} channels, got {}",
            source.display_name(),
            expected_channels,
            channels
        ));
    }

    Ok(())
}
