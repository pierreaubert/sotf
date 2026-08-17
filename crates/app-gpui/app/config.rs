use serde::{Deserialize, Serialize};
use sotf_audio_player::{DirectoryInfo, ReleaseChannel};

use crate::app::constants::recording::DEFAULT_SIGNAL_LEVEL_DB;
use crate::app::types::{
    DensityMode, PlaybackDeviceConfig, RecordingDeviceConfig, RecordingSignalType,
};
use crate::components::plugins::theme::RackThemeState;
use crate::i18n::Language;
use crate::keybindings::KeymapPreset;
use crate::theme::{CommunityThemeId, ThemeAccentPreference, ThemeId};
use gpui_themes::{AccessibilityPalette, ThemeModePreference};

pub(crate) fn default_recording_paths() -> (Option<String>, Option<String>) {
    let Some(base_dir) = sotf_audio_player::config::get_recordings_dir() else {
        return (None, None);
    };

    let timestamp_dir = format!("recording-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"));
    let recording_dir = base_dir.join(timestamp_dir);
    let _ = std::fs::create_dir_all(&recording_dir);

    (
        Some(base_dir.to_string_lossy().to_string()),
        Some(recording_dir.to_string_lossy().to_string()),
    )
}

fn default_recording_base_directory() -> Option<String> {
    default_recording_paths().0
}

fn default_recording_directory() -> Option<String> {
    default_recording_paths().1
}

fn directory_is_writable(path: &str) -> bool {
    std::fs::create_dir_all(path).is_ok()
}

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
    /// Per-channel microphone calibration file paths
    #[serde(default)]
    pub mic_calibration_paths: Vec<Option<String>>,
    #[serde(default = "default_recording_directory")]
    pub recording_directory: Option<String>,
    #[serde(default = "default_recording_base_directory")]
    pub recording_base_directory: Option<String>,
}

impl Default for RecordingConfigState {
    fn default() -> Self {
        let (recording_base_directory, recording_directory) = default_recording_paths();
        Self {
            playback: PlaybackDeviceConfig::default(),
            recording: RecordingDeviceConfig::default(),
            signal_type: RecordingSignalType::Sweep,
            signal_duration_secs: 5.0,
            signal_level_db: DEFAULT_SIGNAL_LEVEL_DB,
            mic_calibration_path: None,
            mic_calibration_paths: Vec::new(),
            recording_directory,
            recording_base_directory,
        }
    }
}

impl RecordingConfigState {
    /// Migrate from old single-path format to per-channel format if needed
    pub fn migrate_calibration_paths(&mut self) {
        if self.mic_calibration_paths.is_empty()
            && let Some(ref path) = self.mic_calibration_path
        {
            self.mic_calibration_paths = vec![Some(path.clone())];
        }
    }

    pub fn ensure_writable_recording_directory(&mut self) {
        if let Some(ref directory) = self.recording_directory
            && directory_is_writable(directory)
        {
            return;
        }

        if self.recording_directory.is_none()
            && let Some(ref base_directory) = self.recording_base_directory
            && directory_is_writable(base_directory)
        {
            let timestamp_dir =
                format!("recording-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"));
            let recording_dir = std::path::Path::new(base_directory).join(timestamp_dir);
            let _ = std::fs::create_dir_all(&recording_dir);
            self.recording_directory = Some(recording_dir.to_string_lossy().to_string());
            return;
        }

        let (recording_base_directory, recording_directory) = default_recording_paths();
        self.recording_base_directory = recording_base_directory;
        self.recording_directory = recording_directory;
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
    /// Rack strip height ratio inside Studio (0.0-1.0), default 0.22
    #[serde(default = "default_rack_detail_ratio")]
    pub rack_detail_ratio: f32,
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

fn default_rack_detail_ratio() -> f32 {
    0.22
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
            rack_detail_ratio: default_rack_detail_ratio(),
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
    /// Light/dark mode policy for app-wide theme selection
    #[serde(default)]
    pub theme_mode_preference: ThemeModePreference,
    /// Accessibility palette preset applied to the global app theme
    #[serde(default)]
    pub accessibility_palette: AccessibilityPalette,
    /// Accent override applied to the selected app theme
    #[serde(default)]
    pub theme_accent_preference: ThemeAccentPreference,
    /// Selected curated community theme, when the gallery is active
    #[serde(default)]
    pub community_theme_id: Option<CommunityThemeId>,
    /// Disable theme and UI transition motion
    #[serde(default)]
    pub reduce_motion: bool,
    /// UI density: Standard is calm/single-surface, Expert enables dense multi-panel workflows.
    #[serde(default)]
    pub density_mode: DensityMode,
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
    /// Number of scanner threads for waveform/bliss/replaygain (None = auto-detect)
    #[serde(default)]
    pub scanner_threads: Option<u8>,
    /// Maximum number of CPU cores SotF is allowed to use (None = all available)
    #[serde(default)]
    pub max_cpu_cores: Option<u8>,
    /// Minimum font size in pixels (None = default 12px)
    #[serde(default)]
    pub min_font_size_px: Option<f32>,
    /// Maximum font size in pixels (None = default 32px)
    #[serde(default)]
    pub max_font_size_px: Option<f32>,
    /// Whether the tutorial has been completed/dismissed
    #[serde(default)]
    pub tutorial_completed: bool,
    /// Contextual hints that have been seen and dismissed
    #[serde(default)]
    pub seen_hints: Vec<String>,
    /// Selected design system language (`None` = platform default).
    /// Supported values: "neutral", "apple_hig", "material3", "fluent".
    #[serde(default)]
    pub design_language: Option<String>,
    /// Plugin chassis theme: rack default + per-plugin overrides. Independent
    /// from `theme` (which controls the global app palette).
    #[serde(default)]
    pub rack_theme_state: RackThemeState,
    /// Remote library identity currently associated with the local database.
    /// iOS uses this to clear stale local data when switching remote servers or
    /// when the connected server's library version changes.
    #[serde(default)]
    pub remote_library_identity: Option<RemoteLibraryIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteLibraryIdentity {
    pub server_id: String,
    pub library_version: u64,
}

fn default_font_scale() -> f32 {
    1.0
}

pub fn default_volume() -> f32 {
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
                theme_mode_preference: ThemeModePreference::default(),
                accessibility_palette: AccessibilityPalette::default(),
                theme_accent_preference: ThemeAccentPreference::default(),
                community_theme_id: None,
                reduce_motion: false,
                density_mode: DensityMode::default(),
                language: Language::default(),
                keymap_preset: KeymapPreset::default(),
                panel_layout: PanelLayout::default(),
                window_geometry: WindowGeometry::default(),
                volume: default_volume(),
                muted: false,
                recording_config: RecordingConfigState::default(),
                font_scale: default_font_scale(),
                release_channel: ReleaseChannel::default(),
                scanner_threads: None,
                max_cpu_cores: None,
                min_font_size_px: None,
                max_font_size_px: None,
                tutorial_completed: false,
                seen_hints: Vec::new(),
                design_language: None,
                rack_theme_state: RackThemeState::default(),
                remote_library_identity: None,
            });
        }

        let json = std::fs::read_to_string(&path).map_err(|source| ConfigError::ReadError {
            path: path.clone(),
            source,
        })?;

        let mut config: Self =
            serde_json::from_str(&json).map_err(|source| ConfigError::ParseError {
                path: path.clone(),
                source,
            })?;

        config
            .recording_config
            .ensure_writable_recording_directory();
        Ok(config)
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
