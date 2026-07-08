use super::misc::default_app_config_version;
use serde::{Deserialize, Serialize};

/// Application state that persists between sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Configuration version for migration support
    #[serde(default = "default_app_config_version")]
    pub version: u32,

    /// Currently selected output device name
    #[serde(default)]
    pub output_device: Option<String>,
    /// Queue of albums (artist, title pairs)
    #[serde(default)]
    pub queue: Vec<(String, String)>,
    /// Current position in queue
    #[serde(default)]
    pub queue_index: Option<usize>,
    /// Current track index in the current album
    #[serde(default)]
    pub track_index: usize,
    /// Currently loaded plugin preset name
    #[serde(default)]
    pub plugin_preset: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: default_app_config_version(),
            output_device: None,
            queue: Vec::new(),
            queue_index: None,
            track_index: 0,
            plugin_preset: None,
        }
    }
}

/// Apply all necessary migrations to bring AppConfig to the latest version
pub(super) fn migrate_app_config(
    config: AppConfig,
) -> Result<AppConfig, Box<dyn std::error::Error>> {
    const LATEST_VERSION: u32 = 1;

    // Reject corrupt configs with version below minimum
    if config.version < LATEST_VERSION {
        return Err(format!(
            "Unsupported AppConfig version {} (minimum: {})",
            config.version, LATEST_VERSION
        )
        .into());
    }

    Ok(config)
}
