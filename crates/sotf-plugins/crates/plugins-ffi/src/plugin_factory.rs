// ============================================================================
// Plugin Factory - Delegates to plugins-bridge for all plugin creation
// ============================================================================

use sotf_host::plugin::Plugin;

const LINEAR_PHASE_EQ_DEFAULT_FILTERS: usize = 5;
const LINEAR_PHASE_EQ_MAX_FILTERS: usize = 10;

fn state_integer(
    state: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<Option<usize>, String> {
    state
        .get(key)
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| {
                    format!("LinearPhaseEQ state '{key}' must be a non-negative integer")
                })
        })
        .transpose()
}

fn state_number(
    state: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<Option<f64>, String> {
    state
        .get(key)
        .map(|value| {
            value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| format!("LinearPhaseEQ state '{key}' must be a finite number"))
        })
        .transpose()
}

fn state_bool(
    state: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<Option<bool>, String> {
    state
        .get(key)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| format!("LinearPhaseEQ state '{key}' must be a boolean"))
        })
        .transpose()
}

fn default_linear_phase_eq_band() -> serde_json::Value {
    serde_json::json!({
        "filter_type": "Peak",
        "frequency": 1000.0,
        "q": 1.0,
        "gain_db": 0.0,
        "active": true,
    })
}

/// Merge the flat parameter state used by plugin presets into the structured
/// constructor configuration required by LinearPhaseEQ.
pub(crate) fn merge_linear_phase_eq_state_into_config(
    config_json: &str,
    state_bytes: &[u8],
) -> Result<String, String> {
    let mut config: serde_json::Value =
        if config_json.trim().is_empty() || matches!(config_json.trim(), "null" | "{}") {
            serde_json::json!({})
        } else {
            serde_json::from_str(config_json)
                .map_err(|error| format!("Failed to parse saved LinearPhaseEQ config: {error}"))?
        };
    let config = config
        .as_object_mut()
        .ok_or_else(|| "LinearPhaseEQ constructor config must be a JSON object".to_string())?;
    let state: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(state_bytes)
        .map_err(|error| format!("Failed to parse LinearPhaseEQ state: {error}"))?;

    if let Some(index) = state_integer(&state, "fir_length")? {
        if index > 3 {
            return Err(format!(
                "LinearPhaseEQ fir_length index must be within [0, 3], got {index}"
            ));
        }
        config.insert("fir_length_index".to_string(), index.into());
    }
    if let Some(index) = state_integer(&state, "phase_mode")? {
        if index > 1 {
            return Err(format!(
                "LinearPhaseEQ phase_mode index must be within [0, 1], got {index}"
            ));
        }
        config.remove("phase_mode");
        config.insert("phase_mode_index".to_string(), index.into());
    }
    if let Some(auto_gain) = state_bool(&state, "auto_gain")? {
        config.insert("auto_gain".to_string(), auto_gain.into());
    }
    if let Some(mix) = state_number(&state, "mix")? {
        if !(0.0..=1.0).contains(&mix) {
            return Err(format!(
                "LinearPhaseEQ mix must be within [0, 1], got {mix}"
            ));
        }
        config.insert("mix".to_string(), mix.into());
    }

    let num_filters = state_integer(&state, "num_filters")?
        .or_else(|| {
            config
                .get("num_filters")
                .and_then(serde_json::Value::as_u64)
                .map(|v| v as usize)
        })
        .unwrap_or(LINEAR_PHASE_EQ_DEFAULT_FILTERS);
    if !(1..=LINEAR_PHASE_EQ_MAX_FILTERS).contains(&num_filters) {
        return Err(format!(
            "LinearPhaseEQ num_filters must be within [1, {LINEAR_PHASE_EQ_MAX_FILTERS}], got {num_filters}"
        ));
    }
    config.insert("num_filters".to_string(), num_filters.into());

    let mut filters = match config.remove("filters") {
        Some(serde_json::Value::Array(filters)) => filters,
        Some(_) => return Err("LinearPhaseEQ config 'filters' must be an array".to_string()),
        None => Vec::new(),
    };
    filters.resize_with(num_filters, default_linear_phase_eq_band);
    filters.truncate(num_filters);

    const FILTER_TYPES: [&str; 5] = ["Peak", "Lowshelf", "Highshelf", "Lowpass", "Highpass"];
    for (key, value) in &state {
        let Some(rest) = key.strip_prefix("band_") else {
            continue;
        };
        let Some((index, field)) = rest.split_once('_') else {
            continue;
        };
        let Ok(index) = index.parse::<usize>() else {
            continue;
        };
        if !matches!(field, "type" | "freq" | "q" | "gain" | "active") {
            continue;
        }
        if index >= num_filters {
            return Err(format!(
                "LinearPhaseEQ state '{key}' targets band {index}, but num_filters is {num_filters}"
            ));
        }
        let band = filters[index]
            .as_object_mut()
            .ok_or_else(|| format!("LinearPhaseEQ config filter {index} must be an object"))?;
        match field {
            "type" => {
                let type_index = value
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                    .filter(|index| *index < FILTER_TYPES.len())
                    .ok_or_else(|| {
                        format!("LinearPhaseEQ state '{key}' must be an integer within [0, 4]")
                    })?;
                band.insert("filter_type".to_string(), FILTER_TYPES[type_index].into());
            }
            "active" => {
                let active = value
                    .as_bool()
                    .ok_or_else(|| format!("LinearPhaseEQ state '{key}' must be a boolean"))?;
                band.insert("active".to_string(), active.into());
            }
            numeric_field => {
                let number = value
                    .as_f64()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| {
                        format!("LinearPhaseEQ state '{key}' must be a finite number")
                    })?;
                let config_field = match numeric_field {
                    "freq" => "frequency",
                    "gain" => "gain_db",
                    "q" => "q",
                    _ => unreachable!(),
                };
                band.insert(config_field.to_string(), number.into());
            }
        }
    }
    config.insert("filters".to_string(), filters.into());

    serde_json::to_string(&config)
        .map_err(|error| format!("Failed to serialize rebuilt LinearPhaseEQ config: {error}"))
}

fn normalized_type_is(plugin_type: &str, expected: &str) -> bool {
    plugin_type
        .bytes()
        .filter(u8::is_ascii_alphanumeric)
        .map(|byte| byte.to_ascii_lowercase())
        .eq(expected.bytes())
}

pub(crate) fn canonical_direct_plugin_type(plugin_type: &str) -> &str {
    if normalized_type_is(plugin_type, "linearphaseeq") {
        "LinearPhaseEQ"
    } else if normalized_type_is(plugin_type, "resampler") {
        "Resampler"
    } else {
        plugin_type
    }
}

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
    let plugin_type = canonical_direct_plugin_type(plugin_type);
    if plugin_type == "Resampler" {
        return Err(
            "Resampler is unavailable through the fixed-rate plugin FFI: its input and output frame counts differ; use the engine/catalog variable-rate path instead"
                .to_string(),
        );
    }

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

    #[test]
    fn fixed_rate_factory_rejects_resampler_without_removing_it_from_catalog() {
        let config = r#"{"input_sample_rate":48000,"output_sample_rate":44100}"#;
        let error = create_plugin("Resampler", config, 2, 2, 48_000)
            .err()
            .expect("fixed-rate facade must reject variable-rate processing");
        assert!(error.contains("fixed-rate plugin FFI"));
        assert!(
            sotf_plugins::supported_plugin_types().any(|plugin_type| plugin_type == "resampler")
        );
        assert!(create_plugin("re-sampler", config, 2, 2, 48_000).is_err());
    }

    #[test]
    fn direct_format_type_normalization_is_alias_stable() {
        assert_eq!(
            canonical_direct_plugin_type("linear_phase_eq"),
            "LinearPhaseEQ"
        );
        assert_eq!(
            canonical_direct_plugin_type("Linear-Phase-EQ"),
            "LinearPhaseEQ"
        );
        assert_eq!(canonical_direct_plugin_type("re_sampler"), "Resampler");
        assert_eq!(canonical_direct_plugin_type("Gain"), "Gain");
    }

    #[test]
    fn linear_phase_eq_rebuilt_fir_length_updates_latency() {
        let short_config = r#"{"num_filters":1,"fir_length_index":0,"filters":[]}"#;
        let long_state = br#"{
            "num_filters":1,
            "fir_length":3,
            "phase_mode":0,
            "auto_gain":false,
            "mix":1.0,
            "band_0_type":0,
            "band_0_freq":1000.0,
            "band_0_q":1.0,
            "band_0_gain":0.0,
            "band_0_active":true
        }"#;
        let long_config =
            merge_linear_phase_eq_state_into_config(short_config, long_state).unwrap();
        let mut short = create_plugin("LinearPhaseEQ", short_config, 2, 2, 48_000).unwrap();
        let mut long = create_plugin("LinearPhaseEQ", &long_config, 2, 2, 48_000).unwrap();
        short.initialize(48_000).unwrap();
        long.initialize(48_000).unwrap();
        assert!(long.latency_samples() > short.latency_samples());
    }
}
