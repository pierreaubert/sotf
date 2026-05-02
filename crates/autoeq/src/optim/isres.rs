//! Pure-Rust ISRES backend (no NLopt C-FFI).
//!
//! Wraps [`math_audio_optimisation::isres::isres`] in a [`FilterOptimizer`]
//! impl. Like the COBYLA backend, it installs the four autoeq inequality
//! constraints (ceiling, min-gain, spacing, and crossover monotonicity for
//! `DriversFlat`) natively.
//!
//! Registered as `"autoeq:isres"`. The unprefixed legacy alias `"isres"`
//! resolves to `"nlopt:isres"` first (preserves existing behaviour for
//! roomeq config presets); when the `nlopt` feature is OFF, it falls
//! through to this backend.

use super::backend::{
    AlgorithmType, ConstraintCapabilities, ConstraintInstallation, FilterOptimizer,
    NativeConstraint,
};
use super::constraints_install::{build_crossover_monotonicity_constraint, install_constraints};
use super::params::OptimParams;
use super::{ObjectiveData, OptimProgressCallback, PenaltyMode, compute_fitness_penalties_ref};
use math_audio_optimisation::isres::{IsresConfig, IsresConstraint, IsresConstraintFn, isres};
use ndarray::Array1;
use std::sync::Arc;

/// Pure-Rust ISRES `FilterOptimizer`.
pub struct AutoeqIsresBackend {
    name: &'static str,
}

impl AutoeqIsresBackend {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl FilterOptimizer for AutoeqIsresBackend {
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
            nonlinear_ineq: true,
            // Equality constraints not exposed; ISRES could support them
            // (R&Y 2005 §III.D) but autoeq has no use case.
            nonlinear_eq: false,
            linear: true,
            iteration_callback: false,
            fallback_penalty_mode: PenaltyMode::Disabled,
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
        let installation = install_constraints(self.capabilities(), &mut objective);
        let crossover = build_crossover_monotonicity_constraint(&objective);

        let mut constraints: Vec<IsresConstraint> = Vec::new();
        if let ConstraintInstallation::Native(ncs) = installation {
            for c in ncs {
                constraints.push(native_to_isres(c));
            }
        }
        if let Some(c) = crossover {
            constraints.push(native_to_isres(c));
        }

        let obj = Arc::new(objective);
        let obj_for_call = obj.clone();
        let f = move |x: &Array1<f64>| -> f64 {
            compute_fitness_penalties_ref(x.as_slice().unwrap(), &obj_for_call)
        };

        let bounds: Vec<(f64, f64)> = lower
            .iter()
            .zip(upper.iter())
            .map(|(&lo, &hi)| (lo, hi))
            .collect();
        let x0: Vec<f64> = x
            .iter()
            .zip(bounds.iter())
            .map(|(&xi, (lo, hi))| xi.clamp(*lo, *hi))
            .collect();

        // Population sizing: respect `params.population` when sensible
        // (≥ 2), else use ISRES defaults (μ=30, λ=7μ).
        let mu = params.population.max(2);
        let cfg = IsresConfig {
            bounds,
            x0: Some(Array1::from(x0)),
            mu,
            lambda: 0, // 7μ
            maxeval: params.maxeval.max(mu),
            seed: params.seed,
            ..Default::default()
        };

        match isres(&f, &constraints, cfg) {
            Ok(report) => {
                if report.x.len() == x.len() {
                    x.copy_from_slice(report.x.as_slice().unwrap());
                }
                let label = if report.success {
                    format!("AutoEQ ISRES: {}", report.message)
                } else {
                    format!("AutoEQ ISRES: {} (not converged)", report.message)
                };
                Ok((label, report.fun))
            }
            Err(e) => Err((format!("ISRES setup failed: {:?}", e), f64::INFINITY)),
        }
    }
}

fn native_to_isres(c: NativeConstraint) -> IsresConstraint {
    let f = c.fun;
    let wrapped: IsresConstraintFn = Arc::new(move |x: &Array1<f64>| f(x.as_slice().unwrap()));
    IsresConstraint { fun: wrapped }
}
