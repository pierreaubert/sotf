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
    KeyboardShortcuts,
    About,
    EditingParam,
    SpinoramaSpeakerSearch,
}

/// Active menu dropdown (if any)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveMenu {
    None,
    File,
    Show,
    Help,
}

/// Layout mode based on window height
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Compact,  // Below 800px - tabs bar visible
    Expanded, // Above 800px - split Library/Queue view
}

/// Meter display mode for Queue screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MeterDisplayMode {
    #[default]
    Lufs, // Show LUFS loudness meters
    Levels, // Show level meters
}

/// Settings screen tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Library,
    Appearance,
    AudioDevice,
}

/// Type of scan operation that can show a progress modal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanType {
    /// Library scan for audio files
    Library,
    /// ReplayGain analysis
    ReplayGain,
    /// Bliss audio analysis for similarity
    Bliss,
    /// Waveform generation
    Waveform,
}

impl ScanType {
    pub fn title(&self) -> &'static str {
        match self {
            ScanType::Library => "Library Scan",
            ScanType::ReplayGain => "ReplayGain Analysis",
            ScanType::Bliss => "Bliss Audio Analysis",
            ScanType::Waveform => "Waveform Generation",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ScanType::Library => "Scanning directories for audio files...",
            ScanType::ReplayGain => "Analyzing audio levels for normalization...",
            ScanType::Bliss => "Extracting audio features for similarity...",
            ScanType::Waveform => "Generating visual waveforms...",
        }
    }
}

/// State for the scan progress modal
#[derive(Debug, Clone)]
pub struct ScanProgressModal {
    /// Which type of scan is active
    pub scan_type: ScanType,
    /// Whether the modal is visible (can be dismissed to run in background)
    pub visible: bool,
}

impl ScanProgressModal {
    pub fn new(scan_type: ScanType) -> Self {
        Self {
            scan_type,
            visible: true,
        }
    }
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

/// Cached library statistics to avoid recomputing on every render frame.
/// These stats are expensive to compute (O(n) over all albums/tracks) and should
/// only be invalidated when the library actually changes.
#[derive(Debug, Clone, Default)]
pub struct LibraryStats {
    /// Number of unique artists (case-insensitive)
    pub artists_count: usize,
    /// Number of unique composers (case-insensitive)
    pub composers_count: usize,
    /// Total track count across all albums
    pub total_tracks: usize,
    /// Number of unique genres (case-insensitive)
    pub genres_count: usize,
    /// Count of albums per genre (for selection UI)
    pub genre_counts: std::collections::HashMap<String, usize>,
    /// Count of albums per year (for selection UI)
    pub year_counts: std::collections::HashMap<i32, usize>,
    /// Count of albums per decade (for selection UI) - key is (start_year, end_year)
    pub decade_counts: Vec<(i32, i32, usize)>,
    /// Count of albums per artist (for selection UI)
    pub artist_counts: std::collections::HashMap<String, usize>,
    /// Count of albums per artist first letter (for selection UI)
    pub artist_letter_counts: std::collections::HashMap<char, usize>,
    /// Count of albums per composer (for selection UI)
    pub composer_counts: std::collections::HashMap<String, usize>,
    /// Count of albums per composer first letter (for selection UI)
    pub composer_letter_counts: std::collections::HashMap<char, usize>,
    /// Count of albums per first letter of album name (for selection UI)
    pub album_letter_counts: std::collections::HashMap<char, usize>,
    /// Count of albums per track count range (for selection UI)
    pub track_range_counts: Vec<(usize, usize, usize)>, // (min, max, count)
    /// Minimum year across all albums (0 if none have year)
    pub min_year: i32,
    /// Maximum year across all albums (0 if none have year)
    pub max_year: i32,
    /// Number of mono albums (1 channel)
    pub mono_count: usize,
    /// Number of stereo albums (2 channels)
    pub stereo_count: usize,
    /// Number of surround albums (5.0/5.1 - 5 or 6 channels)
    pub surround_count: usize,
    /// Number of 7.1 albums (8 channels)
    pub surround71_count: usize,
    /// Number of albums with more than 8 channels
    pub surround_plus_count: usize,
    /// Whether stats are valid (false = need recomputation)
    pub valid: bool,
}

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMapping {
    /// Physical channel index on the interface
    pub interface_channel: usize,
    /// Channel group name (e.g., "L", "R", "C", "LFE", "SL", "SR")
    pub group_name: String,
}

/// Playback device configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
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
            // Channel numbers are 1-indexed for display (Channel 1, Channel 2, etc.)
            channel_mappings: vec![
                ChannelMapping {
                    interface_channel: 1,
                    group_name: "L".to_string(),
                },
                ChannelMapping {
                    interface_channel: 2,
                    group_name: "R".to_string(),
                },
            ],
        }
    }
}

/// Recording device configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
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
            // Channel numbers are 1-indexed for display (Channel 1)
            channel_mappings: vec![1],
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeakerConfiguration {
    Stereo,     // 2.0
    Stereo21,   // 2.1
    Surround50, // 5.0
    Surround51, // 5.1
    Surround71, // 7.1
    Surround91, // 9.1
    Atmos512,   // 5.1.2
    Atmos514,   // 5.1.4
    Atmos712,   // 7.1.2
    Atmos714,   // 7.1.4
    Atmos912,   // 9.1.2
    Atmos914,   // 9.1.4
    Atmos916,   // 9.1.6
    Custom,     // User-defined
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
            SpeakerConfiguration::Surround91 => {
                vec!["L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "WL", "WR"]
            }
            SpeakerConfiguration::Atmos512 => vec!["L", "R", "C", "LFE", "SL", "SR", "TFL", "TFR"],
            SpeakerConfiguration::Atmos514 => {
                vec!["L", "R", "C", "LFE", "SL", "SR", "TFL", "TFR", "TBL", "TBR"]
            }
            SpeakerConfiguration::Atmos712 => {
                vec!["L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "TFL", "TFR"]
            }
            SpeakerConfiguration::Atmos714 => vec![
                "L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "TFL", "TFR", "TBL", "TBR",
            ],
            SpeakerConfiguration::Atmos912 => vec![
                "L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "WL", "WR", "TFL", "TFR",
            ],
            SpeakerConfiguration::Atmos914 => vec![
                "L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "WL", "WR", "TFL", "TFR", "TBL",
                "TBR",
            ],
            SpeakerConfiguration::Atmos916 => vec![
                "L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "WL", "WR", "TFL", "TFR", "TML",
                "TMR", "TBL", "TBR",
            ],
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
            Some(Self {
                frequencies,
                spl_db,
            })
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
            config_accordion_expanded: vec!["playback".into(), "output_dir".into()], // Playback and output directory sections open by default
            plot_selected_channel: None,                                             // All channels
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

// Algorithm conversion removed - Algorithm type no longer exported from library

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
            algorithm: RoomEqAlgorithm::DifferentialEvolution,
            num_filters: 5,
            min_q: 0.5,
            max_q: 6.0,
            min_db: -12.0,
            max_db: 3.0,
            min_freq: 20.0,
            max_freq: 16000.0,
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
    pub peq_model_open: bool,
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
            self.channel_results
                .iter()
                .map(|r| r.pre_score)
                .sum::<f64>()
                / self.channel_results.len() as f64
        }
    }

    /// Get average post-score
    pub fn average_post_score(&self) -> f64 {
        if self.channel_results.is_empty() {
            0.0
        } else {
            self.channel_results
                .iter()
                .map(|r| r.post_score)
                .sum::<f64>()
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
    /// PEQ filter model (pk, hp-pk, ls-pk-hs, etc.)
    pub peq_model: String,
    /// Population size for DE
    pub population: usize,
    /// DE mutation factor (F)
    pub de_f: f64,
    /// DE crossover rate (CR)
    pub de_cr: f64,
    /// DE strategy
    pub strategy: String,
    /// Tolerance for convergence
    pub tolerance: f64,
    /// Enable local refinement after global optimization
    pub refine: bool,
    /// Local refinement algorithm
    pub local_algo: String,
    /// Enable smoothing of input curve
    pub smooth: bool,
    /// Smoothing window size
    pub smooth_n: usize,
}

impl Default for HeadphoneEqOptimizerConfig {
    fn default() -> Self {
        Self {
            algorithm: RoomEqAlgorithm::DifferentialEvolution,
            num_filters: 10,
            min_q: 0.5,
            max_q: 10.0,
            min_db: -12.0,
            max_db: 12.0,
            min_freq: 20.0,
            max_freq: 20000.0,
            max_iter: 10000,
            loss: "headphone-score".to_string(),
            peq_model: "pk".to_string(),
            population: 80,
            de_f: 0.8,
            de_cr: 0.9,
            strategy: "currenttobest1bin".to_string(),
            tolerance: 1e-3,
            refine: false,
            local_algo: "cobyla".to_string(),
            smooth: false,
            smooth_n: 1,
        }
    }
}

/// UI state for Headphone EQ dropdowns
#[derive(Debug, Clone, Default)]
pub struct HeadphoneEqDropdowns {
    pub target_open: bool,
    pub algorithm_open: bool,
    pub peq_model_open: bool,
    pub export_format_open: bool,
    pub loss_type_open: bool,
    pub target_curve_open: bool,
    pub strategy_open: bool,
    pub local_algo_open: bool,
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

    // === Goals & Configuration ===
    /// Loss function type ("flat" or "score")
    pub loss_type: String,
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
    /// Filter response (sum of all filters)
    pub filter_response: Option<Vec<(f64, f64)>>,
    /// Deviation from target (target - original)
    pub deviation_response: Option<Vec<(f64, f64)>>,
    /// Residual error (deviation - filter)
    pub error_response: Option<Vec<(f64, f64)>>,
    /// Individual filter responses (for detailed plotting)
    pub individual_responses: Option<Vec<Vec<(f64, f64)>>>,
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
            loss_type: "score".to_string(),
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
            expanded_sections: vec!["measurement".into(), "target".into(), "eq-design".into()],
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

// ============================================================================
// Spinorama EQ Screen Types
// ============================================================================

/// Spinorama EQ workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpinoramaStep {
    /// Step 1: Select speaker from spinorama.org
    #[default]
    SelectSpeaker,
    /// Step 2: Configure and run optimization
    Configure,
    /// Step 3: Review results and visualizations
    Review,
    /// Step 4: Apply to playback and export
    Export,
}

impl SpinoramaStep {
    /// Get all steps in order
    pub fn all() -> &'static [SpinoramaStep] {
        &[
            SpinoramaStep::SelectSpeaker,
            SpinoramaStep::Configure,
            SpinoramaStep::Review,
            SpinoramaStep::Export,
        ]
    }

    /// Get step index (0-based)
    pub fn index(&self) -> usize {
        match self {
            SpinoramaStep::SelectSpeaker => 0,
            SpinoramaStep::Configure => 1,
            SpinoramaStep::Review => 2,
            SpinoramaStep::Export => 3,
        }
    }

    /// Get step label
    pub fn label(&self) -> &'static str {
        match self {
            SpinoramaStep::SelectSpeaker => "Select",
            SpinoramaStep::Configure => "Configure",
            SpinoramaStep::Review => "Review",
            SpinoramaStep::Export => "Export",
        }
    }

    /// Get next step
    pub fn next(&self) -> Option<SpinoramaStep> {
        match self {
            SpinoramaStep::SelectSpeaker => Some(SpinoramaStep::Configure),
            SpinoramaStep::Configure => Some(SpinoramaStep::Review),
            SpinoramaStep::Review => Some(SpinoramaStep::Export),
            SpinoramaStep::Export => None,
        }
    }

    /// Get previous step
    pub fn previous(&self) -> Option<SpinoramaStep> {
        match self {
            SpinoramaStep::SelectSpeaker => None,
            SpinoramaStep::Configure => Some(SpinoramaStep::SelectSpeaker),
            SpinoramaStep::Review => Some(SpinoramaStep::Configure),
            SpinoramaStep::Export => Some(SpinoramaStep::Review),
        }
    }
}

/// Optimization mode for Spinorama EQ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SpinoramaOptimizationMode {
    /// Flatten a target curve (ON, LW, PIR, ER)
    #[default]
    FlatOnPir,
    /// Optimize Harman/Olive speaker preference score
    SpeakerScore,
}

/// Target curve types for spinorama optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SpinoramaTargetCurve {
    /// On-Axis response
    OnAxis,
    /// Listening Window response
    ListeningWindow,
    /// Estimated In-Room Response (default)
    #[default]
    EstimatedInRoom,
    /// Early Reflections
    EarlyReflections,
}

impl SpinoramaTargetCurve {
    pub fn all() -> &'static [SpinoramaTargetCurve] {
        &[
            SpinoramaTargetCurve::OnAxis,
            SpinoramaTargetCurve::ListeningWindow,
            SpinoramaTargetCurve::EstimatedInRoom,
            SpinoramaTargetCurve::EarlyReflections,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SpinoramaTargetCurve::OnAxis => "ON (On-Axis)",
            SpinoramaTargetCurve::ListeningWindow => "LW (Listening Window)",
            SpinoramaTargetCurve::EstimatedInRoom => "PIR (In-Room)",
            SpinoramaTargetCurve::EarlyReflections => "ER (Early Reflections)",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            SpinoramaTargetCurve::OnAxis => "ON",
            SpinoramaTargetCurve::ListeningWindow => "LW",
            SpinoramaTargetCurve::EstimatedInRoom => "PIR",
            SpinoramaTargetCurve::EarlyReflections => "ER",
        }
    }

    /// Get the curve name used in spinorama.org API
    pub fn api_name(&self) -> &'static str {
        match self {
            SpinoramaTargetCurve::OnAxis => "On Axis",
            SpinoramaTargetCurve::ListeningWindow => "Listening Window",
            SpinoramaTargetCurve::EstimatedInRoom => "Estimated In-Room Response",
            SpinoramaTargetCurve::EarlyReflections => "Early Reflections",
        }
    }
}

impl SpinoramaOptimizationMode {
    pub fn all() -> &'static [SpinoramaOptimizationMode] {
        &[
            SpinoramaOptimizationMode::FlatOnPir,
            SpinoramaOptimizationMode::SpeakerScore,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SpinoramaOptimizationMode::FlatOnPir => "Target",
            SpinoramaOptimizationMode::SpeakerScore => "Score",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            SpinoramaOptimizationMode::FlatOnPir => "Flatten the Estimated In-Room Response curve",
            SpinoramaOptimizationMode::SpeakerScore => {
                "Optimize for Harman/Olive speaker preference score"
            }
        }
    }

    pub fn to_loss_string(&self) -> &'static str {
        match self {
            SpinoramaOptimizationMode::FlatOnPir => "speaker-flat",
            SpinoramaOptimizationMode::SpeakerScore => "speaker-score",
        }
    }
}

/// Optimizer configuration for Spinorama EQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpinoramaOptimizerConfig {
    /// Optimization target mode
    pub mode: SpinoramaOptimizationMode,
    /// Target curve for FlatOnPir mode
    pub target_curve: SpinoramaTargetCurve,
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
    /// PEQ model (e.g., "pk", "ls-pk-hs")
    pub peq_model: String,
    /// Population size for evolutionary algorithms
    pub population: usize,
    /// Mutation factor (F) for DE
    pub de_f: f64,
    /// Crossover rate (CR) for DE
    pub de_cr: f64,
    /// DE strategy (e.g., "currenttobest1bin")
    pub strategy: String,
    /// Enable local refinement after global optimization
    pub refine: bool,
    /// Local algorithm for refinement
    pub local_algo: String,
    /// Enable smoothing
    pub smooth: bool,
}

impl Default for SpinoramaOptimizerConfig {
    fn default() -> Self {
        Self {
            mode: SpinoramaOptimizationMode::FlatOnPir,
            target_curve: SpinoramaTargetCurve::default(),
            algorithm: RoomEqAlgorithm::DifferentialEvolution,
            num_filters: 5,
            min_q: 0.5,
            max_q: 6.0,
            min_db: -12.0,
            max_db: 4.0,
            min_freq: 60.0,
            max_freq: 160000.0,
            max_iter: 10000,
            peq_model: "pk".to_string(),
            population: 40,
            de_f: 0.8,
            de_cr: 0.9,
            strategy: "currenttobest1bin".to_string(),
            refine: false,
            local_algo: "cobyla".to_string(),
            smooth: false,
        }
    }
}

/// Result of Spinorama EQ optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpinoramaEqResult {
    /// Optimized biquad filters
    pub biquads: Vec<SpinoramaBiquad>,
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

/// A single directivity curve at a specific angle
#[derive(Debug, Clone, Default)]
pub struct DirectivityCurve {
    /// Angle in degrees
    pub angle: f64,
    /// Frequency points
    pub frequencies: Vec<f64>,
    /// SPL values (dB)
    pub spl: Vec<f64>,
}

/// CEA2034 spinorama curves data for plotting
#[derive(Debug, Clone, Default)]
pub struct SpinoramaCurves {
    /// Frequency points (shared across all curves)
    pub frequencies: Vec<f64>,
    /// On Axis response (dB)
    pub on_axis: Vec<f64>,
    /// Listening Window response (dB)
    pub listening_window: Vec<f64>,
    /// Early Reflections response (dB)
    pub early_reflections: Vec<f64>,
    /// Sound Power response (dB)
    pub sound_power: Vec<f64>,
    /// Early Reflections DI (dB) - for secondary y-axis
    pub early_reflections_di: Vec<f64>,
    /// Sound Power DI (dB) - for secondary y-axis
    pub sound_power_di: Vec<f64>,
    /// Estimated In-Room Response (PIR) - computed from LW, ER, SP
    pub estimated_in_room: Vec<f64>,
    /// Horizontal directivity curves (SPL Horizontal at various angles)
    pub horizontal_directivity: Vec<DirectivityCurve>,
    /// Vertical directivity curves (SPL Vertical at various angles)
    pub vertical_directivity: Vec<DirectivityCurve>,
}

impl SpinoramaCurves {
    /// Check if we have valid CEA2034 data to plot
    pub fn is_valid(&self) -> bool {
        !self.frequencies.is_empty()
            && self.frequencies.len() == self.on_axis.len()
            && self.frequencies.len() == self.listening_window.len()
    }

    /// Check if we have PIR data
    pub fn has_pir(&self) -> bool {
        !self.estimated_in_room.is_empty()
    }

    /// Check if we have horizontal directivity data
    pub fn has_horizontal(&self) -> bool {
        !self.horizontal_directivity.is_empty()
    }

    /// Check if we have vertical directivity data
    pub fn has_vertical(&self) -> bool {
        !self.vertical_directivity.is_empty()
    }
}

/// Biquad filter for Spinorama EQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpinoramaBiquad {
    pub filter_type: String,
    pub freq: f64,
    pub q: f64,
    pub db_gain: f64,
}

/// UI state for Spinorama EQ dropdowns
#[derive(Debug, Clone)]
pub struct SpinoramaEqDropdowns {
    pub version_open: bool,
    pub measurement_open: bool,
    pub curve_open: bool,
    pub mode_open: bool,
    pub algorithm_open: bool,
    pub export_format_open: bool,
    /// Target curve dropdown (ON, LW, PIR, ER)
    pub target_curve_open: bool,
    /// AutoEQ form: EQ mode dropdown (IIR/FIR)
    pub opt_mode_open: bool,
    /// Selected EQ mode ("iir", "fir", "mixed")
    pub opt_mode: String,
    /// AutoEQ form: PEQ model dropdown
    pub peq_model_open: bool,
    /// AutoEQ form: DE strategy dropdown
    pub strategy_open: bool,
    /// AutoEQ form: local algorithm dropdown
    pub local_algo_open: bool,
    /// AutoEQ form editing state
    pub autoeq_editing_field: Option<AutoEqField>,
    /// AutoEQ form edit text
    pub autoeq_edit_text: String,
}

impl Default for SpinoramaEqDropdowns {
    fn default() -> Self {
        Self {
            version_open: false,
            measurement_open: false,
            curve_open: false,
            mode_open: false,
            algorithm_open: false,
            export_format_open: false,
            target_curve_open: false,
            opt_mode_open: false,
            opt_mode: "iir".to_string(),
            peq_model_open: false,
            strategy_open: false,
            local_algo_open: false,
            autoeq_editing_field: None,
            autoeq_edit_text: String::new(),
        }
    }
}

/// Complete Spinorama EQ screen state
#[derive(Debug, Clone)]
pub struct SpinoramaEqState {
    /// Current step in the workflow
    pub step: SpinoramaStep,

    // === Step 1: Speaker Selection ===
    /// Search input text
    pub speaker_search: String,
    /// List of available speakers from API
    pub available_speakers: Vec<String>,
    /// Filtered suggestions based on search
    pub speaker_suggestions: Vec<String>,
    /// Selected speaker name (e.g., "KEF R3")
    pub selected_speaker: Option<String>,
    /// Selected version (e.g., "asr", "erin", "princeton")
    pub selected_version: String,
    /// Selected measurement type (e.g., "CEA2034")
    pub selected_measurement: String,
    /// Selected curve (e.g., "Estimated In-Room Response")
    pub selected_curve: String,
    /// Available versions for selected speaker
    pub available_versions: Vec<String>,
    /// Available measurements for selected speaker/version
    pub available_measurements: Vec<String>,
    /// Available curves for selected measurement
    pub available_curves: Vec<String>,

    // === Step 2: Configuration ===
    /// Optimizer configuration
    pub optimizer_config: SpinoramaOptimizerConfig,

    // === Step 3: Optimization ===
    /// Current optimization status
    pub optimization_status: OptimizationStatus,
    /// Progress (0.0 - 1.0)
    pub progress: f32,
    /// Progress history for loss/score curves (iteration, loss, optional_score)
    pub progress_history: Vec<(usize, f64, Option<f64>)>,
    /// Status message during optimization
    pub status_message: String,
    /// Error message if optimization failed
    pub error_message: Option<String>,

    // === Step 4: Results ===
    /// Optimization result (simplified for UI)
    pub result: Option<SpinoramaEqResult>,
    /// Full optimization result (for graphs)
    pub full_result: Option<sotf_audio_player::autoeq::SpeakerOptimizationResult>,
    /// Export format selection
    pub export_format: String,

    // === UI State ===
    /// Loading indicator for speakers API call
    pub loading_speakers: bool,
    /// Loading indicator for versions API call
    pub loading_versions: bool,
    /// Loading indicator for measurements API call
    pub loading_measurements: bool,
    /// Dropdown states
    pub dropdowns: SpinoramaEqDropdowns,
    /// Expanded accordion sections
    pub expanded_sections: Vec<gpui::SharedString>,
    /// Timestamp when speakers were last fetched (for cache invalidation)
    pub speakers_cached_at: Option<std::time::Instant>,
    /// Focus handle for the search input
    pub search_focus_handle: Option<gpui::FocusHandle>,
    /// Whether the selected measurement has phase data
    pub has_phase_data: bool,

    // === Preview Curves (computed before optimization) ===
    /// Preview frequencies (Hz)
    pub preview_frequencies: Vec<f64>,
    /// Preview input curve (dB) - the raw measurement
    pub preview_input_curve: Vec<f64>,
    /// Preview target curve (dB) - what we're optimizing towards
    pub preview_target_curve: Vec<f64>,
    /// Preview deviation curve (dB) - target minus input
    pub preview_deviation_curve: Vec<f64>,
    /// Whether preview curves are being loaded
    pub loading_preview: bool,
    /// Error message if preview loading failed
    pub preview_error: Option<String>,

    // === Spinorama Curves (for CEA2034 plot in Step 1) ===
    /// CEA2034 curves data for spinorama plot
    pub spinorama_curves: SpinoramaCurves,
    /// Whether spinorama curves are being loaded
    pub loading_spinorama_curves: bool,
    /// Error message if spinorama curves loading failed
    pub spinorama_curves_error: Option<String>,
}

impl Default for SpinoramaEqState {
    fn default() -> Self {
        Self {
            step: SpinoramaStep::SelectSpeaker,
            speaker_search: String::new(),
            available_speakers: Vec::new(),
            speaker_suggestions: Vec::new(),
            selected_speaker: None,
            selected_version: "asr".to_string(),
            selected_measurement: "CEA2034".to_string(),
            selected_curve: "Estimated In-Room Response".to_string(),
            available_versions: Vec::new(),
            available_measurements: Vec::new(),
            available_curves: Vec::new(),
            optimizer_config: SpinoramaOptimizerConfig::default(),
            optimization_status: OptimizationStatus::Idle,
            progress: 0.0,
            progress_history: Vec::new(),
            status_message: String::new(),
            error_message: None,
            result: None,
            full_result: None,
            export_format: "json".to_string(),
            loading_speakers: false,
            loading_versions: false,
            loading_measurements: false,
            dropdowns: SpinoramaEqDropdowns::default(),
            expanded_sections: vec!["speaker".into(), "options".into()],
            speakers_cached_at: None,
            search_focus_handle: None,
            has_phase_data: false,
            preview_frequencies: Vec::new(),
            preview_input_curve: Vec::new(),
            preview_target_curve: Vec::new(),
            preview_deviation_curve: Vec::new(),
            loading_preview: false,
            preview_error: None,
            spinorama_curves: SpinoramaCurves::default(),
            loading_spinorama_curves: false,
            spinorama_curves_error: None,
        }
    }
}

impl SpinoramaEqState {
    /// Check if we can proceed from the current step
    pub fn can_advance(&self) -> bool {
        match self.step {
            SpinoramaStep::SelectSpeaker => self.selected_speaker.is_some(),
            // Configure step now includes optimization - must complete before advancing
            SpinoramaStep::Configure => self.optimization_status == OptimizationStatus::Completed,
            SpinoramaStep::Review => self.result.is_some(),
            SpinoramaStep::Export => true, // Always can proceed (or stay) from export
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

    /// Update speaker suggestions based on search query
    pub fn update_suggestions(&mut self) {
        let query = self.speaker_search.to_lowercase();
        if query.is_empty() {
            self.speaker_suggestions = self.available_speakers.clone();
        } else {
            self.speaker_suggestions = self
                .available_speakers
                .iter()
                .filter(|s| s.to_lowercase().contains(&query))
                .cloned()
                .collect();
        }
        // Limit to reasonable number for UI
        self.speaker_suggestions.truncate(50);
    }

    /// Check if speakers cache needs to be refreshed (older than 1 hour or not loaded)
    pub fn needs_speaker_refresh(&self) -> bool {
        if self.available_speakers.is_empty() {
            return true;
        }
        match self.speakers_cached_at {
            Some(cached_at) => cached_at.elapsed() > std::time::Duration::from_secs(3600),
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // Screen Enum Tests
    // ============================================================================

    #[test]
    fn test_screen_variants() {
        // Ensure all variants exist and are distinct
        let screens = [
            Screen::Library,
            Screen::DirectoryManager,
            Screen::Queue,
            Screen::Spectrum,
            Screen::Settings,
            Screen::Studio,
            Screen::Recording,
            Screen::RoomEq,
            Screen::HeadphoneEq,
            Screen::Spinorama,
            Screen::PluginGraph,
        ];
        assert_eq!(screens.len(), 11);
        assert_ne!(Screen::Library, Screen::Queue);
    }

    #[test]
    fn test_screen_copy_clone() {
        let screen = Screen::Library;
        let copied = screen;
        let cloned = screen.clone();
        assert_eq!(screen, copied);
        assert_eq!(screen, cloned);
    }

    // ============================================================================
    // InputMode Enum Tests
    // ============================================================================

    #[test]
    fn test_input_mode_variants() {
        let modes = [
            InputMode::Normal,
            InputMode::Search,
            InputMode::AddDirectory,
            InputMode::SavePlugins,
            InputMode::LoadPlugins,
            InputMode::LoadApoFile,
            InputMode::LoadSofaFile,
            InputMode::Help,
            InputMode::KeyboardShortcuts,
            InputMode::About,
            InputMode::EditingParam,
            InputMode::SpinoramaSpeakerSearch,
        ];
        assert_eq!(modes.len(), 12);
    }

    // ============================================================================
    // ToastMessage Tests
    // ============================================================================

    #[test]
    fn test_toast_message_new() {
        let toast = ToastMessage::new("Test message".to_string(), ToastType::Info);
        assert_eq!(toast.message, "Test message");
        assert_eq!(toast.toast_type, ToastType::Info);
        assert_eq!(toast.auto_dismiss_ms, Some(5000));
    }

    #[test]
    fn test_toast_message_success() {
        let toast = ToastMessage::success("Success!");
        assert_eq!(toast.message, "Success!");
        assert_eq!(toast.toast_type, ToastType::Success);
    }

    #[test]
    fn test_toast_message_error() {
        let toast = ToastMessage::error("Error occurred");
        assert_eq!(toast.message, "Error occurred");
        assert_eq!(toast.toast_type, ToastType::Error);
    }

    #[test]
    fn test_toast_message_info() {
        let toast = ToastMessage::info("Info message");
        assert_eq!(toast.message, "Info message");
        assert_eq!(toast.toast_type, ToastType::Info);
    }

    #[test]
    fn test_toast_message_warning() {
        let toast = ToastMessage::warning("Warning!");
        assert_eq!(toast.message, "Warning!");
        assert_eq!(toast.toast_type, ToastType::Warning);
    }

    #[test]
    fn test_toast_message_persistent() {
        let toast = ToastMessage::persistent("Persistent message", ToastType::Error);
        assert_eq!(toast.message, "Persistent message");
        assert_eq!(toast.toast_type, ToastType::Error);
        assert_eq!(toast.auto_dismiss_ms, None);
    }

    #[test]
    fn test_toast_message_should_dismiss_not_expired() {
        let toast = ToastMessage::new("Test".to_string(), ToastType::Info);
        // Just created, should not dismiss yet
        assert!(!toast.should_dismiss());
    }

    #[test]
    fn test_toast_message_persistent_never_dismisses() {
        let toast = ToastMessage::persistent("Test", ToastType::Info);
        assert!(!toast.should_dismiss());
    }

    // ============================================================================
    // QueueItem Tests
    // ============================================================================

    fn create_test_album(track_count: usize) -> Album {
        let tracks: Vec<Track> = (0..track_count)
            .map(|i| Track {
                path: std::path::PathBuf::from(format!("/music/track_{}.flac", i)),
                track_number: Some(i as u32 + 1),
                title: Some(format!("Track {}", i + 1)),
                duration_secs: Some(180.0),
                sample_rate: Some(44100),
                channels: Some(2),
                bit_depth: Some(16),
                disc_number: None,
                artist: Some("Test Artist".to_string()),
                album_replay_gain: None,
                track_replay_gain: None,
            })
            .collect();

        Album {
            artist: "Test Artist".to_string(),
            title: "Test Album".to_string(),
            tracks,
            cover_art_path: None,
            path: std::path::PathBuf::from("/music/test_album"),
            year: Some(2024),
            genre: Some("Rock".to_string()),
            min_channels: 2,
            max_channels: 2,
            composer: None,
            tags: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_queue_item_new() {
        let album = create_test_album(5);
        let item = QueueItem::new(album);
        assert_eq!(item.current_track_index, 0);
        assert_eq!(item.album.tracks.len(), 5);
    }

    #[test]
    fn test_queue_item_current_track() {
        let album = create_test_album(3);
        let item = QueueItem::new(album);
        let track = item.current_track().unwrap();
        assert_eq!(track.title, Some("Track 1".to_string()));
    }

    #[test]
    fn test_queue_item_next_track() {
        let album = create_test_album(3);
        let mut item = QueueItem::new(album);

        let track = item.next_track().unwrap();
        assert_eq!(track.title, Some("Track 2".to_string()));
        assert_eq!(item.current_track_index, 1);

        let track = item.next_track().unwrap();
        assert_eq!(track.title, Some("Track 3".to_string()));
        assert_eq!(item.current_track_index, 2);

        // No more tracks
        assert!(item.next_track().is_none());
        assert_eq!(item.current_track_index, 2);
    }

    #[test]
    fn test_queue_item_previous_track() {
        let album = create_test_album(3);
        let mut item = QueueItem::new(album);
        item.current_track_index = 2;

        let track = item.previous_track().unwrap();
        assert_eq!(track.title, Some("Track 2".to_string()));
        assert_eq!(item.current_track_index, 1);

        let track = item.previous_track().unwrap();
        assert_eq!(track.title, Some("Track 1".to_string()));
        assert_eq!(item.current_track_index, 0);

        // Can't go before first track
        assert!(item.previous_track().is_none());
        assert_eq!(item.current_track_index, 0);
    }

    #[test]
    fn test_queue_item_empty_album() {
        let album = create_test_album(0);
        let item = QueueItem::new(album);
        assert!(item.current_track().is_none());
    }

    // ============================================================================
    // SpeakerConfiguration Tests
    // ============================================================================

    #[test]
    fn test_speaker_configuration_as_str() {
        assert_eq!(SpeakerConfiguration::Stereo.as_str(), "2.0");
        assert_eq!(SpeakerConfiguration::Stereo21.as_str(), "2.1");
        assert_eq!(SpeakerConfiguration::Surround50.as_str(), "5.0");
        assert_eq!(SpeakerConfiguration::Surround51.as_str(), "5.1");
        assert_eq!(SpeakerConfiguration::Surround71.as_str(), "7.1");
        assert_eq!(SpeakerConfiguration::Atmos714.as_str(), "7.1.4");
        assert_eq!(SpeakerConfiguration::Custom.as_str(), "Custom");
    }

    #[test]
    fn test_speaker_configuration_channel_count() {
        assert_eq!(SpeakerConfiguration::Stereo.channel_count(), 2);
        assert_eq!(SpeakerConfiguration::Stereo21.channel_count(), 3);
        assert_eq!(SpeakerConfiguration::Surround50.channel_count(), 5);
        assert_eq!(SpeakerConfiguration::Surround51.channel_count(), 6);
        assert_eq!(SpeakerConfiguration::Surround71.channel_count(), 8);
        assert_eq!(SpeakerConfiguration::Surround91.channel_count(), 10);
        assert_eq!(SpeakerConfiguration::Atmos714.channel_count(), 12);
        assert_eq!(SpeakerConfiguration::Atmos916.channel_count(), 16);
    }

    #[test]
    fn test_speaker_configuration_default_channel_names() {
        let stereo = SpeakerConfiguration::Stereo.default_channel_names();
        assert_eq!(stereo, vec!["L", "R"]);

        let surround51 = SpeakerConfiguration::Surround51.default_channel_names();
        assert_eq!(surround51, vec!["L", "R", "C", "LFE", "SL", "SR"]);

        let atmos714 = SpeakerConfiguration::Atmos714.default_channel_names();
        assert_eq!(atmos714.len(), 12);
        assert!(atmos714.contains(&"TFL"));
        assert!(atmos714.contains(&"TBR"));
    }

    #[test]
    fn test_speaker_configuration_from_channel_count() {
        assert_eq!(
            SpeakerConfiguration::from_channel_count(2),
            SpeakerConfiguration::Stereo
        );
        assert_eq!(
            SpeakerConfiguration::from_channel_count(6),
            SpeakerConfiguration::Surround51
        );
        assert_eq!(
            SpeakerConfiguration::from_channel_count(8),
            SpeakerConfiguration::Surround71
        );
        assert_eq!(
            SpeakerConfiguration::from_channel_count(99),
            SpeakerConfiguration::Custom
        );
    }

    #[test]
    fn test_speaker_configuration_all() {
        let all = SpeakerConfiguration::all();
        assert_eq!(all.len(), 14);
        assert!(all.contains(&SpeakerConfiguration::Stereo));
        assert!(all.contains(&SpeakerConfiguration::Custom));
    }

    // ============================================================================
    // PlotSmoothing Tests
    // ============================================================================

    #[test]
    fn test_plot_smoothing_as_str() {
        assert_eq!(PlotSmoothing::None.as_str(), "None");
        assert_eq!(PlotSmoothing::Octave1.as_str(), "1/1 octave");
        assert_eq!(PlotSmoothing::Octave3.as_str(), "1/3 octave");
        assert_eq!(PlotSmoothing::Octave6.as_str(), "1/6 octave");
        assert_eq!(PlotSmoothing::Octave24.as_str(), "1/24 octave");
    }

    #[test]
    fn test_plot_smoothing_octave_fraction() {
        assert_eq!(PlotSmoothing::None.octave_fraction(), None);
        assert_eq!(PlotSmoothing::Octave1.octave_fraction(), Some(1.0));
        assert!((PlotSmoothing::Octave3.octave_fraction().unwrap() - 1.0 / 3.0).abs() < 0.001);
        assert!((PlotSmoothing::Octave6.octave_fraction().unwrap() - 1.0 / 6.0).abs() < 0.001);
        assert!((PlotSmoothing::Octave24.octave_fraction().unwrap() - 1.0 / 24.0).abs() < 0.001);
    }

    #[test]
    fn test_plot_smoothing_default() {
        assert_eq!(PlotSmoothing::default(), PlotSmoothing::None);
    }

    // ============================================================================
    // RecordingSignalType Tests
    // ============================================================================

    #[test]
    fn test_recording_signal_type_as_str() {
        assert_eq!(RecordingSignalType::Sweep.as_str(), "Sweep");
        assert_eq!(RecordingSignalType::WhiteNoise.as_str(), "White Noise");
        assert_eq!(RecordingSignalType::PinkNoise.as_str(), "Pink Noise");
    }

    #[test]
    fn test_recording_signal_type_all() {
        let all = RecordingSignalType::all();
        assert_eq!(all.len(), 3);
        assert!(all.contains(&RecordingSignalType::Sweep));
        assert!(all.contains(&RecordingSignalType::WhiteNoise));
        assert!(all.contains(&RecordingSignalType::PinkNoise));
    }

    // ============================================================================
    // CalibrationData Tests
    // ============================================================================

    #[test]
    fn test_calibration_data_parse_csv() {
        let content = "100, 0.5\n1000, -0.2\n10000, 0.8";
        let data = CalibrationData::parse(content).unwrap();
        assert_eq!(data.frequencies.len(), 3);
        assert_eq!(data.spl_db.len(), 3);
        assert!((data.frequencies[0] - 100.0).abs() < 0.001);
        assert!((data.frequencies[1] - 1000.0).abs() < 0.001);
        assert!((data.spl_db[0] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_calibration_data_parse_with_comments() {
        let content = "# Calibration file\n// Another comment\n100\t0.5\n1000\t-0.2";
        let data = CalibrationData::parse(content).unwrap();
        assert_eq!(data.frequencies.len(), 2);
    }

    #[test]
    fn test_calibration_data_parse_with_header() {
        let content = "Frequency Hz, SPL dB\n100, 0.5\n1000, -0.2";
        let data = CalibrationData::parse(content).unwrap();
        assert_eq!(data.frequencies.len(), 2);
    }

    #[test]
    fn test_calibration_data_parse_empty() {
        let content = "# Only comments\n// Nothing else";
        assert!(CalibrationData::parse(content).is_none());
    }

    #[test]
    fn test_calibration_data_parse_invalid_frequency() {
        // Frequencies out of range (> 100kHz) should be ignored
        let content = "100, 0.5\n200000, 0.5"; // 200kHz is out of range
        let data = CalibrationData::parse(content).unwrap();
        assert_eq!(data.frequencies.len(), 1);
    }

    #[test]
    fn test_calibration_data_is_valid() {
        let valid = CalibrationData {
            frequencies: vec![100.0, 1000.0],
            spl_db: vec![0.5, -0.2],
        };
        assert!(valid.is_valid());

        let empty = CalibrationData::default();
        assert!(!empty.is_valid());

        let mismatched = CalibrationData {
            frequencies: vec![100.0, 1000.0],
            spl_db: vec![0.5],
        };
        assert!(!mismatched.is_valid());
    }

    // ============================================================================
    // RoomEqStep Tests
    // ============================================================================

    #[test]
    fn test_room_eq_step_all() {
        let all = RoomEqStep::all();
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn test_room_eq_step_index() {
        assert_eq!(RoomEqStep::LoadData.index(), 0);
        assert_eq!(RoomEqStep::Configure.index(), 1);
        assert_eq!(RoomEqStep::Optimize.index(), 2);
        assert_eq!(RoomEqStep::Review.index(), 3);
        assert_eq!(RoomEqStep::Export.index(), 4);
    }

    #[test]
    fn test_room_eq_step_label() {
        assert_eq!(RoomEqStep::LoadData.label(), "Load Data");
        assert_eq!(RoomEqStep::Configure.label(), "Configure");
        assert_eq!(RoomEqStep::Export.label(), "Export");
    }

    #[test]
    fn test_room_eq_step_next() {
        assert_eq!(RoomEqStep::LoadData.next(), Some(RoomEqStep::Configure));
        assert_eq!(RoomEqStep::Configure.next(), Some(RoomEqStep::Optimize));
        assert_eq!(RoomEqStep::Export.next(), None);
    }

    #[test]
    fn test_room_eq_step_previous() {
        assert_eq!(RoomEqStep::LoadData.previous(), None);
        assert_eq!(RoomEqStep::Configure.previous(), Some(RoomEqStep::LoadData));
        assert_eq!(RoomEqStep::Export.previous(), Some(RoomEqStep::Review));
    }

    // ============================================================================
    // CrossoverType Tests
    // ============================================================================

    #[test]
    fn test_crossover_type_all() {
        let all = CrossoverType::all();
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn test_crossover_type_as_str() {
        assert_eq!(CrossoverType::LR12.as_str(), "Linkwitz-Riley 12dB");
        assert_eq!(CrossoverType::LR24.as_str(), "Linkwitz-Riley 24dB");
        assert_eq!(CrossoverType::Butterworth12.as_str(), "Butterworth 12dB");
    }

    #[test]
    fn test_crossover_type_default() {
        assert_eq!(CrossoverType::default(), CrossoverType::LR24);
    }

    // ============================================================================
    // RoomEqAlgorithm Tests
    // ============================================================================

    #[test]
    fn test_room_eq_algorithm_all() {
        let all = RoomEqAlgorithm::all();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_room_eq_algorithm_as_str() {
        assert_eq!(RoomEqAlgorithm::Cobyla.as_str(), "COBYLA");
        assert_eq!(
            RoomEqAlgorithm::DifferentialEvolution.as_str(),
            "Differential Evolution"
        );
        assert_eq!(RoomEqAlgorithm::NelderMead.as_str(), "Nelder-Mead");
    }

    #[test]
    fn test_room_eq_algorithm_to_autoeq_string() {
        assert_eq!(RoomEqAlgorithm::Cobyla.to_autoeq_string(), "cobyla");
        assert_eq!(
            RoomEqAlgorithm::DifferentialEvolution.to_autoeq_string(),
            "autoeq:de"
        );
        assert_eq!(
            RoomEqAlgorithm::NelderMead.to_autoeq_string(),
            "nelder-mead"
        );
    }

    // ============================================================================
    // Default Implementations Tests
    // ============================================================================

    #[test]
    fn test_library_stats_default() {
        let stats = LibraryStats::default();
        assert_eq!(stats.artists_count, 0);
        assert_eq!(stats.total_tracks, 0);
        assert!(!stats.valid);
    }

    #[test]
    fn test_measure_state_default() {
        let state = MeasureState::default();
        assert_eq!(state.step, MeasureStep::DeviceSelection);
        assert_eq!(state.signal_type, "sweep");
        assert_eq!(state.level, -20.0);
    }

    #[test]
    fn test_playback_device_config_default() {
        let config = PlaybackDeviceConfig::default();
        assert_eq!(config.num_channels, 2);
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.speaker_configuration, SpeakerConfiguration::Stereo);
        assert_eq!(config.channel_mappings.len(), 2);
    }

    #[test]
    fn test_recording_device_config_default() {
        let config = RecordingDeviceConfig::default();
        assert_eq!(config.num_channels, 1);
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.channel_mappings.len(), 1);
    }

    #[test]
    fn test_recording_state_default() {
        let state = RecordingState::default();
        assert_eq!(state.step, RecordingStep::Config);
        assert_eq!(state.signal_type, RecordingSignalType::Sweep);
        assert_eq!(state.signal_duration_secs, 5.0);
        assert_eq!(state.signal_level_db, -20.0);
    }

    #[test]
    fn test_room_eq_optimizer_config_default() {
        let config = RoomEqOptimizerConfig::default();
        assert_eq!(config.algorithm, RoomEqAlgorithm::DifferentialEvolution);
        assert_eq!(config.num_filters, 5);
        assert!((config.min_q - 0.5).abs() < 0.001);
        assert!((config.max_q - 6.0).abs() < 0.001);
    }

    // ============================================================================
    // RecordingState Method Tests
    // ============================================================================

    #[test]
    fn test_recording_state_init_channel_recordings() {
        let mut state = RecordingState::default();
        state.playback_config.channel_mappings = vec![
            ChannelMapping {
                interface_channel: 1,
                group_name: "L".to_string(),
            },
            ChannelMapping {
                interface_channel: 2,
                group_name: "R".to_string(),
            },
            ChannelMapping {
                interface_channel: 3,
                group_name: "C".to_string(),
            },
        ];

        state.init_channel_recordings();

        assert_eq!(state.channel_recordings.len(), 3);
        assert_eq!(state.channel_recordings[0].channel_name, "L");
        assert_eq!(state.channel_recordings[1].channel_name, "R");
        assert_eq!(state.channel_recordings[2].channel_name, "C");
        assert_eq!(
            state.channel_recordings[0].state,
            ChannelRecordingState::Empty
        );
    }

    #[test]
    fn test_recording_state_all_channels_recorded() {
        let mut state = RecordingState::default();
        state.channel_recordings = vec![
            ChannelRecording {
                channel_index: 0,
                channel_name: "L".to_string(),
                state: ChannelRecordingState::Done,
                result: None,
            },
            ChannelRecording {
                channel_index: 1,
                channel_name: "R".to_string(),
                state: ChannelRecordingState::Done,
                result: None,
            },
        ];

        assert!(state.all_channels_recorded());

        state.channel_recordings[1].state = ChannelRecordingState::Empty;
        assert!(!state.all_channels_recorded());
    }

    #[test]
    fn test_recording_state_is_recording() {
        let mut state = RecordingState::default();
        assert!(!state.is_recording());

        state.current_recording_channel = Some(0);
        assert!(state.is_recording());
    }

    // ============================================================================
    // LayoutMode Tests
    // ============================================================================

    #[test]
    fn test_layout_mode_variants() {
        assert_ne!(LayoutMode::Compact, LayoutMode::Expanded);
    }

    // ============================================================================
    // MeterDisplayMode Tests
    // ============================================================================

    #[test]
    fn test_meter_display_mode_default() {
        assert_eq!(MeterDisplayMode::default(), MeterDisplayMode::Lufs);
    }

    // ============================================================================
    // ContextMenuType Tests
    // ============================================================================

    #[test]
    fn test_context_menu_type_variants() {
        let types = [
            ContextMenuType::Album,
            ContextMenuType::QueueItem,
            ContextMenuType::Plugin,
            ContextMenuType::Directory,
        ];
        assert_eq!(types.len(), 4);
    }

    // ============================================================================
    // ReplayGainMode Tests
    // ============================================================================

    #[test]
    fn test_replay_gain_mode_variants() {
        assert_ne!(ReplayGainMode::Track, ReplayGainMode::Album);
    }

    // ============================================================================
    // PluginViewMode Tests
    // ============================================================================

    #[test]
    fn test_plugin_view_mode_default() {
        assert_eq!(PluginViewMode::default(), PluginViewMode::Rack);
    }

    #[test]
    fn test_plugin_view_mode_variants() {
        assert_ne!(PluginViewMode::Rack, PluginViewMode::Graph);
    }
}
