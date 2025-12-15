//! Type definitions for the GPUI audio player application.
//!
//! Contains enums and simple structs used throughout the application.

use serde::{Deserialize, Serialize};
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
    RoomEq,
    HeadphoneEq,
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
    /// Step 3: Evaluate recordings and view frequency response
    Evaluating,
    /// Step 4: Save recordings to disk
    Saving,
}

/// Smoothing options for frequency response plots (1/N octave)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlotSmoothing {
    /// No smoothing (raw data)
    #[default]
    None,
    /// 1/1 octave smoothing
    Octave1,
    /// 1/3 octave smoothing
    Octave3,
    /// 1/6 octave smoothing
    Octave6,
    /// 1/24 octave smoothing
    Octave24,
}

impl PlotSmoothing {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlotSmoothing::None => "None",
            PlotSmoothing::Octave1 => "1/1 octave",
            PlotSmoothing::Octave3 => "1/3 octave",
            PlotSmoothing::Octave6 => "1/6 octave",
            PlotSmoothing::Octave24 => "1/24 octave",
        }
    }

    /// Get the smoothing factor (fraction of octave)
    pub fn octave_fraction(&self) -> Option<f32> {
        match self {
            PlotSmoothing::None => None,
            PlotSmoothing::Octave1 => Some(1.0),
            PlotSmoothing::Octave3 => Some(1.0 / 3.0),
            PlotSmoothing::Octave6 => Some(1.0 / 6.0),
            PlotSmoothing::Octave24 => Some(1.0 / 24.0),
        }
    }
}

/// State of a single channel's recording
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    pub available_sample_rates: Vec<u32>,
    pub speaker_configuration: SpeakerConfiguration,
    pub channel_mappings: Vec<ChannelMapping>,
}

impl Default for PlaybackDeviceConfig {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            device_name: String::new(),
            num_channels: 2,
            sample_rate: 48000,
            available_sample_rates: vec![44100, 48000, 88200, 96000, 176400, 192000],
            speaker_configuration: SpeakerConfiguration::Stereo,
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
    pub available_sample_rates: Vec<u32>,
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
            available_sample_rates: vec![44100, 48000, 88200, 96000, 176400, 192000],
            channel_mappings: vec![0],
        }
    }
}

/// Recording for a single channel with results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelRecording {
    pub channel_index: usize,
    pub channel_name: String,
    pub state: ChannelRecordingState,
    pub result: Option<RecordingResult>,
}

/// Result of a single channel recording
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Speaker configuration presets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeakerConfiguration {
    Stereo,         // 2.0
    Stereo21,       // 2.1
    Surround50,     // 5.0
    Surround51,     // 5.1
    Surround71,     // 7.1
    Surround91,     // 9.1
    Atmos512,       // 5.1.2
    Atmos514,       // 5.1.4
    Atmos712,       // 7.1.2
    Atmos714,       // 7.1.4
    Atmos912,       // 9.1.2
    Atmos914,       // 9.1.4
    Atmos916,       // 9.1.6
    Custom,         // User-defined
}

impl SpeakerConfiguration {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpeakerConfiguration::Stereo => "2.0",
            SpeakerConfiguration::Stereo21 => "2.1",
            SpeakerConfiguration::Surround50 => "5.0",
            SpeakerConfiguration::Surround51 => "5.1",
            SpeakerConfiguration::Surround71 => "7.1",
            SpeakerConfiguration::Surround91 => "9.1",
            SpeakerConfiguration::Atmos512 => "5.1.2",
            SpeakerConfiguration::Atmos514 => "5.1.4",
            SpeakerConfiguration::Atmos712 => "7.1.2",
            SpeakerConfiguration::Atmos714 => "7.1.4",
            SpeakerConfiguration::Atmos912 => "9.1.2",
            SpeakerConfiguration::Atmos914 => "9.1.4",
            SpeakerConfiguration::Atmos916 => "9.1.6",
            SpeakerConfiguration::Custom => "Custom",
        }
    }

    pub fn all() -> &'static [SpeakerConfiguration] {
        &[
            SpeakerConfiguration::Stereo,
            SpeakerConfiguration::Stereo21,
            SpeakerConfiguration::Surround50,
            SpeakerConfiguration::Surround51,
            SpeakerConfiguration::Surround71,
            SpeakerConfiguration::Surround91,
            SpeakerConfiguration::Atmos512,
            SpeakerConfiguration::Atmos514,
            SpeakerConfiguration::Atmos712,
            SpeakerConfiguration::Atmos714,
            SpeakerConfiguration::Atmos912,
            SpeakerConfiguration::Atmos914,
            SpeakerConfiguration::Atmos916,
            SpeakerConfiguration::Custom,
        ]
    }

    /// Get the number of channels for this configuration
    pub fn channel_count(&self) -> usize {
        match self {
            SpeakerConfiguration::Stereo => 2,
            SpeakerConfiguration::Stereo21 => 3,
            SpeakerConfiguration::Surround50 => 5,
            SpeakerConfiguration::Surround51 => 6,
            SpeakerConfiguration::Surround71 => 8,
            SpeakerConfiguration::Surround91 => 10,
            SpeakerConfiguration::Atmos512 => 8,
            SpeakerConfiguration::Atmos514 => 10,
            SpeakerConfiguration::Atmos712 => 10,
            SpeakerConfiguration::Atmos714 => 12,
            SpeakerConfiguration::Atmos912 => 12,
            SpeakerConfiguration::Atmos914 => 14,
            SpeakerConfiguration::Atmos916 => 16,
            SpeakerConfiguration::Custom => 2, // Default for custom
        }
    }

    /// Get the default channel names for this configuration
    pub fn default_channel_names(&self) -> Vec<&'static str> {
        match self {
            SpeakerConfiguration::Stereo => vec!["L", "R"],
            SpeakerConfiguration::Stereo21 => vec!["L", "R", "LFE"],
            SpeakerConfiguration::Surround50 => vec!["L", "R", "C", "SL", "SR"],
            SpeakerConfiguration::Surround51 => vec!["L", "R", "C", "LFE", "SL", "SR"],
            SpeakerConfiguration::Surround71 => vec!["L", "R", "C", "LFE", "SL", "SR", "BL", "BR"],
            SpeakerConfiguration::Surround91 => vec!["L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "WL", "WR"],
            SpeakerConfiguration::Atmos512 => vec!["L", "R", "C", "LFE", "SL", "SR", "TFL", "TFR"],
            SpeakerConfiguration::Atmos514 => vec!["L", "R", "C", "LFE", "SL", "SR", "TFL", "TFR", "TBL", "TBR"],
            SpeakerConfiguration::Atmos712 => vec!["L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "TFL", "TFR"],
            SpeakerConfiguration::Atmos714 => vec!["L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "TFL", "TFR", "TBL", "TBR"],
            SpeakerConfiguration::Atmos912 => vec!["L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "WL", "WR", "TFL", "TFR"],
            SpeakerConfiguration::Atmos914 => vec!["L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "WL", "WR", "TFL", "TFR", "TBL", "TBR"],
            SpeakerConfiguration::Atmos916 => vec!["L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "WL", "WR", "TFL", "TFR", "TML", "TMR", "TBL", "TBR"],
            SpeakerConfiguration::Custom => vec!["L", "R"],
        }
    }

    /// Try to detect configuration from channel count
    pub fn from_channel_count(count: usize) -> Self {
        match count {
            2 => SpeakerConfiguration::Stereo,
            3 => SpeakerConfiguration::Stereo21,
            5 => SpeakerConfiguration::Surround50,
            6 => SpeakerConfiguration::Surround51,
            8 => SpeakerConfiguration::Surround71,
            10 => SpeakerConfiguration::Surround91,
            _ => SpeakerConfiguration::Custom,
        }
    }
}

/// Parsed microphone calibration data
#[derive(Debug, Clone, Default)]
pub struct CalibrationData {
    /// Frequency points in Hz
    pub frequencies: Vec<f64>,
    /// SPL deviation in dB (positive = mic reads louder)
    pub spl_db: Vec<f64>,
}

impl CalibrationData {
    /// Parse a calibration file from its contents
    pub fn parse(content: &str) -> Option<Self> {
        let mut frequencies = Vec::new();
        let mut spl_db = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            // Skip empty lines, comments, and headers
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }
            // Skip header lines containing text like "frequency" or "hz"
            let lower = line.to_lowercase();
            if lower.contains("frequency") || lower.contains("spl") || lower.contains("hz") {
                continue;
            }

            // Split by comma, tab, or whitespace
            let parts: Vec<&str> = line
                .split(|c| c == ',' || c == '\t' || c == ' ')
                .filter(|s| !s.is_empty())
                .collect();

            if parts.len() >= 2 {
                if let (Ok(freq), Ok(spl)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                    // Validate reasonable frequency range (1 Hz to 100 kHz)
                    if freq > 0.0 && freq <= 100000.0 && spl.is_finite() {
                        frequencies.push(freq);
                        spl_db.push(spl);
                    }
                }
            }
        }

        if frequencies.is_empty() {
            None
        } else {
            Some(Self { frequencies, spl_db })
        }
    }

    /// Check if calibration data is valid
    pub fn is_valid(&self) -> bool {
        !self.frequencies.is_empty() && self.frequencies.len() == self.spl_db.len()
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
    /// Parsed calibration data for display
    pub mic_calibration_data: Option<CalibrationData>,

    // === Capture Step State ===
    pub signal_type: RecordingSignalType,
    pub signal_duration_secs: f32,
    pub signal_level_db: f32,
    pub channel_recordings: Vec<ChannelRecording>,
    pub current_recording_channel: Option<usize>,
    pub recording_progress: f32,
    pub status_message: String,
    pub auto_record_remaining: bool, // Whether to automatically record all remaining channels

    /// Directory where recordings will be stored
    /// Format: user_selected_dir/recording-YYYYMMDD-HHMMSS/
    pub recording_directory: Option<String>,
    /// Base directory selected by user (before adding timestamp subdirectory)
    pub recording_base_directory: Option<String>,

    // === UI State ===
    pub playback_device_dropdown_open: bool,
    pub recording_device_dropdown_open: bool,
    pub playback_sample_rate_dropdown_open: bool,
    pub recording_sample_rate_dropdown_open: bool,
    pub speaker_config_dropdown_open: bool,
    pub signal_type_dropdown_open: bool,
    pub duration_dropdown_open: bool,
    /// Track which channel name dropdown is open (by channel index)
    pub channel_name_dropdown_open: Option<usize>,
    /// Expanded accordion sections in config step
    pub config_accordion_expanded: Vec<gpui::SharedString>,

    // === Evaluating Step State ===
    /// Selected channel filter for plots (None = all channels)
    pub plot_selected_channel: Option<usize>,
    /// Smoothing option for frequency response plots
    pub plot_smoothing: PlotSmoothing,
    /// Channel selector dropdown open
    pub plot_channel_dropdown_open: bool,
    /// Smoothing selector dropdown open
    pub plot_smoothing_dropdown_open: bool,

    // === Saving Step State ===
    /// Name for the recording session (used as subdirectory name)
    pub save_name: String,
}

impl Default for RecordingState {
    fn default() -> Self {
        Self {
            step: RecordingStep::Config,
            playback_config: PlaybackDeviceConfig::default(),
            recording_config: RecordingDeviceConfig::default(),
            mic_calibration_path: None,
            mic_calibration_data: None,
            signal_type: RecordingSignalType::Sweep,
            signal_duration_secs: 5.0,
            signal_level_db: -20.0,
            channel_recordings: Vec::new(),
            current_recording_channel: None,
            recording_progress: 0.0,
            status_message: String::new(),
            auto_record_remaining: false,
            recording_directory: None,
            recording_base_directory: None,
            playback_device_dropdown_open: false,
            recording_device_dropdown_open: false,
            playback_sample_rate_dropdown_open: false,
            recording_sample_rate_dropdown_open: false,
            speaker_config_dropdown_open: false,
            signal_type_dropdown_open: false,
            duration_dropdown_open: false,
            channel_name_dropdown_open: None,
            config_accordion_expanded: vec!["playback".into()], // Playback section open by default
            plot_selected_channel: None, // All channels
            plot_smoothing: PlotSmoothing::None,
            plot_channel_dropdown_open: false,
            plot_smoothing_dropdown_open: false,
            save_name: "recording".to_string(),
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

// ============================================================================
// Room EQ Screen Types
// ============================================================================

/// Room EQ workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoomEqStep {
    /// Step 1: Load/import measurement data
    #[default]
    LoadData,
    /// Step 2: Configure channels and optimizer settings
    Configure,
    /// Step 3: Run optimization (per-channel, then combined)
    Optimize,
    /// Step 4: Review results and visualizations
    Review,
    /// Step 5: Export DSP chain and apply
    Export,
}

impl RoomEqStep {
    /// Get all steps in order
    pub fn all() -> &'static [RoomEqStep] {
        &[
            RoomEqStep::LoadData,
            RoomEqStep::Configure,
            RoomEqStep::Optimize,
            RoomEqStep::Review,
            RoomEqStep::Export,
        ]
    }

    /// Get step index (0-based)
    pub fn index(&self) -> usize {
        match self {
            RoomEqStep::LoadData => 0,
            RoomEqStep::Configure => 1,
            RoomEqStep::Optimize => 2,
            RoomEqStep::Review => 3,
            RoomEqStep::Export => 4,
        }
    }

    /// Get step label
    pub fn label(&self) -> &'static str {
        match self {
            RoomEqStep::LoadData => "Load Data",
            RoomEqStep::Configure => "Configure",
            RoomEqStep::Optimize => "Optimize",
            RoomEqStep::Review => "Review",
            RoomEqStep::Export => "Export",
        }
    }

    /// Get next step
    pub fn next(&self) -> Option<RoomEqStep> {
        match self {
            RoomEqStep::LoadData => Some(RoomEqStep::Configure),
            RoomEqStep::Configure => Some(RoomEqStep::Optimize),
            RoomEqStep::Optimize => Some(RoomEqStep::Review),
            RoomEqStep::Review => Some(RoomEqStep::Export),
            RoomEqStep::Export => None,
        }
    }

    /// Get previous step
    pub fn previous(&self) -> Option<RoomEqStep> {
        match self {
            RoomEqStep::LoadData => None,
            RoomEqStep::Configure => Some(RoomEqStep::LoadData),
            RoomEqStep::Optimize => Some(RoomEqStep::Configure),
            RoomEqStep::Review => Some(RoomEqStep::Optimize),
            RoomEqStep::Export => Some(RoomEqStep::Review),
        }
    }
}

/// Source of measurement data for Room EQ
#[derive(Debug, Clone, PartialEq)]
pub enum RoomEqDataSource {
    /// Use recordings from current session (RecordingState)
    FromRecording,
    /// Loaded from a JSON file
    FromFile(std::path::PathBuf),
}

impl Default for RoomEqDataSource {
    fn default() -> Self {
        RoomEqDataSource::FromRecording
    }
}

/// Recording configuration stored with measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfiguration {
    /// Playback device name
    pub playback_device_name: String,
    /// Playback device ID
    pub playback_device_id: String,
    /// Playback sample rate
    pub playback_sample_rate: u32,
    /// Playback channel count
    pub playback_channels: usize,
    /// Speaker configuration (e.g., "5.1", "7.1.4")
    pub speaker_configuration: String,
    /// Channel names in order
    pub channel_names: Vec<String>,

    /// Recording device name
    pub recording_device_name: String,
    /// Recording device ID
    pub recording_device_id: String,
    /// Recording sample rate
    pub recording_sample_rate: u32,
    /// Recording channel count
    pub recording_channels: usize,

    /// Microphone calibration file path (if used)
    pub mic_calibration_path: Option<String>,
    /// Recording output directory
    pub recording_directory: Option<String>,

    /// Signal type used for measurements
    pub signal_type: String,
    /// Signal duration in seconds
    pub signal_duration_secs: f32,
    /// Signal level in dB
    pub signal_level_db: f32,
}

/// File format for saving/loading room EQ measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqMeasurementsFile {
    /// File format version
    pub version: u32,
    /// Channel measurements
    pub channels: Vec<ChannelMeasurement>,
    /// Recording configuration (devices, settings used)
    #[serde(default)]
    pub configuration: Option<RecordingConfiguration>,
}

impl RoomEqMeasurementsFile {
    pub const CURRENT_VERSION: u32 = 2;

    pub fn new(channels: Vec<ChannelMeasurement>) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            channels,
            configuration: None,
        }
    }

    pub fn with_configuration(
        channels: Vec<ChannelMeasurement>,
        configuration: RecordingConfiguration,
    ) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            channels,
            configuration: Some(configuration),
        }
    }
}

/// Measurement data for a single channel (may have multiple drivers)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMeasurement {
    /// Channel name (e.g., "L", "R", "C")
    pub channel_name: String,
    /// Primary measurement (single driver or combined)
    pub measurement: RecordingResult,
    /// Whether this is a multi-driver setup
    pub is_group: bool,
    /// Individual driver measurements (for multi-driver)
    pub group_drivers: Vec<RecordingResult>,
}

/// Speaker configuration type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SpeakerConfigType {
    /// Single full-range driver or measurement
    #[default]
    Single,
    /// Multi-driver with active crossover
    MultiDriver,
}

/// Crossover type for multi-driver speakers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CrossoverType {
    /// Linkwitz-Riley 2nd order (12dB/octave)
    LR12,
    /// Linkwitz-Riley 4th order (24dB/octave)
    #[default]
    LR24,
    /// Linkwitz-Riley 8th order (48dB/octave)
    LR48,
    /// Butterworth 2nd order (12dB/octave)
    Butterworth12,
    /// Butterworth 4th order (24dB/octave)
    Butterworth24,
}

impl CrossoverType {
    pub fn all() -> &'static [CrossoverType] {
        &[
            CrossoverType::LR12,
            CrossoverType::LR24,
            CrossoverType::LR48,
            CrossoverType::Butterworth12,
            CrossoverType::Butterworth24,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            CrossoverType::LR12 => "Linkwitz-Riley 12dB",
            CrossoverType::LR24 => "Linkwitz-Riley 24dB",
            CrossoverType::LR48 => "Linkwitz-Riley 48dB",
            CrossoverType::Butterworth12 => "Butterworth 12dB",
            CrossoverType::Butterworth24 => "Butterworth 24dB",
        }
    }
}

/// Configuration for a speaker channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqSpeakerConfig {
    /// Channel name
    pub channel_name: String,
    /// Single or multi-driver
    pub config_type: SpeakerConfigType,
    /// Crossover type (for multi-driver)
    pub crossover_type: CrossoverType,
    /// Driver names (for multi-driver), e.g., ["woofer", "tweeter"]
    pub driver_names: Vec<String>,
    /// Initial crossover frequency hints (for multi-driver)
    pub crossover_freq_hints: Vec<f64>,
}

impl Default for RoomEqSpeakerConfig {
    fn default() -> Self {
        Self {
            channel_name: String::new(),
            config_type: SpeakerConfigType::Single,
            crossover_type: CrossoverType::LR24,
            driver_names: Vec::new(),
            crossover_freq_hints: Vec::new(),
        }
    }
}

/// Optimization algorithm selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoomEqAlgorithm {
    /// COBYLA (Constrained Optimization BY Linear Approximations)
    #[default]
    Cobyla,
    /// Differential Evolution
    DifferentialEvolution,
    /// Nelder-Mead simplex
    NelderMead,
}

impl RoomEqAlgorithm {
    pub fn all() -> &'static [RoomEqAlgorithm] {
        &[
            RoomEqAlgorithm::Cobyla,
            RoomEqAlgorithm::DifferentialEvolution,
            RoomEqAlgorithm::NelderMead,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RoomEqAlgorithm::Cobyla => "COBYLA",
            RoomEqAlgorithm::DifferentialEvolution => "Differential Evolution",
            RoomEqAlgorithm::NelderMead => "Nelder-Mead",
        }
    }

    pub fn to_autoeq_string(&self) -> &'static str {
        match self {
            RoomEqAlgorithm::Cobyla => "cobyla",
            RoomEqAlgorithm::DifferentialEvolution => "autoeq:de",
            RoomEqAlgorithm::NelderMead => "nelder-mead",
        }
    }
}

// === Type conversions for room_eq library ===

impl From<SpeakerConfigType> for sotf_audio_player::room_eq::SpeakerConfigType {
    fn from(val: SpeakerConfigType) -> Self {
        match val {
            SpeakerConfigType::Single => sotf_audio_player::room_eq::SpeakerConfigType::Single,
            SpeakerConfigType::MultiDriver => {
                sotf_audio_player::room_eq::SpeakerConfigType::MultiDriver
            }
        }
    }
}

impl From<CrossoverType> for sotf_audio_player::room_eq::CrossoverType {
    fn from(val: CrossoverType) -> Self {
        match val {
            CrossoverType::LR12 => sotf_audio_player::room_eq::CrossoverType::LR12,
            CrossoverType::LR24 => sotf_audio_player::room_eq::CrossoverType::LR24,
            CrossoverType::LR48 => sotf_audio_player::room_eq::CrossoverType::LR48,
            CrossoverType::Butterworth12 => {
                sotf_audio_player::room_eq::CrossoverType::Butterworth12
            }
            CrossoverType::Butterworth24 => {
                // Map to closest available - Butterworth12 (LR24 is closer behavior)
                sotf_audio_player::room_eq::CrossoverType::LR24
            }
        }
    }
}

impl From<RoomEqAlgorithm> for sotf_audio_player::room_eq::Algorithm {
    fn from(val: RoomEqAlgorithm) -> Self {
        match val {
            RoomEqAlgorithm::Cobyla => sotf_audio_player::room_eq::Algorithm::Cobyla,
            RoomEqAlgorithm::DifferentialEvolution => {
                sotf_audio_player::room_eq::Algorithm::DifferentialEvolution
            }
            RoomEqAlgorithm::NelderMead => sotf_audio_player::room_eq::Algorithm::NelderMead,
        }
    }
}

/// Optimizer configuration for Room EQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqOptimizerConfig {
    /// Optimization algorithm
    pub algorithm: RoomEqAlgorithm,
    /// Number of PEQ filters per channel
    pub num_filters: usize,
    /// Minimum Q factor
    pub min_q: f64,
    /// Maximum Q factor
    pub max_q: f64,
    /// Minimum gain in dB
    pub min_db: f64,
    /// Maximum gain in dB
    pub max_db: f64,
    /// Minimum frequency in Hz
    pub min_freq: f64,
    /// Maximum frequency in Hz
    pub max_freq: f64,
    /// Maximum number of iterations
    pub max_iter: usize,
}

impl Default for RoomEqOptimizerConfig {
    fn default() -> Self {
        Self {
            algorithm: RoomEqAlgorithm::Cobyla,
            num_filters: 10,
            min_q: 0.5,
            max_q: 10.0,
            min_db: -12.0,
            max_db: 12.0,
            min_freq: 20.0,
            max_freq: 20000.0,
            max_iter: 10000,
        }
    }
}

/// Optimization status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptimizationStatus {
    /// Not started
    #[default]
    Idle,
    /// Currently running
    Running,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed,
    /// Cancelled by user
    Cancelled,
}

/// EQ filter configuration (for display and export)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqFilterConfig {
    /// Filter type (peak, lowshelf, highshelf)
    pub filter_type: String,
    /// Center frequency in Hz
    pub frequency: f64,
    /// Q factor
    pub q: f64,
    /// Gain in dB
    pub gain_db: f64,
}

/// Optimization result for a single channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelOptResult {
    /// Channel name
    pub channel_name: String,
    /// Pre-optimization score (RMS error)
    pub pre_score: f64,
    /// Post-optimization score
    pub post_score: f64,
    /// Optimized EQ filters
    pub eq_filters: Vec<EqFilterConfig>,
    /// Optimized crossover frequencies (for multi-driver)
    pub crossover_freqs: Option<Vec<f64>>,
    /// Optimized driver gains in dB (for multi-driver)
    pub driver_gains: Option<Vec<f64>>,
    /// Original frequency response
    pub original_response: Option<Vec<(f64, f64)>>,
    /// Corrected frequency response
    pub corrected_response: Option<Vec<(f64, f64)>>,
}

/// DSP chain output format (matches roomeq output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspChainOutput {
    /// Per-channel DSP chains
    pub channels: std::collections::HashMap<String, ChannelDspChain>,
    /// Optimization metadata
    pub metadata: Option<DspChainMetadata>,
}

/// DSP chain for a single channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelDspChain {
    /// Channel name
    pub channel: String,
    /// Ordered list of plugins
    pub plugins: Vec<DspPluginConfig>,
    /// Per-driver chains (for multi-driver)
    pub drivers: Option<Vec<DriverDspChain>>,
}

/// DSP chain for a driver in multi-driver setup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverDspChain {
    /// Driver name
    pub name: String,
    /// Driver index (0 = lowest frequency)
    pub index: usize,
    /// Plugins for this driver
    pub plugins: Vec<DspPluginConfig>,
}

/// DSP plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspPluginConfig {
    /// Plugin type (eq, gain, crossover)
    pub plugin_type: String,
    /// Plugin parameters as JSON
    pub parameters: serde_json::Value,
}

/// DSP chain metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspChainMetadata {
    /// Average pre-optimization score
    pub pre_score: f64,
    /// Average post-optimization score
    pub post_score: f64,
    /// Algorithm used
    pub algorithm: String,
    /// Number of iterations
    pub iterations: usize,
    /// Timestamp
    pub timestamp: String,
}

/// UI state for Room EQ dropdowns
#[derive(Debug, Clone, Default)]
pub struct RoomEqDropdowns {
    pub data_source_open: bool,
    pub algorithm_open: bool,
    pub crossover_type_open: bool,
    pub export_format_open: bool,
    /// AutoEQ form editing state
    pub autoeq_editing_field: Option<AutoEqField>,
    /// AutoEQ form edit text
    pub autoeq_edit_text: String,
}

/// Field identifiers for AutoEQ form editing (legacy compatibility)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoEqField {
    NumFilters,
    MinQ,
    MaxQ,
    MinDb,
    MaxDb,
    MinFreq,
    MaxFreq,
    MaxIter,
}

/// Complete Room EQ screen state
#[derive(Debug, Clone)]
pub struct RoomEqState {
    /// Current step in the workflow
    pub step: RoomEqStep,

    // === Step 1: Load Data ===
    /// Source of measurement data
    pub data_source: RoomEqDataSource,
    /// Loaded channel measurements
    pub channel_measurements: Vec<ChannelMeasurement>,

    // === Step 2: Configuration ===
    /// Per-channel speaker configurations
    pub speaker_configs: Vec<RoomEqSpeakerConfig>,
    /// Global optimizer configuration
    pub optimizer_config: RoomEqOptimizerConfig,

    // === Step 3: Optimization ===
    /// Current optimization status
    pub optimization_status: OptimizationStatus,
    /// Currently optimizing channel name
    pub current_channel: Option<String>,
    /// Per-channel optimization results
    pub channel_results: Vec<ChannelOptResult>,
    /// Overall progress (0.0 - 1.0)
    pub overall_progress: f32,

    // === Step 5: Export ===
    /// Generated DSP chain output
    pub dsp_output: Option<DspChainOutput>,

    // === UI State ===
    pub dropdowns: RoomEqDropdowns,
    pub status_message: String,
    pub error_message: Option<String>,
}

impl Default for RoomEqState {
    fn default() -> Self {
        Self {
            step: RoomEqStep::LoadData,
            data_source: RoomEqDataSource::FromRecording,
            channel_measurements: Vec::new(),
            speaker_configs: Vec::new(),
            optimizer_config: RoomEqOptimizerConfig::default(),
            optimization_status: OptimizationStatus::Idle,
            current_channel: None,
            channel_results: Vec::new(),
            overall_progress: 0.0,
            dsp_output: None,
            dropdowns: RoomEqDropdowns::default(),
            status_message: String::new(),
            error_message: None,
        }
    }
}

impl RoomEqState {
    /// Check if we have measurement data loaded
    pub fn has_measurements(&self) -> bool {
        !self.channel_measurements.is_empty()
    }

    /// Get the number of channels
    pub fn channel_count(&self) -> usize {
        self.channel_measurements.len()
    }

    /// Check if optimization is complete
    pub fn is_optimization_complete(&self) -> bool {
        self.optimization_status == OptimizationStatus::Completed
    }

    /// Check if optimization is running
    pub fn is_optimizing(&self) -> bool {
        self.optimization_status == OptimizationStatus::Running
    }

    /// Initialize speaker configs from measurements
    pub fn init_speaker_configs(&mut self) {
        self.speaker_configs = self
            .channel_measurements
            .iter()
            .map(|m| RoomEqSpeakerConfig {
                channel_name: m.channel_name.clone(),
                config_type: if m.is_group {
                    SpeakerConfigType::MultiDriver
                } else {
                    SpeakerConfigType::Single
                },
                crossover_type: CrossoverType::LR24,
                driver_names: if m.is_group {
                    m.group_drivers
                        .iter()
                        .enumerate()
                        .map(|(i, _)| format!("driver_{}", i + 1))
                        .collect()
                } else {
                    Vec::new()
                },
                crossover_freq_hints: Vec::new(),
            })
            .collect();
    }

    /// Load measurements from recording state
    pub fn load_from_recording(&mut self, recording_state: &RecordingState) {
        self.channel_measurements = recording_state
            .channel_recordings
            .iter()
            .filter_map(|r| {
                r.result.as_ref().map(|result| ChannelMeasurement {
                    channel_name: r.channel_name.clone(),
                    measurement: result.clone(),
                    is_group: false,
                    group_drivers: Vec::new(),
                })
            })
            .collect();

        self.data_source = RoomEqDataSource::FromRecording;
        self.init_speaker_configs();
    }

    /// Reset optimization state
    pub fn reset_optimization(&mut self) {
        self.optimization_status = OptimizationStatus::Idle;
        self.current_channel = None;
        self.channel_results.clear();
        self.overall_progress = 0.0;
        self.error_message = None;
    }

    /// Get average pre-score
    pub fn average_pre_score(&self) -> f64 {
        if self.channel_results.is_empty() {
            0.0
        } else {
            self.channel_results.iter().map(|r| r.pre_score).sum::<f64>()
                / self.channel_results.len() as f64
        }
    }

    /// Get average post-score
    pub fn average_post_score(&self) -> f64 {
        if self.channel_results.is_empty() {
            0.0
        } else {
            self.channel_results.iter().map(|r| r.post_score).sum::<f64>()
                / self.channel_results.len() as f64
        }
    }
}

// ============================================================================
// Headphone EQ Screen Types
// ============================================================================

/// Headphone EQ workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeadphoneEqStep {
    /// Step 1: Select measurement file and target curve
    #[default]
    MeasurementTarget,
    /// Step 2: EQ design, fine tuning, and generate EQ
    Optimization,
    /// Step 3: Preview and apply EQ to playback
    Listen,
    /// Step 4: Export format selection and save
    Save,
}

impl HeadphoneEqStep {
    /// Get all steps in order
    pub fn all() -> &'static [HeadphoneEqStep] {
        &[
            HeadphoneEqStep::MeasurementTarget,
            HeadphoneEqStep::Optimization,
            HeadphoneEqStep::Listen,
            HeadphoneEqStep::Save,
        ]
    }

    /// Get step index (0-based)
    pub fn index(&self) -> usize {
        match self {
            HeadphoneEqStep::MeasurementTarget => 0,
            HeadphoneEqStep::Optimization => 1,
            HeadphoneEqStep::Listen => 2,
            HeadphoneEqStep::Save => 3,
        }
    }

    /// Get step label
    pub fn label(&self) -> &'static str {
        match self {
            HeadphoneEqStep::MeasurementTarget => "Measurement",
            HeadphoneEqStep::Optimization => "Optimization",
            HeadphoneEqStep::Listen => "Listen",
            HeadphoneEqStep::Save => "Save",
        }
    }

    /// Get next step
    pub fn next(&self) -> Option<HeadphoneEqStep> {
        match self {
            HeadphoneEqStep::MeasurementTarget => Some(HeadphoneEqStep::Optimization),
            HeadphoneEqStep::Optimization => Some(HeadphoneEqStep::Listen),
            HeadphoneEqStep::Listen => Some(HeadphoneEqStep::Save),
            HeadphoneEqStep::Save => None,
        }
    }

    /// Get previous step
    pub fn previous(&self) -> Option<HeadphoneEqStep> {
        match self {
            HeadphoneEqStep::MeasurementTarget => None,
            HeadphoneEqStep::Optimization => Some(HeadphoneEqStep::MeasurementTarget),
            HeadphoneEqStep::Listen => Some(HeadphoneEqStep::Optimization),
            HeadphoneEqStep::Save => Some(HeadphoneEqStep::Listen),
        }
    }
}

/// Headphone EQ optimizer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadphoneEqOptimizerConfig {
    /// Optimization algorithm
    pub algorithm: RoomEqAlgorithm,
    /// Number of PEQ filters
    pub num_filters: usize,
    /// Minimum Q factor
    pub min_q: f64,
    /// Maximum Q factor
    pub max_q: f64,
    /// Minimum gain in dB
    pub min_db: f64,
    /// Maximum gain in dB
    pub max_db: f64,
    /// Minimum frequency in Hz
    pub min_freq: f64,
    /// Maximum frequency in Hz
    pub max_freq: f64,
    /// Maximum number of iterations
    pub max_iter: usize,
    /// Loss function
    pub loss: String,
}

impl Default for HeadphoneEqOptimizerConfig {
    fn default() -> Self {
        Self {
            algorithm: RoomEqAlgorithm::Cobyla,
            num_filters: 10,
            min_q: 0.5,
            max_q: 10.0,
            min_db: -12.0,
            max_db: 12.0,
            min_freq: 20.0,
            max_freq: 20000.0,
            max_iter: 10000,
            loss: "headphone-score".to_string(),
        }
    }
}

/// UI state for Headphone EQ dropdowns
#[derive(Debug, Clone, Default)]
pub struct HeadphoneEqDropdowns {
    pub target_open: bool,
    pub algorithm_open: bool,
    pub export_format_open: bool,
    /// AutoEQ form editing state
    pub autoeq_editing_field: Option<AutoEqField>,
    /// AutoEQ form edit text
    pub autoeq_edit_text: String,
}

/// Complete Headphone EQ screen state
#[derive(Debug, Clone)]
pub struct HeadphoneEqState {
    /// Current step in the workflow
    pub step: HeadphoneEqStep,

    // === Step 1: Select Files ===
    /// Path to headphone measurement file (CSV)
    pub measurement_path: Option<String>,
    /// Target curve selection (preset name or "custom")
    pub target_preset: String,
    /// Path to custom target file (if target_preset == "custom")
    pub custom_target_path: Option<String>,

    // === Step 2: Configuration ===
    /// Optimizer configuration
    pub optimizer_config: HeadphoneEqOptimizerConfig,

    // === Step 3: Optimization ===
    /// Current optimization status
    pub optimization_status: OptimizationStatus,
    /// Progress (0.0 - 1.0)
    pub progress: f32,
    /// Progress history for loss curve (iteration, loss)
    pub progress_history: Vec<(usize, f64)>,

    // === Step 4: Apply ===
    /// Optimization result (biquads, etc.)
    pub result: Option<HeadphoneEqResult>,
    /// Export format selection
    pub export_format: String,
    /// EQ preset name for saving
    pub save_name: String,

    // === UI State ===
    pub dropdowns: HeadphoneEqDropdowns,
    pub status_message: String,
    pub error_message: Option<String>,
    /// Expanded accordion sections
    pub expanded_sections: Vec<gpui::SharedString>,
}

/// Result of headphone EQ optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadphoneEqResult {
    /// Optimized biquad filters
    pub biquads: Vec<HeadphoneEqBiquad>,
    /// Pre-optimization score
    pub pre_score: f64,
    /// Post-optimization score
    pub post_score: f64,
    /// Original frequency response (for plotting)
    pub original_response: Option<Vec<(f64, f64)>>,
    /// Corrected frequency response (for plotting)
    pub corrected_response: Option<Vec<(f64, f64)>>,
    /// Target curve (for plotting)
    pub target_response: Option<Vec<(f64, f64)>>,
}

/// Biquad filter for headphone EQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadphoneEqBiquad {
    pub filter_type: String,
    pub freq: f64,
    pub q: f64,
    pub db_gain: f64,
}

impl Default for HeadphoneEqState {
    fn default() -> Self {
        Self {
            step: HeadphoneEqStep::MeasurementTarget,
            measurement_path: None,
            target_preset: "harman-over-ear-2018".to_string(),
            custom_target_path: None,
            optimizer_config: HeadphoneEqOptimizerConfig::default(),
            optimization_status: OptimizationStatus::Idle,
            progress: 0.0,
            progress_history: Vec::new(),
            result: None,
            export_format: "json".to_string(),
            save_name: String::new(),
            dropdowns: HeadphoneEqDropdowns::default(),
            status_message: String::new(),
            error_message: None,
            expanded_sections: vec![
                "measurement".into(),
                "target".into(),
                "eq-design".into(),
            ],
        }
    }
}

impl HeadphoneEqState {
    /// Check if we can proceed from the current step
    pub fn can_advance(&self) -> bool {
        match self.step {
            HeadphoneEqStep::MeasurementTarget => self.measurement_path.is_some(),
            HeadphoneEqStep::Optimization => {
                self.optimization_status == OptimizationStatus::Completed
            }
            HeadphoneEqStep::Listen => self.result.is_some(),
            HeadphoneEqStep::Save => true,
        }
    }

    /// Check if optimization is running
    pub fn is_optimizing(&self) -> bool {
        self.optimization_status == OptimizationStatus::Running
    }

    /// Reset optimization state
    pub fn reset_optimization(&mut self) {
        self.optimization_status = OptimizationStatus::Idle;
        self.progress = 0.0;
        self.progress_history.clear();
        self.result = None;
        self.error_message = None;
    }
}
