use super::super::speaker::CallbackConfig;
use super::speaker_measurement_data::SpeakerMeasurementData;

/// Configuration for multi-speaker optimization (legacy)
#[derive(Debug, Clone)]
pub struct MultiSpeakerOptimizationConfig {
    /// Speakers to optimize together
    pub speakers: Vec<SpeakerMeasurementData>,
    /// Optimization arguments (shared across all speakers)
    pub args: autoeq_optim_types::OptimizationConfig,
    /// Callback configuration
    pub callback_config: Option<CallbackConfig>,
}

impl Default for MultiSpeakerOptimizationConfig {
    fn default() -> Self {
        Self {
            speakers: Vec::new(),
            args: autoeq_optim_types::OptimizationConfig::speaker_defaults(),
            callback_config: Some(CallbackConfig::default()),
        }
    }
}
