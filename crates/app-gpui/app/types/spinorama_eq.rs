// ============================================================================
// Spinorama EQ Screen Types
// ============================================================================

use strsim::jaro_winkler;

use super::room_eq::{AutoEqField, OptimizationStatus};

// Re-export shared domain types from player crate
pub use sotf_audio_player::spinorama_eq_types::{
    SpinoramaBiquad, SpinoramaEqResult, SpinoramaOptimizationMode, SpinoramaOptimizerConfig,
    SpinoramaStep, SpinoramaTargetCurve,
};

/// A single directivity curve at a specific angle
#[derive(Debug, Clone, Default)]
pub struct DirectivityCurve {
    /// Angle in degrees
    pub angle: f64,
    /// Frequency points
    pub frequencies: Vec<f64>,
    /// SPL values (dB)
    pub spl: Vec<f64>,
}

/// CEA2034 spinorama curves data for plotting
#[derive(Debug, Clone, Default)]
pub struct SpinoramaCurves {
    /// Frequency points (shared across all curves)
    pub frequencies: Vec<f64>,
    /// On Axis response (dB)
    pub on_axis: Vec<f64>,
    /// Listening Window response (dB)
    pub listening_window: Vec<f64>,
    /// Early Reflections response (dB)
    pub early_reflections: Vec<f64>,
    /// Sound Power response (dB)
    pub sound_power: Vec<f64>,
    /// Early Reflections DI (dB) - for secondary y-axis
    pub early_reflections_di: Vec<f64>,
    /// Sound Power DI (dB) - for secondary y-axis
    pub sound_power_di: Vec<f64>,
    /// Estimated In-Room Response (PIR) - computed from LW, ER, SP
    pub estimated_in_room: Vec<f64>,
    /// Horizontal directivity curves (SPL Horizontal at various angles)
    pub horizontal_directivity: Vec<DirectivityCurve>,
    /// Vertical directivity curves (SPL Vertical at various angles)
    pub vertical_directivity: Vec<DirectivityCurve>,
}

impl SpinoramaCurves {
    /// Check if we have valid CEA2034 data to plot
    pub fn is_valid(&self) -> bool {
        !self.frequencies.is_empty()
            && self.frequencies.len() == self.on_axis.len()
            && self.frequencies.len() == self.listening_window.len()
    }

    /// Check if we have PIR data
    pub fn has_pir(&self) -> bool {
        !self.estimated_in_room.is_empty()
    }

    /// Check if we have horizontal directivity data
    pub fn has_horizontal(&self) -> bool {
        !self.horizontal_directivity.is_empty()
    }

    /// Check if we have vertical directivity data
    pub fn has_vertical(&self) -> bool {
        !self.vertical_directivity.is_empty()
    }
}

/// UI state for Spinorama EQ dropdowns
#[derive(Debug, Clone)]
pub struct SpinoramaEqDropdowns {
    pub version_open: bool,
    pub measurement_open: bool,
    pub curve_open: bool,
    pub mode_open: bool,
    pub algorithm_open: bool,
    pub export_format_open: bool,
    /// Target curve dropdown (ON, LW, PIR, ER)
    pub target_curve_open: bool,
    /// AutoEQ form: EQ mode dropdown (IIR/FIR)
    pub opt_mode_open: bool,
    /// Selected EQ mode ("iir", "fir", "mixed")
    pub opt_mode: String,
    /// AutoEQ form: FIR phase dropdown
    pub fir_phase_open: bool,
    /// AutoEQ form: PEQ model dropdown
    pub peq_model_open: bool,
    /// AutoEQ form: DE strategy dropdown
    pub strategy_open: bool,
    /// AutoEQ form: local algorithm dropdown
    pub local_algo_open: bool,
    /// AutoEQ form editing state
    pub autoeq_editing_field: Option<AutoEqField>,
    /// AutoEQ form edit text
    pub autoeq_edit_text: String,
}

impl Default for SpinoramaEqDropdowns {
    fn default() -> Self {
        Self {
            version_open: false,
            measurement_open: false,
            curve_open: false,
            mode_open: false,
            algorithm_open: false,
            export_format_open: false,
            target_curve_open: false,
            opt_mode_open: false,
            opt_mode: "iir".to_string(),
            fir_phase_open: false,
            peq_model_open: false,
            strategy_open: false,
            local_algo_open: false,
            autoeq_editing_field: None,
            autoeq_edit_text: String::new(),
        }
    }
}

/// Complete Spinorama EQ screen state
#[derive(Debug, Clone)]
pub struct SpinoramaEqState {
    /// Current step in the workflow
    pub step: SpinoramaStep,

    // === Step 1: Speaker Selection ===
    /// Search input text
    pub speaker_search: String,
    /// List of available speakers from API
    pub available_speakers: Vec<String>,
    /// Filtered suggestions based on search
    pub speaker_suggestions: Vec<String>,
    /// Selected speaker name (e.g., "KEF R3")
    pub selected_speaker: Option<String>,
    /// Selected version (e.g., "asr", "erin", "princeton")
    pub selected_version: String,
    /// Selected measurement type (e.g., "CEA2034")
    pub selected_measurement: String,
    /// Selected curve (e.g., "Estimated In-Room Response")
    pub selected_curve: String,
    /// Available versions for selected speaker
    pub available_versions: Vec<String>,
    /// Available measurements for selected speaker/version
    pub available_measurements: Vec<String>,
    /// Available curves for selected measurement
    pub available_curves: Vec<String>,

    // === Step 2: Configuration ===
    /// Optimizer configuration
    pub optimizer_config: SpinoramaOptimizerConfig,

    // === Step 3: Optimization ===
    /// Current optimization status
    pub optimization_status: OptimizationStatus,
    /// Progress (0.0 - 1.0)
    pub progress: f32,
    /// Progress history for loss/score curves (iteration, loss, optional_score)
    pub progress_history: Vec<(usize, f64, Option<f64>)>,
    /// Status message during optimization
    pub status_message: String,
    /// Error message if optimization failed
    pub error_message: Option<String>,

    // === Step 4: Results ===
    /// Optimization result (simplified for UI)
    pub result: Option<SpinoramaEqResult>,
    /// Full optimization result (for graphs)
    pub full_result: Option<sotf_audio_player::autoeq::SpeakerOptimizationResult>,
    /// Export format selection
    pub export_format: String,

    // === UI State ===
    /// Detail level for the configuration form (Simple / Intermediate / Expert)
    pub detail_level: sotf_audio_player::autoeq::DetailLevel,
    /// Currently selected preset id
    pub selected_preset: String,
    /// Loading indicator for speakers API call
    pub loading_speakers: bool,
    /// Loading indicator for versions API call
    pub loading_versions: bool,
    /// Loading indicator for measurements API call
    pub loading_measurements: bool,
    /// Dropdown states
    pub dropdowns: SpinoramaEqDropdowns,
    /// Expanded accordion sections
    pub expanded_sections: Vec<gpui::SharedString>,
    /// Timestamp when speakers were last fetched (for cache invalidation)
    pub speakers_cached_at: Option<std::time::Instant>,
    /// Focus handle for the search input
    pub search_focus_handle: Option<gpui::FocusHandle>,
    /// Whether the selected measurement has phase data
    pub has_phase_data: bool,

    // === Preview Curves (computed before optimization) ===
    /// Preview frequencies (Hz)
    pub preview_frequencies: Vec<f64>,
    /// Preview input curve (dB) - the raw measurement
    pub preview_input_curve: Vec<f64>,
    /// Preview target curve (dB) - what we're optimizing towards
    pub preview_target_curve: Vec<f64>,
    /// Preview deviation curve (dB) - target minus input
    pub preview_deviation_curve: Vec<f64>,
    /// Whether preview curves are being loaded
    pub loading_preview: bool,
    /// Error message if preview loading failed
    pub preview_error: Option<String>,

    // === Spinorama Curves (for CEA2034 plot in Step 1) ===
    /// CEA2034 curves data for spinorama plot
    pub spinorama_curves: SpinoramaCurves,
    /// Whether spinorama curves are being loaded
    pub loading_spinorama_curves: bool,
    /// Error message if spinorama curves loading failed
    pub spinorama_curves_error: Option<String>,
}

impl Default for SpinoramaEqState {
    fn default() -> Self {
        Self {
            step: SpinoramaStep::SelectSpeaker,
            speaker_search: String::new(),
            available_speakers: Vec::new(),
            speaker_suggestions: Vec::new(),
            selected_speaker: None,
            selected_version: "asr".to_string(),
            selected_measurement: "CEA2034".to_string(),
            selected_curve: "Estimated In-Room Response".to_string(),
            available_versions: Vec::new(),
            available_measurements: Vec::new(),
            available_curves: Vec::new(),
            optimizer_config: SpinoramaOptimizerConfig::default(),
            optimization_status: OptimizationStatus::Idle,
            progress: 0.0,
            progress_history: Vec::new(),
            status_message: String::new(),
            error_message: None,
            result: None,
            full_result: None,
            export_format: "json".to_string(),
            detail_level: sotf_audio_player::autoeq::DetailLevel::Simple,
            selected_preset: "balanced".to_string(),
            loading_speakers: false,
            loading_versions: false,
            loading_measurements: false,
            dropdowns: SpinoramaEqDropdowns::default(),
            expanded_sections: vec!["speaker".into(), "options".into()],
            speakers_cached_at: None,
            search_focus_handle: None,
            has_phase_data: false,
            preview_frequencies: Vec::new(),
            preview_input_curve: Vec::new(),
            preview_target_curve: Vec::new(),
            preview_deviation_curve: Vec::new(),
            loading_preview: false,
            preview_error: None,
            spinorama_curves: SpinoramaCurves::default(),
            loading_spinorama_curves: false,
            spinorama_curves_error: None,
        }
    }
}

impl SpinoramaEqState {
    /// Check if we can proceed from the current step
    pub fn can_advance(&self) -> bool {
        match self.step {
            SpinoramaStep::SelectSpeaker => self.selected_speaker.is_some(),
            // Configure step now includes optimization - must complete before advancing
            SpinoramaStep::Configure => self.optimization_status == OptimizationStatus::Completed,
            SpinoramaStep::Review => self.result.is_some(),
            SpinoramaStep::Export => true, // Always can proceed (or stay) from export
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

    pub fn supported_eq_modes(&self) -> &'static [&'static str] {
        &["iir"]
    }

    pub fn selected_eq_mode(&self) -> &'static str {
        "iir"
    }

    pub fn set_selected_eq_mode(&mut self, _mode: &str) {
        self.dropdowns.opt_mode = "iir".to_string();
    }

    /// Update speaker suggestions based on search query with fuzzy matching
    pub fn update_suggestions(&mut self) {
        if self.speaker_search.is_empty() {
            // Show all speakers when search is empty (original behavior)
            self.speaker_suggestions = self.available_speakers.clone();
            return;
        }

        let query_lower = self.speaker_search.to_lowercase();
        let exact_matches: Vec<String> = self
            .available_speakers
            .iter()
            .filter(|s| s.to_lowercase().contains(&query_lower))
            .cloned()
            .collect();

        if !exact_matches.is_empty() {
            self.speaker_suggestions = exact_matches;
        } else {
            let mut scored: Vec<(String, f64)> = self
                .available_speakers
                .iter()
                .filter_map(|s| {
                    fuzzy_match_score(&self.speaker_search, s).map(|score| (s.clone(), score))
                })
                .collect();

            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            self.speaker_suggestions = scored.into_iter().map(|(s, _)| s).collect();
        }

        // Limit to reasonable number for UI
        self.speaker_suggestions.truncate(50);
    }

    /// Check if speakers cache needs to be refreshed (older than 1 hour or not loaded)
    pub fn needs_speaker_refresh(&self) -> bool {
        if self.available_speakers.is_empty() {
            return true;
        }
        match self.speakers_cached_at {
            Some(cached_at) => cached_at.elapsed() > std::time::Duration::from_secs(3600),
            None => true,
        }
    }
}

/// Score how well a query matches a speaker name using fuzzy multi-word matching.
/// Returns Some(score) if all query words match, None otherwise.
/// Uses Jaro-Winkler similarity with a 0.8 threshold for typo tolerance.
fn fuzzy_match_score(query: &str, speaker: &str) -> Option<f64> {
    let query_lower = query.to_lowercase();
    let speaker_lower = speaker.to_lowercase();

    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
    let speaker_words: Vec<&str> = speaker_lower.split_whitespace().collect();

    if query_words.is_empty() {
        return Some(1.0);
    }

    let mut total_score = 0.0;

    for query_word in &query_words {
        // Find best matching word in speaker name
        let best_match = speaker_words
            .iter()
            .map(|sw| jaro_winkler(query_word, sw))
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        // Require minimum similarity threshold (0.8 = ~80% similar)
        if best_match < 0.8 {
            return None; // Word doesn't match
        }
        total_score += best_match;
    }

    // Average score across all query words
    Some(total_score / query_words.len() as f64)
}
