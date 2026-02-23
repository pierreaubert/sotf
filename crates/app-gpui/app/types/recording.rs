// ============================================================================
// Recording Screen Types
// ============================================================================
//
// Domain types are shared via the player crate. UI-specific state stays here.

use super::calibration::CalibrationData;

// Re-export shared domain types from player crate
pub use sotf_audio_player::recording_types::{
    ChannelMapping, ChannelRecording, ChannelRecordingState, PlaybackDeviceConfig, PlotSmoothing,
    RecordingDeviceConfig, RecordingResult, RecordingSignalType, RecordingStep,
    SpeakerConfiguration,
};

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
    /// Track which speaker mode dropdown is open (by speaker index)
    pub speaker_mode_dropdown_open: Option<usize>,
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

    // === Migration Modal State ===
    /// Whether the migration modal is currently shown
    pub migration_modal_open: bool,
    /// Path to the file being migrated (if migration modal is open)
    pub migration_file_path: Option<String>,
    /// Directory containing the file being migrated
    pub migration_file_dir: Option<String>,
    /// Original file size in bytes (for display)
    pub migration_file_size: Option<u64>,
    /// Number of channels in the file being migrated
    pub migration_channel_count: usize,
    /// Raw JSON content for migration (temporary storage)
    pub migration_pending_json: Option<String>,
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
            speaker_mode_dropdown_open: None,
            config_accordion_expanded: vec!["playback".into(), "output_dir".into()], // Playback and output directory sections open by default
            plot_selected_channel: None,                                             // All channels
            plot_smoothing: PlotSmoothing::None,
            plot_channel_dropdown_open: false,
            plot_smoothing_dropdown_open: false,
            save_name: "recording".to_string(),
            noise_floor_warning: None,
            migration_modal_open: false,
            migration_file_path: None,
            migration_file_dir: None,
            migration_file_size: None,
            migration_channel_count: 0,
            migration_pending_json: None,
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
