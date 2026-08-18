//! Shared, UI-agnostic view model for the Recording wizard.
//!
//! Domain state (session configuration, capture progress, probe/bass-anchor/SPL
//! results, save metadata) lives here; view state (dropdowns, focus flags, edit
//! buffers) stays in `app-gpui` / `app-tui`.

use crate::recording_helpers::{DEFAULT_SWEEP_END_FREQ, DEFAULT_SWEEP_START_FREQ};
use crate::recording_types::{
    BassAnchorCaptureState, CalibrationData, ChannelRecording, ChannelRecordingState,
    PlaybackDeviceConfig, PlotSmoothing, ProbeCaptureState, RecordingDeviceConfig,
    RecordingSignalType, RecordingStep, RoomDimensionUnit, SplCalibrationCaptureState,
    TransferMatrixLoopbackRecording,
};
use sotf_audio::signal_recorder::CancelFlag;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Default signal level (dBFS) used as the SPL-calibrated reference for
/// capture, delay probe, and bass anchor.
pub const DEFAULT_SIGNAL_LEVEL_DB: f32 = -6.0206;

/// What the UI should do to advance an auto-record batch, computed by
/// [`RecordingScreenModel::batch_next_action`] from state only; the view
/// performs the side effects (start recording, open the move-position
/// modal, save).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchNextAction {
    /// No batch in progress, a capture is in flight, or takes are still
    /// parked for review — stay put.
    Hold,
    /// Start recording `channel_recordings[idx]` (same position).
    StartChannel(usize),
    /// `finished_position` is fully recorded and more positions remain —
    /// prompt the user to move the mics.
    PromptMovePosition {
        /// Position (0-based) that just finished; the next one is +1.
        finished_position: usize,
    },
    /// Every position done — persist the session.
    Save,
}

/// Shared, UI-agnostic domain state for the Recording wizard.
#[derive(Debug, Clone)]
pub struct RecordingScreenModel {
    /// Current step in the recording workflow.
    pub step: RecordingStep,

    // === Config Step State ===
    pub playback_config: PlaybackDeviceConfig,
    pub recording_config: RecordingDeviceConfig,
    /// Single global microphone calibration file path (legacy / simple setups).
    pub mic_calibration_path: Option<String>,
    /// Per-channel microphone calibration file paths (parallel to
    /// `recording_config.channel_mappings`).
    ///
    /// This is the wizard's working copy: the editing accessors, capture
    /// (`recording_helpers::resolve_mic_calibration`) and save
    /// (`build_recording_configuration`) all use it. The same-named field on
    /// `recording_config` is only the serde persistence layer that frontends
    /// load into / save back from this vec.
    pub mic_calibration_paths: Vec<Option<String>>,
    /// Parsed calibration data for display.
    pub mic_calibration_data: Option<CalibrationData>,
    /// Per-channel parsed calibration data for display.
    pub mic_calibration_data_per_channel: Vec<Option<CalibrationData>>,

    // === Capture Step State ===
    pub signal_type: RecordingSignalType,
    pub signal_duration_secs: f32,
    pub signal_level_db: f32,
    /// Sweep start frequency in Hz.
    pub sweep_start_freq: f32,
    /// Sweep end frequency in Hz.
    pub sweep_end_freq: f32,
    pub channel_recordings: Vec<ChannelRecording>,
    pub transfer_matrix_loopbacks: Vec<TransferMatrixLoopbackRecording>,
    pub ctc_reference_sweep_path: Option<String>,
    /// Actual duration (seconds) of the persisted `ctc_reference_sweep.wav`
    /// stimulus, measured from the generated signal at capture time. The
    /// octave-scaled sweep (B1) is self-timed, so `signal_duration_secs` does
    /// NOT describe that WAV — persist this as `CtcConfig::sweep_duration_s`
    /// (`None` when unknown) instead of the nominal knob value.
    pub ctc_reference_sweep_duration_s: Option<f32>,
    pub current_recording_channel: Option<usize>,
    pub recording_progress: f32,
    /// Whether to automatically record all remaining channels.
    pub auto_record_remaining: bool,
    /// Cancel-request flag passed to the sweep-capture engine
    /// (`record_and_analyze` / `record_and_analyze_multi`); polled by the
    /// engine at ~50 ms cadence (R8).
    pub sweep_cancel_requested: CancelFlag,

    /// Directory where recordings will be stored.
    /// Format: `user_selected_dir/recording-YYYYMMDD-HHMMSS/`.
    pub recording_directory: Option<String>,
    /// Base directory selected by the user (before adding timestamp subdirectory).
    pub recording_base_directory: Option<String>,

    // === Probe Step State ===
    pub probe_capture: ProbeCaptureState,
    /// Cancel-request flag polled by the probe-capture engine.
    pub probe_cancel_requested: Arc<AtomicBool>,

    // === Bass Anchor Step State ===
    pub bass_anchor_capture: BassAnchorCaptureState,
    pub bass_anchor_cancel_requested: Arc<AtomicBool>,

    // === SPL Calibration Step State ===
    pub spl_calibration_capture: SplCalibrationCaptureState,
    pub spl_cancel_requested: Arc<AtomicBool>,

    // === Evaluating Step State ===
    /// Selected channel filter for plots (`None` = all channels).
    pub plot_selected_channel: Option<usize>,
    pub plot_smoothing: PlotSmoothing,

    // === Saving Step State ===
    pub save_name: String,
    /// Room width — interpreted in `room_dimension_unit`. Zero means
    /// "not specified" and is skipped during serialization.
    pub room_width_input: f64,
    /// Room depth — interpreted in `room_dimension_unit`. Zero means
    /// "not specified".
    pub room_depth_input: f64,
    /// Room height — interpreted in `room_dimension_unit`. Zero means
    /// "not specified".
    pub room_height_input: f64,
    /// Unit the three `room_*_input` values are expressed in.
    pub room_dimension_unit: RoomDimensionUnit,
    /// Free-form description of the listening setup.
    pub setup_description: String,
    /// Per-channel speaker identity (brand + model). Indices align with the
    /// physical playback channel list.
    pub channel_speakers: Vec<String>,

    // === Advanced measurement quality settings ===
    /// Default 3.0 s/octave below 100 Hz.
    pub bass_octave_duration_s: f32,
    /// Default 2.0 s noise-floor window before the sweep.
    pub pre_silence_s: f32,
    /// `None` = derive from RT60 estimate; `Some(x)` = user-specified.
    pub post_silence_s: Option<f32>,
    /// Sweeps captured per channel (Task 8/9). UI range: 1 (single sweep) or
    /// 3–8 — 2 is never offered because two takes cannot reject outliers
    /// (the engine clamps it up to 3 anyway). Default
    /// `sotf_audio::signal_recorder::DEFAULT_NUM_SWEEPS` (4).
    pub num_sweeps: u16,

    // === Noise Floor Warning ===
    pub noise_floor_warning: Option<String>,

    // === Multi-Position Modal State ===
    pub move_position_modal_open: bool,
    pub pending_next_position: Option<usize>,

    // === Migration Modal State ===
    pub migration_modal_open: bool,
    pub migration_file_path: Option<String>,
    pub migration_file_dir: Option<String>,
    pub migration_file_size: Option<u64>,
    pub migration_channel_count: usize,
    pub migration_pending_json: Option<String>,

    /// Short status message surfaced by the view.
    pub status_message: String,
}

impl Default for RecordingScreenModel {
    fn default() -> Self {
        Self {
            step: RecordingStep::Config,
            playback_config: PlaybackDeviceConfig::default(),
            recording_config: RecordingDeviceConfig::default(),
            mic_calibration_path: None,
            mic_calibration_paths: Vec::new(),
            mic_calibration_data: None,
            mic_calibration_data_per_channel: Vec::new(),
            signal_type: RecordingSignalType::Sweep,
            signal_duration_secs: 5.0,
            signal_level_db: DEFAULT_SIGNAL_LEVEL_DB,
            sweep_start_freq: DEFAULT_SWEEP_START_FREQ,
            sweep_end_freq: DEFAULT_SWEEP_END_FREQ,
            channel_recordings: Vec::new(),
            transfer_matrix_loopbacks: Vec::new(),
            ctc_reference_sweep_path: None,
            ctc_reference_sweep_duration_s: None,
            current_recording_channel: None,
            recording_progress: 0.0,
            auto_record_remaining: false,
            sweep_cancel_requested: CancelFlag::default(),
            recording_directory: None,
            recording_base_directory: None,
            probe_capture: ProbeCaptureState::default(),
            probe_cancel_requested: Arc::new(AtomicBool::new(false)),
            bass_anchor_capture: BassAnchorCaptureState::default(),
            bass_anchor_cancel_requested: Arc::new(AtomicBool::new(false)),
            spl_calibration_capture: SplCalibrationCaptureState::default(),
            spl_cancel_requested: Arc::new(AtomicBool::new(false)),
            plot_selected_channel: None,
            plot_smoothing: PlotSmoothing::None,
            save_name: "recording".to_string(),
            room_width_input: 0.0,
            room_depth_input: 0.0,
            room_height_input: 0.0,
            room_dimension_unit: RoomDimensionUnit::default(),
            setup_description: String::new(),
            channel_speakers: Vec::new(),
            bass_octave_duration_s: 3.0,
            pre_silence_s: 2.0,
            post_silence_s: None,
            num_sweeps: sotf_audio::signal_recorder::DEFAULT_NUM_SWEEPS,
            noise_floor_warning: None,
            move_position_modal_open: false,
            pending_next_position: None,
            migration_modal_open: false,
            migration_file_path: None,
            migration_file_dir: None,
            migration_file_size: None,
            migration_channel_count: 0,
            migration_pending_json: None,
            status_message: String::new(),
        }
    }
}

impl RecordingScreenModel {
    /// Filesystem-safe name used for the saved recording directory and
    /// session-level file names.
    pub fn safe_save_name(&self) -> String {
        crate::recording_helpers::sanitize_recording_name(&self.save_name)
    }

    /// Directory implied by the user-selected base directory and current
    /// save name.
    pub fn named_recording_directory(&self) -> Option<std::path::PathBuf> {
        self.recording_base_directory
            .as_ref()
            .map(|base| std::path::Path::new(base).join(self.safe_save_name()))
    }

    /// Initialize channel recordings from playback × recording × position
    /// configuration. Order is `position-major, speaker-mid, mic-minor` so
    /// every (speaker, mic) pair at position 0 comes before any entry at
    /// position 1.
    pub fn init_channel_recordings(&mut self) {
        let raw_num_mics = self.recording_config.channel_mappings.len();
        let num_mics = raw_num_mics.max(1);
        let num_positions = self.recording_config.num_positions.max(1);
        let speakers = &self.playback_config.channel_mappings;

        let mut out: Vec<ChannelRecording> =
            Vec::with_capacity(num_positions * speakers.len() * num_mics);

        for pos_idx in 0..num_positions {
            for (speaker_idx, mapping) in speakers.iter().enumerate() {
                for mic_idx in 0..num_mics {
                    let name = match (num_positions > 1, raw_num_mics > 1) {
                        (false, false) => mapping.group_name.clone(),
                        (false, true) => {
                            format!("{} (Mic {})", mapping.group_name, mic_idx + 1)
                        }
                        (true, false) => {
                            format!("{} (Pos {})", mapping.group_name, pos_idx + 1)
                        }
                        (true, true) => format!(
                            "{} (Pos {} / Mic {})",
                            mapping.group_name,
                            pos_idx + 1,
                            mic_idx + 1
                        ),
                    };
                    out.push(ChannelRecording::with_mic_position(
                        speaker_idx,
                        name,
                        mic_idx,
                        pos_idx,
                    ));
                }
            }
        }

        self.channel_recordings = out;
        self.transfer_matrix_loopbacks.clear();
        // Reset the CTC reference sweep together with its measured duration
        // (task 10: path and duration describe one artifact; never clear one
        // without the other).
        self.ctc_reference_sweep_path = None;
        self.ctc_reference_sweep_duration_s = None;
    }

    /// Position index (0-based) of the next pending recording.
    pub fn current_position(&self) -> usize {
        let num_positions = self.recording_config.num_positions.max(1);
        self.channel_recordings
            .iter()
            .filter(|r| r.state != ChannelRecordingState::Done)
            .map(|r| r.mic_position_index)
            .min()
            .unwrap_or(num_positions)
    }

    /// True when every recording with `mic_position_index == pos` is `Done`.
    pub fn position_complete(&self, pos: usize) -> bool {
        let mut saw_any = false;
        for r in &self.channel_recordings {
            if r.mic_position_index == pos {
                saw_any = true;
                if r.state != ChannelRecordingState::Done {
                    return false;
                }
            }
        }
        saw_any
    }

    /// Index (into `channel_recordings`) of the next recording at `pos`
    /// whose `channel_index` (speaker) hasn't been started yet.
    pub fn next_channel_in_position(&self, pos: usize) -> Option<usize> {
        let mut seen = std::collections::HashSet::new();
        self.channel_recordings
            .iter()
            .enumerate()
            .find(|(_, r)| {
                r.mic_position_index == pos
                    && r.state == ChannelRecordingState::Empty
                    && seen.insert(r.channel_index)
            })
            .map(|(idx, _)| idx)
    }

    /// Check if all channels have been recorded.
    pub fn all_channels_recorded(&self) -> bool {
        !self.channel_recordings.is_empty()
            && self
                .channel_recordings
                .iter()
                .all(|r| r.state == ChannelRecordingState::Done)
    }

    /// Check if any recording is in progress.
    pub fn is_recording(&self) -> bool {
        self.current_recording_channel.is_some()
    }

    /// True while a sweep capture is in flight (any channel in the
    /// `Recording` state). Unlike `is_recording`, this is independent of
    /// the UI cursor (`current_recording_channel`).
    pub fn capture_in_progress(&self) -> bool {
        self.channel_recordings
            .iter()
            .any(|r| r.state == ChannelRecordingState::Recording)
    }

    /// Indices of channels parked in `ReviewNeeded` (capture succeeded but
    /// the per-take quality verdict was not trustworthy — Task 9).
    pub fn review_needed_indices(&self) -> Vec<usize> {
        self.channel_recordings
            .iter()
            .enumerate()
            .filter(|(_, r)| r.state == ChannelRecordingState::ReviewNeeded)
            .map(|(idx, _)| idx)
            .collect()
    }

    /// Accept every take parked in `ReviewNeeded`, marking them `Done` so
    /// the save paths pick them up. The take keeps its
    /// `RecordingResult.quality` (`trustworthy == false`), which is how the
    /// UIs distinguish accepted-with-warning from clean `Done` without a new
    /// state. Returns the number of takes accepted.
    pub fn accept_all_review_needed(&mut self) -> usize {
        let mut accepted = 0;
        for r in &mut self.channel_recordings {
            if r.state == ChannelRecordingState::ReviewNeeded {
                r.state = ChannelRecordingState::Done;
                accepted += 1;
            }
        }
        accepted
    }

    /// Accept the parked takes for one (speaker, position) pair — the GPUI
    /// per-row "accept anyway" affordance. Returns the number accepted.
    pub fn accept_review_needed_for(
        &mut self,
        channel_index: usize,
        mic_position_index: usize,
    ) -> usize {
        let mut accepted = 0;
        for r in &mut self.channel_recordings {
            if r.state == ChannelRecordingState::ReviewNeeded
                && r.channel_index == channel_index
                && r.mic_position_index == mic_position_index
            {
                r.state = ChannelRecordingState::Done;
                accepted += 1;
            }
        }
        accepted
    }

    /// Session quality summary: one line per recorded channel with score +
    /// warnings (see [`crate::recording_helpers::session_quality_summary`]).
    pub fn session_quality_summary(&self) -> Vec<String> {
        crate::recording_helpers::session_quality_summary(&self.channel_recordings)
    }

    /// Structured session quality lines (text + severity kind) for UIs that
    /// color by verdict (see [`crate::recording_helpers::session_quality_lines`]).
    pub fn session_quality_lines(
        &self,
    ) -> Vec<(String, crate::recording_helpers::SessionQualityLineKind)> {
        crate::recording_helpers::session_quality_lines(&self.channel_recordings)
    }

    /// Request cancellation of the in-flight sweep capture. The engine
    /// polls the flag (~50 ms cadence) and returns
    /// `Err(CANCELLED_ERR)`, which the frontends map back to idle state.
    pub fn request_sweep_cancel(&self) {
        self.sweep_cancel_requested
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Decide how an auto-record batch advances after a capture at
    /// `finished_position` completes — or after warned takes were accepted
    /// mid-batch (task-9 review R1, same decision). Pure state read; the
    /// view performs the side effects (start recording, open the
    /// move-position modal, save).
    ///
    /// `finished_position` is the 0-based position whose capture just
    /// completed (or whose parked takes were just accepted). Do NOT pass
    /// [`Self::current_position`]: once a position is fully `Done` it
    /// advances to the next one, which would skip the move-position prompt
    /// and record the next seat without the mics being moved.
    ///
    /// `auto_record_remaining` is kept set while a batch is parked for
    /// review, so it doubles as "a batch is in progress" intent. Takes
    /// still parked (`ReviewNeeded`) hold the batch: the user must resolve
    /// every warned take before it moves on.
    pub fn batch_next_action(&self, finished_position: usize) -> BatchNextAction {
        if !self.auto_record_remaining
            || self.capture_in_progress()
            || !self.review_needed_indices().is_empty()
        {
            return BatchNextAction::Hold;
        }
        if let Some(idx) = self.next_channel_in_position(finished_position) {
            return BatchNextAction::StartChannel(idx);
        }
        // No Empty entries left at `finished_position` (parked takes were
        // excluded above): the position is complete.
        let num_positions = self.recording_config.num_positions.max(1);
        if finished_position + 1 < num_positions {
            BatchNextAction::PromptMovePosition { finished_position }
        } else {
            BatchNextAction::Save
        }
    }

    /// Reset the sweep cancel flag before starting a fresh capture so a
    /// stale request cannot abort the new run.
    pub fn reset_sweep_cancel(&self) {
        self.sweep_cancel_requested
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// Ensure `channel_speakers` has one slot per physical playback
    /// channel.
    pub fn sync_channel_speakers_length(&mut self) {
        self.channel_speakers
            .resize(self.playback_config.channel_mappings.len(), String::new());
    }

    /// Build the canonical-metric [`RoomDimensions`] to persist in
    /// `RecordingConfiguration`. Returns `None` when the user left any
    /// dimension blank (zero).
    pub fn room_dimensions_for_save(&self) -> Option<autoeq::roomeq::RoomDimensions> {
        if self.room_width_input <= 0.0
            || self.room_depth_input <= 0.0
            || self.room_height_input <= 0.0
        {
            return None;
        }
        let unit = self.room_dimension_unit;
        Some(autoeq::roomeq::RoomDimensions {
            // `length` in RoomDimensions is the depth of the room
            // (front-to-back distance); the UI exposes it as "Depth".
            length: unit.to_meters(self.room_depth_input),
            width: unit.to_meters(self.room_width_input),
            height: unit.to_meters(self.room_height_input),
        })
    }

    /// Build the `channel_name → "brand model"` map persisted in
    /// `RecordingConfiguration`. Returns `None` when every entry is blank.
    pub fn channel_speakers_map_for_save(&self) -> Option<HashMap<String, String>> {
        let mut map = HashMap::new();
        for (i, mapping) in self.playback_config.channel_mappings.iter().enumerate() {
            if let Some(name) = self.channel_speakers.get(i) {
                let trimmed = name.trim();
                if !trimmed.is_empty() {
                    map.insert(mapping.group_name.clone(), trimmed.to_string());
                }
            }
        }
        if map.is_empty() { None } else { Some(map) }
    }

    /// Build the complete [`autoeq::roomeq::RecordingConfiguration`]
    /// persisted alongside the measurements in `recordings.json`.
    ///
    /// Single canonical builder shared by all frontends (fixes B4: the TUI
    /// previously dropped bass-anchor / SPL / sweep metadata via
    /// `..Default::default()`). The GD-Opt sweep metadata
    /// (`bass_octave_duration_s`, `pre_silence_s`, `post_silence_s`) is only
    /// persisted when the actual stimulus is a sweep — frontends must route
    /// sweep capture through
    /// [`crate::recording_helpers::capture_signal_params`] with these same
    /// values so the saved metadata describes the audio that was really
    /// played (B1). The values are clamped here exactly as the engine's
    /// `sweep_params_from_config` clamps them, so the two can never diverge.
    ///
    /// `recording_directory` is the session output directory; `None` or an
    /// empty string omits the field.
    pub fn build_recording_configuration(
        &self,
        recording_directory: Option<&str>,
    ) -> autoeq::roomeq::RecordingConfiguration {
        let is_sweep = self.signal_type == RecordingSignalType::Sweep;
        autoeq::roomeq::RecordingConfiguration {
            playback_device_name: Some(self.playback_config.device_name.clone()),
            playback_device_id: Some(self.playback_config.device_id.clone()),
            playback_sample_rate: Some(self.playback_config.sample_rate),
            playback_channels: Some(self.playback_config.num_channels),
            speaker_configuration: Some(
                self.playback_config
                    .speaker_configuration
                    .as_str()
                    .to_string(),
            ),
            channel_names: Some(
                self.playback_config
                    .channel_mappings
                    .iter()
                    .map(|m| m.group_name.clone())
                    .collect(),
            ),
            recording_device_name: Some(self.recording_config.device_name.clone()),
            recording_device_id: Some(self.recording_config.device_id.clone()),
            recording_sample_rate: Some(self.recording_config.sample_rate),
            recording_channels: Some(self.recording_config.num_channels),
            mic_calibration_path: self.mic_calibration_path.clone().filter(|s| !s.is_empty()),
            mic_calibration_paths: if self.mic_calibration_paths.is_empty() {
                None
            } else {
                Some(self.mic_calibration_paths.clone())
            },
            recording_directory: recording_directory
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            signal_type: Some(self.signal_type.as_str().to_string()),
            signal_duration_secs: Some(self.signal_duration_secs),
            signal_level_db: Some(self.signal_level_db),
            sweep_start_freq: Some(self.sweep_start_freq),
            sweep_end_freq: Some(self.sweep_end_freq),
            room_dimensions: self.room_dimensions_for_save(),
            setup_description: {
                let s = self.setup_description.trim();
                if s.is_empty() {
                    None
                } else {
                    Some(s.to_string())
                }
            },
            channel_speakers: self.channel_speakers_map_for_save(),
            // Translate the engine `ProbeDelayResults` into the
            // autoeq-local `ProbeResultsLegacy` mirror so the RoomConfig
            // JSON only depends on autoeq types.
            probe_results: self.probe_capture.results.as_ref().map(|r| {
                autoeq::roomeq::ProbeResultsLegacy {
                    channels: r
                        .channels
                        .iter()
                        .map(|c| autoeq::roomeq::ProbeChannelResultLegacy {
                            channel_name: c.channel_name.clone(),
                            channel_index: c.channel_index,
                            arrival_ms: c.arrival_ms,
                            gain_db: c.gain_db,
                            snr_db: c.snr_db,
                        })
                        .collect(),
                    sample_rate: r.sample_rate,
                    alignment_delays_ms: r.alignment_delays_ms.clone(),
                }
            }),
            probe_wav_relative: self
                .probe_capture
                .wav_path
                .as_ref()
                .and_then(|p| std::path::Path::new(p).file_name())
                .map(|f| f.to_string_lossy().to_string()),
            bass_anchor_results: self.bass_anchor_capture.results.as_ref().map(|r| {
                autoeq::roomeq::BassAnchorResultsLegacy {
                    channels: r
                        .channels
                        .iter()
                        .map(|c| autoeq::roomeq::BassAnchorChannelResultLegacy {
                            channel_name: c.channel_name.clone(),
                            channel_index: c.channel_index,
                            bass_anchor_phase_deg: c.bass_anchor_phase_deg,
                            bass_anchor_magnitude: c.bass_anchor_magnitude,
                            bass_anchor_stability_deg: c.bass_anchor_stability_deg,
                            bass_anchor_loopback_phase_deg: c.bass_anchor_loopback_phase_deg,
                            bass_anchor_coherence: c.bass_anchor_coherence,
                        })
                        .collect(),
                    sample_rate: r.sample_rate,
                    bass_freq_hz: r.bass_freq_hz,
                    bass_duration_s: r.bass_duration_s,
                }
            }),
            bass_anchor_wav_relative: self
                .bass_anchor_capture
                .wav_path
                .as_ref()
                .and_then(|p| std::path::Path::new(p).file_name())
                .map(|f| f.to_string_lossy().to_string()),
            // GD-Opt v2 sweep metadata — only persisted when the stimulus
            // is actually a sweep generated through `capture_signal_params`
            // with these same values (B1). Clamped exactly like the engine's
            // `sweep_params_from_config` (bass to [1.0, 10.0], silences to
            // >= 0.0) so persisted metadata can never diverge from the
            // actual stimulus.
            bass_octave_duration_s: is_sweep
                .then_some(self.bass_octave_duration_s.clamp(1.0, 10.0)),
            pre_silence_s: is_sweep.then_some(self.pre_silence_s.max(0.0)),
            post_silence_s: if is_sweep {
                self.post_silence_s.map(|s| s.max(0.0))
            } else {
                None
            },
            // Remaining GD-Opt v2 fields (later phases): leave as None.
            sweep_level_db_spl: None,
            // Truthful take count (Task 8/9): the minimum engine-reported
            // accepted-take count across completed channels, NOT the
            // requested `self.num_sweeps` — a rejected outlier take must not
            // be covered up. None when no channel carries quality data
            // (legacy / loaded sessions).
            num_sweeps: crate::recording_helpers::accepted_num_sweeps_for_save(
                &self.channel_recordings,
            ),
            coherence_threshold: None,
            bass_probe_freq_hz: Some(self.bass_anchor_capture.bass_freq_hz),
            bass_probe_duration_s: Some(self.bass_anchor_capture.bass_duration_s),
            mic_phase_calibration_path: None,
            mic_phase_calibration_paths: None,
            spl_calibration: self.spl_calibration_capture.to_spl_calibration(),
            recording_seed: None,
            num_positions: {
                let n = self.recording_config.num_positions.max(1);
                if n > 1 { Some(n) } else { None }
            },
        }
    }

    /// Currently-active mic-calibration path string (read-only). Returns
    /// `""` when `editing_channel` is `None` or when the channel slot is empty.
    ///
    /// Reads the model-level `mic_calibration_paths` — the wizard's working
    /// copy that capture (`resolve_mic_calibration`) and save
    /// (`build_recording_configuration`) both consult. The
    /// `recording_config.mic_calibration_paths` field remains only as the
    /// serde persistence layer frontends load from / save back to.
    pub fn active_mic_cal_path(&self, editing_channel: Option<usize>) -> &str {
        match editing_channel {
            Some(ch) => self
                .mic_calibration_paths
                .get(ch)
                .and_then(|o| o.as_deref())
                .unwrap_or(""),
            None => "",
        }
    }

    /// Mutable reference to the mic-calibration path for `editing_channel`,
    /// growing the underlying `Vec` and lazily inserting an empty `String`
    /// in the slot if needed. Operates on the model-level
    /// `mic_calibration_paths` (see [`Self::active_mic_cal_path`]).
    pub fn active_mic_cal_path_mut(
        &mut self,
        editing_channel: Option<usize>,
    ) -> Option<&mut String> {
        let ch = editing_channel?;
        let paths = &mut self.mic_calibration_paths;
        while paths.len() <= ch {
            paths.push(None);
        }
        if paths[ch].is_none() {
            paths[ch] = Some(String::new());
        }
        paths[ch].as_mut()
    }

    /// Replace the mic-calibration path for `editing_channel`, normalising an
    /// empty string back to `None`. Operates on the model-level
    /// `mic_calibration_paths` (see [`Self::active_mic_cal_path`]).
    pub fn set_active_mic_cal_path(&mut self, editing_channel: Option<usize>, val: String) {
        if let Some(ch) = editing_channel {
            let paths = &mut self.mic_calibration_paths;
            while paths.len() <= ch {
                paths.push(None);
            }
            paths[ch] = if val.is_empty() { None } else { Some(val) };
        }
    }

    /// Resize the mic-calibration and recording-channel-mapping vecs to
    /// match `num_channels`. The mic-calibration working copy is the
    /// model-level `mic_calibration_paths` (see [`Self::active_mic_cal_path`]);
    /// `channel_mappings` still lives on `recording_config`.
    pub fn sync_recording_channel_vecs(&mut self) {
        let target = self.recording_config.num_channels.max(1);
        let cm = &mut self.recording_config.channel_mappings;
        while cm.len() < target {
            cm.push(cm.len());
        }
        cm.truncate(target);

        let cal = &mut self.mic_calibration_paths;
        while cal.len() < target {
            cal.push(None);
        }
        cal.truncate(target);
    }

    /// How many fields are in the Save-step form given the current
    /// channel list. Layout:
    ///   0        save_name
    ///   1..=3    room width / depth / height
    ///   4        unit toggle
    ///   5        setup description
    ///   6..6+N-1 per-playback-channel speaker entries
    pub fn save_field_count(&self) -> usize {
        6 + self.playback_config.channel_mappings.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording_types::{
        BassAnchorChannelResult, BassAnchorResults, DelayProbeChannelResult, DelayProbeResults,
        SplCalibrationResult,
    };

    #[test]
    fn build_recording_configuration_persists_devices_and_signal() {
        let mut model = RecordingScreenModel::default();
        model.playback_config.device_name = "DAC".to_string();
        model.recording_config.device_name = "Mic".to_string();
        model.recording_config.sample_rate = 96_000;
        model.mic_calibration_path = Some("global-cal.txt".to_string());
        model.mic_calibration_paths = vec![Some("mic0.txt".to_string()), None];

        let config = model.build_recording_configuration(Some("/tmp/session"));

        assert_eq!(config.playback_device_name.as_deref(), Some("DAC"));
        assert_eq!(config.recording_device_name.as_deref(), Some("Mic"));
        assert_eq!(config.recording_sample_rate, Some(96_000));
        assert_eq!(config.signal_type.as_deref(), Some("Sweep"));
        assert_eq!(
            config.mic_calibration_path.as_deref(),
            Some("global-cal.txt")
        );
        assert_eq!(
            config.mic_calibration_paths,
            Some(vec![Some("mic0.txt".to_string()), None])
        );
        assert_eq!(config.recording_directory.as_deref(), Some("/tmp/session"));
        assert_eq!(config.num_positions, None);
    }

    #[test]
    fn build_recording_configuration_gates_sweep_metadata_on_signal_type() {
        // Sweep sessions persist the GD-Opt metadata the capture used.
        let sweep = RecordingScreenModel::default().build_recording_configuration(None);
        assert_eq!(sweep.bass_octave_duration_s, Some(3.0));
        assert_eq!(sweep.pre_silence_s, Some(2.0));
        assert_eq!(sweep.post_silence_s, None);
        assert_eq!(sweep.recording_directory, None);

        // Non-sweep stimuli must not claim sweep metadata (B1).
        let mut noise = RecordingScreenModel::default();
        noise.signal_type = RecordingSignalType::PinkNoise;
        let noise = noise.build_recording_configuration(None);
        assert_eq!(noise.bass_octave_duration_s, None);
        assert_eq!(noise.pre_silence_s, None);
        assert_eq!(noise.post_silence_s, None);
    }

    #[test]
    fn build_recording_configuration_includes_probe_bass_anchor_and_spl() {
        let mut model = RecordingScreenModel::default();
        model.probe_capture.results = Some(DelayProbeResults {
            channels: vec![DelayProbeChannelResult {
                channel_name: "L".to_string(),
                channel_index: 0,
                arrival_ms: 1.5,
                gain_db: -2.0,
                snr_db: 40.0,
            }],
            sample_rate: 48_000,
            alignment_delays_ms: vec![0.0],
        });
        model.probe_capture.wav_path = Some("/tmp/session/probe.wav".to_string());
        model.bass_anchor_capture.results = Some(BassAnchorResults {
            channels: vec![BassAnchorChannelResult {
                channel_name: "L".to_string(),
                channel_index: 0,
                bass_anchor_phase_deg: 12.0,
                bass_anchor_magnitude: 0.8,
                bass_anchor_stability_deg: 5.0,
                bass_anchor_loopback_phase_deg: Some(1.0),
                bass_anchor_coherence: Some(0.95),
            }],
            sample_rate: 48_000,
            bass_freq_hz: 30.0,
            bass_duration_s: 2.0,
        });
        model.spl_calibration_capture.engine_result = Some(SplCalibrationResult {
            sample_rate: 48_000,
            peak_sample_level: 0.4,
            rms_sample_level: 0.25,
            reference_freq_hz: 1000.0,
            output_channel: 0,
        });
        model.spl_calibration_capture.reported_db_spl = Some(75.0);

        let config = model.build_recording_configuration(None);

        let probe = config.probe_results.expect("probe results persisted");
        assert_eq!(probe.channels.len(), 1);
        assert_eq!(probe.channels[0].channel_name, "L");
        assert_eq!(probe.channels[0].arrival_ms, 1.5);
        assert_eq!(probe.sample_rate, 48_000);
        assert_eq!(config.probe_wav_relative.as_deref(), Some("probe.wav"));

        let bass = config.bass_anchor_results.expect("bass anchor persisted");
        assert_eq!(bass.channels.len(), 1);
        assert_eq!(bass.channels[0].bass_anchor_phase_deg, 12.0);
        assert_eq!(bass.bass_freq_hz, 30.0);

        let spl = config.spl_calibration.expect("spl calibration persisted");
        assert_eq!(spl.reported_db_spl, 75.0);
        assert_eq!(spl.reference_freq_hz, 1000.0);
    }

    #[test]
    fn build_recording_configuration_num_positions_only_when_multi() {
        let mut model = RecordingScreenModel::default();
        model.recording_config.num_positions = 3;
        assert_eq!(
            model.build_recording_configuration(None).num_positions,
            Some(3)
        );
        model.recording_config.num_positions = 1;
        assert_eq!(
            model.build_recording_configuration(None).num_positions,
            None
        );
    }

    #[test]
    fn build_recording_configuration_clamps_sweep_metadata_like_engine() {
        // Same clamps as the engine's sweep_params_from_config: bass to
        // [1.0, 10.0], silences to >= 0.0 — so persisted metadata matches
        // the actual stimulus.
        let mut model = RecordingScreenModel::default();
        model.bass_octave_duration_s = 0.5;
        model.pre_silence_s = -1.0;
        model.post_silence_s = Some(-5.0);
        let config = model.build_recording_configuration(None);
        assert_eq!(config.bass_octave_duration_s, Some(1.0));
        assert_eq!(config.pre_silence_s, Some(0.0));
        assert_eq!(config.post_silence_s, Some(0.0));

        let mut model = RecordingScreenModel::default();
        model.bass_octave_duration_s = 42.0;
        let config = model.build_recording_configuration(None);
        assert_eq!(config.bass_octave_duration_s, Some(10.0));

        // In-range values pass through unchanged.
        let mut model = RecordingScreenModel::default();
        model.bass_octave_duration_s = 4.5;
        model.pre_silence_s = 1.5;
        model.post_silence_s = Some(3.0);
        let config = model.build_recording_configuration(None);
        assert_eq!(config.bass_octave_duration_s, Some(4.5));
        assert_eq!(config.pre_silence_s, Some(1.5));
        assert_eq!(config.post_silence_s, Some(3.0));
    }

    #[test]
    fn mic_cal_accessors_use_model_level_vec() {
        let mut model = RecordingScreenModel::default();
        // Pre-existing persisted config storage must stay untouched.
        model.recording_config.mic_calibration_paths = vec![Some("persisted.txt".to_string())];

        model.set_active_mic_cal_path(Some(1), "mic1.txt".to_string());
        assert_eq!(model.mic_calibration_paths.len(), 2);
        assert_eq!(model.mic_calibration_paths[1].as_deref(), Some("mic1.txt"));
        assert_eq!(model.active_mic_cal_path(Some(1)), "mic1.txt");
        assert_eq!(model.active_mic_cal_path(Some(0)), "");
        assert_eq!(model.active_mic_cal_path(None), "");
        // The config-level (serde) vec is NOT written by the wizard.
        assert_eq!(
            model.recording_config.mic_calibration_paths,
            vec![Some("persisted.txt".to_string())]
        );

        // Empty strings normalise back to None.
        model.set_active_mic_cal_path(Some(1), String::new());
        assert_eq!(model.mic_calibration_paths[1], None);
    }

    #[test]
    fn active_mic_cal_path_mut_grows_model_level_vec() {
        let mut model = RecordingScreenModel::default();
        let slot = model
            .active_mic_cal_path_mut(Some(2))
            .expect("slot for in-range channel");
        slot.push_str("edited.txt");
        assert_eq!(model.mic_calibration_paths.len(), 3);
        assert_eq!(
            model.mic_calibration_paths[2].as_deref(),
            Some("edited.txt")
        );
        assert!(model.recording_config.mic_calibration_paths.len() <= 1);
        assert!(model.active_mic_cal_path_mut(None).is_none());
    }

    #[test]
    fn sweep_cancel_accessors_drive_shared_flag() {
        use std::sync::atomic::Ordering;

        let model = RecordingScreenModel::default();
        // Starts disarmed so a fresh capture is not aborted by default.
        assert!(!model.sweep_cancel_requested.load(Ordering::Relaxed));

        model.request_sweep_cancel();
        assert!(model.sweep_cancel_requested.load(Ordering::Relaxed));
        // The flag is shared: a clone handed to the capture thread sees
        // the same state.
        let shared = model.sweep_cancel_requested.clone();
        assert!(shared.load(Ordering::Relaxed));

        model.reset_sweep_cancel();
        assert!(!shared.load(Ordering::Relaxed));
    }

    #[test]
    fn capture_in_progress_tracks_recording_state_not_cursor() {
        let mut model = RecordingScreenModel::default();
        model.channel_recordings = vec![ChannelRecording::new(0, "FL".to_string())];
        assert!(!model.capture_in_progress());

        model.channel_recordings[0].state = ChannelRecordingState::Recording;
        assert!(model.capture_in_progress());

        model.channel_recordings[0].state = ChannelRecordingState::Done;
        assert!(!model.capture_in_progress());
    }

    #[test]
    fn sync_recording_channel_vecs_resizes_model_level_cal_vec() {
        let mut model = RecordingScreenModel::default();
        model.recording_config.num_channels = 3;
        model.mic_calibration_paths = vec![Some("mic0.txt".to_string())];
        model.sync_recording_channel_vecs();
        assert_eq!(model.recording_config.channel_mappings.len(), 3);
        assert_eq!(model.mic_calibration_paths.len(), 3);
        assert_eq!(model.mic_calibration_paths[0].as_deref(), Some("mic0.txt"));
        assert_eq!(model.mic_calibration_paths[1], None);

        // Shrinking num_channels truncates the model-level vec too.
        model.recording_config.num_channels = 1;
        model.sync_recording_channel_vecs();
        assert_eq!(model.mic_calibration_paths.len(), 1);
        assert_eq!(model.recording_config.channel_mappings.len(), 1);
    }

    // === Task 9: num_sweeps plumbing + quality-gate accept flow ===

    use crate::recording_types::{RecordingResult, TakeQualitySummary};

    fn done_channel_with_quality(
        channel_index: usize,
        name: &str,
        state: ChannelRecordingState,
        accepted: usize,
    ) -> ChannelRecording {
        let mut ch = ChannelRecording::new(channel_index, name.to_string());
        ch.state = state;
        ch.result = Some(RecordingResult {
            channel: channel_index,
            wav_path: None,
            csv_path: None,
            frequencies: vec![],
            magnitude_db: vec![],
            phase_deg: vec![],
            impulse_response: None,
            impulse_time_ms: None,
            thd_percent: None,
            harmonic_distortion_db: None,
            excess_group_delay_ms: None,
            rt60_ms: None,
            clarity_c50_db: None,
            clarity_c80_db: None,
            spectrogram_db: None,
            quality: Some(TakeQualitySummary {
                trustworthy: true,
                score: 0.9,
                issues: vec![],
                mean_coherence: None,
                median_snr_db: None,
                clip_fraction: 0.0,
                drift_ppm: None,
                drift_corrected: false,
                dropped_samples: 0,
                accepted_count: accepted,
                rejected_count: 0,
            }),
        });
        ch
    }

    #[test]
    fn num_sweeps_defaults_to_engine_default() {
        let model = RecordingScreenModel::default();
        assert_eq!(
            model.num_sweeps,
            sotf_audio::signal_recorder::DEFAULT_NUM_SWEEPS
        );
    }

    #[test]
    fn build_recording_configuration_persists_truthful_num_sweeps() {
        // Without capture quality data the field stays unknown (None) — the
        // requested count is never persisted as if it were measured.
        let mut model = RecordingScreenModel::default();
        model.num_sweeps = 5;
        assert_eq!(model.build_recording_configuration(None).num_sweeps, None);

        // With captures, the persisted value is the minimum accepted-take
        // count across Done channels, not the requested 5.
        model.channel_recordings = vec![
            done_channel_with_quality(0, "FL", ChannelRecordingState::Done, 4),
            done_channel_with_quality(1, "FR", ChannelRecordingState::Done, 3),
            // A parked (not yet accepted) take must not count.
            done_channel_with_quality(2, "C", ChannelRecordingState::ReviewNeeded, 1),
        ];
        assert_eq!(
            model.build_recording_configuration(None).num_sweeps,
            Some(3)
        );
    }

    #[test]
    fn accept_all_review_needed_marks_done_and_counts() {
        let mut model = RecordingScreenModel::default();
        model.channel_recordings = vec![
            done_channel_with_quality(0, "FL", ChannelRecordingState::ReviewNeeded, 4),
            done_channel_with_quality(1, "FR", ChannelRecordingState::Done, 4),
            done_channel_with_quality(2, "C", ChannelRecordingState::Empty, 4),
        ];
        assert_eq!(model.review_needed_indices(), vec![0]);
        assert_eq!(model.accept_all_review_needed(), 1);
        assert_eq!(
            model.channel_recordings[0].state,
            ChannelRecordingState::Done
        );
        // The accepted take keeps its quality summary (OK* distinction).
        assert!(model.channel_recordings[0]
            .result
            .as_ref()
            .and_then(|r| r.quality.as_ref())
            .is_some());
        assert!(model.review_needed_indices().is_empty());
    }

    #[test]
    fn accept_review_needed_for_scopes_to_speaker_and_position() {
        let mut model = RecordingScreenModel::default();
        let mut pos1 = done_channel_with_quality(0, "FL (Pos 2)", ChannelRecordingState::ReviewNeeded, 4);
        pos1.mic_position_index = 1;
        model.channel_recordings = vec![
            done_channel_with_quality(0, "FL (Pos 1)", ChannelRecordingState::ReviewNeeded, 4),
            pos1,
            done_channel_with_quality(1, "FR (Pos 1)", ChannelRecordingState::ReviewNeeded, 4),
        ];
        assert_eq!(model.accept_review_needed_for(0, 1), 1);
        assert_eq!(
            model.channel_recordings[1].state,
            ChannelRecordingState::Done
        );
        assert_eq!(model.review_needed_indices(), vec![0, 2]);
    }

    #[test]
    fn session_quality_summary_delegates_to_helper() {
        let mut model = RecordingScreenModel::default();
        model.channel_recordings = vec![done_channel_with_quality(
            0,
            "FL",
            ChannelRecordingState::Done,
            4,
        )];
        let lines = model.session_quality_summary();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("FL: OK"), "{}", lines[0]);
    }

    // === Task 9 review R1: batch advance / resume-after-accept ===

    use super::BatchNextAction;

    #[test]
    fn batch_next_holds_without_batch_or_while_parked_or_recording() {
        let mut model = RecordingScreenModel::default();
        model.channel_recordings = vec![
            done_channel_with_quality(0, "FL", ChannelRecordingState::Done, 4),
            done_channel_with_quality(1, "FR", ChannelRecordingState::Empty, 4),
        ];
        // No batch in progress: a manual single-channel flow must not
        // suddenly start recording the rest.
        assert_eq!(model.batch_next_action(0), BatchNextAction::Hold);

        // Batch intent set, but a take is still parked: hold until every
        // warned take is resolved.
        model.auto_record_remaining = true;
        model.channel_recordings.push(done_channel_with_quality(
            2,
            "C",
            ChannelRecordingState::ReviewNeeded,
            4,
        ));
        assert_eq!(model.batch_next_action(0), BatchNextAction::Hold);

        // Parked take resolved, but a capture is in flight: hold.
        model.accept_all_review_needed();
        model.channel_recordings[1].state = ChannelRecordingState::Recording;
        assert_eq!(model.batch_next_action(0), BatchNextAction::Hold);
    }

    #[test]
    fn batch_next_starts_next_channel_in_same_position() {
        let mut model = RecordingScreenModel::default();
        model.auto_record_remaining = true;
        model.channel_recordings = vec![
            done_channel_with_quality(0, "FL", ChannelRecordingState::ReviewNeeded, 4),
            done_channel_with_quality(1, "FR", ChannelRecordingState::Empty, 4),
        ];
        assert_eq!(model.accept_review_needed_for(0, 0), 1);
        assert_eq!(
            model.batch_next_action(0),
            BatchNextAction::StartChannel(1)
        );
    }

    #[test]
    fn batch_next_prompts_position_move_only_after_position_done() {
        let mut model = RecordingScreenModel::default();
        model.recording_config.num_positions = 2;
        model.auto_record_remaining = true;
        let mut pos1 = done_channel_with_quality(0, "FL (Pos 2)", ChannelRecordingState::Empty, 4);
        pos1.mic_position_index = 1;
        model.channel_recordings = vec![
            done_channel_with_quality(0, "FL (Pos 1)", ChannelRecordingState::ReviewNeeded, 4),
            pos1,
        ];
        model.accept_all_review_needed();
        // Position 0 is Done; position 1 entries exist but must NOT start
        // until the user confirms the mics were moved.
        assert_eq!(
            model.batch_next_action(0),
            BatchNextAction::PromptMovePosition {
                finished_position: 0
            }
        );
    }

    #[test]
    fn batch_next_saves_when_last_position_done() {
        let mut model = RecordingScreenModel::default();
        model.auto_record_remaining = true;
        model.channel_recordings = vec![done_channel_with_quality(
            0,
            "FL",
            ChannelRecordingState::ReviewNeeded,
            4,
        )];
        model.accept_all_review_needed();
        assert_eq!(model.batch_next_action(0), BatchNextAction::Save);
    }
}
