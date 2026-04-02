// ============================================================================
// Headphone EQ Screen Types
// ============================================================================
//
// Domain types are shared via the player crate. UI-specific state stays here.

use strsim::jaro_winkler;

use super::room_eq::{AutoEqField, OptimizationStatus};

// Re-export shared domain types from player crate
pub use sotf_audio_player::headphone_eq_types::{
    HeadphoneEqBiquad, HeadphoneEqOptimizerConfig, HeadphoneEqResult, HeadphoneEqStep,
    HeadphoneMeasurementSource,
};

/// UI state for Headphone EQ dropdowns
#[derive(Debug, Clone, Default)]
pub struct HeadphoneEqDropdowns {
    pub target_open: bool,
    pub algorithm_open: bool,
    pub peq_model_open: bool,
    pub export_format_open: bool,
    pub loss_type_open: bool,
    pub target_curve_open: bool,
    pub strategy_open: bool,
    pub local_algo_open: bool,
    /// AutoEQ form editing state
    pub autoeq_editing_field: Option<AutoEqField>,
    /// AutoEQ form edit text
    pub autoeq_edit_text: String,
}

/// Complete Headphone EQ screen state
#[derive(Debug, Clone)]
pub struct HeadphoneEqState {
    /// Current step in the workflow
    pub step: HeadphoneEqStep,

    // === Step 1: Select Files ===
    /// Measurement source: File or Spinorama download
    pub measurement_source: HeadphoneMeasurementSource,
    /// Path to headphone measurement file (CSV)
    pub measurement_path: Option<String>,

    // === Step 1 (Spinorama mode): Headphone Search ===
    /// Search input text
    pub headphone_search: String,
    /// List of available headphones from API
    pub available_headphones: Vec<String>,
    /// Filtered suggestions based on search
    pub headphone_suggestions: Vec<String>,
    /// Selected headphone name
    pub selected_headphone: Option<String>,
    /// Loading indicator for headphones API call
    pub loading_headphones: bool,
    /// Loading indicator for download chain (versions + measurements + curve)
    pub loading_download: bool,
    /// Timestamp when headphones were last fetched
    pub headphones_cached_at: Option<std::time::Instant>,
    /// Downloaded frequency response curve (freq, spl) for preview graph
    pub downloaded_curve: Option<Vec<(f64, f64)>>,

    // === Goals & Configuration ===
    /// Loss function type ("flat" or "score")
    pub loss_type: String,
    /// Target curve selection (preset name or "custom")
    pub target_preset: String,
    /// Path to custom target file (if target_preset == "custom")
    pub custom_target_path: Option<String>,

    // === Step 2: Configuration ===
    /// Optimizer configuration
    pub optimizer_config: HeadphoneEqOptimizerConfig,

    // === Step 3: Optimization ===
    /// Current optimization status
    pub optimization_status: OptimizationStatus,
    /// Progress (0.0 - 1.0)
    pub progress: f32,
    /// Progress history for loss curve (iteration, loss)
    pub progress_history: Vec<(usize, f64)>,

    // === Step 4: Apply ===
    /// Optimization result (biquads, etc.)
    pub result: Option<HeadphoneEqResult>,
    /// Export format selection
    pub export_format: String,
    /// EQ preset name for saving
    pub save_name: String,

    // === UI State ===
    pub dropdowns: HeadphoneEqDropdowns,
    pub status_message: String,
    pub error_message: Option<String>,
    /// Expanded accordion sections
    pub expanded_sections: Vec<gpui::SharedString>,
}

impl Default for HeadphoneEqState {
    fn default() -> Self {
        Self {
            step: HeadphoneEqStep::MeasurementTarget,
            measurement_source: HeadphoneMeasurementSource::default(),
            measurement_path: None,
            headphone_search: String::new(),
            available_headphones: Vec::new(),
            headphone_suggestions: Vec::new(),
            selected_headphone: None,
            loading_headphones: false,
            loading_download: false,
            headphones_cached_at: None,
            downloaded_curve: None,
            loss_type: "score".to_string(),
            target_preset: "harman-over-ear-2018".to_string(),
            custom_target_path: None,
            optimizer_config: HeadphoneEqOptimizerConfig::default(),
            optimization_status: OptimizationStatus::Idle,
            progress: 0.0,
            progress_history: Vec::new(),
            result: None,
            export_format: "json".to_string(),
            save_name: String::new(),
            dropdowns: HeadphoneEqDropdowns::default(),
            status_message: String::new(),
            error_message: None,
            expanded_sections: vec!["measurement".into(), "target".into(), "eq-design".into()],
        }
    }
}

impl HeadphoneEqState {
    pub fn ui_loss_type(&self) -> &'static str {
        match self.optimizer_config.loss.as_str() {
            "flat" | "headphone-flat" => "flat",
            "score" | "headphone-score" => "score",
            _ => "score",
        }
    }

    pub fn set_ui_loss_type(&mut self, loss_type: &str) {
        self.loss_type = match loss_type {
            "flat" | "headphone-flat" => "flat".to_string(),
            _ => "score".to_string(),
        };
        self.optimizer_config.loss = match loss_type {
            "flat" | "headphone-flat" => "headphone-flat".to_string(),
            _ => "headphone-score".to_string(),
        };
    }

    pub fn requires_custom_target_path(&self) -> bool {
        self.target_preset == "custom"
    }

    pub fn has_custom_target_path(&self) -> bool {
        self.custom_target_path
            .as_ref()
            .is_some_and(|path| !path.trim().is_empty())
    }

    /// Check if we can proceed from the current step
    pub fn can_advance(&self) -> bool {
        match self.step {
            HeadphoneEqStep::MeasurementTarget => self.measurement_path.is_some(),
            HeadphoneEqStep::Optimization => {
                self.optimization_status == OptimizationStatus::Completed
            }
            HeadphoneEqStep::Listen => self.result.is_some(),
            HeadphoneEqStep::Export => true,
        }
    }

    /// Check if optimization is running
    pub fn is_optimizing(&self) -> bool {
        self.optimization_status == OptimizationStatus::Running
    }

    /// Reset optimization state
    pub fn reset_optimization(&mut self) {
        self.optimization_status = OptimizationStatus::Idle;
        self.progress = 0.0;
        self.progress_history.clear();
        self.result = None;
        self.error_message = None;
    }

    /// Update headphone suggestions based on search query with fuzzy matching
    pub fn update_headphone_suggestions(&mut self) {
        if self.headphone_search.is_empty() {
            self.headphone_suggestions = self.available_headphones.clone();
            return;
        }

        let query_lower = self.headphone_search.to_lowercase();
        let exact_matches: Vec<String> = self
            .available_headphones
            .iter()
            .filter(|s| s.to_lowercase().contains(&query_lower))
            .cloned()
            .collect();

        if !exact_matches.is_empty() {
            self.headphone_suggestions = exact_matches;
        } else {
            let mut scored: Vec<(String, f64)> = self
                .available_headphones
                .iter()
                .filter_map(|s| {
                    headphone_fuzzy_match_score(&self.headphone_search, s)
                        .map(|score| (s.clone(), score))
                })
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            self.headphone_suggestions = scored.into_iter().map(|(s, _)| s).collect();
        }

        self.headphone_suggestions.truncate(50);
    }

    /// Check if headphones cache needs refresh (older than 1 hour or not loaded)
    pub fn needs_headphone_refresh(&self) -> bool {
        if self.available_headphones.is_empty() {
            return true;
        }
        match self.headphones_cached_at {
            Some(cached_at) => cached_at.elapsed() > std::time::Duration::from_secs(3600),
            None => true,
        }
    }
}

fn headphone_fuzzy_match_score(query: &str, name: &str) -> Option<f64> {
    let query_lower = query.to_lowercase();
    let name_lower = name.to_lowercase();
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
    let name_words: Vec<&str> = name_lower.split_whitespace().collect();

    if query_words.is_empty() {
        return Some(1.0);
    }

    let mut total_score = 0.0;
    for query_word in &query_words {
        let best_match = name_words
            .iter()
            .map(|nw| jaro_winkler(query_word, nw))
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);
        if best_match < 0.8 {
            return None;
        }
        total_score += best_match;
    }

    Some(total_score / query_words.len() as f64)
}
