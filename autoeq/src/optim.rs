//! AutoEQ - A library for audio equalization and filter optimization
//!
//! Copyright (C) 2025 Pierre Aubert pierre(at)spinorama(dot)org
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
    headphone_loss, speaker_score_loss,
};
use super::optim_de::optimize_filters_autoeq;
use super::optim_mh::optimize_filters_mh;
#[cfg(feature = "nlopt")]
use super::optim_nlopt::optimize_filters_nlopt;
use super::x2peq::x2spl;
use crate::Curve;
use ndarray::Array1;
#[cfg(feature = "nlopt")]
use nlopt::Algorithm;
use std::process;

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

/// Compute the base fitness value (without penalties) for given parameters
///
/// This is the unified fitness function used by both NLOPT and metaheuristics optimizers.
pub fn compute_base_fitness(x: &[f64], data: &ObjectiveData) -> f64 {
    match data.loss_type {
        LossType::DriversFlat => {
            // Multi-driver crossover optimization
            if let Some(ref drivers_data) = data.drivers_data {
                let n_drivers = drivers_data.drivers.len();
                // Parameter layout: [gain1, gain2, ..., gainN, xover_freq1, xover_freq2, ..., xover_freq(N-1)]
                let gains = &x[0..n_drivers];
                let xover_freqs_log10 = &x[n_drivers..];

                // Convert crossover frequencies from log10 to Hz
                let xover_freqs: Vec<f64> = xover_freqs_log10
                    .iter()
                    .map(|f| 10.0_f64.powf(*f))
                    .collect();

                drivers_flat_loss(
                    drivers_data,
                    gains,
                    &xover_freqs,
                    data.srate,
                    data.min_freq,
                    data.max_freq,
                )
            } else {
                eprintln!("Error: drivers-flat loss requested but driver data is missing");
                process::exit(1);
            }
        }
        LossType::HeadphoneFlat | LossType::SpeakerFlat => {
            let peq_spl = x2spl(&data.freqs, x, data.srate, data.peq_model);
            let error = &peq_spl - &data.deviation;
            flat_loss(&data.freqs, &error, data.min_freq, data.max_freq)
        }
        LossType::SpeakerScore => {
            let peq_spl = x2spl(&data.freqs, x, data.srate, data.peq_model);
            if let Some(ref sd) = data.speaker_score_data {
                let error = &peq_spl - &data.deviation;
                let s = speaker_score_loss(sd, &data.freqs, &peq_spl);
                let p = flat_loss(&data.freqs, &error, data.min_freq, data.max_freq) / 3.0;
                100.0 - s + p
            } else {
                eprintln!("Error: speaker score loss requested but score data is missing");
                process::exit(1);
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
                };
                let s = headphone_loss(&error_curve);
                // compute flat error
                let p = flat_loss(&data.freqs, &error, data.min_freq, data.max_freq);
                // wants to maximize the score and improve the flatness
                // println!("DEBUG Headphone score: s={:.3} p={:.3} fitness={:.3}", s, p, 1000.0 - s + p * 20.0);
                1000.0 - s + p * 20.0
            } else {
                eprintln!("Error: headphone score loss requested but headphone data is missing");
                process::exit(1);
            }
        }
    }
}

/// Compute objective function value including penalty terms for constraints
///
/// This function adds penalty terms to the base fitness when using algorithms
/// that don't support native constraint handling.
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
    let fit = compute_base_fitness(x, data);

    // When penalties are enabled (weights > 0), add them to the base fit so that
    // optimizers without nonlinear constraints can still respect our limits.
    let mut penalized = fit;
    let mut penalty_terms = Vec::new();

    if data.penalty_w_ceiling > 0.0 {
        let peq_spl = x2spl(&data.freqs, x, data.srate, data.peq_model);
        let viol = viol_ceiling_from_spl(&peq_spl, data.max_db, data.peq_model);
        let penalty = data.penalty_w_ceiling * viol * viol;
        penalized += penalty;
        if viol > 0.0 {
            penalty_terms.push(format!(
                "ceiling_viol={:.3e}*{:.1e}={:.3e}",
                viol, data.penalty_w_ceiling, penalty
            ));
        }
    }

    if data.penalty_w_spacing > 0.0 {
        let viol = viol_spacing_from_xs(x, data.peq_model, data.min_spacing_oct);
        let penalty = data.penalty_w_spacing * viol * viol;
        penalized += penalty;
        if viol > 0.0 {
            penalty_terms.push(format!(
                "spacing_viol={:.3e}*{:.1e}={:.3e}",
                viol, data.penalty_w_spacing, penalty
            ));
        }
    }

    if data.penalty_w_mingain > 0.0 && data.min_db > 0.0 {
        let viol = viol_min_gain_from_xs(x, data.peq_model, data.min_db);
        let penalty = data.penalty_w_mingain * viol * viol;
        penalized += penalty;
        if viol > 0.0 {
            penalty_terms.push(format!(
                "mingain_viol={:.3e}*{:.1e}={:.3e}",
                viol, data.penalty_w_mingain, penalty
            ));
        }
    }

    // // Log fitness details every 1000 evaluations (approximate)
    // use std::sync::atomic::{AtomicUsize, Ordering};
    // static EVAL_COUNTER: AtomicUsize = AtomicUsize::new(0);
    // let count = EVAL_COUNTER.fetch_add(1, Ordering::Relaxed);
    // if count % 1000 == 0 || (count % 100 == 0 && !penalty_terms.is_empty()) {
    //     let param_summary: Vec<String> = (0..x.len()/3).map(|i| {
    //         let freq = 10f64.powf(x[i*3]);
    //         let q = x[i*3+1];
    //         let gain = x[i*3+2];
    //         format!("f{:.0}Hz/Q{:.2}/G{:.2}dB", freq, q, gain)
    //     }).collect();

    //     eprintln!("TRACE[{}]: fit={:.3e}, penalties=[{}], params=[{}]",
    //               count, fit, penalty_terms.join(", "), param_summary.join(", "));
    // }

    penalized
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
    cli_args: &crate::cli::Args,
) -> Result<(String, f64), (String, f64)> {
    optimize_filters_with_algo_override(
        x,
        lower_bounds,
        upper_bounds,
        objective_data,
        cli_args,
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
    cli_args: &crate::cli::Args,
    algo_override: Option<&str>,
) -> Result<(String, f64), (String, f64)> {
    // Extract parameters from args
    let algo = algo_override.unwrap_or(&cli_args.algo);
    let population = cli_args.population;
    let maxeval = cli_args.maxeval;

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
            cli_args,
        ),
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
