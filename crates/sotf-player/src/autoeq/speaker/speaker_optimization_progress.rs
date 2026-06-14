use super::types::OptimizationStage;
pub use autoeq::ProgressUpdate;

/// Data passed to the optimization callback at each interval
/// This wraps autoeq::ProgressUpdate with additional stage information
#[derive(Debug, Clone)]
pub struct SpeakerOptimizationProgress {
    /// Current iteration number
    pub iteration: usize,
    /// Current loss/objective value
    pub loss: f64,
    /// Optional score value (higher is better, e.g., Harman score)
    pub score: Option<f64>,
    /// Convergence metric (population standard deviation)
    pub convergence: f64,
    /// Current best parameters (raw optimizer params)
    pub current_params: Vec<f64>,
    /// Current best biquad filters (decoded from params)
    pub current_biquads: Vec<math_audio_iir_fir::Biquad>,
    /// Current filter response curve (dB)
    pub current_filter_response: Vec<f64>,
    /// Stage of optimization
    pub stage: OptimizationStage,
    /// Total iterations expected (maxeval)
    pub max_iterations: usize,
}

impl From<&ProgressUpdate> for SpeakerOptimizationProgress {
    fn from(update: &ProgressUpdate) -> Self {
        Self {
            iteration: update.iteration,
            loss: update.loss,
            score: update.score,
            convergence: update.convergence,
            current_params: update.params.clone(),
            current_biquads: update.biquads.clone(),
            current_filter_response: update.filter_response.clone(),
            stage: OptimizationStage::Eq,
            max_iterations: update.max_iterations,
        }
    }
}
