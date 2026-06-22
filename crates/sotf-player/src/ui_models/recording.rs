//! Shared, UI-agnostic view model for the Recording wizard.
//!
//! Domain state (session configuration, capture progress, probe/bass-anchor/SPL
//! results, save metadata) lives here; view state (dropdowns, focus flags, edit
//! buffers) stays in `app-gpui` / `app-tui`.

use crate::recording_types::{
    BassAnchorCaptureState, CalibrationData, ChannelRecording, ChannelRecordingState,
    PlaybackDeviceConfig, PlotSmoothing, ProbeCaptureState, RecordingDeviceConfig,
    RecordingSignalType, RecordingStep, RoomDimensionUnit, SplCalibrationCaptureState,
    TransferMatrixLoopbackRecording,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Default signal level (dBFS) used as the SPL-calibrated reference for
/// capture, delay probe, and bass anchor.
pub const DEFAULT_SIGNAL_LEVEL_DB: f32 = -6.0206;

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
    pub current_recording_channel: Option<usize>,
    pub recording_progress: f32,
    /// Whether to automatically record all remaining channels.
    pub auto_record_remaining: bool,

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
            sweep_start_freq: 20.0,
            sweep_end_freq: 20000.0,
            channel_recordings: Vec::new(),
            transfer_matrix_loopbacks: Vec::new(),
            ctc_reference_sweep_path: None,
            current_recording_channel: None,
            recording_progress: 0.0,
            auto_record_remaining: false,
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
        let safe: String = self
            .save_name
            .trim()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        if safe.is_empty() {
            "recording".to_string()
        } else {
            safe
        }
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
        self.ctc_reference_sweep_path = None;
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

    /// Currently-active mic-calibration path string (read-only). Returns
    /// `""` when `editing_channel` is `None` or when the channel slot is empty.
    pub fn active_mic_cal_path(&self, editing_channel: Option<usize>) -> &str {
        match editing_channel {
            Some(ch) => self
                .recording_config
                .mic_calibration_paths
                .get(ch)
                .and_then(|o| o.as_deref())
                .unwrap_or(""),
            None => "",
        }
    }

    /// Mutable reference to the mic-calibration path for `editing_channel`,
    /// growing the underlying `Vec` and lazily inserting an empty `String`
    /// in the slot if needed.
    pub fn active_mic_cal_path_mut(
        &mut self,
        editing_channel: Option<usize>,
    ) -> Option<&mut String> {
        let ch = editing_channel?;
        let paths = &mut self.recording_config.mic_calibration_paths;
        while paths.len() <= ch {
            paths.push(None);
        }
        if paths[ch].is_none() {
            paths[ch] = Some(String::new());
        }
        paths[ch].as_mut()
    }

    /// Replace the mic-calibration path for `editing_channel`, normalising an
    /// empty string back to `None`.
    pub fn set_active_mic_cal_path(&mut self, editing_channel: Option<usize>, val: String) {
        if let Some(ch) = editing_channel {
            let paths = &mut self.recording_config.mic_calibration_paths;
            while paths.len() <= ch {
                paths.push(None);
            }
            paths[ch] = if val.is_empty() { None } else { Some(val) };
        }
    }

    /// Resize the mic-calibration and recording-channel-mapping vecs to
    /// match `num_channels`.
    pub fn sync_recording_channel_vecs(&mut self) {
        let target = self.recording_config.num_channels.max(1);
        let cm = &mut self.recording_config.channel_mappings;
        while cm.len() < target {
            cm.push(cm.len());
        }
        cm.truncate(target);

        let cal = &mut self.recording_config.mic_calibration_paths;
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
