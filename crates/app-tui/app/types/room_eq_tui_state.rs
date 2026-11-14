use sotf_audio_player::recording_helpers::EffectiveAcquisitionConfig;
use sotf_audio_player::recording_types::{RecordingSourceIdentity, TakeQualitySummary};
use sotf_audio_player::room_eq_types::{RoomEqEasyLayout, validate_room_eq_easy_layout};
use sotf_audio_player::ui_models::room_eq::RoomEqScreenModel;
use std::collections::VecDeque;

/// Quality evidence associated with one loaded `RoomEQ` source.
///
/// `quality: None` means the source has no current, validated sidecar entry;
/// it is presented as unknown evidence, never as a passing take.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomEqQualityReviewEntry {
    pub source: RecordingSourceIdentity,
    /// Read-only provenance label retained from the source measurement name.
    pub source_label: Option<String>,
    pub quality: Option<TakeQualitySummary>,
    /// Per-take generator settings restored from the exact-session sidecar.
    pub stimulus_config: Option<EffectiveAcquisitionConfig>,
}

/// TUI-specific UI state for the Room EQ wizard.
///
/// Domain state (measurements, optimizer config, optimization progress and
/// results) lives in the shared [`RoomEqScreenModel`] from `sotf-player`;
/// this struct only holds view state that is specific to the terminal UI.
#[derive(Debug, Clone, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent UI options and status flags retain their existing state schema"
)]
pub struct RoomEqTuiState {
    /// Shared, UI-agnostic Room EQ domain model.
    pub model: RoomEqScreenModel,
    /// Explicit layout selected by the terminal beginner workflow.
    pub easy_layout: RoomEqEasyLayout,

    /// When true, focus is on the step tabs row; Left/Right/Tab cycle steps.
    /// When false, focus is inside the current step's content.
    pub step_tab_focused: bool,

    // Step 1: load measurement file (JSON)
    pub file_path: String,
    pub editing_file_path: bool,
    pub load_error: Option<String>,
    /// Per-source capture quality loaded from the validated recording sidecar.
    pub quality_review_entries: Vec<RoomEqQualityReviewEntry>,
    /// Explains why no valid quality sidecar could be used.
    pub quality_sidecar_warning: Option<String>,

    // Step 2: delay detection (tone-burst probe). Business state lives in
    // the shared `DelayDetectionState`; the `dd_*` fields below are
    // TUI-only UI state.
    /// Index of the currently focused form field on the delay-detection
    /// step (0..=3: `probe_duration`, `silence_duration`, `input_channel`,
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
    pub selected_field: usize,
    pub selected_section: usize,
    /// True when a numerical field is being directly edited via keyboard
    pub editing_value: bool,
    pub edit_buffer: String,

    // Step 3: optimization
    /// Maximum iterations for the current optimization run (cached from
    /// progress reports for display).
    pub opt_max_iter: usize,
    /// Log buffer for optimization messages (max 300 lines)
    pub opt_log_lines: VecDeque<String>,
    /// Scroll offset from bottom (0 = bottom)
    pub opt_log_scroll: usize,
    /// Loss history used to draw the loss chart (UI-only cache).
    pub loss_history: Vec<(usize, f64)>,

    // Step 4: review
    pub selected_channel: usize,

    // Step 5: export
    pub export_path: String,
    pub editing_export_path: bool,
    pub export_error: Option<String>,
    pub export_success: bool,

    /// Status message shown next to the apply buttons.
    pub apply_status: Option<String>,
    /// Error from the most recent "Apply to chain" attempt.
    pub apply_error: Option<String>,
}

impl RoomEqTuiState {
    pub fn infer_easy_layout(&mut self) {
        let channel_names = self
            .model
            .channel_measurements
            .iter()
            .map(|measurement| measurement.channel_name.as_str())
            .collect::<Vec<_>>();
        if let Some(layout) = RoomEqEasyLayout::ALL.into_iter().find(|layout| {
            validate_room_eq_easy_layout(*layout, channel_names.iter().copied()).is_ok()
        }) {
            self.easy_layout = layout;
            layout.configure_preset_defaults(&mut self.model.simple_preset);
        }
    }

    /// Total number of speakers being optimized.
    ///
    /// Falls back to the number of channel results when measurements have
    /// not yet been converted to speaker configs.
    #[must_use]
    pub fn opt_total_speakers(&self) -> usize {
        let n = self.model.speaker_configs.len();
        if n > 0 {
            n
        } else {
            self.model.channel_results.len()
        }
    }
}
