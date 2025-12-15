use serde::{Deserialize, Serialize};
use sotf_audio_player::DirectoryInfo;

use crate::i18n::Language;
use crate::keybindings::KeymapPreset;
use crate::theme::ThemeId;

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

impl Default for PanelLayout {
    fn default() -> Self {
        Self {
            queue_ratio: default_queue_ratio(),
            meters_ratio: default_meters_ratio(),
            queue_list_ratio: default_queue_list_ratio(),
            lufs_ratio: default_lufs_ratio(),
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
}

fn default_volume() -> f32 {
    0.5
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // Use GPUI-specific state path
        if let Some(path) = sotf_audio_player::config::get_gpui_state_path() {
            if path.exists() {
                let json = std::fs::read_to_string(&path)?;
                let config = serde_json::from_str(&json)?;
                Ok(config)
            } else {
                Ok(Self {
                    directories: Vec::new(),
                    last_loaded_plugin_preset: None,
                    theme: ThemeId::default(),
                    language: Language::default(),
                    keymap_preset: KeymapPreset::default(),
                    panel_layout: PanelLayout::default(),
                    window_geometry: WindowGeometry::default(),
                    volume: default_volume(),
                    muted: false,
                })
            }
        } else {
            Err("Could not determine config directory".into())
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Use GPUI-specific state path
        if let Some(path) = sotf_audio_player::config::get_gpui_state_path() {
            let json = serde_json::to_string_pretty(self)?;
            std::fs::write(&path, json)?;
            Ok(())
        } else {
            Err("Could not determine config directory".into())
        }
    }
}
