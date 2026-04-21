//! Shared optimization parameters used by both AutoEQ CLI and RoomEQ.
//!
//! [`OptimParams`] decouples the optimization infrastructure from the CLI
//! argument struct (`cli::Args`), allowing roomeq to use the same
//! optimization functions without constructing fake `Args` values.

use crate::cli::{Args, PeqModel};
use crate::loss::LossType;

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

    // -- Algorithm --
    pub algo: String,
    pub population: usize,
    pub maxeval: usize,
    pub refine: bool,
    pub local_algo: String,

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
            algo: args.algo.clone(),
            population: args.population,
            maxeval: args.maxeval,
            refine: args.refine,
            local_algo: args.local_algo.clone(),
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
        let peq_model = config
            .peq_model
            .parse::<PeqModel>()
            .unwrap_or(PeqModel::Pk);

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
                log::warn!("Unknown loss_type '{}' in OptimizerConfig, defaulting to SpeakerFlat", other);
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
            algo: config.algorithm.clone(),
            population: config.population,
            maxeval: config.max_iter,
            refine: config.refine,
            local_algo: config.local_algo.clone(),
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
