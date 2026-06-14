use super::super::types::SpeakerConfigType;
use super::callback_config::CallbackConfig;
use super::types::MeasurementInput;
pub use autoeq::CrossoverType;

/// Configuration for speaker optimization
#[derive(Debug, Clone)]
pub struct SpeakerOptimizationConfig {
    /// Speaker configuration type
    pub config_type: SpeakerConfigType,
    /// Main measurement (for single-driver)
    pub main_measurement: Option<MeasurementInput>,
    /// Driver measurements (for multi-driver, ordered low to high frequency)
    pub driver_measurements: Vec<MeasurementInput>,
    /// Crossover type (for multi-driver)
    pub crossover_type: Option<CrossoverType>,
    /// Initial crossover frequency hints (optional)
    pub crossover_freq_hints: Vec<f64>,
    /// Optimization arguments (use Args::speaker_defaults() as base)
    pub args: autoeq::Args,
    /// Callback configuration
    pub callback_config: Option<CallbackConfig>,
    /// Target curve (optional - defaults to flat or curve-name-specific)
    pub target: Option<MeasurementInput>,
}

impl Default for SpeakerOptimizationConfig {
    fn default() -> Self {
        Self {
            config_type: SpeakerConfigType::Single,
            main_measurement: None,
            driver_measurements: Vec::new(),
            crossover_type: None,
            crossover_freq_hints: Vec::new(),
            args: autoeq::Args::speaker_defaults(),
            callback_config: Some(CallbackConfig::default()),
            target: None,
        }
    }
}
