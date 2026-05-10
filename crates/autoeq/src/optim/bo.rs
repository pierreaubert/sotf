//! AutoEQ Bayesian-optimisation backend.
//!
//! Wraps the generic Gaussian-process optimiser from `math-optimisation` in
//! the shared [`FilterOptimizer`](super::backend::FilterOptimizer) interface.
//! AutoEQ constraints are folded into the objective as penalties for this v1
//! backend.

use super::backend::{AlgorithmType, ConstraintCapabilities, FilterOptimizer};
use super::constraints_install::install_constraints;
use super::params::OptimParams;
use super::{
    ObjectiveData, OptimProgressCallback, PenaltyMode, compute_fitness_penalties_ref,
    compute_pareto_objectives,
};
use math_audio_optimisation::{
    BayesAcquisition, BayesOptConfig, BayesOptIntermediate, BayesParetoSolution,
    bayesian_multi_objective, bayesian_optimization,
};
use ndarray::Array1;
use std::sync::Arc;

/// Pure-Rust Bayesian-optimisation `FilterOptimizer`.
pub struct AutoeqBoBackend {
    name: &'static str,
}

impl AutoeqBoBackend {
    /// Create a backend with a canonical registry name.
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl FilterOptimizer for AutoeqBoBackend {
    fn name(&self) -> &'static str {
        self.name
    }

    fn library(&self) -> &'static str {
        "AutoEQ"
    }

    fn algorithm_type(&self) -> AlgorithmType {
        AlgorithmType::Global
    }

    fn capabilities(&self) -> ConstraintCapabilities {
        ConstraintCapabilities {
            nonlinear_ineq: false,
            nonlinear_eq: false,
            linear: false,
            iteration_callback: true,
            fallback_penalty_mode: PenaltyMode::Standard,
        }
    }

    fn optimize(
        &self,
        x: &mut [f64],
        lower: &[f64],
        upper: &[f64],
        objective: ObjectiveData,
        params: &OptimParams,
        callback: Option<OptimProgressCallback>,
    ) -> Result<(String, f64), (String, f64)> {
        if lower.len() != x.len() || upper.len() != x.len() {
            return Err((
                format!(
                    "bounds dimension mismatch: x={}, lower={}, upper={}",
                    x.len(),
                    lower.len(),
                    upper.len(),
                ),
                f64::INFINITY,
            ));
        }

        let mut objective = objective;
        let _ = install_constraints(self.capabilities(), &mut objective);
        let bounds = lower
            .iter()
            .zip(upper.iter())
            .map(|(&lo, &hi)| (lo, hi))
            .collect::<Vec<_>>();
        let x0 = Array1::from(
            x.iter()
                .zip(bounds.iter())
                .map(|(&xi, (lo, hi))| xi.clamp(*lo, *hi))
                .collect::<Vec<_>>(),
        );

        if params.bo_ehvi && objective.multi_objective.is_some() {
            return self.optimize_multi(x, bounds, x0, objective, params);
        }

        let objective_for_refine = objective.clone();
        let objective = Arc::new(objective);
        let obj_for_call = objective.clone();
        let f = move |x: &Array1<f64>| -> f64 {
            compute_fitness_penalties_ref(x.as_slice().unwrap(), &obj_for_call)
        };

        let cfg = bo_config(bounds, x0, params, callback);
        match bayesian_optimization(&f, cfg) {
            Ok(report) => {
                if report.x.len() == x.len() {
                    x.copy_from_slice(report.x.as_slice().unwrap());
                }
                let mut status = if report.success {
                    format!(
                        "AutoEQ BO: {} (nfev={}, posterior_std={:.3e})",
                        report.message, report.nfev, report.posterior_std
                    )
                } else {
                    format!(
                        "AutoEQ BO: {} (not converged, nfev={}, posterior_std={:.3e})",
                        report.message, report.nfev, report.posterior_std
                    )
                };
                let mut fun = report.fun;
                if should_refine(params, report.posterior_std) {
                    let (refine_status, refine_fun) = refine_from_bo(
                        self.name,
                        x,
                        lower,
                        upper,
                        objective_for_refine,
                        params,
                        fun,
                    )?;
                    status.push_str("; ");
                    status.push_str(&refine_status);
                    fun = refine_fun;
                }
                Ok((status, fun))
            }
            Err(e) => Err((format!("BO setup failed: {:?}", e), f64::INFINITY)),
        }
    }
}

impl AutoeqBoBackend {
    fn optimize_multi(
        &self,
        x: &mut [f64],
        bounds: Vec<(f64, f64)>,
        x0: Array1<f64>,
        objective: ObjectiveData,
        params: &OptimParams,
    ) -> Result<(String, f64), (String, f64)> {
        let objective_for_refine = objective.clone();
        let objective = Arc::new(objective);
        let obj_for_call = objective.clone();
        let f = move |x: &Array1<f64>| -> Vec<f64> {
            compute_pareto_objectives(x.as_slice().unwrap(), &obj_for_call)
        };

        let cfg = bo_config(bounds.clone(), x0, params, None);
        match bayesian_multi_objective(&f, cfg) {
            Ok(report) => {
                let front = if report.pareto_front.is_empty() {
                    &report.population
                } else {
                    &report.pareto_front
                };
                let Some(best) = choose_compromise(front, objective.as_ref()) else {
                    return Err((
                        "AutoEQ BO EHVI produced an empty population".into(),
                        f64::INFINITY,
                    ));
                };
                if best.x.len() == x.len() {
                    x.copy_from_slice(best.x.as_slice().unwrap());
                }
                let mut fun = compute_fitness_penalties_ref(x, objective.as_ref());
                let mut status = format!(
                    "AutoEQ BO-EHVI: {} Pareto points, selected compromise scalar loss {:.6}",
                    report.pareto_front.len(),
                    fun
                );
                if params.refine {
                    let lower = bounds.iter().map(|(lo, _)| *lo).collect::<Vec<_>>();
                    let upper = bounds.iter().map(|(_, hi)| *hi).collect::<Vec<_>>();
                    let (refine_status, refine_fun) = refine_from_bo(
                        self.name,
                        x,
                        &lower,
                        &upper,
                        objective_for_refine,
                        params,
                        fun,
                    )?;
                    status.push_str("; ");
                    status.push_str(&refine_status);
                    fun = refine_fun;
                }
                Ok((status, fun))
            }
            Err(e) => Err((format!("BO-EHVI setup failed: {:?}", e), f64::INFINITY)),
        }
    }
}

fn bo_config(
    bounds: Vec<(f64, f64)>,
    x0: Array1<f64>,
    params: &OptimParams,
    callback: Option<OptimProgressCallback>,
) -> BayesOptConfig {
    let free_dims = bounds.iter().filter(|(lo, hi)| hi > lo).count().max(1);
    let batch_size = if params.bo_batch_size == 0 {
        if params.no_parallel {
            1
        } else {
            params.parallel_threads.clamp(1, 16)
        }
    } else {
        params.bo_batch_size
    };
    let initial_samples = if params.bo_initial_samples == 0 {
        (2 * free_dims + 1).max(batch_size * 2).max(8)
    } else {
        params.bo_initial_samples
    };
    let acquisition = parse_acquisition(&params.bo_acquisition);
    let mut user_cb = callback;

    BayesOptConfig {
        bounds,
        x0: Some(x0),
        initial_samples,
        batch_size,
        maxeval: params.maxeval.max(initial_samples),
        candidate_pool_size: (64 * free_dims).max(512),
        posterior_std_threshold: params.bo_posterior_std_threshold.max(0.0),
        seed: params.seed,
        acquisition,
        parallel: math_audio_optimisation::ParallelConfig {
            enabled: !params.no_parallel,
            num_threads: if params.parallel_threads > 0 {
                Some(params.parallel_threads)
            } else {
                None
            },
        },
        callback: user_cb.take().map(|mut cb| {
            Box::new(move |im: &BayesOptIntermediate| cb(im.iter, im.fun, None))
                as math_audio_optimisation::BayesOptCallback
        }),
        ..Default::default()
    }
}

fn parse_acquisition(name: &str) -> BayesAcquisition {
    match name.to_ascii_lowercase().as_str() {
        "ei" | "expected-improvement" | "expected_improvement" => {
            BayesAcquisition::ExpectedImprovement
        }
        "thompson" | "ts" => BayesAcquisition::Thompson,
        _ => BayesAcquisition::QExpectedImprovement,
    }
}

fn should_refine(params: &OptimParams, posterior_std: f64) -> bool {
    if !params.refine {
        return false;
    }
    params.bo_posterior_std_threshold <= 0.0 || posterior_std <= params.bo_posterior_std_threshold
}

fn refine_from_bo(
    bo_name: &str,
    x: &mut [f64],
    lower: &[f64],
    upper: &[f64],
    objective: ObjectiveData,
    params: &OptimParams,
    global_fun: f64,
) -> Result<(String, f64), (String, f64)> {
    let local_algo = params.local_algo.as_str();
    let Some(local) = super::registry::resolve(local_algo) else {
        return Err((
            format!("Unknown local algorithm: {}", local_algo),
            f64::INFINITY,
        ));
    };
    if local.name().eq_ignore_ascii_case(bo_name) {
        return Ok((
            "BO refine skipped because local_algo resolves to autoeq:bo".into(),
            global_fun,
        ));
    }

    let before = x.to_vec();
    match local.optimize(x, lower, upper, objective, params, None) {
        Ok((status, local_fun)) if local_fun.is_finite() && local_fun <= global_fun => {
            Ok((format!("refine {}", status), local_fun))
        }
        Ok((status, local_fun)) => {
            x.copy_from_slice(&before);
            Ok((
                format!(
                    "refine {} regressed {:.6} -> {:.6}; kept BO result",
                    status, global_fun, local_fun
                ),
                global_fun,
            ))
        }
        Err((e, _)) => {
            x.copy_from_slice(&before);
            Ok((format!("refine failed: {}; kept BO result", e), global_fun))
        }
    }
}

fn choose_compromise<'a>(
    front: &'a [BayesParetoSolution],
    objective: &ObjectiveData,
) -> Option<&'a BayesParetoSolution> {
    if front.is_empty() {
        return None;
    }
    let m = front[0].objectives.len();
    if m == 0 {
        return front.first();
    }
    let weights = if let Some(ref mo) = objective.multi_objective {
        if mo.weights.len() == m {
            mo.weights.clone()
        } else {
            vec![1.0 / m as f64; m]
        }
    } else {
        vec![1.0 / m as f64; m]
    };

    let mut ideal = vec![f64::INFINITY; m];
    let mut nadir = vec![f64::NEG_INFINITY; m];
    for sol in front {
        for j in 0..m {
            ideal[j] = ideal[j].min(sol.objectives[j]);
            nadir[j] = nadir[j].max(sol.objectives[j]);
        }
    }

    front.iter().min_by(|a, b| {
        compromise_distance(a, &ideal, &nadir, &weights)
            .total_cmp(&compromise_distance(b, &ideal, &nadir, &weights))
    })
}

fn compromise_distance(
    solution: &BayesParetoSolution,
    ideal: &[f64],
    nadir: &[f64],
    weights: &[f64],
) -> f64 {
    solution
        .objectives
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let span = nadir[i] - ideal[i];
            let norm = if span > 0.0 && span.is_finite() {
                (v - ideal[i]) / span
            } else {
                0.0
            };
            weights[i] * norm * norm
        })
        .sum::<f64>()
        .sqrt()
}
