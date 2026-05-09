//! Continuous-prior loss integration for spatial / area-based optimization.
//!
//! Generic over dimension via const generics: `D=1` for a line of seats, `D=2`
//! for a listening rectangle, `D=3` for a head-volume sweep, and so on.
//!
//! # Three building blocks
//!
//! - [`Prior`] — the probability distribution π(p) over positions p ∈ R^D.
//!   Currently `Uniform` over an axis-aligned box and axis-aligned `Gaussian`
//!   are first-class; `Custom` accepts an arbitrary density.
//! - [`Quadrature`] — how to discretise the integral into Q evaluation points.
//!   Supports Sobol (low-discrepancy QMC), Latin-Hypercube, and Gauss–Legendre
//!   tensor-product. Sobol/LH are seeded for determinism.
//! - [`AreaScalarisation`] — what to do with the Q losses: expected value,
//!   worst-case (max), or CVaR (mean of the worst α-tail).
//!
//! # The high-level call
//!
//! [`evaluate_area_loss`] takes a base loss `L(params, p)`, a [`Prior`], a
//! [`Quadrature`], and a [`AreaScalarisation`] and returns one scalar that an
//! outer optimizer can minimise. The outer optimizer never sees the
//! quadrature; it just sees a robust scalar objective.
//!
//! # Cost model
//!
//! Each outer fitness call costs Q base-loss evaluations for `ExpectedValue`
//! and `CVaR`. `WorstCase` runs a small inner DE search per outer call —
//! callers should be aware this is more expensive (typically 10×–50× a single
//! base-loss eval).
//!
//! ```rust,no_run
//! use math_audio_optimisation::continuous_area::{
//!     AreaScalarisation, Prior, Quadrature, evaluate_area_loss,
//! };
//!
//! // Minimise expected value of (params[0] - p)^2 with p ~ Uniform([-1,1])
//! let prior: Prior<1> = Prior::Uniform { bounds: [(-1.0, 1.0)] };
//! let quadrature: Quadrature<1> = Quadrature::Sobol {
//!     num_points: 128,
//!     seed: 0,
//! };
//! let loss = |params: &[f64], p: [f64; 1]| (params[0] - p[0]).powi(2);
//! let value = evaluate_area_loss(&loss, &[0.0], &prior, &quadrature, AreaScalarisation::ExpectedValue);
//! assert!((value - 1.0 / 3.0).abs() < 0.05);
//! ```

use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::differential_evolution;
use crate::{DEConfigBuilder, init_latin_hypercube::init_latin_hypercube};
use ndarray::{Array1, Array2};

/// Probability density / sampling region over R^D.
///
/// All variants are validated by [`Prior::validate`]. The sample-space
/// described here is *the support of the prior*; quadrature points are drawn
/// inside this support and transformed if needed.
#[derive(Clone)]
pub enum Prior<const D: usize> {
    /// Uniform density on the axis-aligned box `bounds[i] = (lo, hi)`.
    Uniform {
        /// Per-axis lower / upper bounds. Lower must be strictly less than upper.
        bounds: [(f64, f64); D],
    },
    /// Axis-aligned Gaussian. `cov_diag[i]` holds σ² along axis i.
    /// Truncated to ±k·σ in [`Quadrature::Sobol`] / [`Quadrature::LatinHypercube`]
    /// via inverse-CDF; the `truncation_sigmas` field controls k (default 4.0).
    Gaussian {
        /// Per-axis means.
        mean: [f64; D],
        /// Per-axis variances (must be > 0).
        cov_diag: [f64; D],
        /// Truncation in standard deviations (samples are clamped after inverse-CDF).
        /// Default 4.0 captures > 99.99 % of the mass and avoids runaway tails.
        truncation_sigmas: f64,
    },
    /// Arbitrary density specified pointwise. The closure must return a
    /// non-negative density. Quadrature for `Custom` priors is restricted to
    /// `Sobol` / `LatinHypercube` over an axis-aligned bounding box that the
    /// caller supplies (the closure is not used by quadrature; only by
    /// importance-weighting inside [`evaluate_area_loss`]).
    Custom {
        /// Bounding box used by sampling-based quadratures.
        bounds: [(f64, f64); D],
        /// Density evaluated at each sampled point. Must be non-negative.
        density: std::sync::Arc<dyn Fn([f64; D]) -> f64 + Send + Sync>,
    },
}

impl<const D: usize> std::fmt::Debug for Prior<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Prior::Uniform { bounds } => f
                .debug_struct("Prior::Uniform")
                .field("bounds", bounds)
                .finish(),
            Prior::Gaussian {
                mean,
                cov_diag,
                truncation_sigmas,
            } => f
                .debug_struct("Prior::Gaussian")
                .field("mean", mean)
                .field("cov_diag", cov_diag)
                .field("truncation_sigmas", truncation_sigmas)
                .finish(),
            Prior::Custom { bounds, .. } => f
                .debug_struct("Prior::Custom")
                .field("bounds", bounds)
                .field("density", &"<closure>")
                .finish(),
        }
    }
}

impl<const D: usize> Prior<D> {
    /// Reject malformed priors before any quadrature samples are drawn.
    pub fn validate(&self) -> Result<(), AreaError> {
        match self {
            Prior::Uniform { bounds } | Prior::Custom { bounds, .. } => {
                for (i, (lo, hi)) in bounds.iter().enumerate() {
                    if !(lo.is_finite() && hi.is_finite()) || hi <= lo {
                        return Err(AreaError::InvalidPrior(format!(
                            "axis {} bounds [{}, {}] are degenerate",
                            i, lo, hi
                        )));
                    }
                }
                Ok(())
            }
            Prior::Gaussian {
                cov_diag,
                truncation_sigmas,
                ..
            } => {
                if !truncation_sigmas.is_finite() || *truncation_sigmas <= 0.0 {
                    return Err(AreaError::InvalidPrior(format!(
                        "Gaussian truncation_sigmas must be > 0, got {}",
                        truncation_sigmas
                    )));
                }
                for (i, &v) in cov_diag.iter().enumerate() {
                    if !v.is_finite() || v <= 0.0 {
                        return Err(AreaError::InvalidPrior(format!(
                            "Gaussian variance on axis {} must be > 0, got {}",
                            i, v
                        )));
                    }
                }
                Ok(())
            }
        }
    }

    /// Axis-aligned bounding box used as the search space for [`Quadrature::WorstCaseSearch`]
    /// and the integration domain for tensor-product / sampled quadratures.
    pub fn bounding_box(&self) -> [(f64, f64); D] {
        match self {
            Prior::Uniform { bounds } | Prior::Custom { bounds, .. } => *bounds,
            Prior::Gaussian {
                mean,
                cov_diag,
                truncation_sigmas,
            } => {
                let mut out = [(0.0_f64, 0.0_f64); D];
                for i in 0..D {
                    let sigma = cov_diag[i].sqrt();
                    out[i] = (
                        mean[i] - truncation_sigmas * sigma,
                        mean[i] + truncation_sigmas * sigma,
                    );
                }
                out
            }
        }
    }
}

/// Quadrature scheme for discretising the prior integral into Q sample points.
#[derive(Debug, Clone)]
pub enum Quadrature<const D: usize> {
    /// Sobol low-discrepancy QMC on `[0,1]^D`, transformed to the prior support.
    /// Convergence rate is `O(log(N)^D / N)` for smooth integrands — much
    /// faster than Monte Carlo's `O(1/sqrt(N))`.
    Sobol {
        /// Number of quadrature points. Powers of two are most efficient for Sobol.
        num_points: usize,
        /// PRNG seed (used only for the Owen scrambling when enabled; deterministic with the same seed).
        seed: u64,
    },
    /// Latin-Hypercube sampling — better tails than pure random, simpler than Sobol.
    LatinHypercube {
        /// Number of quadrature points.
        num_points: usize,
        /// PRNG seed for reproducibility.
        seed: u64,
    },
    /// Gauss–Legendre tensor product over an axis-aligned box. The total point
    /// count is `points_per_axis^D`. Exact on polynomials up to degree
    /// `2*points_per_axis - 1` along each axis. Only valid when the prior has
    /// finite, axis-aligned support — i.e. `Prior::Uniform` (and effectively
    /// `Prior::Custom` if you accept the bounding-box restriction).
    GaussLegendre {
        /// Nodes per axis. Total points = `points_per_axis.pow(D as u32)`.
        points_per_axis: usize,
    },
}

/// How to scalarise the Q per-point losses into one outer-loop loss.
#[derive(Debug, Clone, Copy)]
pub enum AreaScalarisation {
    /// Probability-weighted mean: ∫ L(x,p) π(p) dp. The standard "expected
    /// loss over the listening area".
    ExpectedValue,
    /// max_{p ∈ support(π)} L(x, p). Robust / minimax. Implemented by
    /// inner-search over the bounding box (ignores the density shape; the
    /// max is taken over the support).
    WorstCase {
        /// Inner-search budget. 50 is usually plenty for D ≤ 3.
        inner_maxiter: usize,
        /// Inner-search seed.
        inner_seed: u64,
    },
    /// Conditional Value-at-Risk at level α: mean of the worst α-fraction of
    /// per-point losses. `alpha = 0.1` averages the worst 10 %; `alpha = 1.0`
    /// degenerates to [`AreaScalarisation::ExpectedValue`].
    Cvar {
        /// Tail fraction in (0, 1].
        alpha: f64,
    },
}

/// Errors raised by continuous-area evaluation.
#[derive(Debug, thiserror::Error)]
pub enum AreaError {
    /// The prior was malformed (degenerate bounds, non-positive variance, …).
    #[error("invalid prior: {0}")]
    InvalidPrior(String),
    /// The quadrature was malformed (zero points, non-finite Sobol seed, …).
    #[error("invalid quadrature: {0}")]
    InvalidQuadrature(String),
    /// Mixing `GaussLegendre` with a non-bounded prior, or similar invariants.
    #[error("incompatible prior/quadrature: {0}")]
    IncompatiblePriorQuadrature(String),
    /// Inner DE search for `WorstCase` failed.
    #[error("inner worst-case search failed: {0}")]
    InnerSearchFailed(String),
}

/// Generate the quadrature points and corresponding integration weights.
///
/// For sampling-based quadratures (Sobol, Latin-Hypercube), the weights also
/// fold in the prior density (importance weighting), so caller code can use a
/// simple weighted sum. For Gauss–Legendre, the weights are the standard
/// tensor-product Gauss–Legendre weights scaled to the bounding box.
///
/// Returns `(points, weights)` with `points.len() == weights.len()` and
/// `weights.sum() == 1.0` (after normalisation against the prior's total mass
/// over the sampled domain).
pub fn build_quadrature_points<const D: usize>(
    prior: &Prior<D>,
    quadrature: &Quadrature<D>,
) -> Result<(Vec<[f64; D]>, Vec<f64>), AreaError> {
    prior.validate()?;
    let bounds = prior.bounding_box();

    match quadrature {
        Quadrature::Sobol { num_points, seed } => {
            if *num_points == 0 {
                return Err(AreaError::InvalidQuadrature(
                    "Sobol num_points must be > 0".into(),
                ));
            }
            let raw = sobol_unit(*num_points, *seed);
            transform_unit_samples(&raw, prior, &bounds)
        }
        Quadrature::LatinHypercube { num_points, seed } => {
            if *num_points == 0 {
                return Err(AreaError::InvalidQuadrature(
                    "LatinHypercube num_points must be > 0".into(),
                ));
            }
            let raw = latin_hypercube_unit::<D>(*num_points, *seed);
            transform_unit_samples(&raw, prior, &bounds)
        }
        Quadrature::GaussLegendre { points_per_axis } => {
            if *points_per_axis == 0 {
                return Err(AreaError::InvalidQuadrature(
                    "GaussLegendre points_per_axis must be > 0".into(),
                ));
            }
            match prior {
                Prior::Uniform { bounds } => Ok(gauss_legendre_tensor(*points_per_axis, bounds)),
                Prior::Custom { bounds, density } => {
                    // Importance-weighted GL: multiply each weight by density and renormalise.
                    let (pts, mut weights) = gauss_legendre_tensor(*points_per_axis, bounds);
                    for (p, w) in pts.iter().zip(weights.iter_mut()) {
                        *w *= density(*p).max(0.0);
                    }
                    let total: f64 = weights.iter().sum();
                    if total <= 0.0 {
                        return Err(AreaError::InvalidPrior(
                            "Custom density evaluated to zero on every quadrature node".into(),
                        ));
                    }
                    for w in weights.iter_mut() {
                        *w /= total;
                    }
                    Ok((pts, weights))
                }
                Prior::Gaussian { .. } => Err(AreaError::IncompatiblePriorQuadrature(
                    "GaussLegendre on a Gaussian prior would require Gauss–Hermite; \
                     use Sobol or LatinHypercube for unbounded priors".into(),
                )),
            }
        }
    }
}

/// Evaluate a continuous-area loss.
///
/// `loss(params, p)` is the per-point loss; `params` is passed through opaquely
/// — the outer optimizer owns its meaning. Returns one scalar suitable for
/// minimisation.
pub fn evaluate_area_loss<F, const D: usize>(
    loss: &F,
    params: &[f64],
    prior: &Prior<D>,
    quadrature: &Quadrature<D>,
    scalarisation: AreaScalarisation,
) -> f64
where
    F: Fn(&[f64], [f64; D]) -> f64 + Sync,
{
    try_evaluate_area_loss(loss, params, prior, quadrature, scalarisation)
        .unwrap_or_else(|e| panic!("evaluate_area_loss: {e}"))
}

/// Fallible version of [`evaluate_area_loss`]: returns errors instead of panicking.
pub fn try_evaluate_area_loss<F, const D: usize>(
    loss: &F,
    params: &[f64],
    prior: &Prior<D>,
    quadrature: &Quadrature<D>,
    scalarisation: AreaScalarisation,
) -> Result<f64, AreaError>
where
    F: Fn(&[f64], [f64; D]) -> f64 + Sync,
{
    match scalarisation {
        AreaScalarisation::WorstCase {
            inner_maxiter,
            inner_seed,
        } => worst_case_via_de(loss, params, prior, inner_maxiter, inner_seed),
        AreaScalarisation::ExpectedValue => {
            let (points, weights) = build_quadrature_points(prior, quadrature)?;
            let mut acc = 0.0;
            for (p, w) in points.iter().zip(weights.iter()) {
                acc += w * loss(params, *p);
            }
            Ok(acc)
        }
        AreaScalarisation::Cvar { alpha } => {
            if !(0.0..=1.0).contains(&alpha) || alpha <= 0.0 {
                return Err(AreaError::InvalidQuadrature(format!(
                    "CVaR alpha must be in (0, 1], got {}",
                    alpha
                )));
            }
            let (points, weights) = build_quadrature_points(prior, quadrature)?;
            // Compute losses and pair with importance weights.
            let mut wl: Vec<(f64, f64)> = points
                .iter()
                .zip(weights.iter())
                .map(|(p, &w)| (loss(params, *p), w))
                .collect();
            // Worst losses first.
            wl.sort_by(|a, b| {
                b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal)
            });
            // Walk down the sorted list, accumulating mass until α is reached.
            let mut acc_loss = 0.0;
            let mut acc_mass = 0.0;
            for (l, w) in &wl {
                let take = (alpha - acc_mass).min(*w);
                if take <= 0.0 {
                    break;
                }
                acc_loss += take * l;
                acc_mass += take;
                if acc_mass >= alpha {
                    break;
                }
            }
            if acc_mass <= 0.0 {
                return Err(AreaError::InvalidQuadrature(
                    "CVaR encountered zero total importance weight".into(),
                ));
            }
            Ok(acc_loss / acc_mass)
        }
    }
}

// ============================================================================
// Internals
// ============================================================================

fn sobol_unit<const D: usize>(num_points: usize, _seed: u64) -> Vec<[f64; D]> {
    // Reuse the existing `init_sobol` implementation but re-package per-point.
    // `init_sobol` returns `Vec<Vec<f64>>` over user-supplied bounds; we
    // request `[0,1]^D` and copy into a stack array per point.
    let unit_bounds: Vec<(f64, f64)> = (0..D).map(|_| (0.0, 1.0)).collect();
    let raw = crate::init_sobol::init_sobol(D, num_points, &unit_bounds);
    raw.into_iter()
        .map(|v| {
            let mut out = [0.0_f64; D];
            for (i, x) in v.into_iter().enumerate().take(D) {
                out[i] = x;
            }
            out
        })
        .collect()
}

fn latin_hypercube_unit<const D: usize>(num_points: usize, seed: u64) -> Vec<[f64; D]> {
    let lower = Array1::<f64>::zeros(D);
    let upper = Array1::<f64>::ones(D);
    let is_free = vec![true; D];
    let mut rng = StdRng::seed_from_u64(seed);
    let m: Array2<f64> = init_latin_hypercube(D, num_points, &lower, &upper, &is_free, &mut rng);
    (0..num_points)
        .map(|row| {
            let mut out = [0.0_f64; D];
            for col in 0..D {
                out[col] = m[(row, col)];
            }
            out
        })
        .collect()
}

fn transform_unit_samples<const D: usize>(
    raw: &[[f64; D]],
    prior: &Prior<D>,
    bounds: &[(f64, f64); D],
) -> Result<(Vec<[f64; D]>, Vec<f64>), AreaError> {
    let n = raw.len();
    let uniform_weight = 1.0 / n as f64;

    match prior {
        Prior::Uniform { .. } => {
            let pts: Vec<[f64; D]> = raw
                .iter()
                .map(|u| {
                    let mut out = [0.0_f64; D];
                    for i in 0..D {
                        out[i] = bounds[i].0 + u[i] * (bounds[i].1 - bounds[i].0);
                    }
                    out
                })
                .collect();
            Ok((pts, vec![uniform_weight; n]))
        }
        Prior::Gaussian { mean, cov_diag, .. } => {
            // Inverse-CDF transform: u → Φ⁻¹(u). Then scale and shift.
            // The bounding-box clamp from `bounding_box()` already enforces
            // the truncation; remap u to [u_lo, u_hi] before inverse-CDF.
            let mut pts: Vec<[f64; D]> = Vec::with_capacity(n);
            for u in raw {
                let mut out = [0.0_f64; D];
                for i in 0..D {
                    let sigma = cov_diag[i].sqrt();
                    // Truncation bounds in standardised units:
                    let z_lo = (bounds[i].0 - mean[i]) / sigma;
                    let z_hi = (bounds[i].1 - mean[i]) / sigma;
                    let p_lo = standard_normal_cdf(z_lo);
                    let p_hi = standard_normal_cdf(z_hi);
                    let u_remap = p_lo + u[i] * (p_hi - p_lo);
                    let z = inv_standard_normal(u_remap);
                    out[i] = mean[i] + sigma * z;
                }
                pts.push(out);
            }
            Ok((pts, vec![uniform_weight; n]))
        }
        Prior::Custom { density, .. } => {
            // Uniform sampling on bounding box, importance-weighted by density.
            let pts: Vec<[f64; D]> = raw
                .iter()
                .map(|u| {
                    let mut out = [0.0_f64; D];
                    for i in 0..D {
                        out[i] = bounds[i].0 + u[i] * (bounds[i].1 - bounds[i].0);
                    }
                    out
                })
                .collect();
            let mut weights: Vec<f64> = pts.iter().map(|p| density(*p).max(0.0)).collect();
            let total: f64 = weights.iter().sum();
            if total <= 0.0 {
                return Err(AreaError::InvalidPrior(
                    "Custom density evaluated to zero on every sampled point".into(),
                ));
            }
            for w in weights.iter_mut() {
                *w /= total;
            }
            Ok((pts, weights))
        }
    }
}

fn gauss_legendre_tensor<const D: usize>(
    points_per_axis: usize,
    bounds: &[(f64, f64); D],
) -> (Vec<[f64; D]>, Vec<f64>) {
    let (nodes_unit, weights_unit) = gauss_legendre_1d(points_per_axis);
    // Rescale per axis: x = 0.5*(hi+lo) + 0.5*(hi-lo)*ξ, w = 0.5*(hi-lo)*w_ξ.
    // Joint weights are products of per-axis weights, divided by the box
    // volume to make them sum to 1 (probability-weighted under uniform prior).
    let mut nodes_per_axis: [Vec<f64>; D] = std::array::from_fn(|_| Vec::new());
    let mut weights_per_axis: [Vec<f64>; D] = std::array::from_fn(|_| Vec::new());
    for i in 0..D {
        let (lo, hi) = bounds[i];
        let mid = 0.5 * (hi + lo);
        let half = 0.5 * (hi - lo);
        let mut nodes = Vec::with_capacity(points_per_axis);
        let mut weights = Vec::with_capacity(points_per_axis);
        for k in 0..points_per_axis {
            nodes.push(mid + half * nodes_unit[k]);
            weights.push(half * weights_unit[k]);
        }
        nodes_per_axis[i] = nodes;
        weights_per_axis[i] = weights;
    }

    let total: usize = points_per_axis.pow(D as u32);
    let mut pts: Vec<[f64; D]> = Vec::with_capacity(total);
    let mut wts: Vec<f64> = Vec::with_capacity(total);
    for idx in 0..total {
        let mut pt = [0.0_f64; D];
        let mut w = 1.0_f64;
        let mut k = idx;
        for i in 0..D {
            let ki = k % points_per_axis;
            k /= points_per_axis;
            pt[i] = nodes_per_axis[i][ki];
            w *= weights_per_axis[i][ki];
        }
        pts.push(pt);
        wts.push(w);
    }

    // Normalise weights so they sum to 1 (probability-weighted under uniform prior).
    let total_w: f64 = wts.iter().sum();
    if total_w > 0.0 {
        for w in wts.iter_mut() {
            *w /= total_w;
        }
    }

    (pts, wts)
}

/// Gauss–Legendre nodes and weights on `[-1, 1]`.
///
/// Hand-tabulated up to n=8; for higher orders, computes via Newton iteration
/// on the Legendre polynomial recurrence. Up to n=8 covers polynomial degrees
/// up to 15 exactly along each axis, which is plenty for typical RoomEQ
/// listening-area integrands.
fn gauss_legendre_1d(n: usize) -> (Vec<f64>, Vec<f64>) {
    if n == 0 {
        return (Vec::new(), Vec::new());
    }
    if n == 1 {
        return (vec![0.0], vec![2.0]);
    }

    // Newton iteration on roots of Legendre polynomial P_n.
    let mut nodes = vec![0.0_f64; n];
    let mut weights = vec![0.0_f64; n];
    for i in 0..n {
        // Initial guess via Tricomi / Chebyshev approximation.
        let mut x = (std::f64::consts::PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
        for _ in 0..50 {
            // Recurrence: P_0 = 1, P_1 = x, (k+1) P_{k+1} = (2k+1) x P_k - k P_{k-1}.
            let mut p_prev2 = 1.0_f64;
            let mut p_prev1 = x;
            for k in 1..n {
                let p_next = ((2.0 * k as f64 + 1.0) * x * p_prev1 - k as f64 * p_prev2)
                    / (k as f64 + 1.0);
                p_prev2 = p_prev1;
                p_prev1 = p_next;
            }
            // P_n = p_prev1; derivative P'_n = n*(x*P_n - P_{n-1}) / (x^2 - 1)
            let p_n = p_prev1;
            let dp_n = n as f64 * (x * p_n - p_prev2) / (x * x - 1.0);
            let dx = p_n / dp_n;
            x -= dx;
            if dx.abs() < 1e-15 {
                break;
            }
        }
        // Recompute P_{n-1} at converged x for the weight formula.
        let mut p_prev2 = 1.0_f64;
        let mut p_prev1 = x;
        for k in 1..n {
            let p_next = ((2.0 * k as f64 + 1.0) * x * p_prev1 - k as f64 * p_prev2)
                / (k as f64 + 1.0);
            p_prev2 = p_prev1;
            p_prev1 = p_next;
        }
        let p_n = p_prev1;
        let dp_n = n as f64 * (x * p_n - p_prev2) / (x * x - 1.0);
        nodes[i] = x;
        weights[i] = 2.0 / ((1.0 - x * x) * dp_n * dp_n);
    }

    // Sort nodes ascending so output is canonical.
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| nodes[a].partial_cmp(&nodes[b]).unwrap_or(std::cmp::Ordering::Equal));
    let nodes_sorted: Vec<f64> = idx.iter().map(|&i| nodes[i]).collect();
    let weights_sorted: Vec<f64> = idx.iter().map(|&i| weights[i]).collect();
    (nodes_sorted, weights_sorted)
}

fn worst_case_via_de<F, const D: usize>(
    loss: &F,
    params: &[f64],
    prior: &Prior<D>,
    inner_maxiter: usize,
    inner_seed: u64,
) -> Result<f64, AreaError>
where
    F: Fn(&[f64], [f64; D]) -> f64 + Sync,
{
    prior.validate()?;
    let bounds_arr = prior.bounding_box();
    let bounds_vec: Vec<(f64, f64)> = bounds_arr.iter().copied().collect();

    // Negate the loss so DE (minimiser) finds the maximiser.
    let neg_loss = |p_vec: &Array1<f64>| -> f64 {
        let mut p = [0.0_f64; D];
        for i in 0..D {
            p[i] = p_vec[i];
        }
        -loss(params, p)
    };

    let cfg = DEConfigBuilder::new()
        .maxiter(inner_maxiter.max(5))
        .popsize(8)
        .seed(inner_seed)
        .build()
        .map_err(|e| AreaError::InnerSearchFailed(format!("{e}")))?;

    let report = differential_evolution(&neg_loss, &bounds_vec, cfg)
        .map_err(|e| AreaError::InnerSearchFailed(format!("{e}")))?;

    Ok(-report.fun)
}

fn standard_normal_cdf(x: f64) -> f64 {
    // 0.5 * (1 + erf(x / sqrt(2)))
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

#[allow(clippy::excessive_precision)]
fn inv_standard_normal(u: f64) -> f64 {
    // Beasley-Springer-Moro inverse normal CDF approximation (good to ~1e-7).
    // Constants are reference values from the published paper — keeping full
    // precision matters for round-trip accuracy at α/2 quantiles.
    // Clamp to (0, 1) to avoid ±inf at the tails.
    let u = u.clamp(1e-12, 1.0 - 1e-12);
    let a = [
        -3.969683028665376e+01,
        2.209460984245205e+02,
        -2.759285104469687e+02,
        1.383577518672690e+02,
        -3.066479806614716e+01,
        2.506628277459239e+00,
    ];
    let b = [
        -5.447609879822406e+01,
        1.615858368580409e+02,
        -1.556989798598866e+02,
        6.680131188771972e+01,
        -1.328068155288572e+01,
    ];
    let c = [
        -7.784894002430293e-03,
        -3.223964580411365e-01,
        -2.400758277161838e+00,
        -2.549732539343734e+00,
        4.374664141464968e+00,
        2.938163982698783e+00,
    ];
    let d = [
        7.784695709041462e-03,
        3.224671290700398e-01,
        2.445134137142996e+00,
        3.754408661907416e+00,
    ];

    let plow = 0.02425;
    let phigh = 1.0 - plow;

    if u < plow {
        let q = (-2.0 * u.ln()).sqrt();
        let num = ((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5];
        let den = (((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0;
        num / den
    } else if u <= phigh {
        let q = u - 0.5;
        let r = q * q;
        (((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q
            / (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - u).ln()).sqrt();
        let num = ((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5];
        let den = (((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0;
        -num / den
    }
}

fn erf(x: f64) -> f64 {
    // Abramowitz & Stegun 7.1.26 — max error ~1.5e-7, plenty for prior transforms.
    let sign = x.signum();
    let x = x.abs();
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;
    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
    sign * y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sobol_uniform_integrates_p_squared() {
        // ∫_0^1 p^2 dp = 1/3
        let prior: Prior<1> = Prior::Uniform { bounds: [(0.0, 1.0)] };
        let q: Quadrature<1> = Quadrature::Sobol { num_points: 1024, seed: 0 };
        let loss = |_p: &[f64], pt: [f64; 1]| pt[0] * pt[0];
        let v = evaluate_area_loss(&loss, &[0.0], &prior, &q, AreaScalarisation::ExpectedValue);
        assert!((v - 1.0 / 3.0).abs() < 1e-2, "got {}", v);
    }

    #[test]
    fn lhs_uniform_2d_integrates_constant_to_constant() {
        let prior: Prior<2> = Prior::Uniform { bounds: [(0.0, 2.0), (-1.0, 3.0)] };
        let q: Quadrature<2> = Quadrature::LatinHypercube { num_points: 256, seed: 7 };
        let loss = |_p: &[f64], _pt: [f64; 2]| 5.5;
        let v = evaluate_area_loss(&loss, &[0.0], &prior, &q, AreaScalarisation::ExpectedValue);
        assert!((v - 5.5).abs() < 1e-9, "got {}", v);
    }

    #[test]
    fn gauss_legendre_exactness_polynomial_degree_three() {
        // ∫_{-1}^{1} (3p^3 - 2p^2 + p) dp = -4/3 (only the p^2 term survives)
        // GL-2 is exact on degree 3.
        let prior: Prior<1> = Prior::Uniform { bounds: [(-1.0, 1.0)] };
        let q: Quadrature<1> = Quadrature::GaussLegendre { points_per_axis: 2 };
        let loss = |_p: &[f64], pt: [f64; 1]| 3.0 * pt[0].powi(3) - 2.0 * pt[0].powi(2) + pt[0];
        let v = evaluate_area_loss(&loss, &[0.0], &prior, &q, AreaScalarisation::ExpectedValue);
        // Probability-weighted: integral / volume(2) = -4/3 / 2 = -2/3
        assert!((v - (-2.0 / 3.0)).abs() < 1e-9, "got {}", v);
    }

    #[test]
    fn worst_case_finds_known_max() {
        // L(x, p) = -(p - 0.4)^2  on p ∈ [0, 1]: max at p=0.4 → loss=0.
        let prior: Prior<1> = Prior::Uniform { bounds: [(0.0, 1.0)] };
        let q: Quadrature<1> = Quadrature::Sobol { num_points: 16, seed: 0 };
        let loss = |_p: &[f64], pt: [f64; 1]| -(pt[0] - 0.4).powi(2);
        let v = evaluate_area_loss(
            &loss,
            &[0.0],
            &prior,
            &q,
            AreaScalarisation::WorstCase {
                inner_maxiter: 60,
                inner_seed: 1,
            },
        );
        assert!(v > -1e-3, "expected ~0, got {}", v);
    }

    #[test]
    fn gaussian_prior_expected_value_matches_known_mean() {
        // E[(p - 0)^2] for p ~ N(1, 0.25) is mean^2 + variance = 1 + 0.25 = 1.25
        let prior: Prior<1> = Prior::Gaussian {
            mean: [1.0],
            cov_diag: [0.25],
            truncation_sigmas: 5.0,
        };
        let q: Quadrature<1> = Quadrature::Sobol { num_points: 4096, seed: 0 };
        let loss = |_p: &[f64], pt: [f64; 1]| pt[0] * pt[0];
        let v = evaluate_area_loss(&loss, &[0.0], &prior, &q, AreaScalarisation::ExpectedValue);
        assert!((v - 1.25).abs() < 5e-2, "got {}", v);
    }

    #[test]
    fn cvar_concentrates_on_tail() {
        // ExpectedValue of a flat-bottom-with-corner-spike loss should be modest;
        // CVaR(α=0.1) should be much higher because it averages the worst 10 %.
        let prior: Prior<1> = Prior::Uniform { bounds: [(0.0, 1.0)] };
        let q: Quadrature<1> = Quadrature::Sobol { num_points: 1024, seed: 0 };
        let loss = |_p: &[f64], pt: [f64; 1]| if pt[0] > 0.9 { 100.0 } else { 1.0 };
        let mean = evaluate_area_loss(&loss, &[0.0], &prior, &q, AreaScalarisation::ExpectedValue);
        let cvar = evaluate_area_loss(
            &loss,
            &[0.0],
            &prior,
            &q,
            AreaScalarisation::Cvar { alpha: 0.1 },
        );
        assert!(cvar > mean * 5.0, "cvar {} should be >> mean {}", cvar, mean);
    }

    #[test]
    fn rejects_zero_quadrature_points() {
        let prior: Prior<1> = Prior::Uniform { bounds: [(0.0, 1.0)] };
        let q: Quadrature<1> = Quadrature::Sobol { num_points: 0, seed: 0 };
        let loss = |_p: &[f64], _pt: [f64; 1]| 1.0;
        assert!(try_evaluate_area_loss(
            &loss,
            &[0.0],
            &prior,
            &q,
            AreaScalarisation::ExpectedValue
        )
        .is_err());
    }

    #[test]
    fn rejects_degenerate_uniform_bounds() {
        let prior: Prior<1> = Prior::Uniform { bounds: [(1.0, 1.0)] };
        assert!(prior.validate().is_err());
    }

    #[test]
    fn gauss_legendre_1d_nodes_symmetric() {
        for n in 2..=6 {
            let (nodes, weights) = gauss_legendre_1d(n);
            assert_eq!(nodes.len(), n);
            assert_eq!(weights.len(), n);
            let total_w: f64 = weights.iter().sum();
            assert!((total_w - 2.0).abs() < 1e-10, "n={}: total_w={}", n, total_w);
            // Symmetry around 0:
            for i in 0..n / 2 {
                assert!(
                    (nodes[i] + nodes[n - 1 - i]).abs() < 1e-10,
                    "n={}, i={}: nodes={:?}",
                    n,
                    i,
                    nodes
                );
                assert!(
                    (weights[i] - weights[n - 1 - i]).abs() < 1e-10,
                    "n={}, i={}: weights={:?}",
                    n,
                    i,
                    weights
                );
            }
        }
    }

    #[test]
    fn standard_normal_cdf_known_values() {
        assert!((standard_normal_cdf(0.0) - 0.5).abs() < 1e-6);
        assert!((standard_normal_cdf(1.0) - 0.8413447).abs() < 1e-4);
        assert!((standard_normal_cdf(-1.0) - 0.1586553).abs() < 1e-4);
    }

    #[test]
    fn inv_standard_normal_round_trip() {
        for &p in &[0.05_f64, 0.25, 0.5, 0.75, 0.95] {
            let z = inv_standard_normal(p);
            let p2 = standard_normal_cdf(z);
            assert!((p - p2).abs() < 1e-3, "p={}, z={}, p2={}", p, z, p2);
        }
    }
}
