use super::SpinUpdateSubStep;
use super::headphone_eq_step::HeadphoneEqStep;
use sotf_audio_player::headphone_eq_types::{
    HeadphoneEqBiquad, HeadphoneEqOptimizerConfig, HeadphoneMeasurementSource,
};
use sotf_audio_player::room_eq_types::OptimizationStatus;

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
