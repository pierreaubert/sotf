// ============================================================================
// Recording Screen Types
// ============================================================================

use serde::{Deserialize, Serialize};

use super::calibration::CalibrationData;

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
    // Advanced metrics
    pub impulse_response: Option<Vec<f32>>,
    pub impulse_time_ms: Option<Vec<f32>>,
    pub thd_percent: Option<Vec<f32>>,
    pub harmonic_distortion_db: Option<Vec<Vec<f32>>>, // [2nd, 3rd, 4th...]
    pub excess_group_delay_ms: Option<Vec<f32>>,
    pub rt60_ms: Option<Vec<f32>>,
    pub clarity_c50_db: Option<Vec<f32>>,
    pub clarity_c80_db: Option<Vec<f32>>,
    pub spectrogram_db: Option<Vec<Vec<f32>>>,
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
    /// Sweep start frequency in Hz
    pub sweep_start_freq: f32,
    /// Sweep end frequency in Hz
    pub sweep_end_freq: f32,
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

    // === Noise Floor Warning ===
    /// Warning message when recording level is close to noise floor
    pub noise_floor_warning: Option<String>,
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
            sweep_start_freq: 20.0,
            sweep_end_freq: 20000.0,
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
            noise_floor_warning: None,
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
