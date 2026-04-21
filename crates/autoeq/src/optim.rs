//! AutoEQ - A library for audio equalization and filter optimization
//!
//! Copyright (C) 2025-2026 Pierre Aubert pierre(at)spinorama(dot)org
//!
//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//! GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program.  If not, see <https://www.gnu.org/licenses/>.

use super::cli::PeqModel;
use super::constraints::{viol_ceiling_from_spl, viol_min_gain_from_xs, viol_spacing_from_xs};
use super::loss::{
    DriversLossData, HeadphoneLossData, LossType, SpeakerLossData, drivers_flat_loss, flat_loss,
    flat_loss_asymmetric, headphone_loss, speaker_score_loss,
};
use self::de::{optimize_filters_autoeq, optimize_filters_autoeq_with_callback};
use self::mh::{optimize_filters_mh, optimize_filters_mh_with_callback};
#[cfg(feature = "nlopt")]
use self::nlopt::optimize_filters_nlopt;
use super::x2peq::x2spl;
use crate::Curve;
use ndarray::Array1;
#[cfg(feature = "nlopt")]
use ::nlopt::Algorithm;

/// Shared callback utilities for optimization
pub mod callback;
/// AutoEQ DE-specific optimization code
pub mod de;
/// Metaheuristics-specific optimization code
pub mod mh;
/// NLOPT-specific optimization code
#[cfg(feature = "nlopt")]
pub mod nlopt;
/// Shared optimization parameters (decoupled from CLI args)
pub mod params;
/// Pareto front analysis
pub mod pareto;
/// Shared optimization setup (bounds, initial guess, objective data)
pub mod setup;

/// Algorithm metadata structure
#[derive(Debug, Clone)]
pub struct AlgorithmInfo {
    /// Algorithm name with library prefix (e.g., "nlopt:isres", "mh:de", "autoeq:de")
    pub name: &'static str,
    /// Library providing this algorithm (e.g., "NLOPT", "Metaheuristics", "AutoEQ")
    pub library: &'static str,
    /// Classification as global or local optimizer
    pub algorithm_type: AlgorithmType,
    /// Whether the algorithm supports linear constraint handling
    pub supports_linear_constraints: bool,
    /// Whether the algorithm supports nonlinear constraint handling
    pub supports_nonlinear_constraints: bool,
}

/// Algorithm classification
#[derive(Debug, Clone, PartialEq)]
pub enum AlgorithmType {
    /// Global optimization algorithm - explores entire solution space, good for finding global optimum
    Global,
    /// Local optimization algorithm - refines solution from starting point, fast but may get trapped in local optimum
    Local,
}

/// Get all available algorithms with their metadata
pub fn get_all_algorithms() -> Vec<AlgorithmInfo> {
    let algorithms = vec![
        #[cfg(feature = "nlopt")]
        // NLOPT algorithms - Global with nonlinear constraint support
        AlgorithmInfo {
            name: "nlopt:isres",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: true,
            supports_nonlinear_constraints: true,
        },
        #[cfg(feature = "nlopt")]
        AlgorithmInfo {
            name: "nlopt:ags",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: true,
        },
        #[cfg(feature = "nlopt")]
        AlgorithmInfo {
            name: "nlopt:origdirect",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: true,
        },
        #[cfg(feature = "nlopt")]
        // NLOPT algorithms - Global without nonlinear constraint support
        AlgorithmInfo {
            name: "nlopt:crs2lm",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        #[cfg(feature = "nlopt")]
        AlgorithmInfo {
            name: "nlopt:direct",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        #[cfg(feature = "nlopt")]
        AlgorithmInfo {
            name: "nlopt:directl",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        #[cfg(feature = "nlopt")]
        AlgorithmInfo {
            name: "nlopt:gmlsl",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        #[cfg(feature = "nlopt")]
        AlgorithmInfo {
            name: "nlopt:gmlsllds",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        #[cfg(feature = "nlopt")]
        AlgorithmInfo {
            name: "nlopt:sbplx",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Local,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        #[cfg(feature = "nlopt")]
        AlgorithmInfo {
            name: "nlopt:slsqp",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Local,
            supports_linear_constraints: true,
            supports_nonlinear_constraints: true,
        },
        #[cfg(feature = "nlopt")]
        AlgorithmInfo {
            name: "nlopt:stogo",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        #[cfg(feature = "nlopt")]
        AlgorithmInfo {
            name: "nlopt:stogorand",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        #[cfg(feature = "nlopt")]
        // NLOPT algorithms - Local
        AlgorithmInfo {
            name: "nlopt:bobyqa",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Local,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        #[cfg(feature = "nlopt")]
        AlgorithmInfo {
            name: "nlopt:cobyla",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Local,
            supports_linear_constraints: true,
            supports_nonlinear_constraints: true,
        },
        #[cfg(feature = "nlopt")]
        AlgorithmInfo {
            name: "nlopt:neldermead",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Local,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        // Metaheuristics algorithms (all global, no constraint support)
        AlgorithmInfo {
            name: "mh:de",
            library: "Metaheuristics",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "mh:pso",
            library: "Metaheuristics",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "mh:rga",
            library: "Metaheuristics",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "mh:tlbo",
            library: "Metaheuristics",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "mh:firefly",
            library: "Metaheuristics",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "autoeq:de",
            library: "AutoEQ",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: true,
            supports_nonlinear_constraints: true,
        },
    ];
    algorithms
}

/// Find algorithm info by name (supports both prefixed and unprefixed names for backward compatibility)
pub fn find_algorithm_info(name: &str) -> Option<AlgorithmInfo> {
    let algorithms = get_all_algorithms();

    // First try exact match
    if let Some(algo) = algorithms
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(name))
    {
        return Some(algo.clone());
    }

    // Then try without prefix for backward compatibility
    let name_lower = name.to_lowercase();
    for algo in &algorithms {
        if let Some(suffix) = algo.name.split(':').nth(1)
            && suffix.eq_ignore_ascii_case(&name_lower)
        {
            return Some(algo.clone());
        }
    }

    None
}

/// Data structure for holding objective function parameters
///
/// This struct contains all the data needed to compute the objective function
/// for filter optimization.
#[derive(Debug, Clone)]
pub struct ObjectiveData {
    /// Frequency points for evaluation
    pub freqs: Array1<f64>,
    /// Target spl
    pub target: Array1<f64>,
    /// Target error values
    pub deviation: Array1<f64>,
    /// Sample rate in Hz
    pub srate: f64,
    #[allow(dead_code)]
    /// Minimum spacing between filters in octaves
    pub min_spacing_oct: f64,
    /// Weight for spacing penalty term
    pub spacing_weight: f64,
    /// Maximum allowed dB level
    pub max_db: f64,
    /// Minimum absolute gain for filters
    pub min_db: f64,
    /// Minimum frequency in Hz for loss function evaluation
    pub min_freq: f64,
    /// Maximum frequency in Hz for loss function evaluation
    pub max_freq: f64,
    /// PEQ model that defines the filter structure
    pub peq_model: PeqModel,
    /// Type of loss function to use
    pub loss_type: LossType,
    /// Optional score data for SpeakerScore loss type
    pub speaker_score_data: Option<SpeakerLossData>,
    /// Optional score data for HeadphoneScore loss type
    pub headphone_score_data: Option<HeadphoneLossData>,
    /// Input curve for headphone loss (optional)
    pub input_curve: Option<Curve>,
    /// Optional data for multi-driver crossover optimization
    pub drivers_data: Option<DriversLossData>,
    /// Fixed crossover frequencies (when not optimizing frequencies)
    pub fixed_crossover_freqs: Option<Vec<f64>>,
    /// Penalty weights used when the optimizer does not support nonlinear constraints
    /// If zero, penalties are disabled and true constraints (if any) are used.
    /// Penalty for ceiling constraint
    pub penalty_w_ceiling: f64,
    /// Penalty for spacing constraint
    pub penalty_w_spacing: f64,
    /// Penalty for min gain constraint
    pub penalty_w_mingain: f64,
    /// Integrality constraints - true for integer parameters, false for continuous
    pub integrality: Option<Vec<bool>>,
    /// Multi-objective data for multi-measurement optimization.
    /// When `Some`, `compute_base_fitness` delegates to `compute_multi_objective_fitness`.
    pub multi_objective: Option<MultiObjectiveData>,
    /// Whether to smooth the error before computing the loss
    pub smooth: bool,
    /// Smoothing resolution as 1/N octave
    pub smooth_n: usize,
    /// Frequency-dependent maximum boost envelope for per-filter gain clamping.
    /// Each entry is (frequency_hz, max_boost_db). Interpolated in log-frequency.
    /// When Some, positive filter gains are clamped before loss evaluation.
    pub max_boost_envelope: Option<Vec<(f64, f64)>>,
    /// CDT-aware minimum cut envelope: limits how deep the optimizer can cut
    /// at frequencies where the ear generates Cubic Distortion Tones.
    /// Each entry is (frequency_hz, max_cut_db) where max_cut_db is negative.
    /// When Some, negative filter gains are clamped before loss evaluation.
    pub min_cut_envelope: Option<Vec<(f64, f64)>>,
    /// EPA psychoacoustic loss configuration.
    ///
    /// Used only when `loss_type == LossType::Epa`. When `None`, the
    /// optimizer falls back to `EpaConfig::default()`.
    pub epa_config: Option<crate::loss::epa::score::EpaConfig>,
    /// Pre-detected frequency problems (usually SSIR / decomposed-correction
    /// room modes) to seed the DE optimizer's smart initial-guess
    /// generator with.
    ///
    /// Each entry is `(frequency_hz, q, suggested_gain_db)` — the sign of
    /// the gain says whether the seed should cut (negative) or boost
    /// (positive) the problem. When this list is non-empty,
    /// `initial_guess::create_smart_initial_guesses` will use it instead
    /// of running its own naive `find_peaks` over the smoothed deviation.
    /// Order should be "most important first" (i.e. sorted by
    /// `|gain_db|` descending) so that if there are fewer filters than
    /// problems the most prominent ones are the ones kept.
    ///
    /// Defaults to empty, which preserves the old behaviour
    /// (auto-detected peaks/dips).
    pub detected_problems: Vec<(f64, f64, f64)>,
    /// Per-frequency dip-suppression mask for
    /// [`LossType::SpeakerFlatAsymmetric`].
    ///
    /// `Some(mask)` scales the dip branch of the asymmetric loss toward
    /// zero inside detected narrow nulls (see
    /// [`crate::roomeq::impulse_analysis::build_null_suppression_mask`]).
    /// `None` disables suppression — dips are weighted with the
    /// full `bass_dip_weight` / `dip_weight` of the asymmetric config.
    /// Must have the same length as `freqs` when provided.
    pub null_suppression: Option<Array1<f64>>,
}

/// Data for multi-objective optimization across multiple measurements
#[derive(Debug, Clone)]
pub struct MultiObjectiveData {
    /// One ObjectiveData per measurement curve
    pub objectives: Vec<ObjectiveData>,
    /// Strategy for combining per-measurement losses
    pub strategy: crate::roomeq::MultiMeasurementStrategy,
    /// Normalized weights (len == objectives.len()), used by WeightedSum
    pub weights: Vec<f64>,
    /// Lambda for VariancePenalized strategy
    pub variance_lambda: f64,
}

/// Penalty configuration mode for optimizers.
///
/// Different optimizers require different penalty weights depending on whether
/// they support native constraints or need penalty-based enforcement.
///
/// # Penalty Scale Rationale
///
/// - **Disabled**: Optimizers with native constraint support (DE) - penalties are handled by the optimizer
/// - **Standard**: Traditional optimizers (NLOPT algorithms like COBYLA, Nelder-Mead) - use 1e4 scale
/// - **Pso**: Particle Swarm Optimization - uses 5e2 scale because PSO needs more exploration space
///
/// The penalty weight determines how strongly constraint violations are penalized.
/// Higher weights push the optimizer away from constraint violations but can also
/// restrict exploration of the solution space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PenaltyMode {
    /// Disable all penalties (use native constraints)
    Disabled,
    /// Standard penalty weights for most optimizers
    Standard,
    /// Moderate penalties for PSO (needs more exploration)
    Pso,
}

impl PenaltyMode {
    /// Ceiling penalty weight - penalizes exceeding the target response ceiling
    pub const fn ceiling_weight(&self) -> f64 {
        match self {
            PenaltyMode::Disabled => 0.0,
            PenaltyMode::Standard => 1e4,
            PenaltyMode::Pso => 5e2,
        }
    }

    /// Minimum gain penalty weight - penalizes going below minimum gain
    pub const fn mingain_weight(&self) -> f64 {
        match self {
            PenaltyMode::Disabled => 0.0,
            PenaltyMode::Standard => 1e3,
            PenaltyMode::Pso => 50.0,
        }
    }
}

impl ObjectiveData {
    /// Configure penalty weights based on the optimizer's requirements.
    ///
    /// Call this before optimization to set appropriate penalty weights.
    /// Use `PenaltyMode::Disabled` when the optimizer supports native constraints.
    pub fn configure_penalties(&mut self, mode: PenaltyMode) {
        self.penalty_w_ceiling = mode.ceiling_weight();
        self.penalty_w_mingain = mode.mingain_weight();
        // Spacing weight is computed from spacing_weight config, scaled by mode
        let spacing_scale = match mode {
            PenaltyMode::Disabled => 0.0,
            PenaltyMode::Standard => 1e3,
            PenaltyMode::Pso => 5e2,
        };
        self.penalty_w_spacing = self.spacing_weight.max(0.0) * spacing_scale;
    }
}

/// Determine algorithm type and return normalized name
#[derive(Debug, Clone)]
pub enum AlgorithmCategory {
    /// NLOPT library algorithm with specific algorithm type
    #[cfg(feature = "nlopt")]
    Nlopt(Algorithm),
    /// Metaheuristics library algorithm with algorithm name
    Metaheuristics(String),
    /// AutoEQ custom algorithm with algorithm name
    AutoEQ(String),
}

/// Parse algorithm name and return category with normalized name
pub fn parse_algorithm_name(name: &str) -> Option<AlgorithmCategory> {
    if let Some(algo_info) = find_algorithm_info(name) {
        let normalized_name = algo_info.name;

        #[cfg(feature = "nlopt")]
        if normalized_name.starts_with("nlopt:") {
            let nlopt_name = normalized_name.strip_prefix("nlopt:").unwrap();
            let nlopt_algo = match nlopt_name {
                "bobyqa" => Algorithm::Bobyqa,
                "cobyla" => Algorithm::Cobyla,
                "neldermead" => Algorithm::Neldermead,
                "isres" => Algorithm::Isres,
                "ags" => Algorithm::Ags,
                "origdirect" => Algorithm::OrigDirect,
                "crs2lm" => Algorithm::Crs2Lm,
                "direct" => Algorithm::Direct,
                "directl" => Algorithm::DirectL,
                "gmlsl" => Algorithm::GMlsl,
                "gmlsllds" => Algorithm::GMlslLds,
                "sbplx" => Algorithm::Sbplx,
                "slsqp" => Algorithm::Slsqp,
                "stogo" => Algorithm::StoGo,
                "stogorand" => Algorithm::StoGoRand,
                _ => Algorithm::Isres, // fallback
            };
            return Some(AlgorithmCategory::Nlopt(nlopt_algo));
        }
        if normalized_name.starts_with("mh:") {
            let mh_name = normalized_name.strip_prefix("mh:").unwrap();
            return Some(AlgorithmCategory::Metaheuristics(mh_name.to_string()));
        } else if normalized_name.starts_with("autoeq:") {
            let autoeq_name = normalized_name.strip_prefix("autoeq:").unwrap();
            return Some(AlgorithmCategory::AutoEQ(autoeq_name.to_string()));
        }
    }

    None
}

/// Compute multi-objective fitness across multiple measurement curves.
///
/// Each objective shares the same filter parameters `x` but evaluates against
/// a different measurement curve. The per-curve losses are combined according
/// to the configured strategy.
fn compute_multi_objective_fitness(x: &[f64], mo: &MultiObjectiveData) -> f64 {
    use crate::roomeq::MultiMeasurementStrategy;

    let losses: Vec<f64> = mo
        .objectives
        .iter()
        .map(|obj| compute_base_fitness_single(x, obj))
        .collect();

    match mo.strategy {
        MultiMeasurementStrategy::Average => {
            // Should not reach here (average mode uses pre-averaged curves),
            // but handle gracefully: simple mean of losses
            let sum: f64 = losses.iter().sum();
            sum / losses.len() as f64
        }
        MultiMeasurementStrategy::WeightedSum => {
            losses.iter().zip(&mo.weights).map(|(l, w)| l * w).sum()
        }
        MultiMeasurementStrategy::Minimax => {
            losses.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
        }
        MultiMeasurementStrategy::VariancePenalized => {
            let n = losses.len() as f64;
            let mean = losses.iter().sum::<f64>() / n;
            let variance = losses.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / n;
            mean + mo.variance_lambda * variance
        }
        MultiMeasurementStrategy::SpatialRobustness => {
            // SpatialRobustness uses single-curve optimization on the RMS-averaged curve
            // and should never reach the multi-objective loss computation.
            unreachable!("SpatialRobustness strategy should not use multi-objective loss path")
        }
    }
}

/// Clamp positive filter gains in the parameter vector using a frequency-dependent envelope.
///
/// For each filter, if its gain is positive (boost), clamp it to the envelope's
/// max boost at that filter's center frequency. Returns a new owned vector.
pub fn clamp_gains_to_envelope(
    x: &[f64],
    envelope: &[(f64, f64)],
    peq_model: PeqModel,
) -> Vec<f64> {
    use crate::param_utils;
    let mut clamped = x.to_vec();
    let num_filters = param_utils::num_filters(x, peq_model);
    for i in 0..num_filters {
        let params = param_utils::get_filter_params(x, i, peq_model);
        let freq_hz = 10f64.powf(params.freq);
        if params.gain > 0.0 {
            let max_boost = interpolate_boost_envelope(envelope, freq_hz);
            if params.gain > max_boost {
                let ppf = param_utils::params_per_filter(peq_model);
                // gain is the last parameter in each filter's group
                let gain_idx = i * ppf + (ppf - 1);
                clamped[gain_idx] = max_boost;
            }
        }
    }
    clamped
}

/// Interpolate a frequency-dependent envelope in log-frequency space.
fn interpolate_boost_envelope(envelope: &[(f64, f64)], freq_hz: f64) -> f64 {
    if envelope.is_empty() {
        return f64::INFINITY;
    }
    if freq_hz <= envelope[0].0 {
        return envelope[0].1;
    }
    let last = envelope.len() - 1;
    if freq_hz >= envelope[last].0 {
        return envelope[last].1;
    }
    for i in 0..last {
        let (f0, db0) = envelope[i];
        let (f1, db1) = envelope[i + 1];
        if freq_hz >= f0 && freq_hz <= f1 {
            let t = (freq_hz.ln() - f0.ln()) / (f1.ln() - f0.ln());
            return db0 + t * (db1 - db0);
        }
    }
    envelope[last].1
}

/// Clamp negative filter gains (cuts) to a frequency-dependent minimum.
///
/// Mirrors `clamp_gains_to_envelope` but for cuts: if a filter's gain is negative
/// and exceeds the envelope's limit (more negative), it is clamped.
/// Used for CDT protection — prevents over-cutting at frequencies where
/// the ear generates distortion tones.
pub fn clamp_cuts_to_envelope(x: &[f64], envelope: &[(f64, f64)], peq_model: PeqModel) -> Vec<f64> {
    use crate::param_utils;
    let mut clamped = x.to_vec();
    let num_filters = param_utils::num_filters(x, peq_model);
    for i in 0..num_filters {
        let params = param_utils::get_filter_params(x, i, peq_model);
        let freq_hz = 10f64.powf(params.freq);
        if params.gain < 0.0 {
            let max_cut = interpolate_boost_envelope(envelope, freq_hz); // returns negative dB
            if params.gain < max_cut {
                let ppf = param_utils::params_per_filter(peq_model);
                let gain_idx = i * ppf + (ppf - 1);
                clamped[gain_idx] = max_cut;
            }
        }
    }
    clamped
}

/// Compute the base fitness for a single ObjectiveData (no multi-objective delegation).
/// This is the inner implementation that does not check `multi_objective`.
fn compute_base_fitness_single(x: &[f64], data: &ObjectiveData) -> f64 {
    // Clamp gains to envelopes before evaluation (boost limits + CDT cut limits).
    let clamped_boost;
    let clamped_cut;
    let x = {
        let skip = matches!(
            data.loss_type,
            LossType::DriversFlat | LossType::MultiSubFlat
        );
        let x = if !skip && let Some(ref env) = data.max_boost_envelope {
            clamped_boost = clamp_gains_to_envelope(x, env, data.peq_model);
            &clamped_boost
        } else {
            x
        };
        if !skip && let Some(ref env) = data.min_cut_envelope {
            clamped_cut = clamp_cuts_to_envelope(x, env, data.peq_model);
            &clamped_cut
        } else {
            x
        }
    };

    match data.loss_type {
        LossType::DriversFlat => {
            if let Some(ref drivers_data) = data.drivers_data {
                let n_drivers = drivers_data.drivers.len();
                let gains = &x[0..n_drivers];
                let delays = &x[n_drivers..2 * n_drivers];
                let xover_freqs: Vec<f64> = if let Some(ref fixed) = data.fixed_crossover_freqs {
                    fixed.clone()
                } else {
                    let xover_freqs_log10 = &x[2 * n_drivers..];
                    xover_freqs_log10
                        .iter()
                        .map(|f| 10.0_f64.powf(*f))
                        .collect()
                };
                drivers_flat_loss(
                    drivers_data,
                    gains,
                    &xover_freqs,
                    Some(delays),
                    data.srate,
                    data.min_freq,
                    data.max_freq,
                )
            } else {
                log::error!("drivers-flat loss requested but driver data is missing");
                f64::INFINITY
            }
        }
        LossType::MultiSubFlat => {
            if let Some(ref drivers_data) = data.drivers_data {
                let n_drivers = drivers_data.drivers.len();
                let gains = &x[0..n_drivers];
                let delays = &x[n_drivers..2 * n_drivers];
                crate::loss::multisub_flat_loss(
                    drivers_data,
                    gains,
                    delays,
                    data.srate,
                    data.min_freq,
                    data.max_freq,
                )
            } else {
                log::error!("multi-sub-flat loss requested but driver data is missing");
                f64::INFINITY
            }
        }
        LossType::HeadphoneFlat | LossType::SpeakerFlat => {
            let peq_spl = x2spl(&data.freqs, x, data.srate, data.peq_model);
            let error = &peq_spl - &data.deviation;
            if data.smooth {
                let curve = Curve {
                    freq: data.freqs.clone(),
                    spl: error,
                    phase: None,
                    ..Default::default()
                };
                let smoothed = crate::read::smooth_one_over_n_octave(&curve, data.smooth_n);
                flat_loss(&data.freqs, &smoothed.spl, data.min_freq, data.max_freq)
            } else {
                flat_loss(&data.freqs, &error, data.min_freq, data.max_freq)
            }
        }
        LossType::SpeakerFlatAsymmetric => {
            let peq_spl = x2spl(&data.freqs, x, data.srate, data.peq_model);
            let error = &peq_spl - &data.deviation;
            let null_mask = data.null_suppression.as_ref();
            if data.smooth {
                let curve = Curve {
                    freq: data.freqs.clone(),
                    spl: error,
                    phase: None,
                    ..Default::default()
                };
                let smoothed = crate::read::smooth_one_over_n_octave(&curve, data.smooth_n);
                flat_loss_asymmetric(
                    &data.freqs,
                    &smoothed.spl,
                    data.min_freq,
                    data.max_freq,
                    null_mask,
                )
            } else {
                flat_loss_asymmetric(&data.freqs, &error, data.min_freq, data.max_freq, null_mask)
            }
        }
        LossType::SpeakerScore => {
            let peq_spl = x2spl(&data.freqs, x, data.srate, data.peq_model);
            if let Some(ref sd) = data.speaker_score_data {
                let error = &peq_spl - &data.deviation;
                let s = speaker_score_loss(sd, &data.freqs, &peq_spl);
                let p = flat_loss(&data.freqs, &error, data.min_freq, data.max_freq) / 3.0;
                // SpeakerScore fitness: minimize (100 - score + flatness/3)
                // - 100.0: reference ceiling for Harman speaker score (typical range 0-100)
                // - /3.0: reduces flatness weight to ~25% vs score (empirically tuned)
                100.0 - s + p
            } else {
                log::error!("speaker score loss requested but score data is missing");
                f64::INFINITY
            }
        }
        LossType::HeadphoneScore => {
            let peq_spl = x2spl(&data.freqs, x, data.srate, data.peq_model);
            if let Some(ref _hd) = data.headphone_score_data {
                let error = &data.deviation - &peq_spl;
                let error_curve = Curve {
                    freq: data.freqs.clone(),
                    spl: error.clone(),
                    phase: None,
                    ..Default::default()
                };
                let s = headphone_loss(&error_curve);
                let p = flat_loss(&data.freqs, &error, data.min_freq, data.max_freq);
                1000.0 - s + p * 20.0
            } else {
                log::error!("headphone score loss requested but headphone data is missing");
                f64::INFINITY
            }
        }
        LossType::Epa => {
            let peq_spl = x2spl(&data.freqs, x, data.srate, data.peq_model);
            let error = &peq_spl - &data.deviation;
            let epa_config = data.epa_config.clone().unwrap_or_default();
            // Flatness now honors the EpaConfig blend (ERB-dominant by
            // default) instead of going through the generic `flat_loss`,
            // so the whole EPA objective is user-tunable.
            let flatness = crate::loss::epa::score::epa_flatness(
                &data.freqs,
                &error,
                data.min_freq,
                data.max_freq,
                &epa_config,
            );
            let freqs_vec: Vec<f64> = data.freqs.iter().copied().collect();
            // The corrected SPL = target + deviation (measurement) + peq correction
            let corrected_spl: Vec<f64> = data
                .freqs
                .iter()
                .enumerate()
                .map(|(i, _)| data.target[i] + data.deviation[i] + peq_spl[i])
                .collect();
            // Use the `_normalized` variant because `corrected_spl` is built
            // from level-relative (target + deviation + PEQ) components —
            // it is not absolute dB SPL. The normalized helper denormalizes
            // against `epa_config.listening_level_phon` so the loudness /
            // loudness-balance penalties are properly calibrated.
            crate::loss::epa::score::epa_loss_normalized(
                &freqs_vec,
                &corrected_spl,
                &epa_config,
                flatness,
            )
        }
    }
}

/// Compute the base fitness value (without penalties) for given parameters
///
/// This is the unified fitness function used by both NLOPT and metaheuristics optimizers.
/// If `multi_objective` is set, delegates to multi-objective fitness computation.
pub fn compute_base_fitness(x: &[f64], data: &ObjectiveData) -> f64 {
    // If multi-objective data is present, delegate to multi-objective fitness
    if let Some(ref mo) = data.multi_objective {
        return compute_multi_objective_fitness(x, mo);
    }

    match data.loss_type {
        LossType::DriversFlat => {
            // Multi-driver crossover optimization
            if let Some(ref drivers_data) = data.drivers_data {
                let n_drivers = drivers_data.drivers.len();
                // Parameter layout depends on whether frequencies are fixed:
                // - Fixed freqs: [gains(N), delays(N)]
                // - Optimizing freqs: [gains(N), delays(N), xovers(N-1)]
                let gains = &x[0..n_drivers];
                let delays = &x[n_drivers..2 * n_drivers];

                // Use fixed frequencies if provided, otherwise extract from parameter vector
                let xover_freqs: Vec<f64> = if let Some(ref fixed) = data.fixed_crossover_freqs {
                    fixed.clone()
                } else {
                    let xover_freqs_log10 = &x[2 * n_drivers..];
                    xover_freqs_log10
                        .iter()
                        .map(|f| 10.0_f64.powf(*f))
                        .collect()
                };

                drivers_flat_loss(
                    drivers_data,
                    gains,
                    &xover_freqs,
                    Some(delays),
                    data.srate,
                    data.min_freq,
                    data.max_freq,
                )
            } else {
                log::error!("drivers-flat loss requested but driver data is missing");
                f64::INFINITY
            }
        }
        LossType::MultiSubFlat => {
            if let Some(ref drivers_data) = data.drivers_data {
                let n_drivers = drivers_data.drivers.len();
                let gains = &x[0..n_drivers];
                let delays = &x[n_drivers..2 * n_drivers];

                crate::loss::multisub_flat_loss(
                    drivers_data,
                    gains,
                    delays,
                    data.srate,
                    data.min_freq,
                    data.max_freq,
                )
            } else {
                log::error!("multi-sub-flat loss requested but driver data is missing");
                f64::INFINITY
            }
        }
        LossType::HeadphoneFlat | LossType::SpeakerFlat => {
            let peq_spl = x2spl(&data.freqs, x, data.srate, data.peq_model);
            let error = &peq_spl - &data.deviation;
            if data.smooth {
                let curve = Curve {
                    freq: data.freqs.clone(),
                    spl: error,
                    phase: None,
                    ..Default::default()
                };
                let smoothed = crate::read::smooth_one_over_n_octave(&curve, data.smooth_n);
                flat_loss(&data.freqs, &smoothed.spl, data.min_freq, data.max_freq)
            } else {
                flat_loss(&data.freqs, &error, data.min_freq, data.max_freq)
            }
        }
        LossType::SpeakerFlatAsymmetric => {
            let peq_spl = x2spl(&data.freqs, x, data.srate, data.peq_model);
            let error = &peq_spl - &data.deviation;
            let null_mask = data.null_suppression.as_ref();
            if data.smooth {
                let curve = Curve {
                    freq: data.freqs.clone(),
                    spl: error,
                    phase: None,
                    ..Default::default()
                };
                let smoothed = crate::read::smooth_one_over_n_octave(&curve, data.smooth_n);
                flat_loss_asymmetric(
                    &data.freqs,
                    &smoothed.spl,
                    data.min_freq,
                    data.max_freq,
                    null_mask,
                )
            } else {
                flat_loss_asymmetric(&data.freqs, &error, data.min_freq, data.max_freq, null_mask)
            }
        }
        LossType::SpeakerScore => {
            let peq_spl = x2spl(&data.freqs, x, data.srate, data.peq_model);
            if let Some(ref sd) = data.speaker_score_data {
                let error = &peq_spl - &data.deviation;
                let s = speaker_score_loss(sd, &data.freqs, &peq_spl);
                let p = flat_loss(&data.freqs, &error, data.min_freq, data.max_freq) / 3.0;
                // SpeakerScore fitness: minimize (100 - score + flatness/3)
                // - 100.0: reference ceiling for Harman speaker score (typical range 0-100)
                // - /3.0: reduces flatness weight to ~25% vs score (empirically tuned)
                100.0 - s + p
            } else {
                log::error!("speaker score loss requested but score data is missing");
                f64::INFINITY
            }
        }
        LossType::HeadphoneScore => {
            let peq_spl = x2spl(&data.freqs, x, data.srate, data.peq_model);
            if let Some(ref _hd) = data.headphone_score_data {
                // Compute remaining deviation: target - (input + peq) = deviation - peq
                // where deviation = target - input
                let error = &data.deviation - &peq_spl;

                // Use headphone_loss on the remaining deviation
                let error_curve = Curve {
                    freq: data.freqs.clone(),
                    spl: error.clone(),
                    phase: None,
                    ..Default::default()
                };
                let s = headphone_loss(&error_curve);
                let p = flat_loss(&data.freqs, &error, data.min_freq, data.max_freq);
                // HeadphoneScore fitness: minimize (1000 - score + flatness*20)
                // - 1000.0: reference ceiling for Olive preference score (max ~114.49)
                // - *20.0: amplifies flatness term (headphone score has small dynamic range)
                1000.0 - s + p * 20.0
            } else {
                log::error!("headphone score loss requested but headphone data is missing");
                f64::INFINITY
            }
        }
        LossType::Epa => {
            let peq_spl = x2spl(&data.freqs, x, data.srate, data.peq_model);
            let error = &peq_spl - &data.deviation;
            let flatness = flat_loss(&data.freqs, &error, data.min_freq, data.max_freq);
            let freqs_vec: Vec<f64> = data.freqs.iter().copied().collect();
            let corrected_spl: Vec<f64> = data
                .freqs
                .iter()
                .enumerate()
                .map(|(i, _)| data.target[i] + data.deviation[i] + peq_spl[i])
                .collect();
            let epa_config = data.epa_config.clone().unwrap_or_default();
            // Use the `_normalized` variant because `corrected_spl` is built
            // from level-relative (target + deviation + PEQ) components —
            // it is not absolute dB SPL. The normalized helper denormalizes
            // against `epa_config.listening_level_phon` so the loudness /
            // loudness-balance penalties are properly calibrated.
            crate::loss::epa::score::epa_loss_normalized(
                &freqs_vec,
                &corrected_spl,
                &epa_config,
                flatness,
            )
        }
    }
}

/// Compute objective function value including penalty terms for constraints
///
/// Non-mutating version used by optimizers that don't require `&mut` data
/// (e.g., metaheuristics). Avoids cloning ObjectiveData on every evaluation.
pub fn compute_fitness_penalties_ref(x: &[f64], data: &ObjectiveData) -> f64 {
    let fit = compute_base_fitness(x, data);

    // PEQ-specific penalties only apply when the parameter vector has PEQ layout
    // (freq/Q/gain triplets). DriversFlat and MultiSubFlat use a different layout
    // (gains/delays/crossovers) and these penalty functions would misinterpret the values.
    let is_peq_loss = !matches!(
        data.loss_type,
        LossType::DriversFlat | LossType::MultiSubFlat
    );

    // When penalties are enabled (weights > 0), add them to the base fit so that
    // optimizers without nonlinear constraints can still respect our limits.
    let mut penalized = fit;

    if data.penalty_w_ceiling > 0.0 && is_peq_loss {
        let peq_spl = x2spl(&data.freqs, x, data.srate, data.peq_model);
        let viol = viol_ceiling_from_spl(&peq_spl, data.max_db, data.peq_model);
        penalized += data.penalty_w_ceiling * viol * viol;
    }

    if data.penalty_w_spacing > 0.0 && is_peq_loss {
        let viol = viol_spacing_from_xs(x, data.peq_model, data.min_spacing_oct);
        penalized += data.penalty_w_spacing * viol * viol;
    }

    if data.penalty_w_mingain > 0.0 && data.min_db > 0.0 && is_peq_loss {
        let viol = viol_min_gain_from_xs(x, data.peq_model, data.min_db);
        penalized += data.penalty_w_mingain * viol * viol;
    }

    penalized
}

/// Compute objective function value including penalty terms for constraints
///
/// NLOPT-compatible wrapper that takes `&mut ObjectiveData` (required by NLOPT's callback
/// signature). Delegates to `compute_fitness_penalties_ref`.
///
/// # Arguments
/// * `x` - Parameter vector
/// * `_gradient` - Gradient vector (unused, for NLOPT compatibility)
/// * `data` - Objective data containing penalty weights and parameters
///
/// # Returns
/// Base fitness value plus weighted penalty terms
pub fn compute_fitness_penalties(
    x: &[f64],
    _gradient: Option<&mut [f64]>,
    data: &mut ObjectiveData,
) -> f64 {
    compute_fitness_penalties_ref(x, data)
}

/// Optimize filter parameters using global optimization algorithms
///
/// # Arguments
/// * `x` - Initial parameter vector to optimize (modified in place)
/// * `lower_bounds` - Lower bounds for each parameter
/// * `upper_bounds` - Upper bounds for each parameter
/// * `objective_data` - Data structure containing optimization parameters
/// * `cli_args` - CLI arguments containing algorithm, population, maxeval, and other parameters
///
/// # Returns
/// * Result containing (status, optimal value) or (error, value)
///
/// # Details
/// Dispatches to appropriate library-specific optimizer based on algorithm name.
/// The parameter vector is organized as [freq, Q, gain] triplets for each filter.
pub fn optimize_filters(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    params: &crate::OptimParams,
) -> Result<(String, f64), (String, f64)> {
    optimize_filters_with_algo_override(
        x,
        lower_bounds,
        upper_bounds,
        objective_data,
        params,
        None,
    )
}

/// Optimize filter parameters with optional algorithm override
///
/// # Arguments
/// * `x` - Initial parameter vector to optimize (modified in place)
/// * `lower_bounds` - Lower bounds for each parameter
/// * `upper_bounds` - Upper bounds for each parameter
/// * `objective_data` - Data structure containing optimization parameters
/// * `cli_args` - CLI arguments containing algorithm, population, maxeval, and other parameters
/// * `algo_override` - Optional algorithm override (e.g., for local refinement)
///
/// # Returns
/// * Result containing (status, optimal value) or (error, value)
pub fn optimize_filters_with_algo_override(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    params: &crate::OptimParams,
    algo_override: Option<&str>,
) -> Result<(String, f64), (String, f64)> {
    // Extract parameters from params
    let algo = algo_override.unwrap_or(&params.algo);
    let population = params.population;
    let maxeval = params.maxeval;

    // Parse algorithm and dispatch to appropriate function
    match parse_algorithm_name(algo) {
        #[cfg(feature = "nlopt")]
        Some(AlgorithmCategory::Nlopt(nlopt_algo)) => optimize_filters_nlopt(
            x,
            lower_bounds,
            upper_bounds,
            objective_data,
            nlopt_algo,
            population,
            maxeval,
        ),
        Some(AlgorithmCategory::Metaheuristics(mh_name)) => optimize_filters_mh(
            x,
            lower_bounds,
            upper_bounds,
            objective_data,
            &mh_name,
            population,
            maxeval,
        ),
        Some(AlgorithmCategory::AutoEQ(autoeq_name)) => optimize_filters_autoeq(
            x,
            lower_bounds,
            upper_bounds,
            objective_data,
            &autoeq_name,
            params,
        ),
        None => Err((format!("Unknown algorithm: {}", algo), f64::INFINITY)),
    }
}

/// Progress callback: (iteration, best_loss, epa_preference) -> continue/stop
///
/// Used to thread per-iteration optimizer progress through the room EQ call chain.
/// `epa_preference` is `Some` when EPA is computed (every N iterations), `None` otherwise.
pub type OptimProgressCallback =
    Box<dyn FnMut(usize, f64, Option<f64>) -> crate::de::CallbackAction + Send>;

/// Optimize filter parameters with a progress callback for per-iteration updates.
///
/// Wraps the simple `(iteration, loss)` callback into the library-specific
/// intermediate types expected by DE and MH optimizers. NLopt has no callback
/// support so the callback is ignored for NLopt algorithms.
#[allow(clippy::too_many_arguments)]
pub fn optimize_filters_with_callback(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    params: &crate::OptimParams,
    mut callback: OptimProgressCallback,
) -> Result<(String, f64), (String, f64)> {
    let algo = &params.algo;
    let population = params.population;
    let maxeval = params.maxeval;

    match parse_algorithm_name(algo) {
        #[cfg(feature = "nlopt")]
        Some(AlgorithmCategory::Nlopt(nlopt_algo)) => {
            // NLopt does not support iteration callbacks — run without one
            optimize_filters_nlopt(
                x,
                lower_bounds,
                upper_bounds,
                objective_data,
                nlopt_algo,
                population,
                maxeval,
            )
        }
        Some(AlgorithmCategory::Metaheuristics(mh_name)) => {
            let mh_cb: Box<
                dyn FnMut(&super::optim_mh::MHIntermediate) -> crate::de::CallbackAction + Send,
            > = Box::new(move |intermediate| callback(intermediate.iter, intermediate.fun, None));
            optimize_filters_mh_with_callback(
                x,
                lower_bounds,
                upper_bounds,
                objective_data,
                &mh_name,
                population,
                maxeval,
                mh_cb,
            )
        }
        Some(AlgorithmCategory::AutoEQ(autoeq_name)) => {
            // Clone data needed for EPA computation inside the DE callback.
            let epa_config = objective_data.epa_config.clone();
            let epa_freqs = ndarray::Array1::from(
                objective_data.freqs.iter().copied().collect::<Vec<f64>>(),
            );
            // The normalized measurement is: target - deviation
            // (since deviation = target - normalized_measurement).
            // The corrected response after applying PEQ is:
            //   corrected = normalized + peq = (target - deviation) + peq
            let epa_normalized: Vec<f64> = objective_data
                .target
                .iter()
                .zip(objective_data.deviation.iter())
                .map(|(&t, &d)| t - d)
                .collect();
            let epa_srate = objective_data.srate;
            let epa_model = objective_data.peq_model;
            let mut epa_gen_counter: usize = 0;
            const EPA_INTERVAL: usize = 10;

            let de_cb: Box<
                dyn FnMut(&crate::de::DEIntermediate) -> crate::de::CallbackAction + Send,
            > = Box::new(move |intermediate| {
                epa_gen_counter += 1;
                let epa = if epa_gen_counter % EPA_INTERVAL == 0 {
                    // Corrected = normalized_measurement + PEQ response
                    let peq_spl = x2spl(
                        &epa_freqs,
                        intermediate.x.as_slice().unwrap(),
                        epa_srate,
                        epa_model,
                    );
                    let corrected: Vec<f64> = epa_normalized
                        .iter()
                        .enumerate()
                        .map(|(i, &n)| n + peq_spl[i])
                        .collect();
                    let cfg = epa_config.clone().unwrap_or_default();
                    let score = crate::loss::epa::score::compute_epa_normalized(
                        epa_freqs.as_slice().unwrap(),
                        &corrected,
                        &cfg,
                    );
                    Some(score.preference)
                } else {
                    None
                };
                callback(intermediate.iter, intermediate.fun, epa)
            });
            optimize_filters_autoeq_with_callback(
                x,
                lower_bounds,
                upper_bounds,
                objective_data,
                &autoeq_name,
                params,
                de_cb,
            )
        }
        None => Err((format!("Unknown algorithm: {}", algo), f64::INFINITY)),
    }
}

/// Extract sorted center frequencies from parameter vector and compute adjacent spacings in octaves.
pub fn compute_sorted_freqs_and_adjacent_octave_spacings(
    x: &[f64],
    peq_model: PeqModel,
) -> (Vec<f64>, Vec<f64>) {
    let n = crate::param_utils::num_filters(x, peq_model);
    let mut freqs: Vec<f64> = Vec::with_capacity(n);
    for i in 0..n {
        let params = crate::param_utils::get_filter_params(x, i, peq_model);
        freqs.push(10f64.powf(params.freq));
    }
    freqs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let spacings: Vec<f64> = if freqs.len() < 2 {
        Vec::new()
    } else {
        freqs
            .windows(2)
            .map(|w| (w[1].max(1e-9) / w[0].max(1e-9)).log2().abs())
            .collect()
    };
    (freqs, spacings)
}

#[cfg(test)]
mod spacing_diag_tests {
    use super::compute_sorted_freqs_and_adjacent_octave_spacings;

    #[test]
    fn adjacent_octave_spacings_basic() {
        // x: [f,q,g, f,q,g, f,q,g]
        let x = [
            100f64.log10(),
            1.0,
            0.0,
            200f64.log10(),
            1.0,
            0.0,
            400f64.log10(),
            1.0,
            0.0,
        ];
        use crate::cli::PeqModel;
        let (_freqs, spacings) =
            compute_sorted_freqs_and_adjacent_octave_spacings(&x, PeqModel::Pk);
        assert!((spacings[0] - 1.0).abs() < 1e-12);
        assert!((spacings[1] - 1.0).abs() < 1e-12);
    }
}
