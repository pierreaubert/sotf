//! Covariance Matrix Adaptation Evolution Strategy (CMA-ES).
//!
//! This is a pure-Rust, bounded, full-covariance CMA-ES implementation for
//! continuous black-box minimisation. Internally the search runs in a
//! normalised `[0, 1]^n` box so parameter scales such as log-frequency, Q, and
//! gain can share one covariance matrix without manual preconditioning.

use nalgebra::{DMatrix, DVector, SymmetricEigen};
use ndarray::Array1;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;

use crate::CallbackAction;
use crate::error::{DEError, Result};
use crate::parallel_eval::ParallelConfig;

/// Per-generation callback payload for [`cma_es`].
pub struct CmaEsIntermediate {
    /// Current best parameter vector in the original bounded coordinates.
    pub x: Array1<f64>,
    /// Current best objective value.
    pub fun: f64,
    /// Current generation index.
    pub iter: usize,
    /// Number of objective evaluations consumed so far.
    pub nfev: usize,
    /// Current global step size in normalised coordinates.
    pub sigma: f64,
}

/// Callback type used by [`CmaEsConfig`].
pub type CmaEsCallback = Box<dyn FnMut(&CmaEsIntermediate) -> CallbackAction + Send>;

/// Configuration for [`cma_es`].
pub struct CmaEsConfig {
    /// `(lower, upper)` bounds per parameter.
    pub bounds: Vec<(f64, f64)>,
    /// Optional initial mean. Values outside bounds are clipped.
    pub x0: Option<Array1<f64>>,
    /// Initial step size in normalised `[0, 1]` coordinates.
    ///
    /// `None` uses `0.3`, the standard broad-search default for bounded
    /// CMA-ES. Smaller values are appropriate for local refinement.
    pub sigma0: Option<f64>,
    /// Offspring population size. `0` uses `4 + floor(3 ln(n))`.
    pub lambda: usize,
    /// Parent count. `0` uses `lambda / 2`.
    pub mu: usize,
    /// Maximum objective evaluations.
    pub maxeval: usize,
    /// Optional RNG seed for deterministic runs.
    pub seed: Option<u64>,
    /// Stop after this many generations with improvement below [`Self::f_tol`].
    pub stagnation_window: usize,
    /// Objective-improvement tolerance for stagnation detection.
    pub f_tol: f64,
    /// Stop once the best objective is at or below this value.
    pub target_f: f64,
    /// Optional per-generation callback. Returning [`CallbackAction::Stop`]
    /// terminates the run early and returns the best point seen so far.
    pub callback: Option<CmaEsCallback>,
    /// Parallel evaluation configuration for offspring fitness calls.
    pub parallel: ParallelConfig,
}

impl Default for CmaEsConfig {
    fn default() -> Self {
        Self {
            bounds: Vec::new(),
            x0: None,
            sigma0: None,
            lambda: 0,
            mu: 0,
            maxeval: 10_000,
            seed: None,
            stagnation_window: 80,
            f_tol: 1e-10,
            target_f: f64::NEG_INFINITY,
            callback: None,
            parallel: ParallelConfig::default(),
        }
    }
}

/// Result of a [`cma_es`] run.
#[derive(Clone)]
pub struct CmaEsReport {
    /// Best parameter vector found.
    pub x: Array1<f64>,
    /// Objective value at [`Self::x`].
    pub fun: f64,
    /// Whether the run met a convergence/target/callback stop condition before
    /// exhausting the evaluation budget.
    pub success: bool,
    /// Human-readable termination message.
    pub message: String,
    /// Objective evaluations consumed.
    pub nfev: usize,
    /// Generations completed.
    pub nit: usize,
    /// Final global step size in normalised coordinates.
    pub sigma: f64,
}

impl std::fmt::Debug for CmaEsReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CmaEsReport")
            .field("x_len", &self.x.len())
            .field("fun", &self.fun)
            .field("success", &self.success)
            .field("message", &self.message)
            .field("nfev", &self.nfev)
            .field("nit", &self.nit)
            .field("sigma", &self.sigma)
            .finish()
    }
}

#[derive(Clone)]
struct Candidate {
    y: DVector<f64>,
    fun: f64,
}

struct Sample {
    y: DVector<f64>,
    x: Array1<f64>,
}

/// Minimise `f` with bounded full-covariance CMA-ES.
///
/// The objective receives parameters in the original coordinate system. Bounds
/// are handled by clipping sampled normalised points before evaluation.
pub fn cma_es<F>(f: &F, mut config: CmaEsConfig) -> Result<CmaEsReport>
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
    if let Some(ref x0) = config.x0
        && x0.len() != n
    {
        return Err(DEError::X0DimensionMismatch {
            expected: n,
            got: x0.len(),
        });
    }

    let lambda = if config.lambda == 0 {
        (4.0 + (3.0 * (n as f64).ln()).floor()).max(4.0) as usize
    } else {
        config.lambda
    };
    if lambda < 2 {
        return Err(DEError::PopulationTooSmall { pop_size: lambda });
    }
    let mu = if config.mu == 0 {
        lambda / 2
    } else {
        config.mu.min(lambda)
    }
    .max(1);

    let weights = recombination_weights(mu);
    let mueff = 1.0 / weights.iter().map(|w| w * w).sum::<f64>();
    let n_f = n as f64;

    let cc = (4.0 + mueff / n_f) / (n_f + 4.0 + 2.0 * mueff / n_f);
    let cs = (mueff + 2.0) / (n_f + mueff + 5.0);
    let c1 = 2.0 / ((n_f + 1.3).powi(2) + mueff);
    let cmu = (1.0 - c1).min(2.0 * (mueff - 2.0 + 1.0 / mueff) / ((n_f + 2.0).powi(2) + mueff));
    let damps = 1.0 + 2.0 * ((mueff - 1.0) / (n_f + 1.0)).sqrt().max(1.0) - 2.0 + cs;
    let chi_n = n_f.sqrt() * (1.0 - 1.0 / (4.0 * n_f) + 1.0 / (21.0 * n_f * n_f));

    let mut mean = initial_mean(&config);
    let mut sigma = config.sigma0.unwrap_or(0.3).clamp(1e-12, 2.0);
    let mut covariance = DMatrix::<f64>::identity(n, n);
    let mut b = DMatrix::<f64>::identity(n, n);
    let mut d = DVector::<f64>::from_element(n, 1.0);
    let mut invsqrt_c = DMatrix::<f64>::identity(n, n);
    let mut pc = DVector::<f64>::zeros(n);
    let mut ps = DVector::<f64>::zeros(n);

    let mut rng: StdRng = match config.seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => {
            let mut thread_rng = rand::rng();
            StdRng::from_rng(&mut thread_rng)
        }
    };

    let initial_x = denormalise(&mean, &config.bounds);
    let initial_fun = finite_or_infinity(f(&initial_x));
    let mut best_x = initial_x;
    let mut best_fun = initial_fun;
    let mut nfev = 1usize;
    let mut nit = 0usize;
    let mut last_improvement_fun = best_fun;
    let mut stagnation_counter = 0usize;
    let mut message = String::from("maximum evaluations reached");
    let mut success = false;

    if let Some(n) = config.parallel.num_threads {
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global();
    }

    while nfev < config.maxeval {
        let old_mean = mean.clone();
        let transform = &b * DMatrix::<f64>::from_diagonal(&d);
        let eval_budget = (config.maxeval - nfev).min(lambda);
        let mut samples: Vec<Sample> = Vec::with_capacity(eval_budget);

        for _ in 0..eval_budget {
            let z = standard_normal_vector(n, &mut rng);
            let step = &transform * z;
            let y = clamp_unit_vector(&(old_mean.clone() + step * sigma));
            let x = denormalise(&y, &config.bounds);
            samples.push(Sample { y, x });
        }

        let mut candidates: Vec<Candidate> = if config.parallel.enabled && samples.len() >= 4 {
            samples
                .par_iter()
                .map(|sample| Candidate {
                    y: sample.y.clone(),
                    fun: finite_or_infinity(f(&sample.x)),
                })
                .collect()
        } else {
            samples
                .iter()
                .map(|sample| Candidate {
                    y: sample.y.clone(),
                    fun: finite_or_infinity(f(&sample.x)),
                })
                .collect()
        };
        nfev += candidates.len();

        for (sample, candidate) in samples.iter().zip(candidates.iter()) {
            if candidate.fun < best_fun {
                best_fun = candidate.fun;
                best_x = sample.x.clone();
            }
        }

        if candidates.is_empty() {
            break;
        }
        candidates.sort_by(|a, b| a.fun.total_cmp(&b.fun));

        mean.fill(0.0);
        for i in 0..mu.min(candidates.len()) {
            mean += candidates[i].y.clone() * weights[i];
        }
        mean = clamp_unit_vector(&mean);

        let y_w = (&mean - &old_mean) / sigma.max(1e-30);
        ps = ps * (1.0 - cs) + (&invsqrt_c * &y_w) * (cs * (2.0 - cs) * mueff).sqrt();
        let norm_ps = ps.norm();
        let hsig_den = (1.0 - (1.0 - cs).powi(2 * (nit as i32 + 1))).sqrt() * chi_n;
        let hsig = if hsig_den > 0.0 {
            norm_ps / hsig_den < 1.4 + 2.0 / (n_f + 1.0)
        } else {
            true
        };
        pc *= 1.0 - cc;
        if hsig {
            pc += y_w.clone() * (cc * (2.0 - cc) * mueff).sqrt();
        }

        let mut rank_mu = DMatrix::<f64>::zeros(n, n);
        for i in 0..mu.min(candidates.len()) {
            let y_i = (&candidates[i].y - &old_mean) / sigma.max(1e-30);
            rank_mu += (&y_i * y_i.transpose()) * weights[i];
        }

        let hsig_correction = if hsig { 0.0 } else { c1 * cc * (2.0 - cc) };
        covariance = covariance * (1.0 - c1 - cmu + hsig_correction)
            + (&pc * pc.transpose()) * c1
            + rank_mu * cmu;
        symmetrise_and_regularise(&mut covariance);

        sigma *= ((cs / damps) * (norm_ps / chi_n - 1.0)).exp();
        sigma = sigma.clamp(1e-14, 10.0);

        let eig = SymmetricEigen::new(covariance.clone());
        b = eig.eigenvectors;
        d = eig.eigenvalues.map(|v| v.max(1e-30).sqrt());
        let inv_d = d.map(|v| 1.0 / v.max(1e-30));
        invsqrt_c = &b * DMatrix::<f64>::from_diagonal(&inv_d) * b.transpose();

        nit += 1;
        if (last_improvement_fun - best_fun).abs() <= config.f_tol {
            stagnation_counter += 1;
        } else {
            stagnation_counter = 0;
            last_improvement_fun = best_fun;
        }

        if let Some(ref mut callback) = config.callback {
            let intermediate = CmaEsIntermediate {
                x: best_x.clone(),
                fun: best_fun,
                iter: nit,
                nfev,
                sigma,
            };
            if matches!(callback(&intermediate), CallbackAction::Stop) {
                success = true;
                message = String::from("stopped by callback");
                break;
            }
        }

        if best_fun <= config.target_f {
            success = true;
            message = format!("target_f reached: {:.6e}", best_fun);
            break;
        }
        if config.stagnation_window > 0 && stagnation_counter >= config.stagnation_window {
            success = true;
            message = format!(
                "stagnated for {} generations below f_tol={:.3e}",
                config.stagnation_window, config.f_tol
            );
            break;
        }
        if sigma < 1e-12 {
            success = true;
            message = String::from("step size collapsed");
            break;
        }
    }

    Ok(CmaEsReport {
        x: best_x,
        fun: best_fun,
        success,
        message,
        nfev,
        nit,
        sigma,
    })
}

fn recombination_weights(mu: usize) -> Vec<f64> {
    let mu_f = mu as f64;
    let mut weights: Vec<f64> = (1..=mu)
        .map(|i| (mu_f + 0.5).ln() - (i as f64).ln())
        .collect();
    let sum = weights.iter().sum::<f64>();
    for w in &mut weights {
        *w /= sum;
    }
    weights
}

fn initial_mean(config: &CmaEsConfig) -> DVector<f64> {
    if let Some(ref x0) = config.x0 {
        let mut y = DVector::<f64>::zeros(config.bounds.len());
        for (i, (lo, hi)) in config.bounds.iter().enumerate() {
            let span = hi - lo;
            y[i] = if span > 0.0 {
                ((x0[i].clamp(*lo, *hi) - lo) / span).clamp(0.0, 1.0)
            } else {
                0.5
            };
        }
        y
    } else {
        DVector::<f64>::from_element(config.bounds.len(), 0.5)
    }
}

fn denormalise(y: &DVector<f64>, bounds: &[(f64, f64)]) -> Array1<f64> {
    let mut x = Vec::with_capacity(bounds.len());
    for (i, (lo, hi)) in bounds.iter().enumerate() {
        x.push(lo + y[i].clamp(0.0, 1.0) * (hi - lo));
    }
    Array1::from(x)
}

fn clamp_unit_vector(y: &DVector<f64>) -> DVector<f64> {
    y.map(|v| v.clamp(0.0, 1.0))
}

fn standard_normal_vector<R: Rng + ?Sized>(n: usize, rng: &mut R) -> DVector<f64> {
    let mut out = DVector::<f64>::zeros(n);
    let mut i = 0usize;
    while i < n {
        let u1 = rng.random::<f64>().max(f64::MIN_POSITIVE);
        let u2 = rng.random::<f64>();
        let radius = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * std::f64::consts::PI * u2;
        out[i] = radius * theta.cos();
        if i + 1 < n {
            out[i + 1] = radius * theta.sin();
        }
        i += 2;
    }
    out
}

fn finite_or_infinity(v: f64) -> f64 {
    if v.is_finite() { v } else { f64::INFINITY }
}

fn symmetrise_and_regularise(c: &mut DMatrix<f64>) {
    let n = c.nrows();
    for i in 0..n {
        for j in 0..i {
            let v = 0.5 * (c[(i, j)] + c[(j, i)]);
            c[(i, j)] = v;
            c[(j, i)] = v;
        }
        c[(i, i)] = c[(i, i)].max(1e-30);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn cma_es_converges_on_sphere() {
        let sphere = |x: &Array1<f64>| x.iter().map(|&xi| xi * xi).sum::<f64>();
        let report = cma_es(
            &sphere,
            CmaEsConfig {
                bounds: vec![(-5.0, 5.0); 4],
                maxeval: 5_000,
                seed: Some(42),
                target_f: 1e-10,
                ..Default::default()
            },
        )
        .expect("CMA-ES should run");

        assert!(
            report.fun < 1e-6,
            "CMA-ES should converge near origin, got {}",
            report.fun
        );
    }

    #[test]
    fn cma_es_handles_coupled_rotated_quadratic() {
        let rotated = |x: &Array1<f64>| {
            let u = (x[0] + x[1]) / 2.0_f64.sqrt();
            let v = (x[0] - x[1]) / 2.0_f64.sqrt();
            1_000.0 * u * u + v * v
        };
        let report = cma_es(
            &rotated,
            CmaEsConfig {
                bounds: vec![(-3.0, 3.0), (-3.0, 3.0)],
                maxeval: 4_000,
                seed: Some(7),
                target_f: 1e-9,
                ..Default::default()
            },
        )
        .expect("CMA-ES should run");

        assert!(
            report.fun < 1e-5,
            "CMA-ES should solve rotated ill-conditioned quadratic, got {}",
            report.fun
        );
    }

    #[test]
    fn cma_es_rejects_empty_bounds() {
        let sphere = |x: &Array1<f64>| x.iter().map(|&xi| xi * xi).sum::<f64>();
        let result = cma_es(
            &sphere,
            CmaEsConfig {
                bounds: vec![],
                ..Default::default()
            },
        );
        assert!(result.is_err(), "Empty bounds should error");
    }

    #[test]
    fn cma_es_rejects_inverted_bounds() {
        let sphere = |x: &Array1<f64>| x.iter().map(|&xi| xi * xi).sum::<f64>();
        let result = cma_es(
            &sphere,
            CmaEsConfig {
                bounds: vec![(1.0, -1.0)],
                ..Default::default()
            },
        );
        assert!(result.is_err(), "Inverted bounds should error");
    }

    #[test]
    fn cma_es_rejects_mismatched_x0() {
        let sphere = |x: &Array1<f64>| x.iter().map(|&xi| xi * xi).sum::<f64>();
        let result = cma_es(
            &sphere,
            CmaEsConfig {
                bounds: vec![(-5.0, 5.0); 3],
                x0: Some(array![0.0, 0.0]),
                ..Default::default()
            },
        );
        assert!(result.is_err(), "Mismatched x0 dimension should error");
    }

    #[test]
    fn cma_es_callback_stop() {
        let sphere = |x: &Array1<f64>| x.iter().map(|&xi| xi * xi).sum::<f64>();
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();
        let report = cma_es(
            &sphere,
            CmaEsConfig {
                bounds: vec![(-5.0, 5.0); 2],
                maxeval: 10_000,
                seed: Some(42),
                callback: Some(Box::new(move |_| {
                    let c = call_count_clone.fetch_add(1, Ordering::SeqCst);
                    if c + 1 >= 3 {
                        crate::CallbackAction::Stop
                    } else {
                        crate::CallbackAction::Continue
                    }
                })),
                ..Default::default()
            },
        )
        .expect("CMA-ES should run");
        assert!(report.success, "Should stop successfully by callback");
        assert_eq!(report.message, "stopped by callback");
    }

    #[test]
    fn cma_es_target_f_reached() {
        let sphere = |x: &Array1<f64>| x.iter().map(|&xi| xi * xi).sum::<f64>();
        let report = cma_es(
            &sphere,
            CmaEsConfig {
                bounds: vec![(-5.0, 5.0); 2],
                maxeval: 10_000,
                seed: Some(42),
                target_f: 1.0,
                ..Default::default()
            },
        )
        .expect("CMA-ES should run");
        assert!(
            report.fun <= 1.0,
            "Should stop when target_f reached: f={}",
            report.fun
        );
        assert!(report.success);
    }

    #[test]
    fn cma_es_single_dimension() {
        let sphere = |x: &Array1<f64>| x[0] * x[0];
        let report = cma_es(
            &sphere,
            CmaEsConfig {
                bounds: vec![(-5.0, 5.0)],
                maxeval: 2_000,
                seed: Some(42),
                target_f: 1e-8,
                ..Default::default()
            },
        )
        .expect("CMA-ES should run");
        assert!(
            report.fun < 1e-6,
            "1D CMA-ES should converge: f={}",
            report.fun
        );
    }

    #[test]
    fn cma_es_custom_lambda_mu() {
        let sphere = |x: &Array1<f64>| x.iter().map(|&xi| xi * xi).sum::<f64>();
        let report = cma_es(
            &sphere,
            CmaEsConfig {
                bounds: vec![(-5.0, 5.0); 2],
                lambda: 20,
                mu: 8,
                maxeval: 3_000,
                seed: Some(42),
                target_f: 1e-8,
                ..Default::default()
            },
        )
        .expect("CMA-ES should run");
        assert!(
            report.fun < 1e-6,
            "Custom lambda/mu should converge: f={}",
            report.fun
        );
    }

    #[test]
    fn cma_es_sigma0_override() {
        let sphere = |x: &Array1<f64>| x.iter().map(|&xi| xi * xi).sum::<f64>();
        let report = cma_es(
            &sphere,
            CmaEsConfig {
                bounds: vec![(-5.0, 5.0); 2],
                sigma0: Some(0.1),
                maxeval: 3_000,
                seed: Some(42),
                target_f: 1e-8,
                ..Default::default()
            },
        )
        .expect("CMA-ES should run");
        assert!(
            report.fun < 1e-6,
            "Small sigma0 should still converge: f={}",
            report.fun
        );
    }
}
