// ============================================================================
// Headphone EQ Screen Types
// ============================================================================

use serde::{Deserialize, Serialize};

use super::room_eq::{AutoEqField, OptimizationStatus, RoomEqAlgorithm};

/// Headphone EQ workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeadphoneEqStep {
    /// Step 1: Select measurement file and target curve
    #[default]
    MeasurementTarget,
    /// Step 2: EQ design, fine tuning, and generate EQ
    Optimization,
    /// Step 3: Preview and apply EQ to playback
    Listen,
    /// Step 4: Export format selection and save
    Save,
}

impl HeadphoneEqStep {
    /// Get all steps in order
    pub fn all() -> &'static [HeadphoneEqStep] {
        &[
            HeadphoneEqStep::MeasurementTarget,
            HeadphoneEqStep::Optimization,
            HeadphoneEqStep::Listen,
            HeadphoneEqStep::Save,
        ]
    }

    /// Get step index (0-based)
    pub fn index(&self) -> usize {
        match self {
            HeadphoneEqStep::MeasurementTarget => 0,
            HeadphoneEqStep::Optimization => 1,
            HeadphoneEqStep::Listen => 2,
            HeadphoneEqStep::Save => 3,
        }
    }

    /// Get step label
    pub fn label(&self) -> &'static str {
        match self {
            HeadphoneEqStep::MeasurementTarget => "Measurement",
            HeadphoneEqStep::Optimization => "Optimization",
            HeadphoneEqStep::Listen => "Listen",
            HeadphoneEqStep::Save => "Save",
        }
    }

    /// Get next step
    pub fn next(&self) -> Option<HeadphoneEqStep> {
        match self {
            HeadphoneEqStep::MeasurementTarget => Some(HeadphoneEqStep::Optimization),
            HeadphoneEqStep::Optimization => Some(HeadphoneEqStep::Listen),
            HeadphoneEqStep::Listen => Some(HeadphoneEqStep::Save),
            HeadphoneEqStep::Save => None,
        }
    }

    /// Get previous step
    pub fn previous(&self) -> Option<HeadphoneEqStep> {
        match self {
            HeadphoneEqStep::MeasurementTarget => None,
            HeadphoneEqStep::Optimization => Some(HeadphoneEqStep::MeasurementTarget),
            HeadphoneEqStep::Listen => Some(HeadphoneEqStep::Optimization),
            HeadphoneEqStep::Save => Some(HeadphoneEqStep::Listen),
        }
    }
}

/// Headphone EQ optimizer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadphoneEqOptimizerConfig {
    /// Optimization algorithm
    pub algorithm: RoomEqAlgorithm,
    /// Number of PEQ filters
    pub num_filters: usize,
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
    /// Loss function
    pub loss: String,
    /// PEQ filter model (pk, hp-pk, ls-pk-hs, etc.)
    pub peq_model: String,
    /// Population size for DE
    pub population: usize,
    /// DE mutation factor (F)
    pub de_f: f64,
    /// DE crossover rate (CR)
    pub de_cr: f64,
    /// DE strategy
    pub strategy: String,
    /// Tolerance for convergence
    pub tolerance: f64,
    /// Enable local refinement after global optimization
    pub refine: bool,
    /// Local refinement algorithm
    pub local_algo: String,
    /// Enable smoothing of input curve
    pub smooth: bool,
    /// Smoothing window size
    pub smooth_n: usize,
}

impl Default for HeadphoneEqOptimizerConfig {
    fn default() -> Self {
        Self {
            algorithm: RoomEqAlgorithm::DifferentialEvolution,
            num_filters: 10,
            min_q: 0.5,
            max_q: 10.0,
            min_db: -12.0,
            max_db: 12.0,
            min_freq: 20.0,
            max_freq: 20000.0,
            max_iter: 10000,
            loss: "headphone-score".to_string(),
            peq_model: "pk".to_string(),
            population: 80,
            de_f: 0.8,
            de_cr: 0.9,
            strategy: "currenttobest1bin".to_string(),
            tolerance: 1e-3,
            refine: false,
            local_algo: "cobyla".to_string(),
            smooth: false,
            smooth_n: 1,
        }
    }
}

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
    /// Path to headphone measurement file (CSV)
    pub measurement_path: Option<String>,

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

/// Result of headphone EQ optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadphoneEqResult {
    /// Optimized biquad filters
    pub biquads: Vec<HeadphoneEqBiquad>,
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
    /// Filter response (sum of all filters)
    pub filter_response: Option<Vec<(f64, f64)>>,
    /// Deviation from target (target - original)
    pub deviation_response: Option<Vec<(f64, f64)>>,
    /// Residual error (deviation - filter)
    pub error_response: Option<Vec<(f64, f64)>>,
    /// Individual filter responses (for detailed plotting)
    pub individual_responses: Option<Vec<Vec<(f64, f64)>>>,
}

/// Biquad filter for headphone EQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadphoneEqBiquad {
    pub filter_type: String,
    pub freq: f64,
    pub q: f64,
    pub db_gain: f64,
}

impl Default for HeadphoneEqState {
    fn default() -> Self {
        Self {
            step: HeadphoneEqStep::MeasurementTarget,
            measurement_path: None,
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
    /// Check if we can proceed from the current step
    pub fn can_advance(&self) -> bool {
        match self.step {
            HeadphoneEqStep::MeasurementTarget => self.measurement_path.is_some(),
            HeadphoneEqStep::Optimization => {
                self.optimization_status == OptimizationStatus::Completed
            }
            HeadphoneEqStep::Listen => self.result.is_some(),
            HeadphoneEqStep::Save => true,
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
}
