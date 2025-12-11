//! Type definitions for the GPUI audio player application.
//!
//! Contains enums and simple structs used throughout the application.

use std::time::{Duration, Instant};

use sotf_audio_player::{Album, Track};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Library,
    DirectoryManager,
    Queue,
    Spectrum,
    Settings,
    Recording,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayGainMode {
    Track,
    Album,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    AddDirectory,
    EditPlugin,
    SavePlugins,
    LoadPlugins,
    LoadApoFile,
    LoadSofaFile,
    Help,
    KeyboardShortcuts,
    About,
    EditingParam,
}

/// Active menu dropdown (if any)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveMenu {
    None,
    File,
    View,
    Help,
}

/// Layout mode based on window height
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Compact,  // Below 800px - tabs bar visible
    Expanded, // Above 800px - split Library/Queue view
}

/// Settings screen tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Library,
    Appearance,
    AudioDevice,
    Plugins,
    RoomEQ,
    Headphone,
    Spinorama,
}

/// Toast message type for color coding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastType {
    Success,
    Error,
    Info,
    Warning,
}

/// Toast message with type and timing
#[derive(Debug, Clone)]
pub struct ToastMessage {
    pub message: String,
    pub toast_type: ToastType,
    pub created_at: Instant,
    pub auto_dismiss_ms: Option<u64>, // None = no auto-dismiss
}

impl ToastMessage {
    pub fn new(message: String, toast_type: ToastType) -> Self {
        Self {
            message,
            toast_type,
            created_at: Instant::now(),
            auto_dismiss_ms: Some(5000), // Default 5 seconds
        }
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message.into(), ToastType::Success)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message.into(), ToastType::Error)
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(message.into(), ToastType::Info)
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message.into(), ToastType::Warning)
    }

    pub fn persistent(message: impl Into<String>, toast_type: ToastType) -> Self {
        Self {
            message: message.into(),
            toast_type,
            created_at: Instant::now(),
            auto_dismiss_ms: None, // No auto-dismiss
        }
    }

    pub fn should_dismiss(&self) -> bool {
        if let Some(dismiss_ms) = self.auto_dismiss_ms {
            self.created_at.elapsed() > Duration::from_millis(dismiss_ms)
        } else {
            false
        }
    }
}

// Enums mapped from library
pub use sotf_audio_player::library::{ChannelFilter, LibrarySortOrder};

#[derive(Debug)]
pub struct QueueItem {
    pub album: Album,
    pub current_track_index: usize,
}

impl QueueItem {
    pub fn new(album: Album) -> Self {
        Self {
            album,
            current_track_index: 0,
        }
    }

    pub fn current_track(&self) -> Option<&Track> {
        self.album.tracks.get(self.current_track_index)
    }

    pub fn next_track(&mut self) -> Option<&Track> {
        if self.current_track_index + 1 < self.album.tracks.len() {
            self.current_track_index += 1;
            self.current_track()
        } else {
            None
        }
    }

    pub fn previous_track(&mut self) -> Option<&Track> {
        if self.current_track_index > 0 {
            self.current_track_index -= 1;
            self.current_track()
        } else {
            None
        }
    }
}

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

// ============================================================================
// Recording Screen Types
// ============================================================================

/// Recording screen workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingStep {
    /// Step 1: Configure devices and channel mapping
    Config,
    /// Step 2: Record frequency response for each channel
    Capture,
}

/// State of a single channel's recording
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelRecordingState {
    /// Not yet recorded
    Empty,
    /// Currently recording
    Recording,
    /// Successfully recorded
    Done,
    /// Recording failed
    Error,
}

/// Configuration for a single channel mapping
#[derive(Debug, Clone)]
pub struct ChannelMapping {
    /// Physical channel index on the interface
    pub interface_channel: usize,
    /// Channel group name (e.g., "L", "R", "C", "LFE", "SL", "SR")
    pub group_name: String,
}

/// Playback device configuration
#[derive(Debug, Clone)]
pub struct PlaybackDeviceConfig {
    pub device_id: String,
    pub device_name: String,
    pub num_channels: usize,
    pub sample_rate: u32,
    pub channel_mappings: Vec<ChannelMapping>,
}

impl Default for PlaybackDeviceConfig {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            device_name: String::new(),
            num_channels: 2,
            sample_rate: 48000,
            channel_mappings: vec![
                ChannelMapping {
                    interface_channel: 0,
                    group_name: "L".to_string(),
                },
                ChannelMapping {
                    interface_channel: 1,
                    group_name: "R".to_string(),
                },
            ],
        }
    }
}

/// Recording device configuration
#[derive(Debug, Clone)]
pub struct RecordingDeviceConfig {
    pub device_id: String,
    pub device_name: String,
    pub num_channels: usize,
    pub sample_rate: u32,
    /// Mapping from physical input channels to recording channels
    pub channel_mappings: Vec<usize>,
}

impl Default for RecordingDeviceConfig {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            device_name: String::new(),
            num_channels: 1,
            sample_rate: 48000,
            channel_mappings: vec![0],
        }
    }
}

/// Recording for a single channel with results
#[derive(Debug, Clone)]
pub struct ChannelRecording {
    pub channel_index: usize,
    pub channel_name: String,
    pub state: ChannelRecordingState,
    pub result: Option<RecordingResult>,
}

/// Result of a single channel recording
#[derive(Debug, Clone)]
pub struct RecordingResult {
    pub channel: usize,
    pub wav_path: Option<String>,
    pub csv_path: Option<String>,
    pub frequencies: Vec<f32>,
    pub magnitude_db: Vec<f32>,
    pub phase_deg: Vec<f32>,
}

/// Signal type for test signal generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingSignalType {
    Sweep,
    WhiteNoise,
    PinkNoise,
}

impl RecordingSignalType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecordingSignalType::Sweep => "Sweep",
            RecordingSignalType::WhiteNoise => "White Noise",
            RecordingSignalType::PinkNoise => "Pink Noise",
        }
    }

    pub fn all() -> &'static [RecordingSignalType] {
        &[
            RecordingSignalType::Sweep,
            RecordingSignalType::WhiteNoise,
            RecordingSignalType::PinkNoise,
        ]
    }
}

/// Complete recording screen state
#[derive(Debug, Clone)]
pub struct RecordingState {
    /// Current step in the recording workflow
    pub step: RecordingStep,

    // === Config Step State ===
    pub playback_config: PlaybackDeviceConfig,
    pub recording_config: RecordingDeviceConfig,
    pub mic_calibration_path: Option<String>,

    // === Capture Step State ===
    pub signal_type: RecordingSignalType,
    pub signal_duration_secs: f32,
    pub signal_level_db: f32,
    pub channel_recordings: Vec<ChannelRecording>,
    pub current_recording_channel: Option<usize>,
    pub recording_progress: f32,
    pub status_message: String,

    // === UI State ===
    pub playback_device_dropdown_open: bool,
    pub recording_device_dropdown_open: bool,
    pub signal_type_dropdown_open: bool,
    pub duration_dropdown_open: bool,
}

impl Default for RecordingState {
    fn default() -> Self {
        Self {
            step: RecordingStep::Config,
            playback_config: PlaybackDeviceConfig::default(),
            recording_config: RecordingDeviceConfig::default(),
            mic_calibration_path: None,
            signal_type: RecordingSignalType::Sweep,
            signal_duration_secs: 5.0,
            signal_level_db: -20.0,
            channel_recordings: Vec::new(),
            current_recording_channel: None,
            recording_progress: 0.0,
            status_message: String::new(),
            playback_device_dropdown_open: false,
            recording_device_dropdown_open: false,
            signal_type_dropdown_open: false,
            duration_dropdown_open: false,
        }
    }
}

impl RecordingState {
    /// Initialize channel recordings from playback config
    pub fn init_channel_recordings(&mut self) {
        self.channel_recordings = self
            .playback_config
            .channel_mappings
            .iter()
            .enumerate()
            .map(|(idx, mapping)| ChannelRecording {
                channel_index: idx,
                channel_name: mapping.group_name.clone(),
                state: ChannelRecordingState::Empty,
                result: None,
            })
            .collect();
    }

    /// Check if all channels have been recorded
    pub fn all_channels_recorded(&self) -> bool {
        !self.channel_recordings.is_empty()
            && self
                .channel_recordings
                .iter()
                .all(|r| r.state == ChannelRecordingState::Done)
    }

    /// Check if any recording is in progress
    pub fn is_recording(&self) -> bool {
        self.current_recording_channel.is_some()
    }
}
