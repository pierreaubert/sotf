// ============================================================================
// Spinorama EQ Screen Types
// ============================================================================

use serde::{Deserialize, Serialize};
use strsim::jaro_winkler;

use super::room_eq::{AutoEqField, OptimizationStatus, RoomEqAlgorithm};

/// Spinorama EQ workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpinoramaStep {
    /// Step 1: Select speaker from spinorama.org
    #[default]
    SelectSpeaker,
    /// Step 2: Configure and run optimization
    Configure,
    /// Step 3: Review results and visualizations
    Review,
    /// Step 4: Apply to playback and export
    Export,
}

impl SpinoramaStep {
    /// Get all steps in order
    pub fn all() -> &'static [SpinoramaStep] {
        &[
            SpinoramaStep::SelectSpeaker,
            SpinoramaStep::Configure,
            SpinoramaStep::Review,
            SpinoramaStep::Export,
        ]
    }

    /// Get step index (0-based)
    pub fn index(&self) -> usize {
        match self {
            SpinoramaStep::SelectSpeaker => 0,
            SpinoramaStep::Configure => 1,
            SpinoramaStep::Review => 2,
            SpinoramaStep::Export => 3,
        }
    }

    /// Get step label
    pub fn label(&self) -> &'static str {
        match self {
            SpinoramaStep::SelectSpeaker => "Select",
            SpinoramaStep::Configure => "Configure",
            SpinoramaStep::Review => "Review",
            SpinoramaStep::Export => "Export",
        }
    }

    /// Get next step
    pub fn next(&self) -> Option<SpinoramaStep> {
        match self {
            SpinoramaStep::SelectSpeaker => Some(SpinoramaStep::Configure),
            SpinoramaStep::Configure => Some(SpinoramaStep::Review),
            SpinoramaStep::Review => Some(SpinoramaStep::Export),
            SpinoramaStep::Export => None,
        }
    }

    /// Get previous step
    pub fn previous(&self) -> Option<SpinoramaStep> {
        match self {
            SpinoramaStep::SelectSpeaker => None,
            SpinoramaStep::Configure => Some(SpinoramaStep::SelectSpeaker),
            SpinoramaStep::Review => Some(SpinoramaStep::Configure),
            SpinoramaStep::Export => Some(SpinoramaStep::Review),
        }
    }
}

/// Optimization mode for Spinorama EQ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SpinoramaOptimizationMode {
    /// Flatten a target curve (ON, LW, PIR, ER)
    #[default]
    FlatOnPir,
    /// Optimize Harman/Olive speaker preference score
    SpeakerScore,
}

/// Target curve types for spinorama optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SpinoramaTargetCurve {
    /// On-Axis response
    OnAxis,
    /// Listening Window response
    ListeningWindow,
    /// Estimated In-Room Response (default)
    #[default]
    EstimatedInRoom,
    /// Early Reflections
    EarlyReflections,
}

impl SpinoramaTargetCurve {
    pub fn all() -> &'static [SpinoramaTargetCurve] {
        &[
            SpinoramaTargetCurve::OnAxis,
            SpinoramaTargetCurve::ListeningWindow,
            SpinoramaTargetCurve::EstimatedInRoom,
            SpinoramaTargetCurve::EarlyReflections,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SpinoramaTargetCurve::OnAxis => "ON (On-Axis)",
            SpinoramaTargetCurve::ListeningWindow => "LW (Listening Window)",
            SpinoramaTargetCurve::EstimatedInRoom => "PIR (In-Room)",
            SpinoramaTargetCurve::EarlyReflections => "ER (Early Reflections)",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            SpinoramaTargetCurve::OnAxis => "ON",
            SpinoramaTargetCurve::ListeningWindow => "LW",
            SpinoramaTargetCurve::EstimatedInRoom => "PIR",
            SpinoramaTargetCurve::EarlyReflections => "ER",
        }
    }

    /// Get the curve name used in spinorama.org API
    pub fn api_name(&self) -> &'static str {
        match self {
            SpinoramaTargetCurve::OnAxis => "On Axis",
            SpinoramaTargetCurve::ListeningWindow => "Listening Window",
            SpinoramaTargetCurve::EstimatedInRoom => "Estimated In-Room Response",
            SpinoramaTargetCurve::EarlyReflections => "Early Reflections",
        }
    }
}

impl SpinoramaOptimizationMode {
    pub fn all() -> &'static [SpinoramaOptimizationMode] {
        &[
            SpinoramaOptimizationMode::FlatOnPir,
            SpinoramaOptimizationMode::SpeakerScore,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SpinoramaOptimizationMode::FlatOnPir => "Target",
            SpinoramaOptimizationMode::SpeakerScore => "Score",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            SpinoramaOptimizationMode::FlatOnPir => "Flatten the Estimated In-Room Response curve",
            SpinoramaOptimizationMode::SpeakerScore => {
                "Optimize for Harman/Olive speaker preference score"
            }
        }
    }

    pub fn to_loss_string(&self) -> &'static str {
        match self {
            SpinoramaOptimizationMode::FlatOnPir => "speaker-flat",
            SpinoramaOptimizationMode::SpeakerScore => "speaker-score",
        }
    }
}

/// Optimizer configuration for Spinorama EQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpinoramaOptimizerConfig {
    /// Optimization target mode
    pub mode: SpinoramaOptimizationMode,
    /// Target curve for FlatOnPir mode
    pub target_curve: SpinoramaTargetCurve,
    /// Optimization algorithm
    pub algorithm: RoomEqAlgorithm,
    /// Number of PEQ filters
    pub num_filters: usize,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of FIR taps (for FIR/Mixed mode)
    pub fir_taps: usize,
    /// FIR phase type (for FIR/Mixed mode): "linear", "minimum", "kirkeby"
    pub fir_phase: String,
    /// Minimum Q factor
    pub min_q: f64,
    /// Maximum Q factor
    pub max_q: f64,
    /// Minimum gain in dB
    pub min_db: f64,
    /// Maximum gain in dB
    pub max_db: f64,
    /// Minimum frequency in Hz
    pub min_freq: f64,
    /// Maximum frequency in Hz
    pub max_freq: f64,
    /// Maximum number of iterations
    pub max_iter: usize,
    /// PEQ model (e.g., "pk", "ls-pk-hs")
    pub peq_model: String,
    /// Population size for evolutionary algorithms
    pub population: usize,
    /// Mutation factor (F) for DE
    pub de_f: f64,
    /// Crossover rate (CR) for DE
    pub de_cr: f64,
    /// DE strategy (e.g., "currenttobest1bin")
    pub strategy: String,
    /// Enable local refinement after global optimization
    pub refine: bool,
    /// Local algorithm for refinement
    pub local_algo: String,
    /// Enable smoothing
    pub smooth: bool,
    /// Smoothing window size (1-24)
    pub smooth_n: usize,
    /// Spacing constraint weight (0-1000)
    pub spacing_weight: f64,
    /// Minimum spacing between filters in octaves (0.01-1.0)
    pub min_spacing_oct: f64,
    /// Relative tolerance for convergence
    pub tolerance: f64,
    /// Absolute tolerance for convergence
    pub atolerance: f64,
    /// Enable psychoacoustic variable smoothing
    pub psychoacoustic: bool,
    /// Enable asymmetric loss weighting
    pub asymmetric_loss: bool,
}

impl Default for SpinoramaOptimizerConfig {
    fn default() -> Self {
        Self {
            mode: SpinoramaOptimizationMode::FlatOnPir,
            target_curve: SpinoramaTargetCurve::default(),
            algorithm: RoomEqAlgorithm::DifferentialEvolution,
            num_filters: 5,
            sample_rate: 48000,
            fir_taps: 4096,
            fir_phase: "kirkeby".to_string(),
            min_q: 0.5,
            max_q: 6.0,
            min_db: -12.0,
            max_db: 4.0,
            min_freq: 60.0,
            max_freq: 16000.0,
            max_iter: 10000,
            peq_model: "pk".to_string(),
            population: 40,
            de_f: 0.8,
            de_cr: 0.9,
            strategy: "currenttobest1bin".to_string(),
            refine: false,
            local_algo: "cobyla".to_string(),
            smooth: false,
            smooth_n: 6,
            spacing_weight: 1.0,
            min_spacing_oct: 0.08,
            tolerance: 0.00001,
            atolerance: 0.00001,
            psychoacoustic: true,
            asymmetric_loss: true,
        }
    }
}

/// Result of Spinorama EQ optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpinoramaEqResult {
    /// Optimized biquad filters
    pub biquads: Vec<SpinoramaBiquad>,
    /// Pre-optimization score
    pub pre_score: f64,
    /// Post-optimization score
    pub post_score: f64,
    /// Original frequency response (for plotting)
    pub original_response: Option<Vec<(f64, f64)>>,
    /// Corrected frequency response (for plotting)
    pub corrected_response: Option<Vec<(f64, f64)>>,
    /// Target curve (for plotting)
    pub target_response: Option<Vec<(f64, f64)>>,
}

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

/// Biquad filter for Spinorama EQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpinoramaBiquad {
    pub filter_type: String,
    pub freq: f64,
    pub q: f64,
    pub db_gain: f64,
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

    /// Update speaker suggestions based on search query with fuzzy matching
    pub fn update_suggestions(&mut self) {
        if self.speaker_search.is_empty() {
            self.speaker_suggestions = self.available_speakers.clone();
        } else {
            // Score and filter speakers using fuzzy matching
            let mut scored: Vec<(String, f64)> = self
                .available_speakers
                .iter()
                .filter_map(|s| {
                    fuzzy_match_score(&self.speaker_search, s).map(|score| (s.clone(), score))
                })
                .collect();

            // Sort by score descending (best matches first)
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
