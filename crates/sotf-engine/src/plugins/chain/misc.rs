use super::super::matrix::upmixer_output_channels;

pub(super) fn default_plugin_preset_version() -> u32 {
    2
}

pub(super) fn upmixer_settings_output_channels(
    speaker_config: &str,
    binaural_preview: bool,
) -> usize {
    if binaural_preview {
        2
    } else {
        upmixer_output_channels(speaker_config)
    }
}

/// Extract a human-readable plugin type name from a raw JSON plugin value.
pub(super) fn plugin_type_from_raw(raw: &serde_json::Value) -> String {
    // PluginSettings is an externally tagged enum, so the settings field is
    // either a string like "LoudnessMonitor" or an object like {"Gain": {...}}
    if let Some(settings) = raw.get("settings") {
        if let Some(s) = settings.as_str() {
            return s.to_string();
        }
        if let Some(obj) = settings.as_object()
            && let Some(key) = obj.keys().next()
        {
            return key.clone();
        }
    }
    "unknown".to_string()
}
