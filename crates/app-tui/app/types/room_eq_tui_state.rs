use sotf_audio_player::room_eq_types::{
    ChannelMeasurement, ChannelOptResult, DelayDetectionState, OptimizationStatus,
    RoomEqOptimizerConfig, RoomEqStep,
};
use std::collections::VecDeque;

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
    pub ctc_measurements: Option<autoeq::roomeq::CtcMeasurementConfig>,
    pub ctc_config: Option<autoeq::roomeq::CtcConfig>,
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
    /// Full DSP chain output captured from the optimizer. Used by the
    /// "Apply to chain" action to drive the rack-vs-graph apply path
    /// (mirrors `app-gpui`'s `RoomEqState::dsp_output`).
    pub dsp_output: Option<sotf_audio_player::room_eq_types::DspChainOutput>,
    /// Status message shown next to the apply buttons.
    pub apply_status: Option<String>,
    /// Error from the most recent "Apply to chain" attempt.
    pub apply_error: Option<String>,
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
            ctc_measurements: None,
            ctc_config: None,
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
            dsp_output: None,
            apply_status: None,
            apply_error: None,
        }
    }
}

impl RoomEqTuiState {
    /// Compute the average slope for L and R channels in dB/octave.
    pub fn compute_lr_slope(&self) -> Option<(f64, f64, f64)> {
        sotf_audio_player::room_eq_types::compute_lr_slope(&self.channel_measurements)
    }
}
