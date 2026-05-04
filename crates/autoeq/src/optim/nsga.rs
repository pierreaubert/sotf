//! AutoEQ NSGA-II/III Pareto backends.
//!
//! The shared optimizer trait returns a single parameter vector, so this
//! backend performs a genuine generational Pareto search, logs a compact
//! summary of the rank-0 front, then writes back a normalised compromise point
//! from that front.

use super::backend::{AlgorithmType, ConstraintCapabilities, FilterOptimizer};
use super::constraints_install::install_constraints;
use super::params::OptimParams;
use super::{
    ObjectiveData, OptimProgressCallback, PenaltyMode, compute_fitness_penalties_ref,
    compute_pareto_objectives,
};
use math_audio_optimisation::{NsgaConfig, NsgaVariant, ParetoSolution, nsga};
use ndarray::Array1;
use std::sync::Arc;

/// Pure-Rust NSGA-II/III `FilterOptimizer`.
pub struct AutoeqNsgaBackend {
    name: &'static str,
    variant: NsgaVariant,
}

impl AutoeqNsgaBackend {
    pub fn new_nsga2(name: &'static str) -> Self {
        Self {
            name,
            variant: NsgaVariant::Nsga2,
        }
    }

    pub fn new_nsga3(name: &'static str) -> Self {
        Self {
            name,
            variant: NsgaVariant::Nsga3,
        }
    }
}

impl FilterOptimizer for AutoeqNsgaBackend {
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
            iteration_callback: false,
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
        _callback: Option<OptimProgressCallback>,
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
        let objective = Arc::new(objective);
        let obj_for_call = objective.clone();
        let f = move |x: &Array1<f64>| -> Vec<f64> {
            compute_pareto_objectives(x.as_slice().unwrap(), &obj_for_call)
        };

        let bounds: Vec<(f64, f64)> = lower
            .iter()
            .zip(upper.iter())
            .map(|(&lo, &hi)| (lo, hi))
            .collect();
        let x0 = Array1::from(
            x.iter()
                .zip(bounds.iter())
                .map(|(&xi, (lo, hi))| xi.clamp(*lo, *hi))
                .collect::<Vec<_>>(),
        );
        let population_size = params.population.max(16);
        let cfg = NsgaConfig {
            bounds,
            x0: Some(x0),
            population_size,
            maxeval: params.maxeval.max(population_size),
            variant: self.variant,
            seed: params.seed,
            ..Default::default()
        };

        match nsga(&f, cfg) {
            Ok(report) => {
                let front = if report.pareto_front.is_empty() {
                    &report.population
                } else {
                    &report.pareto_front
                };
                let Some(best) = choose_compromise(front, objective.as_ref()) else {
                    return Err((
                        format!("{} produced an empty population", self.name),
                        f64::INFINITY,
                    ));
                };

                if best.x.len() == x.len() {
                    x.copy_from_slice(best.x.as_slice().unwrap());
                }
                log_pareto_front(self.name, front, best);
                let loss = compute_fitness_penalties_ref(x, objective.as_ref());
                Ok((
                    format!(
                        "AutoEQ {}: {} Pareto points, selected compromise scalar loss {:.6}",
                        variant_label(self.variant),
                        report.pareto_front.len(),
                        loss
                    ),
                    loss,
                ))
            }
            Err(e) => Err((
                format!("{} setup failed: {:?}", self.name, e),
                f64::INFINITY,
            )),
        }
    }
}

fn choose_compromise<'a>(
    front: &'a [ParetoSolution],
    objective: &ObjectiveData,
) -> Option<&'a ParetoSolution> {
    if front.is_empty() {
        return None;
    }
    let m = front[0].objectives.len();
    if m == 0 {
        return front.first();
    }
    let weights = pareto_weights(objective, m);
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

fn pareto_weights(objective: &ObjectiveData, m: usize) -> Vec<f64> {
    if let Some(ref mo) = objective.multi_objective
        && mo.weights.len() == m
    {
        return mo.weights.clone();
    }
    vec![1.0 / m as f64; m]
}

fn compromise_distance(
    solution: &ParetoSolution,
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

fn log_pareto_front(name: &str, front: &[ParetoSolution], selected: &ParetoSolution) {
    if front.is_empty() {
        return;
    }
    log::info!("{} Pareto front: {} rank-0 points", name, front.len());
    let mut ranked = front.iter().collect::<Vec<_>>();
    ranked.sort_by(|a, b| sum_objectives(&a.objectives).total_cmp(&sum_objectives(&b.objectives)));
    for (i, sol) in ranked.into_iter().take(8).enumerate() {
        log::info!(
            "  Pareto #{:02}: objectives=[{}]{}",
            i + 1,
            format_objectives(&sol.objectives),
            if std::ptr::eq(sol, selected) {
                " selected"
            } else {
                ""
            }
        );
    }
}

fn format_objectives(objectives: &[f64]) -> String {
    objectives
        .iter()
        .map(|v| format!("{:.6}", v))
        .collect::<Vec<_>>()
        .join(", ")
}

fn sum_objectives(objectives: &[f64]) -> f64 {
    objectives.iter().sum::<f64>()
}

fn variant_label(variant: NsgaVariant) -> &'static str {
    match variant {
        NsgaVariant::Nsga2 => "NSGA-II",
        NsgaVariant::Nsga3 => "NSGA-III",
    }
}
