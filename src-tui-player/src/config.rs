use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application state that persists between sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Currently selected output device name
    pub output_device: Option<String>,
    /// Queue of albums (artist, title pairs)
    pub queue: Vec<(String, String)>,
    /// Current position in queue
    pub queue_index: Option<usize>,
    /// Current track index in the current album
    pub track_index: usize,
    /// Currently loaded plugin preset name
    pub plugin_preset: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            output_device: None,
            queue: Vec::new(),
            queue_index: None,
            track_index: 0,
            plugin_preset: None,
        }
    }
}

/// Get the application configuration directory
/// - Linux: ~/.config/sotf
/// - macOS: ~/Library/Application Support/org.spinorama.sotf
/// - Windows: ~/.config/sotf (same as Linux)
/// - iOS: ~/Library/Application Support/org.spinorama.sotf (same as macOS)
pub fn get_app_config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let config_dir = PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("org.spinorama.sotf");
            std::fs::create_dir_all(&config_dir).ok()?;
            return Some(config_dir);
        }
    }

    #[cfg(target_os = "ios")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let config_dir = PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("org.spinorama.sotf");
            std::fs::create_dir_all(&config_dir).ok()?;
            return Some(config_dir);
        }
    }

    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "android"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let config_dir = PathBuf::from(home).join(".config").join("sotf");
            std::fs::create_dir_all(&config_dir).ok()?;
            return Some(config_dir);
        }
    }

    // Fallback for any other platform
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "windows",
        target_os = "android"
    )))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let config_dir = PathBuf::from(home).join(".config").join("sotf");
            std::fs::create_dir_all(&config_dir).ok()?;
            return Some(config_dir);
        }
    }

    None
}

/// Get the path to the music database file
pub fn get_music_db_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("music.db"))
}

/// Get the path to the plugin presets directory
pub fn get_plugin_presets_dir() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| {
        let presets_dir = dir.join("plugin_presets");
        std::fs::create_dir_all(&presets_dir).ok();
        presets_dir
    })
}

/// Get the path to the app state config file
pub fn get_app_state_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("app_state.json"))
}

/// Save app configuration to disk
pub fn save_app_config(config: &AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = get_app_state_path() {
        let json = serde_json::to_string_pretty(config)?;
        std::fs::write(path, json)?;
        Ok(())
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Load app configuration from disk
pub fn load_app_config() -> Result<AppConfig, Box<dyn std::error::Error>> {
    if let Some(path) = get_app_state_path() {
        if path.exists() {
            let json = std::fs::read_to_string(path)?;
            let config = serde_json::from_str(&json)?;
            Ok(config)
        } else {
            Ok(AppConfig::default())
        }
    } else {
        Err("Could not determine config directory".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_dir_exists() {
        let config_dir = get_app_config_dir();
        assert!(config_dir.is_some());

        if let Some(dir) = config_dir {
            // On macOS
            #[cfg(target_os = "macos")]
            assert!(
                dir.to_string_lossy()
                    .contains("Library/Application Support/org.spinorama.sotf")
            );

            // On Linux/Windows
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            assert!(dir.to_string_lossy().contains(".config/sotf"));
        }
    }

    #[test]
    fn test_music_db_path() {
        let db_path = get_music_db_path();
        assert!(db_path.is_some());

        if let Some(path) = db_path {
            assert!(path.to_string_lossy().ends_with("music.db"));
        }
    }

    #[test]
    fn test_plugin_presets_dir() {
        let presets_dir = get_plugin_presets_dir();
        assert!(presets_dir.is_some());

        if let Some(dir) = presets_dir {
            assert!(dir.to_string_lossy().ends_with("plugin_presets"));
        }
    }
}
