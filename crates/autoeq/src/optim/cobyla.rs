//! Pure-Rust COBYLA backend (no NLopt C-FFI).
//!
//! Wraps [`math_audio_optimisation::cobyla::cobyla`] in a [`FilterOptimizer`]
//! impl. Native nonlinear inequalities are honored directly — the four
//! [`crate::constraints`] are installed via
//! [`super::constraints_install::install_constraints`] in the same way the
//! NLopt backend does.
//!
//! This backend is registered as `"autoeq:cobyla"` (and resolves the
//! unprefixed legacy alias `"cobyla"`). The NLopt-backed `"nlopt:cobyla"`
//! remains available for A/B comparison while the wider migration to
//! pure-Rust optimizers is in flight.

use super::backend::{
    AlgorithmType, ConstraintCapabilities, ConstraintInstallation, FilterOptimizer,
    NativeConstraint,
};
use super::constraints_install::{build_crossover_monotonicity_constraint, install_constraints};
use super::params::OptimParams;
use super::{ObjectiveData, OptimProgressCallback, PenaltyMode, compute_fitness_penalties_ref};
use math_audio_optimisation::cobyla::{
    CobylaConfig, CobylaConstraint, CobylaConstraintFn, CobylaRhoBegin, CobylaStopTols, cobyla,
};
use ndarray::Array1;
use std::sync::Arc;

/// Pure-Rust COBYLA `FilterOptimizer`.
pub struct AutoeqCobylaBackend {
    name: &'static str,
}

impl AutoeqCobylaBackend {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl FilterOptimizer for AutoeqCobylaBackend {
    fn name(&self) -> &'static str {
        self.name
    }
    fn library(&self) -> &'static str {
        "AutoEQ"
    }
    fn algorithm_type(&self) -> AlgorithmType {
        AlgorithmType::Local
    }
    fn capabilities(&self) -> ConstraintCapabilities {
        ConstraintCapabilities {
            nonlinear_ineq: true,
            // The underlying solver does not handle equalities; matches
            // NLopt's COBYLA in autoeq's previous behaviour.
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

        // Native constraint installation — matches NLopt path.
        let mut objective = objective;
        let installation = install_constraints(self.capabilities(), &mut objective);
        let crossover = build_crossover_monotonicity_constraint(&objective);

        // Build the unified constraint set and convert to CobylaConstraint.
        let mut constraints: Vec<CobylaConstraint> = Vec::new();
        if let ConstraintInstallation::Native(ncs) = installation {
            for c in ncs {
                constraints.push(native_to_cobyla(c));
            }
        }
        if let Some(c) = crossover {
            constraints.push(native_to_cobyla(c));
        }

        // Shared objective with penalties baked in (penalties are 0 here
        // since we use native constraints).
        let obj = Arc::new(objective);
        let obj_for_call = obj.clone();
        let f = move |x: &Array1<f64>| -> f64 {
            compute_fitness_penalties_ref(x.as_slice().unwrap(), &obj_for_call)
        };

        // Per-dimension initial trust-region radius. autoeq's PEQ
        // optimization is most often called as a *local refine* after DE
        // has already converged near a good basin (the production
        // `--algo autoeq:de --refine` flow). A small radius (5% of bound
        // span) keeps COBYLA in that basin; on the KEF LS50 reference
        // problem it produces loss=0.72 vs the NLopt-default 25% rhobeg's
        // 1.01. The trade-off is worse standalone-cobyla performance
        // (rare in practice — autoeq:cobyla is almost never called
        // without DE first). Pinned dimensions (span = 0) get a tiny
        // non-zero radius so the simplex can still be constructed.
        //
        // Note: matching NLopt 2.7.1's 25% default does NOT recover the
        // small_stereo_2_2_group QA case — that regression comes from
        // implementation drift in the c2rust port itself, not from the
        // rhobeg choice.
        let rho_per_dim: Vec<f64> = lower
            .iter()
            .zip(upper.iter())
            .map(|(lo, hi)| {
                let span = (hi - lo).max(0.0);
                if span <= 0.0 {
                    1e-6
                } else {
                    (span * 0.05).max(1e-6)
                }
            })
            .collect();

        let bounds: Vec<(f64, f64)> = lower
            .iter()
            .zip(upper.iter())
            .map(|(&lo, &hi)| (lo, hi))
            .collect();
        // Clip x0 into the bounds — guards against callers seeding with a
        // point that drifted outside (e.g. a refine after DE that converged
        // exactly on the boundary).
        let x0: Vec<f64> = x
            .iter()
            .zip(bounds.iter())
            .map(|(&xi, (lo, hi))| xi.clamp(*lo, *hi))
            .collect();

        let cfg = CobylaConfig {
            x0: Array1::from(x0),
            bounds,
            rho_begin: CobylaRhoBegin::PerDim(rho_per_dim),
            maxeval: params.maxeval.max(1),
            stop_tol: CobylaStopTols::default(),
        };

        match cobyla(&f, &constraints, cfg) {
            Ok(report) => {
                if report.x.len() == x.len() {
                    x.copy_from_slice(report.x.as_slice().unwrap());
                }
                let label = if report.success {
                    format!("AutoEQ COBYLA: {}", report.message)
                } else {
                    format!("AutoEQ COBYLA: {} (not converged)", report.message)
                };
                Ok((label, report.fun))
            }
            Err(e) => Err((format!("COBYLA setup failed: {:?}", e), f64::INFINITY)),
        }
    }
}

/// Convert a unified `NativeConstraint` (closure over `&[f64]`) to the
/// `CobylaConstraint` shape (closure over `&Array1<f64>`).
fn native_to_cobyla(c: NativeConstraint) -> CobylaConstraint {
    let f = c.fun;
    let wrapped: CobylaConstraintFn = Arc::new(move |x: &Array1<f64>| f(x.as_slice().unwrap()));
    CobylaConstraint { fun: wrapped }
}
