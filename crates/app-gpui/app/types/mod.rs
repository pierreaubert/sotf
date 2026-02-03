//! Type definitions for the GPUI audio player application.
//!
//! Contains enums and simple structs used throughout the application.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Library,
    Queue,
    Spectrum,
    Settings,
    Studio,
    Recording,
    RoomEq,
    HeadphoneEq,
    Spinorama,
    PluginGraph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayGainMode {
    Track,
    Album,
}

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

/// View mode for plugin management (Rack vs Graph)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PluginViewMode {
    /// Traditional linear plugin chain view
    #[default]
    Rack,
    /// 2D graph view with node connections
    Graph,
}

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
    /// Modal shown on startup when library is empty
    EmptyLibraryPrompt,
    /// Modal for editing a plugin node in the graph view
    EditingPluginNode,
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

// Enums mapped from library
pub use crate::app::state::library::LibrarySortOrder;
pub use sotf_audio_player::library::ChannelFilter;

/// Channel group for level meter display
#[derive(Debug, Clone)]
pub struct ChannelGroup {
    pub name: String,
    pub channels: Vec<ChannelInfo>,
    pub muted: bool,
    pub soloed: bool,
    pub dimmed: bool,
}

/// Individual channel information
#[derive(Debug, Clone)]
pub struct ChannelInfo {
    pub index: usize,              // Index in loudness.channel_peaks
    pub name: String,              // e.g., "FL", "FR", "C"
    pub display_name: Vec<String>, // Multi-line display: ["F", "L"] or ["T", "B", "R"]
}

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
pub mod queue;
pub mod settings;
pub mod stats;
pub mod toast;

pub mod headphone_eq;
pub mod recording;
pub mod room_eq;
pub mod spinorama_eq;

mod tests;

// Re-export commonly used types for convenience
pub use calibration::CalibrationData;
pub use headphone_eq::{HeadphoneEqBiquad, HeadphoneEqResult, HeadphoneEqState, HeadphoneEqStep};
pub use queue::QueueItem;
pub use recording::{
    ChannelMapping, ChannelRecording, ChannelRecordingState, PlaybackDeviceConfig, PlotSmoothing,
    RecordingDeviceConfig, RecordingResult, RecordingSignalType, RecordingState, RecordingStep,
    SpeakerConfiguration,
};
pub use room_eq::{
    ChannelDspChain, ChannelMeasurement, ChannelOptResult, CrossoverType, CustomTargetCurve,
    DriverDspChain, DspChainMetadata, DspChainOutput, DspPluginConfig, EqFilterConfig,
    MultiSpeakerMode, OptimizationStatus, RecordingConfiguration, RoomEqAlgorithm,
    RoomEqDataSource, RoomEqMeasurementsFile, RoomEqOptimizationMode, RoomEqOptimizerConfig,
    RoomEqSpeakerConfig, RoomEqState, RoomEqStep, SpeakerConfigType, TargetCurveControlPoint,
};
pub use settings::{ScanProgressModal, ScanType, SettingsTab};
pub use spinorama_eq::{
    DirectivityCurve, SpinoramaBiquad, SpinoramaCurves, SpinoramaEqResult, SpinoramaEqState,
    SpinoramaOptimizationMode, SpinoramaStep, SpinoramaTargetCurve,
};
pub use stats::LibraryStats;
pub use toast::{ToastMessage, ToastType};
