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

/// Explicit physical routing for one imported source, bound to the selected devices.
#[derive(Debug, Clone, Default)]
pub struct ImportedRetakeRoute {
    pub output_device: (String, String),
    pub input_device: (String, String),
    pub input_channels: Vec<usize>,
    pub output_channel: Option<usize>,
    pub microphone_slot: Option<usize>,
}

/// Complete recording screen state. Holds only GPUI-specific view state;
/// all domain state lives in the embedded [`RecordingScreenModel`].
#[derive(Debug, Clone)]
pub struct RecordingState {
    /// Shared, UI-agnostic Recording wizard domain model.
    pub model: RecordingScreenModel,
    pub capture_inputs: Option<serde_json::Value>,
    /// Original topology and anchored references retained for imported-session saves.
    pub imported_session: Option<sotf_audio_player::recording_types::RecordingImport>,
    pub imported_retake_routes: std::collections::HashMap<usize, ImportedRetakeRoute>,
    pub imported_route_dropdown: Option<(usize, bool)>,
    pub imported_route_highlight: Option<usize>,
    pub take_review: sotf_audio_player::ui_models::take_review::TakeReviewState,

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
            capture_inputs: None,
            imported_session: None,
            imported_retake_routes: Default::default(),
            imported_route_dropdown: None,
            imported_route_highlight: None,
            take_review: Default::default(),
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
    pub fn metadata_channel_names(&self) -> Vec<String> {
        if let Some(imported) = &self.imported_session {
            imported
                .channels
                .iter()
                .map(|channel| channel.channel_name.clone())
                .collect()
        } else {
            self.playback_config
                .channel_mappings
                .iter()
                .map(|mapping| mapping.group_name.clone())
                .collect()
        }
    }

    pub fn sync_channel_speakers_length(&mut self) {
        let count = self.metadata_channel_names().len();
        self.model.channel_speakers.resize(count, String::new());
    }

    pub fn select_imported_route(&mut self, index: usize, output: bool, channel: usize) {
        let output_device = (
            self.playback_config.device_id.clone(),
            self.playback_config.device_name.clone(),
        );
        let input_device = (
            self.recording_config.device_id.clone(),
            self.recording_config.device_name.clone(),
        );
        let input_channels = self.recording_config.channel_mappings.clone();
        let route = self.imported_retake_routes.entry(index).or_default();
        if route.output_device != output_device
            || route.input_device != input_device
            || route.input_channels != input_channels
        {
            *route = ImportedRetakeRoute {
                output_device,
                input_device,
                input_channels,
                ..Default::default()
            };
        }
        if output {
            route.output_channel = Some(channel);
        } else {
            route.microphone_slot = Some(channel);
        }
        self.imported_route_dropdown = None;
        self.imported_route_highlight = None;
    }

    pub fn imported_route(&self, index: usize) -> Option<&ImportedRetakeRoute> {
        let route = self.imported_retake_routes.get(&index)?;
        (route.output_device.0 == self.playback_config.device_id
            && route.output_device.1 == self.playback_config.device_name
            && route.input_device.0 == self.recording_config.device_id
            && route.input_device.1 == self.recording_config.device_name
            && route.input_channels == self.recording_config.channel_mappings)
            .then_some(route)
    }

    pub fn validated_imported_route(
        &self,
        index: usize,
        outputs: usize,
        inputs: usize,
    ) -> Option<(u16, usize)> {
        self.channel_recordings
            .get(index)?
            .imported_source
            .as_ref()?;
        self.imported_session.as_ref()?;
        let route = self.imported_route(index)?;
        let output = route.output_channel?;
        let microphone = route.microphone_slot?;
        let input = *self.recording_config.channel_mappings.get(microphone)?;
        if output >= outputs || input >= inputs || microphone >= self.recording_config.num_channels
        {
            return None;
        }
        Some((u16::try_from(output).ok()?, microphone))
    }

    pub fn capture_take_indices(&self, index: usize) -> Vec<usize> {
        let Some(selected) = self.channel_recordings.get(index) else {
            return Vec::new();
        };
        if selected.imported_source.is_some() {
            return vec![index];
        }
        self.channel_recordings
            .iter()
            .enumerate()
            .filter(|(_, take)| {
                take.imported_source.is_none()
                    && take.channel_index == selected.channel_index
                    && take.mic_position_index == selected.mic_position_index
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub fn has_imported_takes(&self) -> bool {
        self.channel_recordings
            .iter()
            .any(|take| take.imported_source.is_some())
    }

    /// Keep absent imported capture indices distinct from an actual first input/seat.
    pub fn take_capture_indices(&self, take: &ChannelRecording) -> (Option<usize>, Option<usize>) {
        if let Some(source) = &take.imported_source {
            let provenance = self
                .imported_session
                .as_ref()
                .and_then(|session| session.source_provenance(source));
            (
                provenance.and_then(|value| value.mic_index),
                provenance.and_then(|value| value.mic_position_index),
            )
        } else {
            (Some(take.mic_index), Some(take.mic_position_index))
        }
    }

    /// Rebase a file only when it belongs to the directory the wizard moved.
    pub fn rebase_owned_path(path: &mut String, old_dir: &std::path::Path, dir: &std::path::Path) {
        if let Ok(relative) = std::path::Path::new(path).strip_prefix(old_dir) {
            *path = dir.join(relative).to_string_lossy().into_owned();
        }
    }

    pub fn init_channel_recordings(&mut self) {
        self.imported_session = None;
        self.imported_retake_routes.clear();
        self.imported_route_dropdown = None;
        self.imported_route_highlight = None;
        self.take_review.clear();
        self.model.init_channel_recordings();
        self.capture_inputs = Some(self.model.capture_input_snapshot());
    }

    pub fn capture_inputs_are_current(&self) -> bool {
        // Imported responses retain their own capture configuration. Changing
        // the next retake's settings does not invalidate untouched sources.
        if self.has_imported_takes() {
            return true;
        }
        self.capture_inputs
            .as_ref()
            .is_none_or(|inputs| *inputs == self.model.capture_input_snapshot())
    }

    pub fn all_channels_recorded(&self) -> bool {
        self.capture_inputs_are_current() && self.model.all_channels_recorded()
    }

    pub fn save_status(&self) -> sotf_audio_player::ui_models::take_review::RecordingSaveStatus {
        if self.take_review.saved_to.is_none() {
            return sotf_audio_player::ui_models::take_review::RecordingSaveStatus::NotSaved;
        }
        self.take_review
            .save_status(&self.model.save_input_snapshot())
    }

    pub fn all_takes_accepted(&self) -> bool {
        self.capture_inputs_are_current()
            && self
                .take_review
                .all_accepted(&self.model.channel_recordings)
    }

    pub fn accept_take(&mut self, index: usize) -> bool {
        if !self.capture_inputs_are_current() || self.model.workflow_is_busy() {
            return false;
        }
        self.model
            .channel_recordings
            .get_mut(index)
            .is_some_and(|take| self.take_review.accept(take))
    }

    pub fn invalidate_take(&mut self, index: usize) {
        if let Some(take) = self.model.channel_recordings.get(index) {
            self.take_review.invalidate(take);
        }
    }

    pub fn accept_review_needed_for(&mut self, channel_index: usize, position: usize) -> usize {
        let indices: Vec<_> = self
            .model
            .channel_recordings
            .iter()
            .enumerate()
            .filter(|(_, take)| {
                take.channel_index == channel_index
                    && take.mic_position_index == position
                    && take.state == ChannelRecordingState::ReviewNeeded
            })
            .map(|(index, _)| index)
            .collect();
        indices
            .into_iter()
            .filter(|index| self.accept_take(*index))
            .count()
    }

    pub fn prepare_capture_inputs(&mut self) {
        if !self.capture_inputs_are_current() {
            self.model
                .reinitialize_channel_recordings_preserving_ranges();
            self.take_review.clear();
            self.capture_inputs = Some(self.model.capture_input_snapshot());
        } else if self.capture_inputs.is_none() {
            self.capture_inputs = Some(self.model.capture_input_snapshot());
        }
    }

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
