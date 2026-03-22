//! Utility functions for plugin configuration and matrix operations

use serde::Deserialize;
use serde_json::json;

/// Get channel label by index and total channel count
pub fn get_channel_label(index: usize, total: usize) -> String {
    if let Some(groups) = sotf_plugins::get_meter_groups_by_channels(total) {
        for group in groups {
            for ch in group.channels {
                if ch.index == index {
                    return ch.label.to_string();
                }
            }
        }
    }
    format!("Ch{}", index)
}

/// Get channel label using a speaker config ID for disambiguation.
/// Falls back to channel-count lookup via `get_channel_label()`.
pub fn get_channel_label_from_config(
    index: usize,
    total: usize,
    speaker_config: Option<&str>,
) -> String {
    if let Some(groups) = speaker_config.and_then(sotf_plugins::get_meter_groups) {
        for group in groups {
            for ch in group.channels {
                if ch.index == index {
                    return ch.label.to_string();
                }
            }
        }
    }
    get_channel_label(index, total)
}

/// Convert linear gain to dB string for display
/// Returns "-∞" for gains below threshold (effectively silent)
pub fn linear_to_db_string(linear: f32) -> String {
    const SILENCE_THRESHOLD: f32 = 0.001; // -60 dB

    if linear < SILENCE_THRESHOLD {
        "-∞".to_string()
    } else {
        format!("{:.1}", 20.0 * linear.log10())
    }
}

/// Convert dB value to linear gain
pub fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// Convert plugin entries from a preset file into a PathConfig JSON string
/// suitable for the AB Compare plugin's path_a_config / path_b_config fields.
pub fn plugins_to_path_config_json(plugins: &[super::Plugin], sample_rate: f64) -> String {
    let configs: Vec<serde_json::Value> = plugins
        .iter()
        .filter(|p| p.enabled)
        .map(|p| {
            let pc = p.settings.to_plugin_config(sample_rate);
            json!({"plugin_type": pc.plugin_type, "parameters": pc.parameters})
        })
        .collect();
    let path_config = match configs.len() {
        0 => json!({"type": "None"}),
        1 => {
            json!({
                "type": "Plugin",
                "plugin_type": configs[0]["plugin_type"],
                "parameters": configs[0]["parameters"],
            })
        }
        _ => json!({"type": "Rack", "plugins": configs}),
    };
    serde_json::to_string(&path_config).unwrap()
}

/// Parse a preset JSON file into a PathConfig JSON string for use in AB Compare.
/// The file is expected to be a `PluginPreset` (with version + plugins array).
pub fn preset_file_to_path_config_json(
    json_content: &str,
    sample_rate: f64,
) -> Result<String, String> {
    #[derive(Deserialize)]
    struct PluginPreset {
        #[serde(default)]
        #[allow(dead_code)]
        version: u32,
        plugins: Vec<super::Plugin>,
    }

    let preset: PluginPreset =
        serde_json::from_str(json_content).map_err(|e| format!("Invalid preset file: {}", e))?;
    Ok(plugins_to_path_config_json(&preset.plugins, sample_rate))
}
