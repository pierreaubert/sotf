use super::super::speaker::OptimizationStage;
pub use autoeq::roomeq::PipelineEvent;

/// Result for a single speaker in multi-speaker optimization
#[derive(Clone, Debug)]
pub struct SingleSpeakerResult {
    /// Speaker name
    pub name: String,
    /// Optimized biquad filters for this speaker
    pub biquads: Vec<math_audio_iir_fir::Biquad>,
    /// Initial loss for this speaker
    pub initial_loss: f64,
    /// Final loss for this speaker
    pub final_loss: f64,
    /// Visualization curves
    pub frequencies: Vec<f64>,
    pub input_curve: Vec<f64>,
    pub target_curve: Vec<f64>,
    pub deviation_curve: Vec<f64>,
    pub filter_response: Vec<f64>,
    pub error_curve: Vec<f64>,
    pub corrected_curve: Vec<f64>,
    pub individual_filter_responses: Vec<Vec<f64>>,
}

/// Result of multi-speaker optimization
#[derive(Clone, Debug)]
pub struct MultiSpeakerOptimizationResult {
    /// Per-speaker optimization results
    pub speaker_results: Vec<SingleSpeakerResult>,
    /// Combined initial loss (weighted average)
    pub combined_initial_loss: f64,
    /// Combined final loss (weighted average)
    pub combined_final_loss: f64,
    /// Optimization history: (iteration, combined_loss)
    pub optimization_history: Vec<(usize, f64)>,
}

/// Progress update for multi-speaker optimization (legacy)
#[derive(Debug, Clone)]
pub struct MultiSpeakerProgress {
    /// Current iteration number (across all speakers)
    pub iteration: usize,
    /// Current combined loss value
    pub combined_loss: f64,
    /// Total iterations expected (maxeval * num_speakers)
    pub max_iterations: usize,
    /// Stage of optimization
    pub stage: OptimizationStage,
    /// Convergence metric
    pub convergence: f64,
}

/// Callback function type for multi-speaker optimization (legacy)
pub type MultiSpeakerOptimizationCallback =
    Box<dyn FnMut(&MultiSpeakerProgress) -> autoeq::de::CallbackAction + Send>;

pub(super) fn multi_speaker_progress_from_pipeline_event(
    event: &PipelineEvent,
) -> MultiSpeakerProgress {
    MultiSpeakerProgress {
        iteration: event.iteration.unwrap_or(0),
        combined_loss: event.loss.unwrap_or(0.0),
        max_iterations: event.max_iterations.unwrap_or(0),
        stage: OptimizationStage::Eq,
        convergence: 0.0,
    }
}
