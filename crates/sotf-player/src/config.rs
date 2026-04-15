use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;

/// Global override for the app config directory (set via `--qa` flag).
static CONFIG_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

/// Set a custom config directory, overriding the platform default.
/// Must be called before any `get_app_config_dir()` usage.
/// Creates the directory if it doesn't exist.
pub fn set_config_dir_override(path: PathBuf) {
    std::fs::create_dir_all(&path).expect("Failed to create config dir override");
    CONFIG_DIR_OVERRIDE
        .set(path)
        .expect("Config dir override already set");
}

/// Application state that persists between sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Configuration version for migration support
    #[serde(default = "default_app_config_version")]
    pub version: u32,

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

fn default_app_config_version() -> u32 {
    1
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

/// Get the application configuration directory
/// - Linux: ~/.config/sotf
/// - macOS: ~/Library/Application Support/org.spinorama.sotf
/// - Windows: ~/.config/sotf (same as Linux)
/// - iOS: ~/Library/Application Support/org.spinorama.sotf (same as macOS)
pub fn get_app_config_dir() -> Option<PathBuf> {
    if let Some(dir) = CONFIG_DIR_OVERRIDE.get() {
        return Some(dir.clone());
    }

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

    #[cfg(target_os = "windows")]
    {
        // On Windows, use LOCALAPPDATA (preferred) or USERPROFILE as fallback
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let config_dir = PathBuf::from(local_app_data).join("sotf");
            std::fs::create_dir_all(&config_dir).ok()?;
            return Some(config_dir);
        } else if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let config_dir = PathBuf::from(user_profile).join(".config").join("sotf");
            std::fs::create_dir_all(&config_dir).ok()?;
            return Some(config_dir);
        }
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
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

/// Get the path to the microphone presets config file
pub fn get_microphone_presets_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("microphones.json"))
}

/// Load microphone presets from disk
pub fn load_microphone_presets()
-> Result<crate::recording_types::MicrophonePresetsConfig, Box<dyn std::error::Error>> {
    if let Some(path) = get_microphone_presets_path() {
        if path.exists() {
            crate::security::validate_config_read_path(&path)?;
            let json = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&json)?)
        } else {
            Ok(crate::recording_types::MicrophonePresetsConfig::default())
        }
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Save microphone presets to disk
pub fn save_microphone_presets(
    config: &crate::recording_types::MicrophonePresetsConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = get_microphone_presets_path() {
        crate::security::validate_write_path(&path)?;
        let json = serde_json::to_string_pretty(config)?;
        std::fs::write(path, json)?;
        Ok(())
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Get the path to the plugin presets directory
pub fn get_plugin_presets_dir() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| {
        let presets_dir = dir.join("plugin_presets");
        std::fs::create_dir_all(&presets_dir).ok();
        presets_dir
    })
}

/// Get the path to the EQ directory (for headphone/speaker EQ curves)
pub fn get_eq_dir() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| {
        let eq_dir = dir.join("EQ");
        std::fs::create_dir_all(&eq_dir).ok();
        eq_dir
    })
}

/// Get the path to the app state config file (deprecated - use app-specific paths)
#[deprecated(note = "Use get_tui_state_path() or get_gpui_state_path() instead")]
pub fn get_app_state_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("app_state.json"))
}

/// Get the path to the TUI app state config file
pub fn get_tui_state_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("app_state_tui.json"))
}

/// Get the path to the GPUI app state config file
pub fn get_gpui_state_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("app_state_gpui.json"))
}

/// Get the path to the TUI log file
pub fn get_tui_log_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("sotf_tui_player.log"))
}

/// Get the path to the GPUI log file
pub fn get_gpui_log_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("sotf_gpui_player.log"))
}

/// Get the path to the server configuration file
pub fn get_server_config_path() -> Option<PathBuf> {
    get_app_config_dir().map(|dir| dir.join("servers.json"))
}

/// Load server configuration from disk.
///
/// # Errors
/// Returns an error if the file exists but cannot be read or parsed.
pub fn load_server_config()
-> Result<crate::federation_config::ServerConfig, Box<dyn std::error::Error>> {
    if let Some(path) = get_server_config_path() {
        if path.exists() {
            crate::security::validate_config_read_path(&path)?;
            let json = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&json)?)
        } else {
            Ok(crate::federation_config::ServerConfig::default())
        }
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Save server configuration to disk.
///
/// # Errors
/// Returns an error if the config directory cannot be determined or the file cannot be written.
pub fn save_server_config(
    config: &crate::federation_config::ServerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = get_server_config_path() {
        crate::security::validate_write_path(&path)?;
        let json = serde_json::to_string_pretty(config)?;
        std::fs::write(path, json)?;
        Ok(())
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Save TUI app configuration to disk
pub fn save_app_config(config: &AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = get_tui_state_path() {
        // Validate that we're writing within the config directory
        crate::security::validate_write_path(&path)?;

        let json = serde_json::to_string_pretty(config)?;
        std::fs::write(path, json)?;
        Ok(())
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Load TUI app configuration from disk, applying migrations if needed
pub fn load_app_config() -> Result<AppConfig, Box<dyn std::error::Error>> {
    if let Some(path) = get_tui_state_path() {
        if path.exists() {
            // Validate that we're reading from within the config directory
            crate::security::validate_config_read_path(&path)?;

            let json = std::fs::read_to_string(&path)?;
            let mut config: AppConfig = serde_json::from_str(&json)?;

            // Check if migration is needed
            const LATEST_VERSION: u32 = 1;
            let original_version = config.version;

            if config.version < LATEST_VERSION {
                log::info!(
                    "Migrating AppConfig from version {} to {}",
                    original_version,
                    LATEST_VERSION
                );

                // Apply migrations
                config = migrate_app_config(config)?;

                // Save upgraded config back to disk
                save_app_config(&config)?;

                log::info!(
                    "Successfully migrated AppConfig from version {} to {}",
                    original_version,
                    LATEST_VERSION
                );
            }

            Ok(config)
        } else {
            Ok(AppConfig::default())
        }
    } else {
        Err("Could not determine config directory".into())
    }
}

/// Apply all necessary migrations to bring AppConfig to the latest version
fn migrate_app_config(config: AppConfig) -> Result<AppConfig, Box<dyn std::error::Error>> {
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

            // On Linux
            #[cfg(target_os = "linux")]
            assert!(dir.to_string_lossy().contains(".config/sotf"));

            // On Windows (uses LOCALAPPDATA\sotf or USERPROFILE\.config\sotf)
            #[cfg(target_os = "windows")]
            assert!(dir.to_string_lossy().contains("sotf"));
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
