// ============================================================================
// Recording Screen Types
// ============================================================================
//
// Domain types are shared via the player crate. UI-specific state stays here.

use super::calibration::CalibrationData;
use crate::app::constants::recording::DEFAULT_SIGNAL_LEVEL_DB;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

// Re-export shared domain types from player crate
pub use sotf_audio_player::recording_types::{
    BassAnchorCaptureState, BassAnchorCaptureStatus, ChannelMapping, ChannelRecording,
    ChannelRecordingState, CtcMatrixExportStrategy, PlaybackDeviceConfig, PlotSmoothing,
    ProbeCaptureState, ProbeCaptureStatus, RecordingDeviceConfig, RecordingResult,
    RecordingSignalType, RecordingStep, SpeakerConfiguration, SplCalibrationCaptureState,
    SplCalibrationCaptureStatus, TransferMatrixLoopbackRecording,
};

/// Measurement-unit preference for the room-dimensions inputs on the
/// Save step. Purely a UI convenience: the canonical unit on disk is
/// always metric (meters). The conversion happens at save time via
/// [`RecordingState::room_dimensions_for_save`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoomDimensionUnit {
    #[default]
    Metric,
    Imperial,
}

impl RoomDimensionUnit {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Metric => "m",
            Self::Imperial => "ft",
        }
    }

    /// Convert a user-entered value in this unit to canonical meters.
    pub fn to_meters(&self, value: f64) -> f64 {
        match self {
            Self::Metric => value,
            // 1 international foot = 0.3048 m exactly.
            Self::Imperial => value * 0.304_8,
        }
    }

    /// Toggle between the two units.
    pub fn toggled(&self) -> Self {
        match self {
            Self::Metric => Self::Imperial,
            Self::Imperial => Self::Metric,
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
    /// Per-channel microphone calibration file paths (parallel to recording_config.channel_mappings)
    pub mic_calibration_paths: Vec<Option<String>>,
    /// Parsed calibration data for display
    pub mic_calibration_data: Option<CalibrationData>,
    /// Per-channel parsed calibration data for display
    pub mic_calibration_data_per_channel: Vec<Option<CalibrationData>>,

    // === Capture Step State ===
    pub signal_type: RecordingSignalType,
    pub signal_duration_secs: f32,
    pub signal_level_db: f32,
    /// Sweep start frequency in Hz
    pub sweep_start_freq: f32,
    /// Sweep end frequency in Hz
    pub sweep_end_freq: f32,
    pub channel_recordings: Vec<ChannelRecording>,
    pub transfer_matrix_loopbacks: Vec<TransferMatrixLoopbackRecording>,
    pub ctc_reference_sweep_path: Option<String>,
    pub current_recording_channel: Option<usize>,
    pub recording_progress: f32,
    pub status_message: String,
    pub auto_record_remaining: bool, // Whether to automatically record all remaining channels

    /// Directory where recordings will be stored
    /// Format: user_selected_dir/recording-YYYYMMDD-HHMMSS/
    pub recording_directory: Option<String>,
    /// Base directory selected by user (before adding timestamp subdirectory)
    pub recording_base_directory: Option<String>,

    // === Probe Step State ===
    /// Shared business state for the tone-burst delay probe step.
    /// Populated by `start_probe_capture` on success; consumed by
    /// `save_recordings` to embed results in the session JSON.
    pub probe_capture: ProbeCaptureState,
    /// Cancel-request flag polled by the probe-capture engine call. UI
    /// sets this to true when the user clicks Cancel; the engine returns
    /// `Err(CANCELLED_ERR)` on the next stability poll. Lives behind
    /// `Arc<AtomicBool>` so the spawn closure can clone it cheaply.
    pub probe_cancel_requested: Arc<AtomicBool>,

    // === BassAnchor Step State (GD-Opt v2 Phase GD-1e) ===
    /// Shared business state for the bass anchor step. Populated by
    /// the BassAnchor wizard step on success; consumed by
    /// `save_recordings` to embed results in the session JSON.
    pub bass_anchor_capture: BassAnchorCaptureState,
    /// Cancel-request flag for the bass anchor capture. Same shape and
    /// lifetime as `probe_cancel_requested`.
    pub bass_anchor_cancel_requested: Arc<AtomicBool>,

    // === SPL Calibration Step State (GD-Opt v2 Phase GD-1e.5) ===
    /// Shared business state for the SPL calibration step.
    pub spl_calibration_capture: SplCalibrationCaptureState,
    /// Cancel-request flag for the SPL calibration capture. Same shape
    /// and lifetime as `probe_cancel_requested`.
    pub spl_cancel_requested: Arc<AtomicBool>,

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
    /// Room width — interpreted in `room_dimension_unit`. Zero means
    /// "not specified" and is skipped during serialization.
    pub room_width_input: f64,
    /// Room depth — interpreted in `room_dimension_unit`. Zero means
    /// "not specified".
    pub room_depth_input: f64,
    /// Room height — interpreted in `room_dimension_unit`. Zero means
    /// "not specified".
    pub room_height_input: f64,
    /// Unit the three `room_*_input` values are expressed in. UI-only;
    /// the persisted JSON always stores metric.
    pub room_dimension_unit: RoomDimensionUnit,
    /// Free-form description of the listening setup (treatment,
    /// seating, notes). Persisted verbatim in `RecordingConfiguration`.
    pub setup_description: String,
    /// Per-channel speaker identity (brand + model). Indices align
    /// with `channel_recordings[i].channel_name` at render time.
    /// Short or padded to match the channel list in the UI.
    pub channel_speakers: Vec<String>,
    /// Index of the channel-speaker row whose autocomplete suggestions
    /// are currently visible, or `None` when no dropdown is open.
    pub channel_speaker_autocomplete_open: Option<usize>,

    // === GD-Opt v2 Phase GD-1b: Advanced measurement quality settings ===
    // These are surfaced in the "Advanced: measurement quality" accordion
    // section on the Config step. They map directly to the same-named fields
    // in `autoeq::RecordingConfiguration`.
    //
    // Default 3.0 s/octave below 100 Hz (see docs/gd_opt_v2_plan.md §2.7).
    pub bass_octave_duration_s: f32,
    // Default 2.0 s noise-floor window before the sweep.
    pub pre_silence_s: f32,
    // `None` = derive from RT60 estimate; `Some(x)` = user-specified.
    pub post_silence_s: Option<f32>,

    // === Noise Floor Warning ===
    /// Warning message when recording level is close to noise floor
    pub noise_floor_warning: Option<String>,

    // === Multi-Position Modal State ===
    /// Whether the "move microphones to next position" modal is shown.
    /// Set to `true` after every recording at the current position
    /// completes and another position is still pending; cleared when
    /// the user clicks Continue or Cancel.
    pub move_position_modal_open: bool,
    /// Position index (0-based) the user is being asked to move the
    /// microphones to. `Some(n)` while the modal is open; `None` when
    /// dismissed. Mirrors the modal's "expected next position" so the
    /// auto-record loop can resume cleanly when Continue fires.
    pub pending_next_position: Option<usize>,

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
        let (recording_base_directory, recording_directory) =
            crate::app::config::default_recording_paths();

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
            status_message: String::new(),
            auto_record_remaining: false,
            recording_directory,
            recording_base_directory,
            probe_capture: ProbeCaptureState::default(),
            probe_cancel_requested: Arc::new(AtomicBool::new(false)),
            bass_anchor_capture: BassAnchorCaptureState::default(),
            bass_anchor_cancel_requested: Arc::new(AtomicBool::new(false)),
            spl_calibration_capture: SplCalibrationCaptureState::default(),
            spl_cancel_requested: Arc::new(AtomicBool::new(false)),
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
            room_width_input: 0.0,
            room_depth_input: 0.0,
            room_height_input: 0.0,
            room_dimension_unit: RoomDimensionUnit::default(),
            setup_description: String::new(),
            channel_speakers: Vec::new(),
            channel_speaker_autocomplete_open: None,
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
        }
    }
}

impl RecordingState {
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
    /// position 1. The auto-record loop uses this ordering to advance
    /// through one full sweep pass before prompting the user to move the
    /// mics to the next seat.
    ///
    /// Naming:
    /// * 1 mic, 1 position  → `"L"`
    /// * N mics, 1 position → `"L (Mic 1)"`
    /// * 1 mic,  M positions → `"L (Pos 1)"`
    /// * N mics, M positions → `"L (Pos 1 / Mic 1)"`
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

    /// Position index (0-based) of the next pending recording. Returns
    /// `num_positions` when every entry is `Done`. Used by the capture
    /// loop to decide whether the next start should advance to a new
    /// seat (and thus open the move-position modal).
    pub fn current_position(&self) -> usize {
        let num_positions = self.recording_config.num_positions.max(1);
        self.channel_recordings
            .iter()
            .filter(|r| r.state != ChannelRecordingState::Done)
            .map(|r| r.mic_position_index)
            .min()
            .unwrap_or(num_positions)
    }

    /// True when every recording with `mic_position_index == pos` is
    /// `Done`. Returns `true` for positions that have no entries (e.g.
    /// when `num_positions == 1` and `pos == 0` after all are Done).
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

    /// Index (into `channel_recordings`) of the next recording at
    /// `pos` whose `channel_index` (speaker) hasn't been started yet
    /// at this position. Mirrors the dedup-by-speaker logic the
    /// auto-record loop uses, but constrained to a single position so
    /// the modal flow can pause cleanly between passes.
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

    /// Ensure `channel_speakers` has one slot per physical playback
    /// channel. Capture rows can be multiplied by mic/position, but
    /// speaker identity is attached to the speaker channel itself.
    pub fn sync_channel_speakers_length(&mut self) {
        self.channel_speakers
            .resize(self.playback_config.channel_mappings.len(), String::new());
    }

    /// Build the canonical-metric [`RoomDimensions`] to persist in
    /// `RecordingConfiguration`. Returns `None` when the user left any
    /// dimension blank (zero) — partial data is not worth storing and
    /// would mislead the Schroeder-frequency auto-detector.
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
            // (front-to-back distance); the UI exposes it as "Depth" so
            // it matches how most people describe their listening space.
            length: unit.to_meters(self.room_depth_input),
            width: unit.to_meters(self.room_width_input),
            height: unit.to_meters(self.room_height_input),
        })
    }

    /// Build the `channel_name → "brand model"` map persisted in
    /// `RecordingConfiguration`. Returns `None` when every entry is
    /// blank so absence round-trips through serialization.
    pub fn channel_speakers_map_for_save(
        &self,
    ) -> Option<std::collections::HashMap<String, String>> {
        let mut map = std::collections::HashMap::new();
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
}
