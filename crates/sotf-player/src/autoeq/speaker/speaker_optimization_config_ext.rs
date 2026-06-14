use super::callback_config::CallbackConfig;
use super::types::MeasurementInput;
use super::types::SpeakerConfigTypeExt;
pub use autoeq::CrossoverType;

/// Extended configuration for speaker optimization including multi-sub and DBA
#[derive(Debug, Clone)]
pub struct SpeakerOptimizationConfigExt {
    pub config_type: SpeakerConfigTypeExt,
    pub main_measurement: Option<MeasurementInput>,
    pub driver_measurements: Vec<MeasurementInput>,
    pub front_measurements: Vec<MeasurementInput>,
    pub rear_measurements: Vec<MeasurementInput>,
    pub crossover_type: Option<CrossoverType>,
    pub crossover_freq_hints: Vec<f64>,
    pub args: autoeq::Args,
    pub callback_config: Option<CallbackConfig>,
    pub target: Option<MeasurementInput>,
}

impl Default for SpeakerOptimizationConfigExt {
    fn default() -> Self {
        Self {
            config_type: SpeakerConfigTypeExt::Single,
            main_measurement: None,
            driver_measurements: Vec::new(),
            front_measurements: Vec::new(),
            rear_measurements: Vec::new(),
            crossover_type: None,
            crossover_freq_hints: Vec::new(),
            args: autoeq::Args::speaker_defaults(),
            callback_config: Some(CallbackConfig::default()),
            target: None,
        }
    }
}
