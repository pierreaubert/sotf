use serde::{Deserialize, Serialize};
use sotf_audio_player::DirectoryInfo;

/// GPUI-specific application configuration persisted between sessions
/// Uses shared library's config helper functions for paths
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Directories to scan for music files
    pub directories: Vec<DirectoryInfo>,
    /// Last loaded plugin preset name
    pub last_loaded_plugin_preset: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // Use shared library function
        if let Some(path) = sotf_audio_player::config::get_app_state_path() {
            if path.exists() {
                let json = std::fs::read_to_string(path)?;
                let config = serde_json::from_str(&json)?;
                Ok(config)
            } else {
                Ok(Self {
                    directories: Vec::new(),
                    last_loaded_plugin_preset: None,
                })
            }
        } else {
            Err("Could not determine config directory".into())
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Use shared library function
        if let Some(path) = sotf_audio_player::config::get_app_state_path() {
            let json = serde_json::to_string_pretty(self)?;
            std::fs::write(path, json)?;
            Ok(())
        } else {
            Err("Could not determine config directory".into())
        }
    }
}
