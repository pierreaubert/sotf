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
    /// Semantic severity for the user-visible recording status.  Display
    /// text is translated independently, so it must never determine this.
    pub status_severity: RecordingStatusSeverity,
    /// Debug-only deterministic capture source. It configures the environment
    /// for a UI test; results are produced only after the visible Capture
    /// action is invoked.
    #[cfg(feature = "dev-api")]
    pub qa_fake_capture: Option<QaFakeCapture>,
    // NOTE: `capture_generation` / `is_current_capture` live on the shared
    // `RecordingScreenModel` (reached via Deref) so the TUI can use the same
    // stale-completion guard.
}

#[cfg(feature = "dev-api")]
#[derive(Debug, Clone)]
pub struct QaFakeCapture {
    pub points: usize,
    /// A one-shot deterministic failure injected after the visible Capture
    /// action. Taking it on the first attempt makes Retry exercise the same
    /// UI control and then succeed.
    pub fault: Option<QaFakeCaptureFault>,
}

/// Failure modes that the dev-only recording fixture can reproduce.
#[cfg(feature = "dev-api")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QaFakeCaptureFault {
    DeviceLost,
    Clipping,
    IoFailure,
}

#[cfg(feature = "dev-api")]
impl QaFakeCaptureFault {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "device-loss" => Some(Self::DeviceLost),
            "clipping" => Some(Self::Clipping),
            "io-failure" => Some(Self::IoFailure),
            _ => None,
        }
    }

    pub const fn status_message(self) -> &'static str {
        match self {
            Self::DeviceLost => "Recording error: capture device was disconnected",
            Self::Clipping => "Recording error: input clipped during capture",
            Self::IoFailure => "Recording error: unable to write capture data",
        }
    }
}

/// Severity associated with a recording workflow status message.
///
/// This deliberately lives in the GPUI wrapper rather than the shared player
/// model: it is presentation metadata, not recording-domain state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RecordingStatusSeverity {
    #[default]
    Idle,
    Working,
    Success,
    Warning,
    Error,
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
            status_severity: RecordingStatusSeverity::Idle,
            #[cfg(feature = "dev-api")]
            qa_fake_capture: None,
        }
    }
}

impl RecordingState {
    /// Set the presentation status and its semantic severity together.
    ///
    /// Keeping these in one operation prevents translated display text from
    /// becoming an implicit source of truth for success or failure styling.
    pub fn set_status(&mut self, message: impl Into<String>, severity: RecordingStatusSeverity) {
        self.status_message = message.into();
        self.status_severity = severity;
    }

    /// Clear a transient status when a workflow returns to its idle state.
    pub fn clear_status(&mut self) {
        self.status_message.clear();
        self.status_severity = RecordingStatusSeverity::Idle;
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
