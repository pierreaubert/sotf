use super::SpinUpdateSubStep;
use super::headphone_eq_step::HeadphoneEqStep;
use sotf_audio_player::autoeq::DetailLevel;
use sotf_audio_player::ui_models::headphone_eq::HeadphoneEqScreenModel;

/// TUI state for the Headphone EQ wizard.
///
/// Domain state (measurement source, headphone selection, optimizer config,
/// optimization progress/results) lives in the shared [`HeadphoneEqScreenModel`]
/// from `sotf-player`; this struct only holds view state that is specific to
/// the terminal UI.
#[derive(Debug, Clone, Default)]
pub struct HeadphoneEqTuiState {
    /// Shared, UI-agnostic Headphone EQ wizard domain model.
    pub model: HeadphoneEqScreenModel,

    /// Current step in the TUI workflow.
    pub step: HeadphoneEqStep,
    /// When true, the wizard step tab bar has focus (Left/Right change step).
    pub step_tab_focused: bool,
    /// Detail level for the Configure step (Simple / Intermediate / Expert).
    pub detail_level: DetailLevel,
    /// Currently selected preset id (e.g. "balanced", "custom").
    pub selected_preset: String,

    // Step 1: measurement source
    pub editing_measurement: bool,
    pub editing_custom_target: bool,
    pub selected_field: usize,

    // Step 1 (Spinorama mode): headphone search
    pub selected_headphone_idx: usize,
    pub headphones_error: Option<String>,
    pub editing_search: bool,

    // Step 2: configuration (shared config struct)
    pub config_selected_field: usize,
    /// True when a numerical field is being directly edited via keyboard
    pub editing_value: bool,
    pub edit_buffer: String,

    // Step 3: optimization progress
    pub opt_max_iter: usize,

    // Step 5: update plugin confirmation
    pub update_substep: SpinUpdateSubStep,
    /// (slot_index, filter_count) of existing EQ to overwrite
    pub update_existing_eq_info: Option<(usize, usize)>,
}

impl HeadphoneEqTuiState {
    /// Update filtered headphones based on search query.
    pub fn update_filter(&mut self) {
        if self.model.headphone_search.is_empty() {
            self.model.headphone_suggestions = self.model.available_headphones.clone();
        } else {
            let query_lower = self.model.headphone_search.to_lowercase();
            self.model.headphone_suggestions = self
                .model
                .available_headphones
                .iter()
                .filter(|h| h.to_lowercase().contains(&query_lower))
                .cloned()
                .collect();
        }
        // Clamp index
        if !self.model.headphone_suggestions.is_empty() {
            self.selected_headphone_idx = self
                .selected_headphone_idx
                .min(self.model.headphone_suggestions.len() - 1);
        } else {
            self.selected_headphone_idx = 0;
        }
    }
}
