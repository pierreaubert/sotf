//! Shared headphone EQ domain types used by both GPUI and TUI apps.

use serde::{Deserialize, Serialize};

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
    /// Step 4: Apply & Export
    Export,
}

impl HeadphoneEqStep {
    /// Get all steps in order
    pub fn all() -> &'static [HeadphoneEqStep] {
        &[
            HeadphoneEqStep::MeasurementTarget,
            HeadphoneEqStep::Optimization,
            HeadphoneEqStep::Listen,
            HeadphoneEqStep::Export,
        ]
    }

    /// Get step index (0-based)
    pub fn index(&self) -> usize {
        match self {
            HeadphoneEqStep::MeasurementTarget => 0,
            HeadphoneEqStep::Optimization => 1,
            HeadphoneEqStep::Listen => 2,
            HeadphoneEqStep::Export => 3,
        }
    }

    /// Get step label
    pub fn label(&self) -> &'static str {
        match self {
            HeadphoneEqStep::MeasurementTarget => "Measurement",
            HeadphoneEqStep::Optimization => "Optimization",
            HeadphoneEqStep::Listen => "Listen",
            HeadphoneEqStep::Export => "Export",
        }
    }

    /// Get next step
    pub fn next(&self) -> Option<HeadphoneEqStep> {
        match self {
            HeadphoneEqStep::MeasurementTarget => Some(HeadphoneEqStep::Optimization),
            HeadphoneEqStep::Optimization => Some(HeadphoneEqStep::Listen),
            HeadphoneEqStep::Listen => Some(HeadphoneEqStep::Export),
            HeadphoneEqStep::Export => None,
        }
    }

    /// Get previous step
    pub fn previous(&self) -> Option<HeadphoneEqStep> {
        match self {
            HeadphoneEqStep::MeasurementTarget => None,
            HeadphoneEqStep::Optimization => Some(HeadphoneEqStep::MeasurementTarget),
            HeadphoneEqStep::Listen => Some(HeadphoneEqStep::Optimization),
            HeadphoneEqStep::Export => Some(HeadphoneEqStep::Listen),
        }
    }
}

fn default_atolerance() -> f64 {
    1e-5
}

/// Headphone EQ optimizer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadphoneEqOptimizerConfig {
    /// Optimization algorithm (uses same enum as room_eq_types)
    pub algorithm: crate::room_eq_types::RoomEqAlgorithm,
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
    /// Adaptive weight for F parameter (DE adaptive strategies only)
    pub adaptive_weight_f: f64,
    /// Adaptive weight for CR parameter (DE adaptive strategies only)
    pub adaptive_weight_cr: f64,
    /// Tolerance for convergence
    pub tolerance: f64,
    /// Absolute tolerance for convergence
    #[serde(default = "default_atolerance")]
    pub atolerance: f64,
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
            algorithm: crate::room_eq_types::RoomEqAlgorithm::DifferentialEvolution,
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
            adaptive_weight_f: 0.8,
            adaptive_weight_cr: 0.7,
            tolerance: 1e-5,
            atolerance: 1e-5,
            refine: false,
            local_algo: "cobyla".to_string(),
            smooth: false,
            smooth_n: 1,
        }
    }
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
