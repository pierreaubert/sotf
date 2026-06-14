use super::default::default_bo_acquisition;
use super::misc::canonical_multi_measurement_strategy;
use super::room_eq_optimizer_config::RoomEqOptimizerConfig;
use crate::ReleaseChannel;
pub use autoeq::roomeq::{
    SimpleCrossoverChoice, SimpleLossChoice, SimplePresetConfig, SimpleProcessingChoice,
};
use serde::{Deserialize, Serialize};

/// Apply the user's Simple Wizard choices to a flat UI optimizer config.
///
/// Fields not controlled by the preset keep their current values so the
/// user doesn't lose any manual tuning done in a previous Full Wizard
/// session.  This is the "mutate in place" path used when the full wizard
/// needs to incorporate simple-mode choices into an existing config.
pub fn apply_simple_preset(preset: &SimplePresetConfig, config: &mut RoomEqOptimizerConfig) {
    // Processing mode
    config.mode = match preset.processing {
        SimpleProcessingChoice::Iir => RoomEqOptimizationMode::Iir,
        SimpleProcessingChoice::MixedPhase => RoomEqOptimizationMode::MixedPhase,
    };

    // Loss function
    config.loss_type = match preset.loss {
        SimpleLossChoice::Flat => "flat".to_string(),
        SimpleLossChoice::Epa => "epa".to_string(),
    };

    // Target response derived from measurement
    config.target_response.enabled = true;
    config.target_response.shape = "from_measurement".to_string();
    config.target_response.slope_db_per_octave = 0.0;

    // Crossover (2.1+ only)
    if !preset.bass_management.is_empty() || matches!(preset.crossover, SimpleCrossoverChoice::Lr48)
    {
        config.schroeder_split.enabled = true;
    }

    // Sane defaults for params not exposed in Simple mode
    config.num_filters = 7;
    config.algorithm = "autoeq:cmaes".to_string();
    config.population = 300;
    config.max_iter = 50_000;
    config.bo_initial_samples = 0;
    config.bo_batch_size = 0;
    config.bo_posterior_std_threshold = 0.0;
    config.bo_acquisition = default_bo_acquisition();
    config.bo_ehvi = false;
    config.min_freq = 20.0;
    config.max_freq = 1600.0;
    config.min_db = -12.0;
    config.max_db = 4.0;
    config.min_q = 0.5;
    config.max_q = 6.0;
    config.peq_model = "pk".to_string();
    config.tolerance = 1e-5;
    config.atolerance = 1e-5;
    config.psychoacoustic = true;
    config.asymmetric_loss = true;
    config.refine = true;
    config.local_algo = "cobyla".to_string();

    // Multi-position strategy
    if !preset.multi_position_strategy.is_empty() {
        config.multi_measurement.enabled = true;
        config.multi_measurement.strategy =
            canonical_multi_measurement_strategy(&preset.multi_position_strategy)
                .unwrap_or("average")
                .to_string();
    }
}

/// Optimization mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoomEqOptimizationMode {
    #[default]
    Iir,
    Fir,
    Mixed,
    MixedPhase,
}

impl RoomEqOptimizationMode {
    pub fn all() -> &'static [RoomEqOptimizationMode] {
        &[
            RoomEqOptimizationMode::Iir,
            RoomEqOptimizationMode::Fir,
            RoomEqOptimizationMode::Mixed,
            RoomEqOptimizationMode::MixedPhase,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RoomEqOptimizationMode::Iir => "IIR (Parametric EQ)",
            RoomEqOptimizationMode::Fir => "FIR (Convolution)",
            RoomEqOptimizationMode::Mixed => "Mixed (IIR + FIR)",
            RoomEqOptimizationMode::MixedPhase => "Mixed-Phase (IIR + short FIR)",
        }
    }

    pub fn to_code(&self) -> &'static str {
        match self {
            RoomEqOptimizationMode::Iir => "iir",
            RoomEqOptimizationMode::Fir => "fir",
            RoomEqOptimizationMode::Mixed => "mixed",
            RoomEqOptimizationMode::MixedPhase => "mixed_phase",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "fir" => RoomEqOptimizationMode::Fir,
            "mixed" => RoomEqOptimizationMode::Mixed,
            "mixed_phase" => RoomEqOptimizationMode::MixedPhase,
            _ => RoomEqOptimizationMode::Iir,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            RoomEqOptimizationMode::Iir => "Uses standard biquad filters. Low latency, efficient.",
            RoomEqOptimizationMode::Fir => {
                "Uses impulse response convolution. Can correct phase, but higher latency."
            }
            RoomEqOptimizationMode::Mixed => {
                "Combines IIR for high frequencies and FIR for low frequencies."
            }
            RoomEqOptimizationMode::MixedPhase => {
                "IIR for minimum-phase + short FIR for excess phase. Low latency (~10ms)."
            }
        }
    }

    pub fn maturity(&self) -> ReleaseChannel {
        match self {
            RoomEqOptimizationMode::Iir => ReleaseChannel::Beta,
            RoomEqOptimizationMode::Fir => ReleaseChannel::Alpha,
            RoomEqOptimizationMode::Mixed => ReleaseChannel::Alpha,
            RoomEqOptimizationMode::MixedPhase => ReleaseChannel::Alpha,
        }
    }

    pub fn available(channel: ReleaseChannel) -> Vec<Self> {
        Self::all()
            .iter()
            .copied()
            .filter(|mode| channel.allows(mode.maturity()))
            .collect()
    }
}
