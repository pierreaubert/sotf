// ============================================================================
// Recording Screen Types
// ============================================================================
//
// Domain types are shared via the player crate. UI-specific state stays here.

use sotf_audio_player::ui_models::recording::RecordingScreenModel;
use std::ops::{Deref, DerefMut};

// Re-export shared domain types from player crate
pub use sotf_audio_player::recording_types::{
    BassAnchorCaptureState, BassAnchorCaptureStatus, ChannelMapping, ChannelRecording,
    ChannelRecordingState, CtcMatrixExportStrategy, PlaybackDeviceConfig, PlotSmoothing,
    ProbeCaptureState, ProbeCaptureStatus, RecordingDeviceConfig, RecordingResult,
    RecordingSignalType, RecordingStep, RoomDimensionUnit, SpeakerConfiguration,
    SplCalibrationCaptureState, SplCalibrationCaptureStatus, TransferMatrixLoopbackRecording,
};

/// Complete recording screen state. Holds only GPUI-specific view state;
/// all domain state lives in the embedded [`RecordingScreenModel`].
#[derive(Debug, Clone)]
pub struct RecordingState {
    /// Shared, UI-agnostic Recording wizard domain model.
    pub model: RecordingScreenModel,

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

    /// Channel selector dropdown open
    pub plot_channel_dropdown_open: bool,
    /// Smoothing selector dropdown open
    pub plot_smoothing_dropdown_open: bool,

    /// Index of the channel-speaker row whose autocomplete suggestions
    /// are currently visible, or `None` when no dropdown is open.
    pub channel_speaker_autocomplete_open: Option<usize>,

    /// Actual duration (seconds) of the persisted `ctc_reference_sweep.wav`
    /// stimulus. Tracked GPUI-side because the octave-scaled sweep (B1) is
    /// self-timed: `signal_duration_secs` no longer describes the reference
    /// WAV, so the real length is recorded at capture time and persisted as
    /// `CtcConfig::sweep_duration_s` at save time.
    pub ctc_reference_sweep_duration_s: Option<f32>,
}

impl Default for RecordingState {
    fn default() -> Self {
        let (recording_base_directory, recording_directory) =
            crate::app::config::default_recording_paths();

        Self {
            model: RecordingScreenModel {
                recording_base_directory,
                recording_directory,
                ..Default::default()
            },
            playback_device_dropdown_open: false,
            recording_device_dropdown_open: false,
            playback_sample_rate_dropdown_open: false,
            recording_sample_rate_dropdown_open: false,
            speaker_config_dropdown_open: false,
            signal_type_dropdown_open: false,
            duration_dropdown_open: false,
            channel_name_dropdown_open: None,
            speaker_mode_dropdown_open: None,
            config_accordion_expanded: vec!["playback".into(), "output_dir".into()],
            plot_channel_dropdown_open: false,
            plot_smoothing_dropdown_open: false,
            channel_speaker_autocomplete_open: None,
            ctc_reference_sweep_duration_s: None,
        }
    }
}

impl Deref for RecordingState {
    type Target = RecordingScreenModel;

    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

impl DerefMut for RecordingState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.model
    }
}
