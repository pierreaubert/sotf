//! Shared, UI-agnostic view model for the Headphone EQ wizard.
//!
//! Domain state (measurement source, headphone selection, optimizer config,
//! optimization progress/results) lives here; view state (dropdowns, focus
//! flags, edit buffers) stays in `app-gpui` / `app-tui`.

use crate::headphone_eq_types::{
    HeadphoneEqBiquad, HeadphoneEqOptimizerConfig, HeadphoneEqResult, HeadphoneEqStep,
    HeadphoneMeasurementSource,
};
use crate::room_eq_types::OptimizationStatus;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use strsim::jaro_winkler;

/// Domain state for the Headphone EQ wizard, independent of any UI toolkit.
#[derive(Debug, Clone)]
pub struct HeadphoneEqScreenModel {
    /// Current step in the workflow.
    pub step: HeadphoneEqStep,

    // === Step 1: Select Files ===
    /// Measurement source: File or Spinorama download.
    pub measurement_source: HeadphoneMeasurementSource,
    /// Path to headphone measurement file (CSV).
    pub measurement_path: String,

    // === Step 1 (Spinorama mode): Headphone Search ===
    /// Search input text.
    pub headphone_search: String,
    /// List of available headphones from API.
    pub available_headphones: Vec<String>,
    /// Filtered suggestions based on search.
    pub headphone_suggestions: Vec<String>,
    /// Selected headphone name.
    pub selected_headphone: Option<String>,
    /// Loading indicator for headphones API call.
    pub loading_headphones: bool,
    /// Loading indicator for download chain (versions + measurements + curve).
    pub loading_download: bool,
    /// Timestamp when headphones were last fetched.
    pub headphones_cached_at: Option<std::time::Instant>,
    /// Downloaded frequency response curve (freq, spl) for preview graph.
    pub downloaded_curve: Option<Vec<(f64, f64)>>,

    // === Goals & Configuration ===
    /// Loss function type ("flat" or "score").
    pub loss_type: String,
    /// Target curve selection (preset name or "custom").
    pub target_preset: String,
    /// Path to custom target file (if target_preset == "custom").
    pub custom_target_path: String,

    // === Step 2: Configuration ===
    /// Optimizer configuration.
    pub optimizer_config: HeadphoneEqOptimizerConfig,

    // === Step 3: Optimization ===
    /// Current optimization status.
    pub optimization_status: OptimizationStatus,
    /// Cancel-request flag polled by the optimisation callback. UI sets
    /// this to true when the user clicks Cancel; the callback returns
    /// `CallbackAction::Stop` on the next iteration.
    pub cancel_requested: Arc<AtomicBool>,
    /// Progress (0.0 - 1.0).
    pub progress: f32,
    /// Progress history for loss curve (iteration, loss).
    pub progress_history: Vec<(usize, f64)>,
    /// Current loss value reported by the optimizer.
    pub current_loss: f64,
    /// Current iteration number reported by the optimizer.
    pub current_iteration: usize,

    // === Step 4: Apply ===
    /// Optimization result (biquads, etc.).
    pub result: Option<HeadphoneEqResult>,
    /// Filter list derived from the optimization result (TUI cache).
    pub filters: Vec<HeadphoneEqBiquad>,
    /// Pre-optimization loss (TUI cache).
    pub pre_loss: f64,
    /// Post-optimization loss (TUI cache).
    pub post_loss: f64,
    /// Frequency response curve frequencies (TUI cache).
    pub curve_frequencies: Vec<f64>,
    /// Input frequency response curve (TUI cache).
    pub curve_input: Vec<f64>,
    /// Target frequency response curve (TUI cache).
    pub curve_target: Vec<f64>,
    /// Corrected frequency response curve (TUI cache).
    pub curve_corrected: Vec<f64>,
    /// Combined filter response curve (TUI cache).
    pub curve_filter_response: Vec<f64>,

    // === Step 5: Export ===
    /// Export format selection.
    pub export_format: String,
    /// EQ preset name for saving.
    pub save_name: String,

    // === Messages ===
    /// Short status message surfaced by the view.
    pub status_message: String,
    /// Active error message, if any.
    pub error_message: Option<String>,
}

impl Default for HeadphoneEqScreenModel {
    fn default() -> Self {
        Self {
            step: HeadphoneEqStep::MeasurementTarget,
            measurement_source: HeadphoneMeasurementSource::default(),
            measurement_path: String::new(),
            headphone_search: String::new(),
            available_headphones: Vec::new(),
            headphone_suggestions: Vec::new(),
            selected_headphone: None,
            loading_headphones: false,
            loading_download: false,
            headphones_cached_at: None,
            downloaded_curve: None,
            loss_type: "flat".to_string(),
            target_preset: "harman-over-ear-2018".to_string(),
            custom_target_path: String::new(),
            optimizer_config: HeadphoneEqOptimizerConfig::default(),
            optimization_status: OptimizationStatus::Idle,
            cancel_requested: Arc::new(AtomicBool::new(false)),
            progress: 0.0,
            progress_history: Vec::new(),
            current_loss: 0.0,
            current_iteration: 0,
            result: None,
            filters: Vec::new(),
            pre_loss: 0.0,
            post_loss: 0.0,
            curve_frequencies: Vec::new(),
            curve_input: Vec::new(),
            curve_target: Vec::new(),
            curve_corrected: Vec::new(),
            curve_filter_response: Vec::new(),
            export_format: "json".to_string(),
            save_name: String::new(),
            status_message: String::new(),
            error_message: None,
        }
    }
}

impl HeadphoneEqScreenModel {
    /// Return the UI-normalized loss type ("flat" or "score").
    pub fn ui_loss_type(&self) -> &'static str {
        match self.optimizer_config.loss.as_str() {
            "flat" | "headphone-flat" => "flat",
            "score" | "headphone-score" => "score",
            _ => "score",
        }
    }

    /// Set the loss type from a UI value, normalizing to the optimizer value.
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

    /// True when the selected target preset requires a custom target file.
    pub fn requires_custom_target_path(&self) -> bool {
        self.target_preset == "custom"
    }

    /// True when a non-empty custom target path has been provided.
    pub fn has_custom_target_path(&self) -> bool {
        !self.custom_target_path.trim().is_empty()
    }

    /// Check if we can proceed from the current step.
    pub fn can_advance(&self) -> bool {
        match self.step {
            HeadphoneEqStep::MeasurementTarget => !self.measurement_path.is_empty(),
            HeadphoneEqStep::Optimization => {
                self.optimization_status == OptimizationStatus::Completed
            }
            HeadphoneEqStep::Listen => self.result.is_some(),
            HeadphoneEqStep::Export => true,
        }
    }

    /// Check if optimization is running.
    pub fn is_optimizing(&self) -> bool {
        self.optimization_status == OptimizationStatus::Running
    }

    /// Reset optimization state.
    pub fn reset_optimization(&mut self) {
        self.optimization_status = OptimizationStatus::Idle;
        self.cancel_requested.store(false, Ordering::Relaxed);
        self.progress = 0.0;
        self.progress_history.clear();
        self.current_loss = 0.0;
        self.current_iteration = 0;
        self.result = None;
        self.filters.clear();
        self.pre_loss = 0.0;
        self.post_loss = 0.0;
        self.curve_frequencies.clear();
        self.curve_input.clear();
        self.curve_target.clear();
        self.curve_corrected.clear();
        self.curve_filter_response.clear();
        self.error_message = None;
        self.status_message = String::new();
    }

    /// Update headphone suggestions based on search query with fuzzy matching.
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

    /// Check if headphones cache needs refresh (older than 1 hour or not loaded).
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
