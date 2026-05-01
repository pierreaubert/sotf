//! Unified optimizer backend trait.
//!
//! All filter-fitting algorithms (AutoEQ DE, NLopt, metaheuristics_nature)
//! implement [`FilterOptimizer`]. The registry maps algorithm names with
//! library prefixes (`"autoeq:de"`, `"nlopt:cobyla"`, `"mh:pso"`, …) plus
//! their unprefixed legacy aliases to concrete optimizer impls.
//!
//! This module replaces the old per-library `match` dispatchers in
//! [`super::optimize_filters`] and the parallel `AlgorithmInfo` table.

use super::params::OptimParams;
use super::{ObjectiveData, OptimProgressCallback, PenaltyMode};

/// Algorithm classification (mirrors the previous `AlgorithmType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlgorithmType {
    /// Global optimization algorithm — explores entire solution space.
    Global,
    /// Local optimization algorithm — refines a starting point.
    Local,
}

/// What an optimizer backend can handle natively.
///
/// The constraint-vs-penalty decision in [`super::constraints_install`] is
/// driven entirely by these flags — backends no longer need their own
/// `use_penalties` match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstraintCapabilities {
    /// Backend supports `g_i(x) <= 0` constraints registered natively.
    pub nonlinear_ineq: bool,
    /// Backend supports `h_j(x) = 0` constraints registered natively.
    pub nonlinear_eq: bool,
    /// Backend supports linear constraints `lb <= A x <= ub` natively.
    pub linear: bool,
    /// Backend invokes a user-supplied per-iteration callback.
    pub iteration_callback: bool,
    /// Penalty mode used to fold constraints into the objective when the
    /// backend lacks native support. Ignored when `nonlinear_ineq == true`.
    pub fallback_penalty_mode: PenaltyMode,
}

/// A pluggable filter-parameter optimizer.
///
/// Implementations live in [`super::de`], [`super::mh`], and [`super::nlopt`]
/// (the last is feature-gated behind `nlopt`).
pub trait FilterOptimizer: Send + Sync {
    /// Canonical algorithm name with library prefix (e.g. `"nlopt:cobyla"`).
    fn name(&self) -> &'static str;

    /// Source library label shown in the algorithm list (e.g. `"NLOPT"`).
    fn library(&self) -> &'static str;

    /// Whether this is a global or local algorithm.
    fn algorithm_type(&self) -> AlgorithmType;

    /// What this backend can handle natively.
    fn capabilities(&self) -> ConstraintCapabilities;

    /// Optimize filter parameters.
    ///
    /// `x` is the in/out parameter vector — on success the best-found
    /// solution is written back. The return tuple is `(status_message,
    /// best_objective_value)`. The `Err` variant means the optimizer ran
    /// to completion but did not converge; an `Err` is *not* a panic or
    /// hard failure — callers should still consider `x` updated to the
    /// best point seen.
    ///
    /// `callback` is honored only when [`Self::capabilities`] reports
    /// `iteration_callback = true`; backends that lack callback support
    /// silently ignore it (NLopt is the typical case).
    fn optimize(
        &self,
        x: &mut [f64],
        lower: &[f64],
        upper: &[f64],
        objective: ObjectiveData,
        params: &OptimParams,
        callback: Option<OptimProgressCallback>,
    ) -> Result<(String, f64), (String, f64)>;
}

/// Outcome of installing constraints into the objective for a given backend.
///
/// Returned by [`super::constraints_install::install_constraints`]; consumed
/// by each backend in `optimize` to know whether to register native
/// constraints (DE, NLopt-with-support) or rely on penalty terms folded
/// into `compute_fitness_penalties_ref` (NLopt-without-support, MH).
pub enum ConstraintInstallation {
    /// Backend should register these as native nonlinear inequalities.
    Native(Vec<NativeConstraint>),
    /// Constraints are folded into the objective via penalty weights —
    /// the backend has nothing further to do.
    Penalty,
}

/// Erased constraint closure: takes the parameter vector and returns the
/// inequality `fun(x) <= 0` violation amount.
pub type ConstraintFn = Box<dyn Fn(&[f64]) -> f64 + Send + Sync>;

/// A nonlinear inequality constraint `fun(x) <= 0` paired with the data it
/// closes over. Decoupled from any specific optimizer's signature so the
/// same descriptor feeds DE's `NonlinearConstraintHelper`, NLopt's
/// `add_inequality_constraint`, and the future in-tree COBYLA/ISRES.
pub struct NativeConstraint {
    /// Human-readable name for logging (e.g. `"ceiling"`, `"min_gain"`).
    pub label: &'static str,
    /// Closure evaluating the constraint at `x`. Feasible when `<= 0`.
    pub fun: ConstraintFn,
    /// Numerical tolerance for native-constraint registration (NLopt).
    /// DE ignores this and uses its own per-helper weighting.
    pub tol: f64,
}

/// Resolve an algorithm name (prefixed or legacy unprefixed) to its backend.
pub fn resolve(name: &str) -> Option<Box<dyn FilterOptimizer>> {
    super::registry::resolve(name)
}

/// All algorithms currently exposed by autoeq, in registration order.
pub fn all_algorithms() -> Vec<Box<dyn FilterOptimizer>> {
    super::registry::all_algorithms()
}
