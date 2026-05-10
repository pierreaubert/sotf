//! AutoEQ CMA-ES backend.
//!
//! Wraps [`math_audio_optimisation::cma_es`] in the shared
//! [`FilterOptimizer`](super::backend::FilterOptimizer) interface. CMA-ES does
//! not support native AutoEQ constraints, so ceiling/spacing/min-gain are
//! folded into the scalar objective as penalties.

use super::backend::{AlgorithmType, ConstraintCapabilities, FilterOptimizer};
use super::constraints_install::install_constraints;
use super::params::OptimParams;
use super::{ObjectiveData, OptimProgressCallback, PenaltyMode, compute_fitness_penalties_ref};
use math_audio_optimisation::{CmaEsConfig, CmaEsIntermediate, ParallelConfig, cma_es};
use ndarray::Array1;
use std::sync::Arc;

/// Pure-Rust CMA-ES `FilterOptimizer`.
pub struct AutoeqCmaEsBackend {
    name: &'static str,
}

impl AutoeqCmaEsBackend {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl FilterOptimizer for AutoeqCmaEsBackend {
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
        let objective = Arc::new(objective);
        let obj_for_call = objective.clone();
        let f = move |x: &Array1<f64>| -> f64 {
            compute_fitness_penalties_ref(x.as_slice().unwrap(), &obj_for_call)
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

        let lambda = params.population.max(4);
        let mut cfg = CmaEsConfig {
            bounds,
            x0: Some(x0),
            sigma0: Some(0.20),
            lambda,
            mu: 0,
            maxeval: params.maxeval.max(lambda + 1),
            seed: params.seed,
            f_tol: params.atolerance.max(1e-12),
            stagnation_window: 80,
            parallel: ParallelConfig {
                enabled: !params.no_parallel,
                num_threads: if params.parallel_threads == 0 {
                    None
                } else {
                    Some(params.parallel_threads)
                },
            },
            ..Default::default()
        };

        if let Some(mut user_cb) = callback {
            cfg.callback = Some(Box::new(move |im: &CmaEsIntermediate| {
                user_cb(im.iter, im.fun, None)
            }));
        }

        match cma_es(&f, cfg) {
            Ok(report) => {
                if report.x.len() == x.len() {
                    x.copy_from_slice(report.x.as_slice().unwrap());
                }
                let label = if report.success {
                    format!("AutoEQ CMA-ES: {}", report.message)
                } else {
                    format!("AutoEQ CMA-ES: {} (not converged)", report.message)
                };
                Ok((label, report.fun))
            }
            Err(e) => Err((format!("CMA-ES setup failed: {:?}", e), f64::INFINITY)),
        }
    }
}
