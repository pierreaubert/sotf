use serde::{Deserialize, Serialize};
use sotf_audio_player::{DirectoryInfo, ReleaseChannel};

use crate::app::types::{PlaybackDeviceConfig, RecordingDeviceConfig, RecordingSignalType};
use crate::i18n::Language;
use crate::keybindings::KeymapPreset;
use crate::theme::ThemeId;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to determine config directory")]
    NoConfigDirectory,
    #[error("Failed to read config file: {path}")]
    ReadError {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("Failed to parse config file: {path}")]
    ParseError {
        path: std::path::PathBuf,
        source: serde_json::Error,
    },
    #[error("Failed to write config file: {path}")]
    WriteError {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("Failed to serialize config: {source}")]
    SerializeError { source: serde_json::Error },
}

/// Persisted state for recording screen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfigState {
    pub playback: PlaybackDeviceConfig,
    pub recording: RecordingDeviceConfig,
    pub signal_type: RecordingSignalType,
    pub signal_duration_secs: f32,
    pub signal_level_db: f32,
    pub mic_calibration_path: Option<String>,
    pub recording_directory: Option<String>,
    pub recording_base_directory: Option<String>,
}

impl Default for RecordingConfigState {
    fn default() -> Self {
        Self {
            playback: PlaybackDeviceConfig::default(),
            recording: RecordingDeviceConfig::default(),
            signal_type: RecordingSignalType::Sweep,
            signal_duration_secs: 5.0,
            signal_level_db: -20.0,
            mic_calibration_path: None,
            recording_directory: None,
            recording_base_directory: None,
        }
    }
}

/// Window geometry for persisting window size and position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowGeometry {
    /// Window X position
    pub x: f32,
    /// Window Y position
    pub y: f32,
    /// Window width
    pub width: f32,
    /// Window height
    pub height: f32,
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self {
            x: 100.0,
            y: 100.0,
            width: 1200.0,
            height: 800.0,
        }
    }
}

/// Panel layout configuration for resizable panels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelLayout {
    /// Queue panel height ratio (0.0-1.0), default 0.35
    #[serde(default = "default_queue_ratio")]
    pub queue_ratio: f32,
    /// Level meters width ratio (0.0-1.0), default 0.25
    #[serde(default = "default_meters_ratio")]
    pub meters_ratio: f32,
    /// Queue list width ratio (0.0-1.0), default 0.30
    #[serde(default = "default_queue_list_ratio")]
    pub queue_list_ratio: f32,
    /// LUFS panel width ratio (0.0-1.0), default 0.25
    #[serde(default = "default_lufs_ratio")]
    pub lufs_ratio: f32,
    // 3-Panel Layout ratios (horizontal mode)
    #[serde(default = "default_library_h_ratio")]
    pub library_h_ratio: f32,
    #[serde(default = "default_queue_h_ratio")]
    pub queue_h_ratio: f32,
    #[serde(default = "default_rack_h_ratio")]
    pub rack_h_ratio: f32,
    // 3-Panel Layout ratios (vertical mode)
    #[serde(default = "default_library_v_ratio")]
    pub library_v_ratio: f32,
    #[serde(default = "default_queue_v_ratio")]
    pub queue_v_ratio: f32,
    #[serde(default = "default_rack_v_ratio")]
    pub rack_v_ratio: f32,
}

fn default_queue_ratio() -> f32 {
    0.35
}

fn default_meters_ratio() -> f32 {
    0.25
}

fn default_lufs_ratio() -> f32 {
    0.25
}

fn default_queue_list_ratio() -> f32 {
    0.30
}

fn default_library_h_ratio() -> f32 {
    0.30
}

fn default_queue_h_ratio() -> f32 {
    0.40
}

fn default_rack_h_ratio() -> f32 {
    0.30
}

fn default_library_v_ratio() -> f32 {
    0.40
}

fn default_queue_v_ratio() -> f32 {
    0.35
}

fn default_rack_v_ratio() -> f32 {
    0.25
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self {
            queue_ratio: default_queue_ratio(),
            meters_ratio: default_meters_ratio(),
            queue_list_ratio: default_queue_list_ratio(),
            lufs_ratio: default_lufs_ratio(),
            library_h_ratio: default_library_h_ratio(),
            queue_h_ratio: default_queue_h_ratio(),
            rack_h_ratio: default_rack_h_ratio(),
            library_v_ratio: default_library_v_ratio(),
            queue_v_ratio: default_queue_v_ratio(),
            rack_v_ratio: default_rack_v_ratio(),
        }
    }
}

/// GPUI-specific application configuration persisted between sessions
/// Uses shared library's config helper functions for paths
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Directories to scan for music files
    pub directories: Vec<DirectoryInfo>,
    /// Last loaded plugin preset name
    pub last_loaded_plugin_preset: Option<String>,
    /// Selected theme
    #[serde(default)]
    pub theme: ThemeId,
    /// Selected language
    #[serde(default)]
    pub language: Language,
    /// Selected keymap preset
    #[serde(default)]
    pub keymap_preset: KeymapPreset,
    /// Panel layout configuration
    #[serde(default)]
    pub panel_layout: PanelLayout,
    /// Window geometry (size and position)
    #[serde(default)]
    pub window_geometry: WindowGeometry,
    /// Volume level (0.0-1.0)
    #[serde(default = "default_volume")]
    pub volume: f32,
    /// Muted state
    #[serde(default)]
    pub muted: bool,
    /// Recording screen configuration
    #[serde(default)]
    pub recording_config: RecordingConfigState,
    /// Font scale factor (1.0 = normal)
    #[serde(default = "default_font_scale")]
    pub font_scale: f32,
    /// Feature release channel (Prod/Beta/Alpha)
    #[serde(default)]
    pub release_channel: ReleaseChannel,
}

fn default_font_scale() -> f32 {
    1.0
}

fn default_volume() -> f32 {
    crate::app::constants::ui::DEFAULT_CONFIG_VOLUME
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let path = sotf_audio_player::config::get_gpui_state_path()
            .ok_or(ConfigError::NoConfigDirectory)?;

        if !path.exists() {
            return Ok(Self {
                directories: Vec::new(),
                last_loaded_plugin_preset: None,
                theme: ThemeId::default(),
                language: Language::default(),
                keymap_preset: KeymapPreset::default(),
                panel_layout: PanelLayout::default(),
                window_geometry: WindowGeometry::default(),
                volume: default_volume(),
                muted: false,
                recording_config: RecordingConfigState::default(),
                font_scale: default_font_scale(),
                release_channel: ReleaseChannel::default(),
            });
        }

        let json = std::fs::read_to_string(&path).map_err(|source| ConfigError::ReadError {
            path: path.clone(),
            source,
        })?;

        serde_json::from_str(&json).map_err(|source| ConfigError::ParseError {
            path: path.clone(),
            source,
        })
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let path = sotf_audio_player::config::get_gpui_state_path()
            .ok_or(ConfigError::NoConfigDirectory)?;

        let json = serde_json::to_string_pretty(self)
            .map_err(|source| ConfigError::SerializeError { source })?;

        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&path)
                .map_err(|source| ConfigError::WriteError {
                    path: path.clone(),
                    source,
                })?;
            file.write_all(json.as_bytes())
                .map_err(|source| ConfigError::WriteError {
                    path: path.clone(),
                    source,
                })
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&path, json).map_err(|source| ConfigError::WriteError {
                path: path.clone(),
                source,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::types::{PlaybackDeviceConfig, RecordingDeviceConfig, RecordingSignalType};

    #[test]
    fn test_recording_config_state_default() {
        let config = RecordingConfigState::default();
        assert_eq!(config.signal_duration_secs, 5.0);
        assert_eq!(config.signal_level_db, -20.0);
        assert!(config.mic_calibration_path.is_none());
        assert!(config.recording_directory.is_none());
    }

    #[test]
    fn test_window_geometry_default() {
        let geometry = WindowGeometry::default();
        assert_eq!(geometry.x, 100.0);
        assert_eq!(geometry.y, 100.0);
        assert_eq!(geometry.width, 1200.0);
        assert_eq!(geometry.height, 800.0);
    }

    #[test]
    fn test_panel_layout_default() {
        let layout = PanelLayout::default();
        assert!((layout.queue_ratio - 0.35).abs() < 0.001);
        assert!((layout.meters_ratio - 0.25).abs() < 0.001);
        assert!((layout.queue_list_ratio - 0.30).abs() < 0.001);
        assert!((layout.lufs_ratio - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_panel_layout_serialization() {
        let layout = PanelLayout {
            queue_ratio: 0.5,
            meters_ratio: 0.3,
            queue_list_ratio: 0.4,
            lufs_ratio: 0.2,
            library_h_ratio: 0.30,
            queue_h_ratio: 0.40,
            rack_h_ratio: 0.30,
            library_v_ratio: 0.40,
            queue_v_ratio: 0.35,
            rack_v_ratio: 0.25,
        };
        let json = serde_json::to_string(&layout).unwrap();
        let deserialized: PanelLayout = serde_json::from_str(&json).unwrap();
        assert!((deserialized.queue_ratio - 0.5).abs() < 0.001);
        assert!((deserialized.meters_ratio - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            directories: Vec::new(),
            last_loaded_plugin_preset: Some("test_preset".to_string()),
            theme: ThemeId::default(),
            language: Language::default(),
            keymap_preset: KeymapPreset::default(),
            panel_layout: PanelLayout::default(),
            window_geometry: WindowGeometry::default(),
            volume: 0.75,
            muted: true,
            recording_config: RecordingConfigState::default(),
            font_scale: 1.0,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(
            deserialized.last_loaded_plugin_preset,
            Some("test_preset".to_string())
        );
        assert!((deserialized.volume - 0.75).abs() < 0.001);
        assert!(deserialized.muted);
    }

    #[test]
    fn test_config_default_volume() {
        // Test that default_volume returns 0.5
        assert!((default_volume() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_window_geometry_serialization() {
        let geometry = WindowGeometry {
            x: 200.0,
            y: 150.0,
            width: 1600.0,
            height: 900.0,
        };
        let json = serde_json::to_string(&geometry).unwrap();
        let deserialized: WindowGeometry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.x, 200.0);
        assert_eq!(deserialized.y, 150.0);
        assert_eq!(deserialized.width, 1600.0);
        assert_eq!(deserialized.height, 900.0);
    }

    #[test]
    fn test_recording_config_state_serialization() {
        let config = RecordingConfigState {
            playback: PlaybackDeviceConfig::default(),
            recording: RecordingDeviceConfig::default(),
            signal_type: RecordingSignalType::PinkNoise,
            signal_duration_secs: 10.0,
            signal_level_db: -12.0,
            mic_calibration_path: Some("/path/to/cal.txt".to_string()),
            recording_directory: Some("/recordings".to_string()),
            recording_base_directory: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RecordingConfigState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.signal_type, RecordingSignalType::PinkNoise);
        assert_eq!(deserialized.signal_duration_secs, 10.0);
        assert_eq!(
            deserialized.mic_calibration_path,
            Some("/path/to/cal.txt".to_string())
        );
    }
}
