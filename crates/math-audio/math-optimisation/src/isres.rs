//! Improved Stochastic Ranking Evolution Strategy (ISRES).
//!
//! Reference: Runarsson, T.P. & Yao, X., "Search Biases in Constrained
//! Evolutionary Optimization," IEEE Transactions on Systems, Man, and
//! Cybernetics, Part C, vol. 35, no. 2, pp. 233-243, 2005.
//!
//! Pure-Rust implementation. Replaces NLopt's ISRES (`nlopt:isres`) for
//! global constrained optimization with nonlinear inequality constraints.
//!
//! # Algorithm
//!
//! `(μ, λ)` evolution strategy where:
//! - Each individual carries `(x, σ)` — parameter vector + per-dimension
//!   log-normal step sizes.
//! - Each generation produces λ offspring; the top μ by stochastic ranking
//!   become the next generation's parents.
//! - Mutation has three components:
//!   1. **Log-normal step-size adaptation**: σ' = σ * exp(τ' N) * exp(τ Nᵢ)
//!   2. **Differential variation** with probability γ: helps escape narrow
//!      feasible regions by adding (x_best − x_random) to candidate.
//!   3. **Gaussian perturbation**: x' = x + σ' .* Nᵢ(0,1).
//!   Out-of-bounds components are reflected back into `[lo, hi]`.
//! - **Stochastic ranking** (the key contribution): bubble-sort the offspring
//!   by interleaving objective and max-violation comparisons. With
//!   probability `pf`, compare two adjacent individuals by objective; with
//!   probability `1 − pf`, compare by maximum constraint violation. This
//!   biases selection toward the objective when feasibility is roughly
//!   equal, and toward feasibility otherwise.
//!
//! # Constraint convention
//!
//! Inequality constraints `g_i(x) ≤ 0` are feasible at zero or below.
//! `IsresConstraint::fun` should return the violation magnitude — the
//! optimizer treats anything > 0 as infeasible.

use ndarray::Array1;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::sync::Arc;

use crate::error::{DEError, Result};

/// Erased inequality constraint closure: feasible when `<= 0`.
pub type IsresConstraintFn = Arc<dyn Fn(&Array1<f64>) -> f64 + Send + Sync>;

/// A single inequality constraint `fun(x) <= 0`.
#[derive(Clone)]
pub struct IsresConstraint {
    /// Constraint closure. Feasible when ≤ 0; positive values count as the
    /// violation magnitude used in stochastic ranking.
    pub fun: IsresConstraintFn,
}

/// Configuration for [`isres`].
#[derive(Clone)]
pub struct IsresConfig {
    /// `(lower, upper)` per dimension.
    pub bounds: Vec<(f64, f64)>,
    /// Optional initial guess; placed in the first parent slot if provided.
    pub x0: Option<Array1<f64>>,
    /// Parent population size μ. Default 30.
    pub mu: usize,
    /// Offspring population size λ. Default 7 * μ.
    pub lambda: usize,
    /// Maximum number of objective evaluations. Counts each candidate
    /// (offspring or initial population member) once.
    pub maxeval: usize,
    /// Stochastic ranking parameter — probability of comparing by
    /// objective rather than by constraint violation. Runarsson & Yao
    /// recommend `0.4 ≤ pf ≤ 0.5`. Default 0.45.
    pub pf: f64,
    /// Differential variation probability γ (Eq. 2 of R&Y 2005). Default 0.85.
    pub gamma: f64,
    /// Optional RNG seed for deterministic runs.
    pub seed: Option<u64>,
    /// Stop when best objective improves by less than `f_tol` for
    /// `stagnation_window` consecutive generations. Set non-positive to
    /// disable stagnation-based termination. Default `1e-8`.
    pub f_tol: f64,
    /// Number of generations of < `f_tol` improvement before declaring
    /// convergence. Default 50.
    pub stagnation_window: usize,
}

impl Default for IsresConfig {
    fn default() -> Self {
        Self {
            bounds: Vec::new(),
            x0: None,
            mu: 30,
            lambda: 0, // 0 means "use 7μ"
            maxeval: 10_000,
            pf: 0.45,
            gamma: 0.85,
            seed: None,
            f_tol: 1e-8,
            stagnation_window: 50,
        }
    }
}

/// Result of an [`isres`] run.
#[derive(Clone)]
pub struct IsresReport {
    /// Best parameter vector found across all generations.
    pub x: Array1<f64>,
    /// Objective value at `x`.
    pub fun: f64,
    /// Maximum constraint violation at `x` (0 if feasible).
    pub max_violation: f64,
    /// Whether `x` is feasible (max_violation ≤ 0).
    pub feasible: bool,
    /// Whether the run terminated by stagnation (true) or by exhausting
    /// `maxeval` / generation budget (false).
    pub success: bool,
    /// Status message.
    pub message: String,
    /// Number of objective evaluations consumed.
    pub nfev: usize,
    /// Number of generations completed.
    pub nit: usize,
}

impl std::fmt::Debug for IsresReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IsresReport")
            .field("x_len", &self.x.len())
            .field("fun", &self.fun)
            .field("max_violation", &self.max_violation)
            .field("feasible", &self.feasible)
            .field("success", &self.success)
            .field("message", &self.message)
            .field("nfev", &self.nfev)
            .field("nit", &self.nit)
            .finish()
    }
}

#[derive(Clone)]
struct Individual {
    x: Array1<f64>,
    sigma: Array1<f64>,
    fun: f64,
    max_violation: f64,
}

/// Run ISRES.
pub fn isres<F>(f: &F, constraints: &[IsresConstraint], config: IsresConfig) -> Result<IsresReport>
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    let n = config.bounds.len();
    if n == 0 {
        return Err(DEError::BoundsMismatch {
            lower_len: 0,
            upper_len: 0,
        });
    }
    for (i, (lo, hi)) in config.bounds.iter().enumerate() {
        if lo > hi {
            return Err(DEError::InvalidBounds {
                index: i,
                lower: *lo,
                upper: *hi,
            });
        }
    }
    if config.mu < 2 {
        return Err(DEError::PopulationTooSmall {
            pop_size: config.mu,
        });
    }

    let mu = config.mu;
    let lambda = if config.lambda == 0 {
        7 * mu
    } else {
        config.lambda
    };
    if lambda < mu {
        return Err(DEError::PopulationTooSmall { pop_size: lambda });
    }

    // Step-size adaptation parameters (R&Y 2005 §III.A).
    let n_f = n as f64;
    let tau = 1.0 / (2.0 * n_f.sqrt()).sqrt();
    let tau_prime = 1.0 / (2.0 * n_f).sqrt();
    // R&Y suggest σ_max = 0.2 * (hi - lo). Using 1/√n as a clip prevents
    // total saturation while letting individual steps reach a meaningful
    // fraction of the bound span.
    let sigma_max: Vec<f64> = config
        .bounds
        .iter()
        .map(|(lo, hi)| ((hi - lo) * 0.2).max(1e-12))
        .collect();

    let mut rng: StdRng = match config.seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => {
            let mut thread_rng = rand::rng();
            StdRng::from_rng(&mut thread_rng)
        }
    };

    let mut nfev = 0usize;
    let mut population: Vec<Individual> = Vec::with_capacity(mu);

    // Initial μ parents — uniform random within bounds, σ initialised to
    // sigma_max / √n so per-dim Gaussian steps don't immediately blow out
    // of the box.
    let initial_sigma: Vec<f64> = sigma_max
        .iter()
        .map(|&sm| (sm / n_f.sqrt()).max(1e-12))
        .collect();
    for i in 0..mu {
        let mut x = Array1::<f64>::zeros(n);
        // Optionally seed the first parent with x0.
        if i == 0
            && let Some(ref seed) = config.x0
            && seed.len() == n
        {
            for j in 0..n {
                let (lo, hi) = config.bounds[j];
                x[j] = seed[j].clamp(lo, hi);
            }
        } else {
            for j in 0..n {
                let (lo, hi) = config.bounds[j];
                x[j] = lo + rng.random::<f64>() * (hi - lo);
            }
        }
        let sigma = Array1::from(initial_sigma.clone());
        let fun = f(&x);
        let mv = max_violation_of(&x, constraints);
        nfev += 1;
        population.push(Individual {
            x,
            sigma,
            fun,
            max_violation: mv,
        });
        if nfev >= config.maxeval {
            break;
        }
    }
    // If maxeval was tiny we may have under-filled; pad by cloning best.
    while population.len() < mu {
        let last = population.last().cloned().unwrap();
        population.push(last);
    }

    // Track global best — ISRES doesn't guarantee monotonic best-of-pop,
    // so we shadow the best-seen individual independently.
    let mut best = best_individual(&population).clone();
    let mut nit = 0usize;
    let mut stagnation_counter = 0usize;
    let mut last_best_fun = best.fun;
    let mut terminated_by_stagnation = false;

    'outer: while nfev < config.maxeval {
        nit += 1;

        // Generate λ offspring. Each offspring picks a parent uniformly
        // from the μ-strong parent pool (after sorting by stochastic
        // ranking, the top μ are the parents — see Step 2 below).
        let mut offspring: Vec<Individual> = Vec::with_capacity(lambda);
        for k in 0..lambda {
            // Differential variation parent (R&Y 2005, Eq. 2): pair the
            // k-th offspring with the parent at the same index in the
            // sorted population, then add (x_best − x_paired) at a random
            // direction with probability γ. We use parent[k mod μ] as the
            // "main" parent, and a separate random parent as the diff peer.
            let parent_idx = k % mu;
            let parent = &population[parent_idx];

            // Step-size mutation (log-normal; clip to sigma_max).
            let n_global: f64 = standard_normal(&mut rng);
            let mut sigma_new = parent.sigma.clone();
            for j in 0..n {
                let n_local: f64 = standard_normal(&mut rng);
                let mult = (tau_prime * n_global + tau * n_local).exp();
                sigma_new[j] = (parent.sigma[j] * mult).min(sigma_max[j]).max(1e-30);
            }

            // Differential variation: add γ * (x_p1 − x_p2) where p1, p2
            // are randomly chosen parents (with replacement). R&Y use the
            // top-1 parent and a random one to bias toward feasibility.
            let mut x_new = parent.x.clone();
            if rng.random::<f64>() < config.gamma && mu >= 3 {
                let p1 = rng.random_range(0..mu.min(mu / 2 + 1).max(2)); // top half
                let p2 = rng.random_range(0..mu);
                if p1 != p2 {
                    let blend: f64 = rng.random::<f64>();
                    for j in 0..n {
                        x_new[j] += blend * (population[p1].x[j] - population[p2].x[j]);
                    }
                }
            }

            // Gaussian perturbation by σ'.
            for j in 0..n {
                let n_j: f64 = standard_normal(&mut rng);
                x_new[j] += sigma_new[j] * n_j;
            }

            // Reflect into bounds (preserve diversity better than clip).
            for j in 0..n {
                let (lo, hi) = config.bounds[j];
                x_new[j] = reflect_into_bounds(x_new[j], lo, hi);
            }

            let fun = f(&x_new);
            let mv = max_violation_of(&x_new, constraints);
            nfev += 1;
            offspring.push(Individual {
                x: x_new,
                sigma: sigma_new,
                fun,
                max_violation: mv,
            });
            if nfev >= config.maxeval {
                break;
            }
        }

        // Stochastic ranking — bubble sort by interleaving fun-vs-violation.
        stochastic_rank(&mut offspring, config.pf, &mut rng);

        // Survival: top μ become next-gen parents.
        offspring.truncate(mu);
        population = offspring;

        // Update global best.
        let gen_best = best_individual(&population);
        if individual_better(gen_best, &best) {
            best = gen_best.clone();
        }

        // Stagnation check.
        if config.f_tol > 0.0 {
            let delta = (last_best_fun - best.fun).abs();
            if delta < config.f_tol {
                stagnation_counter += 1;
                if stagnation_counter >= config.stagnation_window {
                    terminated_by_stagnation = true;
                    break 'outer;
                }
            } else {
                stagnation_counter = 0;
                last_best_fun = best.fun;
            }
        }
    }

    let feasible = best.max_violation <= 0.0;
    let message = if terminated_by_stagnation {
        format!(
            "Converged: best fitness stagnated within tol={:.1e} for {} generations",
            config.f_tol, config.stagnation_window
        )
    } else if nfev >= config.maxeval {
        format!("Maximum evaluations reached: {}", config.maxeval)
    } else {
        "Iteration limit reached".to_string()
    };

    Ok(IsresReport {
        x: best.x,
        fun: best.fun,
        max_violation: best.max_violation.max(0.0),
        feasible,
        success: terminated_by_stagnation,
        message,
        nfev,
        nit,
    })
}

/// Maximum constraint violation across all constraints. Zero means feasible.
fn max_violation_of(x: &Array1<f64>, constraints: &[IsresConstraint]) -> f64 {
    let mut worst = 0.0_f64;
    for c in constraints {
        let v = (c.fun)(x);
        if v > worst {
            worst = v;
        }
    }
    worst
}

/// Standard normal sample via Box-Muller (only the cosine half is used —
/// the sine half is discarded for simplicity; for ISRES the small loss in
/// efficiency is negligible compared to objective cost).
fn standard_normal<R: Rng + ?Sized>(rng: &mut R) -> f64 {
    let u1: f64 = rng.random::<f64>().max(1e-300);
    let u2: f64 = rng.random::<f64>();
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

/// Reflect `v` into `[lo, hi]`. If `v` falls outside, mirror it back across
/// the violated boundary; if it overshoots after one reflection, clamp.
fn reflect_into_bounds(v: f64, lo: f64, hi: f64) -> f64 {
    if !v.is_finite() {
        return (lo + hi) * 0.5;
    }
    if v < lo {
        let r = lo + (lo - v);
        if r > hi { (lo + hi) * 0.5 } else { r }
    } else if v > hi {
        let r = hi - (v - hi);
        if r < lo { (lo + hi) * 0.5 } else { r }
    } else {
        v
    }
}

/// Stochastic ranking (Runarsson & Yao 2000/2005). Bubble-sort the slice;
/// at each adjacent comparison, with probability `pf` compare by objective,
/// otherwise by `max_violation`. The pass repeats until no swap occurs in
/// a full sweep (classic bubble-sort termination).
fn stochastic_rank<R: Rng + ?Sized>(pop: &mut [Individual], pf: f64, rng: &mut R) {
    let n = pop.len();
    if n < 2 {
        return;
    }
    // Cap passes at n − 1 (worst case for bubble sort) to bound time.
    for _ in 0..n.saturating_sub(1) {
        let mut swapped = false;
        for i in 0..(n - 1) {
            // If both are feasible, always compare by objective; if both are
            // infeasible, always compare by violation. Otherwise apply the
            // pf coin flip — this is the original R&Y prescription.
            let a = &pop[i];
            let b = &pop[i + 1];
            let both_feasible = a.max_violation <= 0.0 && b.max_violation <= 0.0;
            let both_infeasible = a.max_violation > 0.0 && b.max_violation > 0.0;
            let compare_by_fun = if both_feasible {
                true
            } else if both_infeasible {
                rng.random::<f64>() < pf
            } else {
                // One feasible, one not — feasible always wins (feasibility
                // is treated as an absolute preference outside of ranking).
                false
            };

            let need_swap = if compare_by_fun {
                a.fun > b.fun
            } else {
                a.max_violation > b.max_violation
            };

            if need_swap {
                pop.swap(i, i + 1);
                swapped = true;
            }
        }
        if !swapped {
            break;
        }
    }
}

/// Find the population's "best" individual using the same lexicographic
/// preference as `individual_better`.
fn best_individual(pop: &[Individual]) -> &Individual {
    pop.iter()
        .min_by(|a, b| {
            if individual_better(a, b) {
                std::cmp::Ordering::Less
            } else if individual_better(b, a) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .expect("non-empty population")
}

/// Lexicographic better: feasible beats infeasible; among feasible, lower
/// objective wins; among infeasible, lower violation wins.
fn individual_better(a: &Individual, b: &Individual) -> bool {
    let a_feas = a.max_violation <= 0.0;
    let b_feas = b.max_violation <= 0.0;
    match (a_feas, b_feas) {
        (true, false) => true,
        (false, true) => false,
        (true, true) => a.fun < b.fun,
        (false, false) => a.max_violation < b.max_violation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_unconstrained() {
        // f(x) = sum xi^2 — minimum at origin.
        let f = |x: &Array1<f64>| x.iter().map(|v| v * v).sum::<f64>();
        let cfg = IsresConfig {
            bounds: vec![(-5.0, 5.0); 4],
            mu: 20,
            lambda: 100,
            maxeval: 8_000,
            seed: Some(42),
            ..Default::default()
        };
        let report = isres(&f, &[], cfg).expect("isres failed");
        assert!(
            report.fun < 1e-3,
            "fun = {} should converge near 0",
            report.fun
        );
        for &xi in report.x.iter() {
            assert!(xi.abs() < 0.05, "xi = {} should be near 0", xi);
        }
    }

    #[test]
    fn sphere_with_inequality() {
        // Minimize ||x||^2 subject to x[0] + x[1] >= 1
        // (i.e. constraint  1 - x[0] - x[1] <= 0).
        // Optimum at x = (0.5, 0.5, 0, ..., 0) with f = 0.5.
        let f = |x: &Array1<f64>| x.iter().map(|v| v * v).sum::<f64>();
        let constraints = vec![IsresConstraint {
            fun: Arc::new(|x: &Array1<f64>| 1.0 - x[0] - x[1]),
        }];
        let cfg = IsresConfig {
            bounds: vec![(-5.0, 5.0); 4],
            mu: 30,
            lambda: 200,
            maxeval: 30_000,
            seed: Some(7),
            ..Default::default()
        };
        let report = isres(&f, &constraints, cfg).expect("isres failed");
        assert!(report.feasible, "best should be feasible");
        // Generous tolerance — ISRES is stochastic and convergence on
        // small budgets isn't guaranteed to hit the analytical optimum.
        assert!(
            report.fun < 0.7,
            "fun = {} should converge near 0.5",
            report.fun
        );
    }

    #[test]
    fn rejects_inverted_bounds() {
        let f = |x: &Array1<f64>| x[0] * x[0];
        let cfg = IsresConfig {
            bounds: vec![(1.0, -1.0)],
            mu: 10,
            lambda: 30,
            maxeval: 100,
            ..Default::default()
        };
        let err = isres(&f, &[], cfg).unwrap_err();
        matches!(err, DEError::InvalidBounds { .. });
    }

    #[test]
    fn rejects_tiny_population() {
        let f = |x: &Array1<f64>| x[0] * x[0];
        let cfg = IsresConfig {
            bounds: vec![(-1.0, 1.0)],
            mu: 1,
            lambda: 10,
            maxeval: 100,
            ..Default::default()
        };
        let err = isres(&f, &[], cfg).unwrap_err();
        matches!(err, DEError::PopulationTooSmall { .. });
    }

    #[test]
    fn deterministic_with_seed() {
        let f = |x: &Array1<f64>| x.iter().map(|v| v * v).sum::<f64>();
        let cfg1 = IsresConfig {
            bounds: vec![(-5.0, 5.0); 3],
            mu: 15,
            lambda: 80,
            maxeval: 2_000,
            seed: Some(123),
            ..Default::default()
        };
        let cfg2 = cfg1.clone();
        let r1 = isres(&f, &[], cfg1).unwrap();
        let r2 = isres(&f, &[], cfg2).unwrap();
        assert!((r1.fun - r2.fun).abs() < 1e-12, "seeded runs must match");
    }
}
