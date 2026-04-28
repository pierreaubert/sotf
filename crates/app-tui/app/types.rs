//! Core types for the TUI application state management
pub use sotf_audio_player::QueueItem;
use sotf_audio_player::headphone_eq_types::{
    HeadphoneEqBiquad, HeadphoneEqOptimizerConfig, HeadphoneMeasurementSource,
};
use sotf_audio_player::recording_types::{
    ChannelRecording, PlaybackDeviceConfig, ProbeCaptureState, RecordingDeviceConfig,
    RecordingSignalType, RecordingStep, RoomDimensionUnit,
};
use sotf_audio_player::room_eq_types::{
    ChannelMeasurement, ChannelOptResult, DelayDetectionState, OptimizationStatus,
    RoomEqOptimizerConfig, RoomEqStep,
};
use sotf_audio_player::spinorama_eq_types::{SpinoramaBiquad, SpinoramaOptimizerConfig};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Loading,
    Library,
    Queue,
    Playlists,
    Plugins,
    Devices,
    Configure,
}

/// Sub-mode within the Playlists screen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistMode {
    /// Browsing the list of playlists
    List,
    /// Browsing tracks within the open playlist
    Tracks,
    /// Text input for creating a new playlist
    Create,
    /// Text input for renaming
    Rename,
    /// Confirmation prompt before deleting
    ConfirmDelete,
}

/// Sub-screens within the Configure section
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigureSubScreen {
    Directories,
    Recording,
    RoomEq,
    HeadphoneEq,
    SpinoramaEq,
    FederationSources,
    Servers,
}

/// Step in the Spinorama EQ wizard (TUI-specific: 5 steps)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpinoramaStep {
    #[default]
    Select,
    Configure,
    Optimize,
    Results,
    UpdatePlugin,
}

impl SpinoramaStep {
    pub fn label(self) -> &'static str {
        match self {
            SpinoramaStep::Select => "Select",
            SpinoramaStep::Configure => "Configure",
            SpinoramaStep::Optimize => "Optimize",
            SpinoramaStep::Results => "Results",
            SpinoramaStep::UpdatePlugin => "Update Plugin",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpinUpdateSubStep {
    #[default]
    Ready,
    ConfirmOverwrite,
}

/// TUI state for the Spinorama EQ wizard
#[derive(Debug, Clone)]
pub struct SpinoramaEqTuiState {
    pub step: SpinoramaStep,
    /// When true, the wizard step tab bar has focus (Left/Right change step).
    pub step_tab_focused: bool,
    // Step 1: speaker selection
    pub search_query: String,
    pub available_speakers: Vec<String>,
    pub filtered_speakers: Vec<String>,
    pub selected_speaker_idx: usize,
    pub selected_speaker: Option<String>,
    pub loading_speakers: bool,
    pub speakers_error: Option<String>,
    // Step 2: configuration (shared config struct)
    pub config: SpinoramaOptimizerConfig,
    pub selected_field: usize, // which config field is selected
    /// True when a numerical field is being directly edited via keyboard
    pub editing_value: bool,
    pub edit_buffer: String,
    // Step 3: optimization progress
    pub opt_status: OptimizationStatus,
    pub opt_error: Option<String>,
    pub opt_progress: f32,
    pub opt_loss: f64,
    pub opt_iteration: usize,
    pub opt_max_iter: usize,
    // Step 4: results
    pub filters: Vec<SpinoramaBiquad>,
    pub pre_loss: f64,
    pub post_loss: f64,
    // Frequency response curves (log-spaced Hz, dB values)
    pub curve_frequencies: Vec<f64>,
    pub curve_input: Vec<f64>,
    pub curve_target: Vec<f64>,
    pub curve_corrected: Vec<f64>,
    pub curve_filter_response: Vec<f64>,
    // Optimization loss history: (iteration, loss, optional score)
    pub loss_history: Vec<(usize, f64, Option<f64>)>,
    // Step 5: update plugin confirmation
    pub update_substep: SpinUpdateSubStep,
    /// (slot_index, filter_count) of existing EQ to overwrite
    pub update_existing_eq_info: Option<(usize, usize)>,
}

impl Default for SpinoramaEqTuiState {
    fn default() -> Self {
        // TUI uses slightly different defaults than GPUI
        let config = SpinoramaOptimizerConfig {
            population: 50,
            smooth: true,
            smooth_n: 1,
            spacing_weight: 20.0,
            min_spacing_oct: 0.5,
            tolerance: 1e-3,
            atolerance: 1e-4,
            ..SpinoramaOptimizerConfig::default()
        };
        Self {
            step: SpinoramaStep::Select,
            step_tab_focused: false,
            search_query: String::new(),
            available_speakers: Vec::new(),
            filtered_speakers: Vec::new(),
            selected_speaker_idx: 0,
            selected_speaker: None,
            loading_speakers: false,
            speakers_error: None,
            config,
            selected_field: 0,
            editing_value: false,
            edit_buffer: String::new(),
            opt_status: OptimizationStatus::Idle,
            opt_error: None,
            opt_progress: 0.0,
            opt_loss: 0.0,
            opt_iteration: 0,
            opt_max_iter: 0,
            filters: Vec::new(),
            pre_loss: 0.0,
            post_loss: 0.0,
            curve_frequencies: Vec::new(),
            curve_input: Vec::new(),
            curve_target: Vec::new(),
            curve_corrected: Vec::new(),
            curve_filter_response: Vec::new(),
            loss_history: Vec::new(),
            update_substep: SpinUpdateSubStep::Ready,
            update_existing_eq_info: None,
        }
    }
}

impl SpinoramaEqTuiState {
    pub fn update_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_speakers = self.available_speakers.clone();
        } else {
            let q = self.search_query.to_lowercase();
            self.filtered_speakers = self
                .available_speakers
                .iter()
                .filter(|s| s.to_lowercase().contains(&q))
                .cloned()
                .collect();
        }
        self.selected_speaker_idx = 0;
    }
}

/// Step in the Headphone EQ wizard (TUI-specific: 5 steps)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeadphoneEqStep {
    #[default]
    SelectFile,
    Configure,
    Optimize,
    Results,
    UpdatePlugin,
}

impl HeadphoneEqStep {
    pub fn label(self) -> &'static str {
        match self {
            HeadphoneEqStep::SelectFile => "File",
            HeadphoneEqStep::Configure => "Configure",
            HeadphoneEqStep::Optimize => "Optimize",
            HeadphoneEqStep::Results => "Results",
            HeadphoneEqStep::UpdatePlugin => "Update Plugin",
        }
    }
}

/// Available headphone target curve presets
pub const HEADPHONE_TARGET_PRESETS: &[&str] = &[
    "harman-over-ear-2018",
    "harman-over-ear-2015",
    "harman-over-ear-2013",
    "harman-in-ear-2019",
    "custom",
];

/// TUI state for the Headphone EQ wizard
#[derive(Debug, Clone)]
pub struct HeadphoneEqTuiState {
    pub step: HeadphoneEqStep,
    /// When true, the wizard step tab bar has focus (Left/Right change step).
    pub step_tab_focused: bool,
    /// Detail level for the Configure step (Simple / Intermediate / Expert).
    pub detail_level: sotf_audio_player::autoeq::DetailLevel,
    /// Currently selected preset id (e.g. "balanced", "custom").
    pub selected_preset: String,
    // Step 1: measurement source
    pub measurement_source: HeadphoneMeasurementSource,
    // Step 1 (File mode): file selection
    pub measurement_path: String,
    pub target_preset: String,
    pub custom_target_path: String,
    pub editing_measurement: bool,
    pub editing_custom_target: bool,
    pub selected_field: usize,
    // Step 1 (Spinorama mode): headphone search
    pub search_query: String,
    pub available_headphones: Vec<String>,
    pub filtered_headphones: Vec<String>,
    pub selected_headphone_idx: usize,
    pub selected_headphone: Option<String>,
    pub loading_headphones: bool,
    pub loading_download: bool,
    pub headphones_error: Option<String>,
    pub editing_search: bool,
    // Step 2: configuration (shared config struct)
    pub config: HeadphoneEqOptimizerConfig,
    pub config_selected_field: usize,
    /// True when a numerical field is being directly edited via keyboard
    pub editing_value: bool,
    pub edit_buffer: String,
    // Step 3: optimization progress
    pub opt_status: OptimizationStatus,
    pub opt_error: Option<String>,
    pub opt_progress: f32,
    pub opt_loss: f64,
    pub opt_iteration: usize,
    pub opt_max_iter: usize,
    // Step 4: results
    pub filters: Vec<HeadphoneEqBiquad>,
    pub pre_loss: f64,
    pub post_loss: f64,
    pub curve_frequencies: Vec<f64>,
    pub curve_input: Vec<f64>,
    pub curve_target: Vec<f64>,
    pub curve_corrected: Vec<f64>,
    pub curve_filter_response: Vec<f64>,
    pub loss_history: Vec<(usize, f64)>,
    // Step 5: update plugin confirmation
    pub update_substep: SpinUpdateSubStep,
    /// (slot_index, filter_count) of existing EQ to overwrite
    pub update_existing_eq_info: Option<(usize, usize)>,
}

impl HeadphoneEqTuiState {
    /// Update filtered headphones based on search query
    pub fn update_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_headphones = self.available_headphones.clone();
        } else {
            let query_lower = self.search_query.to_lowercase();
            self.filtered_headphones = self
                .available_headphones
                .iter()
                .filter(|h| h.to_lowercase().contains(&query_lower))
                .cloned()
                .collect();
        }
        // Clamp index
        if !self.filtered_headphones.is_empty() {
            self.selected_headphone_idx = self
                .selected_headphone_idx
                .min(self.filtered_headphones.len() - 1);
        } else {
            self.selected_headphone_idx = 0;
        }
    }
}

impl Default for HeadphoneEqTuiState {
    fn default() -> Self {
        Self {
            step: HeadphoneEqStep::SelectFile,
            step_tab_focused: false,
            detail_level: sotf_audio_player::autoeq::DetailLevel::Simple,
            selected_preset: "balanced".to_string(),
            measurement_source: HeadphoneMeasurementSource::default(),
            measurement_path: String::new(),
            target_preset: "harman-over-ear-2018".to_string(),
            custom_target_path: String::new(),
            editing_measurement: false,
            editing_custom_target: false,
            selected_field: 0,
            search_query: String::new(),
            available_headphones: Vec::new(),
            filtered_headphones: Vec::new(),
            selected_headphone_idx: 0,
            selected_headphone: None,
            loading_headphones: false,
            loading_download: false,
            headphones_error: None,
            editing_search: false,
            config: HeadphoneEqOptimizerConfig::default(),
            config_selected_field: 0,
            editing_value: false,
            edit_buffer: String::new(),
            opt_status: OptimizationStatus::Idle,
            opt_error: None,
            opt_progress: 0.0,
            opt_loss: 0.0,
            opt_iteration: 0,
            opt_max_iter: 0,
            filters: Vec::new(),
            pre_loss: 0.0,
            post_loss: 0.0,
            curve_frequencies: Vec::new(),
            curve_input: Vec::new(),
            curve_target: Vec::new(),
            curve_corrected: Vec::new(),
            curve_filter_response: Vec::new(),
            loss_history: Vec::new(),
            update_substep: SpinUpdateSubStep::Ready,
            update_existing_eq_info: None,
        }
    }
}

/// TUI state for the Room EQ wizard
#[derive(Debug, Clone)]
pub struct RoomEqTuiState {
    pub step: RoomEqStep,
    /// When true, focus is on the step tabs row; Left/Right/Tab cycle steps.
    /// When false, focus is inside the current step's content.
    pub step_tab_focused: bool,
    /// Wizard mode selected in the Process step.
    pub wizard_mode: sotf_audio_player::room_eq_types::RoomEqWizardMode,
    // Step 1: load measurement file (JSON)
    pub file_path: String,
    pub editing_file_path: bool,
    pub channel_measurements: Vec<ChannelMeasurement>,
    pub load_error: Option<String>,
    // Step 2: delay detection (tone-burst probe). Business state lives in
    // the shared `DelayDetectionState`; the `dd_*` fields below are
    // TUI-only UI state.
    pub delay_detection: DelayDetectionState,
    /// Index of the currently focused form field on the delay-detection
    /// step (0..=3: probe_duration, silence_duration, input_channel,
    /// Run button). Scroll-local cursor only — no semantic meaning.
    pub dd_field: usize,
    /// Row index of the results table currently highlighted for editing.
    /// Row selection navigates with `j` / `k`; `e` starts editing the
    /// row pointed to by this cursor.
    pub dd_selected_row: usize,
    /// Row index of the results table being edited, or `None` when no
    /// override edit is in progress.
    pub dd_edit_row: Option<usize>,
    /// Set when the user hits `r` while `edited_arrival_ms` is non-empty.
    /// A second `r` within the same focus session confirms and starts a
    /// fresh measurement (which wipes the overrides); any other key
    /// clears the flag, so the next `r` re-prompts.
    pub dd_pending_rerun_confirm: bool,
    // Step 3: configure (shared config struct)
    pub config: RoomEqOptimizerConfig,
    pub selected_field: usize,
    pub selected_section: usize,
    /// True when a numerical field is being directly edited via keyboard
    pub editing_value: bool,
    pub edit_buffer: String,
    // Step 3: optimization
    pub opt_status: OptimizationStatus,
    pub opt_error: Option<String>,
    pub opt_progress: f32,
    pub opt_iteration: usize,
    pub opt_max_iter: usize,
    pub opt_loss: f64,
    /// Name of the speaker currently being optimized
    pub opt_current_speaker: String,
    /// Total number of speakers being optimized
    pub opt_total_speakers: usize,
    /// Status message from the optimizer (e.g. post-processing phase name)
    pub opt_status_message: Option<String>,
    pub channel_results: Vec<ChannelOptResult>,
    pub loss_history: Vec<(usize, f64)>,
    /// Log buffer for optimization messages (max 300 lines)
    pub opt_log_lines: VecDeque<String>,
    /// Scroll offset from bottom (0 = bottom)
    pub opt_log_scroll: usize,
    // Step 4: review
    pub selected_channel: usize,
    // Step 5: export
    pub export_path: String,
    pub editing_export_path: bool,
    pub export_format: usize,
    pub export_error: Option<String>,
    pub export_success: bool,
}

impl Default for RoomEqTuiState {
    fn default() -> Self {
        Self {
            step: RoomEqStep::LoadData,
            step_tab_focused: false,
            wizard_mode: sotf_audio_player::room_eq_types::RoomEqWizardMode::default(),
            file_path: String::new(),
            editing_file_path: false,
            channel_measurements: Vec::new(),
            load_error: None,
            delay_detection: DelayDetectionState::default(),
            dd_field: 0,
            dd_selected_row: 0,
            dd_edit_row: None,
            dd_pending_rerun_confirm: false,
            config: RoomEqOptimizerConfig::default(),
            selected_field: 0,
            selected_section: 0,
            editing_value: false,
            edit_buffer: String::new(),
            opt_status: OptimizationStatus::Idle,
            opt_error: None,
            opt_progress: 0.0,
            opt_iteration: 0,
            opt_max_iter: 0,
            opt_loss: 0.0,
            opt_current_speaker: String::new(),
            opt_total_speakers: 0,
            opt_status_message: None,
            channel_results: Vec::new(),
            loss_history: Vec::new(),
            opt_log_lines: VecDeque::new(),
            opt_log_scroll: 0,
            selected_channel: 0,
            export_path: String::new(),
            editing_export_path: false,
            export_format: 0,
            export_error: None,
            export_success: false,
        }
    }
}

impl RoomEqTuiState {
    /// Compute the average slope for L and R channels in dB/octave.
    pub fn compute_lr_slope(&self) -> Option<(f64, f64, f64)> {
        sotf_audio_player::room_eq_types::compute_lr_slope(&self.channel_measurements)
    }
}

/// TUI state for the Recording wizard
#[derive(Debug, Clone)]
pub struct RecordingTuiState {
    pub step: RecordingStep,
    /// When true, the wizard step tab bar has focus (Left/Right change step).
    pub step_tab_focused: bool,
    // Step 1: config
    pub playback_config: PlaybackDeviceConfig,
    pub recording_config: RecordingDeviceConfig,
    pub available_playback_devices: Vec<(String, String)>, // (id, name)
    pub available_recording_devices: Vec<(String, String)>,
    pub selected_playback_idx: usize,
    pub selected_recording_idx: usize,
    pub signal_type: RecordingSignalType,
    pub signal_duration_secs: f32,
    pub signal_level_db: f32,
    pub sweep_start_freq: f32,
    pub sweep_end_freq: f32,
    pub output_directory: String,
    pub editing_output_dir: bool,
    /// `Some(ch)` while editing channel `ch`'s mic-calibration path; `None`
    /// otherwise. The path itself lives in
    /// `recording_config.mic_calibration_paths[ch]` — there is no separate
    /// scratch buffer (mirrors the GPUI per-channel calibration model).
    pub editing_mic_cal_channel: Option<usize>,
    pub selected_field: usize,
    /// True when a numerical field is being directly edited via keyboard
    pub editing_value: bool,
    pub edit_buffer: String,
    // Step 2: capture
    pub channel_recordings: Vec<ChannelRecording>,
    pub current_channel: Option<usize>,
    pub recording_progress: f32,
    pub auto_record: bool,
    pub status_message: String,
    // Step 3 (Probe): tone-burst delay probe capture. Shared
    // business state lives in ProbeCaptureState; the TUI-only fields
    // below are cursor + in-progress-edit state.
    pub probe_capture: ProbeCaptureState,
    /// Form-field cursor for the Probe step: 0=duration, 1=silence,
    /// 2=mic channel, 3=Run button.
    pub probe_selected_field: usize,
    pub probe_editing_value: bool,
    // Step 4: evaluate
    pub selected_channel_view: usize,
    // Step 4: save
    pub save_name: String,
    pub editing_save_name: bool,
    /// Cursor within the save-step form. 0 = save_name, 1..=3 = room
    /// width/depth/height, 4 = unit toggle, 5 = setup description,
    /// 6..6+N-1 = per-channel speaker entries (N = channel count).
    pub selected_save_field: usize,
    /// When true, a text/number field under the save cursor is being
    /// typed into. Re-uses `edit_buffer` for the keystroke buffer.
    pub editing_save_value: bool,
    /// Room dimensions — interpreted in `save_room_unit`. Zero means
    /// "not specified" and is dropped at save time.
    pub save_room_width: f64,
    pub save_room_depth: f64,
    pub save_room_height: f64,
    pub save_room_unit: RoomDimensionUnit,
    /// Free-form description of the listening setup.
    pub setup_description: String,
    /// Per-channel speaker identity (brand + model). Indices align
    /// with `channel_recordings[i].channel_name`. Auto-resized at
    /// render time.
    pub channel_speakers: Vec<String>,
    pub save_error: Option<String>,
    pub save_success: bool,
}

impl Default for RecordingTuiState {
    fn default() -> Self {
        Self {
            step: RecordingStep::Config,
            step_tab_focused: false,
            playback_config: PlaybackDeviceConfig::default(),
            recording_config: RecordingDeviceConfig::default(),
            available_playback_devices: Vec::new(),
            available_recording_devices: Vec::new(),
            selected_playback_idx: 0,
            selected_recording_idx: 0,
            signal_type: RecordingSignalType::Sweep,
            signal_duration_secs: 5.0,
            signal_level_db: -20.0,
            sweep_start_freq: 20.0,
            sweep_end_freq: 20000.0,
            output_directory: String::new(),
            editing_output_dir: false,
            editing_mic_cal_channel: None,
            selected_field: 0,
            editing_value: false,
            edit_buffer: String::new(),
            channel_recordings: Vec::new(),
            current_channel: None,
            recording_progress: 0.0,
            auto_record: false,
            status_message: String::new(),
            probe_capture: ProbeCaptureState::default(),
            probe_selected_field: 0,
            probe_editing_value: false,
            selected_channel_view: 0,
            save_name: String::new(),
            editing_save_name: false,
            selected_save_field: 0,
            editing_save_value: false,
            save_room_width: 0.0,
            save_room_depth: 0.0,
            save_room_height: 0.0,
            save_room_unit: RoomDimensionUnit::default(),
            setup_description: String::new(),
            channel_speakers: Vec::new(),
            save_error: None,
            save_success: false,
        }
    }
}

/// Logical identity of every cursor position on the Recording → Config
/// step. The TUI's flat `selected_field: usize` is mapped to one of these
/// via `recording_field_at`, so the renderer and the event handler agree
/// on what each row means even when the dynamic per-channel rows change
/// length with `recording_config.num_channels`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordingField {
    PlaybackDevice,
    RecordingDevice,
    SpeakerConfig,
    SignalType,
    Duration,
    Level,
    SweepStart,
    SweepEnd,
    OutputDir,
    NumRecordingChannels,
    /// Per-channel mic-calibration path (`recording_config.mic_calibration_paths[i]`).
    MicCal(usize),
    /// Per-channel input mapping (`recording_config.channel_mappings[i]`).
    ChannelInput(usize),
}

/// Map a flat field index to its logical identity. Returns `None` past the
/// last valid row.
pub fn recording_field_at(s: &RecordingTuiState, idx: usize) -> Option<RecordingField> {
    use RecordingField::*;
    let n = s.recording_config.num_channels.max(1);
    match idx {
        0 => Some(PlaybackDevice),
        1 => Some(RecordingDevice),
        2 => Some(SpeakerConfig),
        3 => Some(SignalType),
        4 => Some(Duration),
        5 => Some(Level),
        6 => Some(SweepStart),
        7 => Some(SweepEnd),
        8 => Some(OutputDir),
        9 => Some(NumRecordingChannels),
        i if i < 10 + n => Some(MicCal(i - 10)),
        i if i < 10 + 2 * n => Some(ChannelInput(i - 10 - n)),
        _ => None,
    }
}

/// Total number of selectable rows for the current state.
pub fn recording_field_count(s: &RecordingTuiState) -> usize {
    10 + 2 * s.recording_config.num_channels.max(1)
}

impl RecordingTuiState {
    /// Currently-active mic-calibration path string (read-only). Returns
    /// `""` when not editing or when the channel slot is empty.
    pub fn active_mic_cal_path(&self) -> &str {
        match self.editing_mic_cal_channel {
            Some(ch) => self
                .recording_config
                .mic_calibration_paths
                .get(ch)
                .and_then(|o| o.as_deref())
                .unwrap_or(""),
            None => "",
        }
    }

    /// Mutable reference to the currently-active mic-calibration string,
    /// growing the underlying `Vec` and lazily inserting an empty `String`
    /// in the slot if needed. Returns `None` if no channel is being edited.
    pub fn active_mic_cal_path_mut(&mut self) -> Option<&mut String> {
        let ch = self.editing_mic_cal_channel?;
        let paths = &mut self.recording_config.mic_calibration_paths;
        while paths.len() <= ch {
            paths.push(None);
        }
        if paths[ch].is_none() {
            paths[ch] = Some(String::new());
        }
        paths[ch].as_mut()
    }

    /// Replace the active mic-calibration path, normalising an empty
    /// string back to `None` so downstream code can treat both states
    /// uniformly via `Option`.
    pub fn set_active_mic_cal_path(&mut self, val: String) {
        if let Some(ch) = self.editing_mic_cal_channel {
            let paths = &mut self.recording_config.mic_calibration_paths;
            while paths.len() <= ch {
                paths.push(None);
            }
            paths[ch] = if val.is_empty() { None } else { Some(val) };
        }
    }

    /// Resize the mic-calibration and recording-channel-mapping vecs to
    /// match `num_channels`. Mirrors GPUI's `update_recording_channel_mappings`.
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

    /// Ensure `channel_speakers` has one slot per current channel row.
    /// Call this whenever the channel list changes so the UI never
    /// indexes a short vec. Preserves any pre-existing values.
    pub fn sync_channel_speakers_length(&mut self) {
        self.channel_speakers
            .resize(self.channel_recordings.len(), String::new());
    }

    /// Build the canonical-metric `RoomDimensionsLegacy` to persist in
    /// `RecordingConfiguration`. Returns `None` when any dimension is
    /// blank (zero) — a partial triple would mislead downstream
    /// consumers (e.g. the optimizer's Schroeder auto-detect).
    pub fn room_dimensions_for_save(
        &self,
    ) -> Option<sotf_audio_player::room_eq_types::RoomDimensionsLegacy> {
        if self.save_room_width <= 0.0
            || self.save_room_depth <= 0.0
            || self.save_room_height <= 0.0
        {
            return None;
        }
        let u = self.save_room_unit;
        Some(sotf_audio_player::room_eq_types::RoomDimensionsLegacy {
            length: u.to_meters(self.save_room_depth),
            width: u.to_meters(self.save_room_width),
            height: u.to_meters(self.save_room_height),
        })
    }

    /// Build the channel-name → "brand model" map persisted in
    /// `RecordingConfiguration`. Blank entries are skipped; returns
    /// `None` when every entry is empty.
    pub fn channel_speakers_map_for_save(
        &self,
    ) -> Option<std::collections::HashMap<String, String>> {
        let mut map = std::collections::HashMap::new();
        for (i, rec) in self.channel_recordings.iter().enumerate() {
            if let Some(entry) = self.channel_speakers.get(i) {
                let trimmed = entry.trim();
                if !trimmed.is_empty() {
                    map.insert(rec.channel_name.clone(), trimmed.to_string());
                }
            }
        }
        if map.is_empty() { None } else { Some(map) }
    }

    /// How many fields are in the Save-step form given the current
    /// channel list. Layout:
    ///   0        save_name
    ///   1..=3    room width / depth / height
    ///   4        unit toggle
    ///   5        setup description
    ///   6..6+N-1 per-channel speaker entries
    pub fn save_field_count(&self) -> usize {
        6 + self.channel_recordings.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    AddPlugin,
    EditPlugin,
    SavePlugins,
    LoadPlugins,
    LoadApoFile,
    LoadSofaFile,
    FileExplorer,
    ShowHelp,
    ShowError,
    /// Shown when a multichannel file conflicts with the upmixer plugin
    ChannelConflict,
    /// Level meters pane is focused
    LevelMeters,
    /// Configure tab bar is focused
    Configure,
    /// Configure sub-screen: Directories
    ConfigureDirectories,
    /// Configure sub-screen: Recording
    ConfigureRecording,
    /// Configure sub-screen: Room EQ
    ConfigureRoomEq,
    /// Configure sub-screen: Headphone EQ
    ConfigureHeadphoneEq,
    /// Configure sub-screen: Spinorama EQ
    ConfigureSpinoramaEq,
    /// Configure sub-screen: Federation Sources
    ConfigureFederationSources,
    /// Configure sub-screen: Servers
    ConfigureServers,
}

impl InputMode {
    /// Returns true for Configure tab bar and all 5 sub-screens
    pub fn is_configure(self) -> bool {
        matches!(
            self,
            InputMode::Configure
                | InputMode::ConfigureDirectories
                | InputMode::ConfigureRecording
                | InputMode::ConfigureRoomEq
                | InputMode::ConfigureHeadphoneEq
                | InputMode::ConfigureSpinoramaEq
                | InputMode::ConfigureFederationSources
                | InputMode::ConfigureServers
        )
    }

    /// Returns true for configure sub-screens only (not the tab bar)
    pub fn is_configure_sub_screen(self) -> bool {
        matches!(
            self,
            InputMode::ConfigureDirectories
                | InputMode::ConfigureRecording
                | InputMode::ConfigureRoomEq
                | InputMode::ConfigureHeadphoneEq
                | InputMode::ConfigureSpinoramaEq
                | InputMode::ConfigureFederationSources
                | InputMode::ConfigureServers
        )
    }

    /// Convert a ConfigureSubScreen to the corresponding InputMode
    pub fn from_configure_sub_screen(sub: ConfigureSubScreen) -> Self {
        match sub {
            ConfigureSubScreen::Directories => InputMode::ConfigureDirectories,
            ConfigureSubScreen::Recording => InputMode::ConfigureRecording,
            ConfigureSubScreen::RoomEq => InputMode::ConfigureRoomEq,
            ConfigureSubScreen::HeadphoneEq => InputMode::ConfigureHeadphoneEq,
            ConfigureSubScreen::SpinoramaEq => InputMode::ConfigureSpinoramaEq,
            ConfigureSubScreen::FederationSources => InputMode::ConfigureFederationSources,
            ConfigureSubScreen::Servers => InputMode::ConfigureServers,
        }
    }

    /// Return the corresponding ConfigureSubScreen, if this is a configure sub-screen mode
    pub fn configure_sub_screen(self) -> Option<ConfigureSubScreen> {
        match self {
            InputMode::ConfigureDirectories => Some(ConfigureSubScreen::Directories),
            InputMode::ConfigureRecording => Some(ConfigureSubScreen::Recording),
            InputMode::ConfigureRoomEq => Some(ConfigureSubScreen::RoomEq),
            InputMode::ConfigureHeadphoneEq => Some(ConfigureSubScreen::HeadphoneEq),
            InputMode::ConfigureSpinoramaEq => Some(ConfigureSubScreen::SpinoramaEq),
            InputMode::ConfigureFederationSources => Some(ConfigureSubScreen::FederationSources),
            InputMode::ConfigureServers => Some(ConfigureSubScreen::Servers),
            _ => None,
        }
    }
}

/// Whether the file picker selects a file or a directory
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerMode {
    File,
    Directory,
}

/// Tracks which feature opened the file explorer so we can apply the result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerOrigin {
    SofaFile,
    IrFile,
    RecordingOutputDir,
    RecordingMicCalibration,
    RoomEqFilePath,
    RoomEqExportPath,
    HeadphoneMeasurement,
    HeadphoneCustomTarget,
    AddDirectory,
    ApoFile,
    ABConfigA,
    ABConfigB,
    PlaylistImport,
    PlaylistExport,
}

/// Options presented in the channel conflict dialog
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelConflictChoice {
    /// Suspend incompatible plugins and play (auto-restores on next compatible track)
    SuspendIncompatible,
    /// Remove incompatible plugins from the chain permanently
    RemoveIncompatible,
    /// Cancel playback
    Cancel,
}

/// Matrix plugin editor mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatrixEditMode {
    #[default]
    Header, // Editing input/output channels, preset
    Grid, // Editing matrix cells
}

/// Tree view mode for library
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryViewMode {
    Flat,     // Original list view
    TreeView, // Hierarchical artist → albums
}

/// Library sort order
pub use sotf_audio_player::library::LibrarySortOrder;

/// Channel filter options
pub use sotf_audio_player::library::ChannelFilter;

/// Artist node in tree view
#[derive(Debug, Clone)]
pub struct ArtistNode {
    pub artist: String,
    pub album_indices: Vec<usize>, // Indices into library.albums
    pub expanded: bool,
}

/// Tree item type for rendering
#[derive(Debug, Clone)]
pub enum TreeItem {
    Artist { name: String, expanded: bool },
    Album { index: usize },
}

pub use sotf_audio_player::ReplayGainMode;

pub use sotf_audio_player::{ChannelGroup, ChannelInfo};

/// Pending parameter update for zero-dropout updates
#[derive(Debug, Clone)]
pub struct PendingParameterUpdate {
    pub plugin_index: usize,
    pub param_id: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub item: QueueItem,
    pub expanded: bool,
}

impl QueueEntry {
    pub fn new(item: QueueItem) -> Self {
        Self {
            item,
            expanded: false,
        }
    }
}

// ============================================================================
// Federation & Server TUI state
// ============================================================================

use sotf_audio_player::federation_config::{ConnectionStatus, FederationSourceEntry, ServerConfig};
use std::collections::HashMap;

/// TUI state for the Federation Sources configuration screen.
#[derive(Debug, Clone)]
pub struct FederationTuiState {
    pub sources: Vec<FederationSourceEntry>,
    pub statuses: HashMap<String, ConnectionStatus>,
    pub selected_idx: usize,
    pub mode: FederationMode,
    pub edit: Option<FederationEditState>,
}

impl Default for FederationTuiState {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            statuses: HashMap::new(),
            selected_idx: 0,
            mode: FederationMode::List,
            edit: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FederationMode {
    List,
    EditSource,
    AddSource,
}

#[derive(Debug, Clone)]
pub struct FederationEditState {
    pub source: FederationSourceEntry,
    /// Field index within the source-specific connection fields
    /// 0..N are connection fields, N is display_name, N+1 is priority, N+2 is enabled
    pub selected_field: usize,
    pub editing_value: bool,
    pub edit_buffer: String,
    pub is_new: bool,
}

impl FederationEditState {
    pub fn new(source: FederationSourceEntry, is_new: bool) -> Self {
        Self {
            source,
            selected_field: 0,
            editing_value: false,
            edit_buffer: String::new(),
            is_new,
        }
    }

    /// Total number of editable fields (connection fields + name + priority)
    pub fn field_count(&self) -> usize {
        self.source.connection.field_names().len() + 2
    }

    /// Get label for the field at the given index
    pub fn field_label(&self, index: usize) -> &str {
        let conn_fields = self.source.connection.field_names();
        if index < conn_fields.len() {
            conn_fields[index]
        } else if index == conn_fields.len() {
            "Display Name"
        } else {
            "Priority"
        }
    }

    /// Get value for the field at the given index
    pub fn field_value(&self, index: usize) -> String {
        let conn_fields = self.source.connection.field_names();
        if index < conn_fields.len() {
            self.source.connection.field_value(index)
        } else if index == conn_fields.len() {
            self.source.display_name.clone()
        } else {
            self.source.priority.to_string()
        }
    }

    /// Set value for the field at the given index
    pub fn set_field_value(&mut self, index: usize, value: &str) {
        let conn_field_count = self.source.connection.field_names().len();
        if index < conn_field_count {
            self.source.connection.set_field_value(index, value);
        } else if index == conn_field_count {
            self.source.display_name = value.to_string();
        } else if let Ok(p) = value.parse() {
            self.source.priority = p;
        }
    }
}

/// TUI state for the Servers configuration screen.
#[derive(Debug, Clone)]
pub struct ServersTuiState {
    pub config: ServerConfig,
    pub selected_section: ServerSection,
    pub selected_field: usize,
    pub editing_value: bool,
    pub edit_buffer: String,
    pub tls_fingerprint: Option<String>,
}

impl Default for ServersTuiState {
    fn default() -> Self {
        Self {
            config: ServerConfig::default(),
            selected_section: ServerSection::Mpd,
            selected_field: 0,
            editing_value: false,
            edit_buffer: String::new(),
            tls_fingerprint: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerSection {
    Mpd,
    Dlna,
}

/// Source type names for the "Add Source" selection.
pub const SOURCE_TYPE_NAMES: &[(&str, &str)] = &[
    ("subsonic", "Subsonic"),
    ("mpd", "MPD"),
    ("dlna", "DLNA"),
    ("peer", "Peer (SotF)"),
    ("tidal", "Tidal"),
    ("spotify", "Spotify"),
    ("icy_radio", "Radio"),
];

/// Index of the selected source type when in AddSource mode
pub static ADD_SOURCE_TYPE_IDX: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
