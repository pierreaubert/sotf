//! Shared optimization parameters used by both AutoEQ CLI and RoomEQ.
//!
//! [`OptimParams`] decouples the optimization infrastructure from the CLI
//! argument struct (`cli::Args`), allowing roomeq to use the same
//! optimization functions without constructing fake `Args` values.

use crate::cli::{Args, PeqModel};
use crate::loss::LossType;
use crate::optim::SmoothnessPenaltyConfig;

/// Optimization-relevant parameters extracted from either `cli::Args`
/// (for the autoeq binary) or `roomeq::OptimizerConfig` (for room EQ).
///
/// The optimization functions (`setup_objective_data`, `setup_bounds`,
/// `initial_guess`, `perform_optimization`, etc.) accept this struct
/// instead of the full CLI `Args`.
#[derive(Debug, Clone)]
pub struct OptimParams {
    // -- Filter topology --
    pub num_filters: usize,
    pub peq_model: PeqModel,
    pub sample_rate: f64,

    // -- Bounds --
    pub min_freq: f64,
    pub max_freq: f64,
    pub min_q: f64,
    pub max_q: f64,
    pub min_db: f64,
    pub max_db: f64,

    // -- Loss / objective --
    pub loss: LossType,
    pub smooth: bool,
    pub smooth_n: usize,
    pub min_spacing_oct: f64,
    pub spacing_weight: f64,
    pub smoothness_penalty: Option<SmoothnessPenaltyConfig>,
    pub audibility_deadband: Option<crate::roomeq::AudibilityDeadbandConfig>,

    // -- Algorithm --
    pub algo: String,
    pub population: usize,
    pub maxeval: usize,
    pub refine: bool,
    pub local_algo: String,
    pub bo_initial_samples: usize,
    pub bo_batch_size: usize,
    pub bo_posterior_std_threshold: f64,
    pub bo_acquisition: String,
    pub bo_ehvi: bool,

    // -- DE-specific --
    pub strategy: String,
    pub tolerance: f64,
    pub atolerance: f64,
    pub recombination: f64,
    pub adaptive_weight_f: f64,
    pub adaptive_weight_cr: f64,

    // -- Execution --
    pub no_parallel: bool,
    pub parallel_threads: usize,
    pub seed: Option<u64>,

    /// Suppress non-essential logging (replaces `args.qa.is_some()`).
    pub quiet: bool,
}

pub fn resolve_smoothness_schroeder_hz(config: &crate::roomeq::OptimizerConfig) -> Option<f64> {
    config
        .schroeder_split
        .as_ref()
        .filter(|split| split.enabled)
        .map(|split| {
            split
                .room_dimensions
                .as_ref()
                .map(crate::roomeq::RoomDimensions::schroeder_frequency)
                .unwrap_or(split.schroeder_freq)
        })
}

pub fn resolve_smoothness_penalty_config(
    config: &crate::roomeq::OptimizerConfig,
) -> Option<SmoothnessPenaltyConfig> {
    let mut smoothness = config
        .smoothness_penalty
        .as_ref()
        .map(SmoothnessPenaltyConfig::from)?;
    if smoothness.schroeder_hz.is_none() {
        smoothness.schroeder_hz = resolve_smoothness_schroeder_hz(config);
    }
    Some(smoothness)
}

impl From<&Args> for OptimParams {
    fn from(args: &Args) -> Self {
        Self {
            num_filters: args.num_filters,
            peq_model: args.effective_peq_model(),
            sample_rate: args.sample_rate,
            min_freq: args.min_freq,
            max_freq: args.max_freq,
            min_q: args.min_q,
            max_q: args.max_q,
            min_db: args.min_db,
            max_db: args.max_db,
            loss: args.loss,
            smooth: args.smooth,
            smooth_n: args.smooth_n,
            min_spacing_oct: args.min_spacing_oct,
            spacing_weight: args.spacing_weight,
            smoothness_penalty: if args.smoothness_weight > 0.0 {
                Some(SmoothnessPenaltyConfig {
                    tv2_weight: args.smoothness_weight,
                    schroeder_hz: args.smoothness_schroeder_hz,
                    modal_weight_scale: args.smoothness_modal_scale,
                    exponent: args.smoothness_exponent,
                })
            } else {
                None
            },
            audibility_deadband: None,
            algo: args.algo.clone(),
            population: args.population,
            maxeval: args.maxeval,
            refine: args.refine,
            local_algo: args.local_algo.clone(),
            bo_initial_samples: args.bo_initial_samples,
            bo_batch_size: args.bo_batch_size,
            bo_posterior_std_threshold: args.bo_posterior_std_threshold,
            bo_acquisition: args.bo_acquisition.clone(),
            bo_ehvi: args.bo_ehvi,
            strategy: args.strategy.clone(),
            tolerance: args.tolerance,
            atolerance: args.atolerance,
            recombination: args.recombination,
            adaptive_weight_f: args.adaptive_weight_f,
            adaptive_weight_cr: args.adaptive_weight_cr,
            no_parallel: args.no_parallel,
            parallel_threads: args.parallel_threads,
            seed: args.seed,
            quiet: args.qa.is_some(),
        }
    }
}

impl From<&crate::roomeq::OptimizerConfig> for OptimParams {
    fn from(config: &crate::roomeq::OptimizerConfig) -> Self {
        // Parse peq_model string to enum, defaulting to Pk
        let peq_model = config.peq_model.parse::<PeqModel>().unwrap_or(PeqModel::Pk);

        // Parse loss_type string to enum, defaulting to SpeakerFlat
        let loss = match config.loss_type.as_str() {
            "flat" => {
                if config.asymmetric_loss {
                    LossType::SpeakerFlatAsymmetric
                } else {
                    LossType::SpeakerFlat
                }
            }
            "score" => LossType::SpeakerScore,
            "epa" => LossType::Epa,
            other => {
                log::warn!(
                    "Unknown loss_type '{}' in OptimizerConfig, defaulting to SpeakerFlat",
                    other
                );
                LossType::SpeakerFlat
            }
        };

        Self {
            num_filters: config.num_filters,
            peq_model,
            sample_rate: 48000.0, // Overridden by callers with actual sample rate
            min_freq: config.min_freq,
            max_freq: config.max_freq,
            min_q: config.min_q,
            max_q: config.max_q,
            min_db: config.min_db,
            max_db: config.max_db,
            loss,
            smooth: true,
            smooth_n: config.smooth_n,
            min_spacing_oct: 0.2,
            spacing_weight: 20.0,
            smoothness_penalty: resolve_smoothness_penalty_config(config),
            audibility_deadband: config.audibility_deadband_config(),
            algo: config.algorithm.clone(),
            population: config.population,
            maxeval: config.max_iter,
            refine: config.refine,
            local_algo: config.local_algo.clone(),
            bo_initial_samples: config.bo_initial_samples.unwrap_or(0),
            bo_batch_size: config.bo_batch_size.unwrap_or(0),
            bo_posterior_std_threshold: config.bo_posterior_std_threshold.unwrap_or(0.0),
            bo_acquisition: config
                .bo_acquisition
                .clone()
                .unwrap_or_else(|| "qei".to_string()),
            bo_ehvi: config.bo_ehvi.unwrap_or(false),
            strategy: config.strategy.clone(),
            tolerance: config.tolerance,
            atolerance: config.atolerance,
            recombination: 0.9,
            adaptive_weight_f: 0.9,
            adaptive_weight_cr: 0.9,
            no_parallel: false,
            parallel_threads: num_cpus::get(),
            seed: config.seed,
            quiet: false,
        }
    }
}

impl std::str::FromStr for PeqModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "pk" => Ok(PeqModel::Pk),
            "hp-pk" => Ok(PeqModel::HpPk),
            "hp-pk-lp" => Ok(PeqModel::HpPkLp),
            "ls-pk" => Ok(PeqModel::LsPk),
            "ls-pk-hs" => Ok(PeqModel::LsPkHs),
            "free-pk-free" => Ok(PeqModel::FreePkFree),
            "free" => Ok(PeqModel::Free),
            _ => Err(format!("Unknown PEQ model: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roomeq::{
        OptimizerConfig, RoomDimensions, SchroederSplitConfig, SmoothnessPenaltyConfigSerde,
    };

    fn smoothness_config(schroeder_hz: Option<f64>) -> SmoothnessPenaltyConfigSerde {
        SmoothnessPenaltyConfigSerde {
            tv2_weight: 0.05,
            schroeder_hz,
            modal_weight_scale: 0.1,
            exponent: 1.0,
        }
    }

    #[test]
    fn roomeq_smoothness_penalty_defaults_schroeder_in_optim_params_from() {
        let config = OptimizerConfig {
            smoothness_penalty: Some(smoothness_config(None)),
            schroeder_split: Some(SchroederSplitConfig {
                enabled: true,
                schroeder_freq: 280.0,
                room_dimensions: Some(RoomDimensions {
                    length: 4.0,
                    width: 3.0,
                    height: 2.5,
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        let params = OptimParams::from(&config);
        let schroeder = params.smoothness_penalty.unwrap().schroeder_hz.unwrap();
        let expected = config
            .schroeder_split
            .as_ref()
            .unwrap()
            .room_dimensions
            .as_ref()
            .unwrap()
            .schroeder_frequency();
        assert!((schroeder - expected).abs() < 1e-9);

        let explicit = OptimizerConfig {
            smoothness_penalty: Some(smoothness_config(Some(123.0))),
            ..config
        };
        let params = OptimParams::from(&explicit);
        assert_eq!(params.smoothness_penalty.unwrap().schroeder_hz, Some(123.0));
    }

    #[test]
    fn roomeq_bo_options_flow_into_optim_params() {
        let config = OptimizerConfig {
            bo_initial_samples: Some(24),
            bo_batch_size: Some(4),
            bo_posterior_std_threshold: Some(0.02),
            bo_acquisition: Some("ei".to_string()),
            bo_ehvi: Some(true),
            ..Default::default()
        };

        let params = OptimParams::from(&config);
        assert_eq!(params.bo_initial_samples, 24);
        assert_eq!(params.bo_batch_size, 4);
        assert_eq!(params.bo_posterior_std_threshold, 0.02);
        assert_eq!(params.bo_acquisition, "ei");
        assert!(params.bo_ehvi);
    }
}
