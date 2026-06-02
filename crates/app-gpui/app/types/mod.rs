//! Type definitions for the GPUI audio player application.
//!
//! Contains enums and simple structs used throughout the application.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    NowPlaying,
    Library,
    Queue,
    Playlists,
    Spectrum,
    Settings,
    Studio,
    Recording,
    RoomEq,
    HeadphoneEq,
    Spinorama,
    PluginGraph,
}

impl Screen {
    pub fn primary_destinations() -> &'static [Self] {
        const DESTINATIONS: &[Screen] = &[
            Screen::NowPlaying,
            Screen::Library,
            Screen::Queue,
            Screen::Studio,
        ];

        DESTINATIONS
    }

    pub fn primary_destination_index(self) -> usize {
        if let Some(index) = Self::primary_destinations()
            .iter()
            .position(|screen| *screen == self)
        {
            return index;
        }

        if self.is_studio_tool() { 3 } else { 0 }
    }

    pub fn is_studio_tool(self) -> bool {
        matches!(
            self,
            Screen::PluginGraph
                | Screen::Recording
                | Screen::RoomEq
                | Screen::HeadphoneEq
                | Screen::Spinorama
                | Screen::Spectrum
        )
    }

    pub fn from_view_menu_id(id: &str) -> Option<Self> {
        match id {
            "now-playing" => Some(Screen::NowPlaying),
            "library" => Some(Screen::Library),
            "queue" => Some(Screen::Queue),
            "studio" => Some(Screen::Studio),
            "plugingraph" => Some(Screen::PluginGraph),
            "recording" => Some(Screen::Recording),
            "roomeq" => Some(Screen::RoomEq),
            "headphoneeq" => Some(Screen::HeadphoneEq),
            "spinorama" => Some(Screen::Spinorama),
            "settings" => Some(Screen::Settings),
            _ => None,
        }
    }
}

pub use sotf_audio_player::ReplayGainMode;

/// Audio playback source mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PlaybackSource {
    /// Play from audio files (normal music player mode)
    #[default]
    File,
    /// Process audio from HAL virtual device (macOS only)
    /// This captures system-wide audio and processes it through the plugin chain
    #[cfg(all(target_os = "macos", feature = "hal"))]
    HalDevice,
}

// PluginViewMode is defined in state::plugin and re-exported from state::mod
pub use crate::app::state::plugin::PluginViewMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    AddDirectory,
    SavePlugins,
    LoadPlugins,
    LoadApoFile,
    LoadSofaFile,
    Help,
    HelpSupport,
    KeyboardShortcuts,
    About,
    EditingParam,
    SpinoramaSpeakerSearch,
    HeadphoneSearch,
    /// Modal shown on startup when library is empty
    EmptyLibraryPrompt,
    /// Modal for editing a plugin node in the graph view
    EditingPluginNode,
    /// Modal shown when track channels conflict with plugins in the chain
    ChannelConflict,
    /// Context menu is open (album, queue item, etc.)
    ContextMenu,
    /// Tutorial dialog shown on first launch
    Tutorial,
    /// Contextual help guide for the current screen
    ScreenGuide,
}

impl InputMode {
    /// Check if this mode captures text input (blocking keyboard shortcuts).
    /// Use this to determine if actions should be blocked when in text entry modes.
    pub fn is_text_input(&self) -> bool {
        matches!(
            self,
            InputMode::Search
                | InputMode::AddDirectory
                | InputMode::SavePlugins
                | InputMode::LoadPlugins
                | InputMode::LoadApoFile
                | InputMode::LoadSofaFile
                | InputMode::SpinoramaSpeakerSearch
                | InputMode::HeadphoneSearch
        )
    }
}

/// Active menu dropdown (if any)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveMenu {
    None,
    File,
    Show,
    Help,
    AddPlugin, // Plugin rack "Add" menu
}

/// Layout mode based on window height (legacy - kept for compatibility)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutMode {
    Compact, // Below 800px - tabs bar visible
    #[default]
    Expanded, // Above 800px - split Library/Queue view
}

/// Product density mode.
///
/// Standard keeps one primary destination visible at a time. Expert opts into
/// the dense multi-panel Library | Queue | Rack workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DensityMode {
    #[default]
    Standard,
    Expert,
}

impl DensityMode {
    pub fn all() -> &'static [Self] {
        const MODES: &[DensityMode] = &[DensityMode::Standard, DensityMode::Expert];

        MODES
    }

    pub fn label(self) -> &'static str {
        match self {
            DensityMode::Standard => "Standard",
            DensityMode::Expert => "Expert",
        }
    }

    pub fn value(self) -> &'static str {
        match self {
            DensityMode::Standard => "standard",
            DensityMode::Expert => "expert",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "standard" => Some(DensityMode::Standard),
            "expert" => Some(DensityMode::Expert),
            _ => None,
        }
    }

    pub fn layout_mode_for_window(self, width: f32, height: f32) -> LayoutMode {
        match self {
            DensityMode::Expert if width >= 600.0 && height >= 500.0 => LayoutMode::Expanded,
            _ => LayoutMode::Compact,
        }
    }
}

/// Layout orientation based on window aspect ratio
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutOrientation {
    #[default]
    Horizontal, // width > height: panels side-by-side (Library | Queue | Rack)
    Vertical, // height >= width: panels stacked vertically
}

/// Rack display mode based on available space
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RackDisplayMode {
    #[default]
    Full, // Full rack with all controls
    Mini,      // Compact mode with output level meters only
    Collapsed, // Hidden
}

/// Meter display mode for Queue screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MeterDisplayMode {
    #[default]
    Lufs, // Show LUFS loudness meters
    Levels, // Show level meters
}

// Library enums (shared via player crate)
pub use sotf_audio_player::{ChannelFilter, LibrarySortOrder};

pub use sotf_audio_player::{ChannelGroup, ChannelInfo};

/// Context menu state
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub menu_type: ContextMenuType,
    pub position_x: f32,
    pub position_y: f32,
    pub item_index: usize, // Index of the item that was right-clicked
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContextMenuType {
    Album,
    QueueItem,
    Plugin,
    Directory,
}

/// Type of plugin update needed for audio engine synchronization
#[derive(Debug, Clone)]
pub enum PluginUpdateType {
    /// Single parameter change - use set_plugin_parameter() for zero-dropout update
    Parameter {
        plugin_index: usize,
        param_index: usize,
    },
    /// Parameter change addressed by graph node ID (works for non-linear graphs)
    ParameterByNodeId {
        node_id: sotf_audio_player::GraphNodeId,
        param_index: usize,
    },
    /// Structural change (add/remove/reorder/toggle) - use update_plugins() for full rebuild
    Structural,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeasureStep {
    DeviceSelection,
    SignalConfig,
    Running,
    Results,
}

#[derive(Debug, Clone)]
pub struct MeasurementResult {
    pub frequencies: Vec<f32>,
    pub magnitude_db: Vec<f32>,
    pub phase_deg: Vec<f32>,
    pub csv_path: String,
}

#[derive(Debug, Clone)]
pub struct MeasureState {
    pub step: MeasureStep,
    pub signal_type: String, // "sweep", "pink-noise"
    pub duration: String,    // "5.0", "10.0"
    pub level: f32,          // -20dB etc
    pub output_channel: usize,
    pub input_channel: usize,
    pub progress: f32,
    pub status_message: String,
    pub measurement_result: Option<MeasurementResult>,
    // UI state for dropdowns
    pub output_ch_open: bool,
    pub input_ch_open: bool,
}

impl Default for MeasureState {
    fn default() -> Self {
        Self {
            step: MeasureStep::DeviceSelection,
            signal_type: "sweep".to_string(),
            duration: "5.0".to_string(),
            level: -20.0,
            output_channel: 0,
            input_channel: 0,
            progress: 0.0,
            status_message: String::new(),
            measurement_result: None,
            output_ch_open: false,
            input_ch_open: false,
        }
    }
}

/// UI state for optimization forms (dropdowns open/closed)
#[derive(Debug, Clone, Default)]
pub struct OptimizationUiState {
    pub peq_model_open: bool,
    pub algo_open: bool,
    pub strategy_open: bool,
    pub local_algo_open: bool,
}

pub mod calibration;
pub mod maturity;
pub mod queue;
pub mod settings;
pub mod stats;
pub mod toast;

pub mod headphone_eq;
pub mod recording;
pub mod room_eq;
pub mod spinorama_eq;

// Re-export commonly used types for convenience
pub use calibration::CalibrationData;
pub use headphone_eq::{HeadphoneEqBiquad, HeadphoneEqResult, HeadphoneEqState, HeadphoneEqStep};
pub use queue::QueueItem;
pub use recording::{
    ChannelMapping, ChannelRecording, ChannelRecordingState, CtcMatrixExportStrategy,
    PlaybackDeviceConfig, PlotSmoothing, RecordingDeviceConfig, RecordingResult,
    RecordingSignalType, RecordingState, RecordingStep, SpeakerConfiguration,
    TransferMatrixLoopbackRecording,
};
pub use room_eq::{
    ChannelDspChain, ChannelMeasurement, ChannelOptResult, CrossoverType, CustomTargetCurve,
    DriverDspChain, DspChainMetadata, DspChainOutput, DspPluginConfig, EqFilterConfig,
    MultiSpeakerMode, OptimizationStatus, RoomEqAlgorithm, RoomEqDataSource,
    RoomEqMeasurementsFile, RoomEqOptimizationMode, RoomEqOptimizerConfig, RoomEqSpeakerConfig,
    RoomEqState, RoomEqStep, SpeakerConfigType, TargetCurveControlPoint,
};
pub use settings::{ScanProgressModal, ScanType, SettingsTab};
pub use spinorama_eq::{
    DirectivityCurve, SpinoramaBiquad, SpinoramaCurves, SpinoramaEqResult, SpinoramaEqState,
    SpinoramaOptimizationMode, SpinoramaStep, SpinoramaTargetCurve,
};
pub use stats::LibraryStats;
pub use toast::{ToastMessage, ToastType};
