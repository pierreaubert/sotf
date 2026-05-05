//! Bayesian optimisation for expensive bounded continuous objectives.
//!
//! The implementation is intentionally domain-agnostic. It models objective
//! values with a Gaussian-process surrogate over normalised `[0, 1]^n`
//! coordinates, uses a Matérn-5/2 kernel with ARD lengthscales, and proposes
//! one or more candidates per iteration through EI or Monte-Carlo batch
//! q-EI/qEHVI acquisition.

use nalgebra::{DMatrix, DVector};
use ndarray::Array1;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;

use crate::CallbackAction;
use crate::error::{DEError, Result};
use crate::init_sobol::init_sobol;
use crate::parallel_eval::ParallelConfig;

const MIN_LENGTHSCALE: f64 = 1e-3;
const MAX_LENGTHSCALE: f64 = 10.0;
const LENGTHSCALE_SEARCH_PASSES: usize = 8;
const QEI_MC_DRAWS: usize = 128;
const EHVI_POSTERIOR_DRAWS: usize = 64;
const EHVI_HV_DRAWS: usize = 512;
const CHOLESKY_ATTEMPTS: usize = 10;

/// Acquisition strategy used to select candidates from the surrogate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BayesAcquisition {
    /// Expected Improvement for scalar minimisation.
    ExpectedImprovement,
    /// Monte-Carlo batch Expected Improvement under the joint GP posterior.
    QExpectedImprovement,
    /// Thompson sampling from independent posterior marginals.
    Thompson,
}

/// Per-iteration callback payload for [`bayesian_optimization`].
#[derive(Debug, Clone)]
pub struct BayesOptIntermediate {
    /// Current best parameter vector in original bounded coordinates.
    pub x: Array1<f64>,
    /// Current best objective value.
    pub fun: f64,
    /// Batch/iteration index.
    pub iter: usize,
    /// Objective evaluations consumed.
    pub nfev: usize,
    /// Largest posterior standard deviation observed in the candidate pool.
    pub posterior_std: f64,
}

/// Callback type used by [`BayesOptConfig`].
pub type BayesOptCallback = Box<dyn FnMut(&BayesOptIntermediate) -> CallbackAction + Send>;

/// Configuration for [`bayesian_optimization`].
pub struct BayesOptConfig {
    /// `(lower, upper)` bounds per parameter.
    pub bounds: Vec<(f64, f64)>,
    /// Optional initial point. Values outside bounds are clipped.
    pub x0: Option<Array1<f64>>,
    /// Number of Sobol hot-start samples. `0` uses a dimension-dependent default.
    pub initial_samples: usize,
    /// Number of candidates evaluated per BO iteration. `0` uses `1`.
    pub batch_size: usize,
    /// Maximum objective evaluations.
    pub maxeval: usize,
    /// Number of acquisition candidates considered per iteration.
    pub candidate_pool_size: usize,
    /// Optional ARD lengthscales in normalised parameter coordinates.
    pub lengthscales: Option<Vec<f64>>,
    /// Signal variance for the Matérn-5/2 kernel.
    pub kernel_variance: f64,
    /// Diagonal observation-noise/jitter floor.
    pub noise: f64,
    /// EI exploration offset. Larger values prefer uncertain candidates.
    pub exploration: f64,
    /// Stop once the candidate-pool posterior standard deviation drops below
    /// this value. `0.0` disables the uncertainty stop.
    pub posterior_std_threshold: f64,
    /// Parallel batch-evaluation configuration.
    pub parallel: ParallelConfig,
    /// Optional RNG seed for deterministic runs.
    pub seed: Option<u64>,
    /// Acquisition strategy.
    pub acquisition: BayesAcquisition,
    /// Optional per-iteration callback. Returning [`CallbackAction::Stop`]
    /// terminates early with the best point seen so far.
    pub callback: Option<BayesOptCallback>,
}

impl Default for BayesOptConfig {
    fn default() -> Self {
        Self {
            bounds: Vec::new(),
            x0: None,
            initial_samples: 0,
            batch_size: 1,
            maxeval: 500,
            candidate_pool_size: 0,
            lengthscales: None,
            kernel_variance: 1.0,
            noise: 1e-8,
            exploration: 0.01,
            posterior_std_threshold: 0.0,
            parallel: ParallelConfig::default(),
            seed: None,
            acquisition: BayesAcquisition::QExpectedImprovement,
            callback: None,
        }
    }
}

/// Result of a [`bayesian_optimization`] run.
#[derive(Debug, Clone)]
pub struct BayesOptReport {
    /// Best parameter vector found.
    pub x: Array1<f64>,
    /// Objective value at [`Self::x`].
    pub fun: f64,
    /// Whether the run stopped by callback, target uncertainty, or budget after
    /// at least one surrogate-guided iteration.
    pub success: bool,
    /// Human-readable termination message.
    pub message: String,
    /// Objective evaluations consumed.
    pub nfev: usize,
    /// BO batches completed after the initial design.
    pub nit: usize,
    /// Largest posterior standard deviation seen in the final candidate pool.
    pub posterior_std: f64,
}

/// Pareto solution returned by [`BayesOptParetoReport`].
#[derive(Debug, Clone)]
pub struct BayesParetoSolution {
    /// Parameter vector.
    pub x: Array1<f64>,
    /// Objective values, all minimised.
    pub objectives: Vec<f64>,
}

/// Result of a [`bayesian_multi_objective`] run.
#[derive(Debug, Clone)]
pub struct BayesOptParetoReport {
    /// Non-dominated front from all evaluated samples.
    pub pareto_front: Vec<BayesParetoSolution>,
    /// All evaluated samples.
    pub population: Vec<BayesParetoSolution>,
    /// Objective evaluations consumed.
    pub nfev: usize,
    /// BO batches completed after the initial design.
    pub nit: usize,
    /// Whether at least one surrogate-guided iteration completed.
    pub success: bool,
    /// Human-readable termination message.
    pub message: String,
}

#[derive(Clone)]
struct GaussianProcess {
    x: Vec<Array1<f64>>,
    y_mean: f64,
    y_scale: f64,
    alpha: DVector<f64>,
    chol_l: DMatrix<f64>,
    lengthscales: Vec<f64>,
    kernel_variance: f64,
}

impl GaussianProcess {
    fn fit(
        x: &[Array1<f64>],
        y: &[f64],
        lengthscales: &[f64],
        kernel_variance: f64,
        noise: f64,
    ) -> Result<Self> {
        if x.is_empty() || y.is_empty() || x.len() != y.len() {
            return Err(DEError::InvalidConfig {
                message: "GP training data must be non-empty and dimensionally aligned".into(),
            });
        }

        let n = y.len();
        let y_mean = y.iter().sum::<f64>() / n as f64;
        let variance = y.iter().map(|v| (v - y_mean).powi(2)).sum::<f64>() / n as f64;
        let y_scale = variance.sqrt().max(1e-12);
        let y_norm = DVector::from_iterator(n, y.iter().map(|v| (v - y_mean) / y_scale));

        let k = kernel_matrix(x, lengthscales, kernel_variance);
        let base_noise = noise.max(1e-12);
        let Some((chol_l, _jitter)) = cholesky_with_jitter(&k, base_noise) else {
            return Err(DEError::InvalidConfig {
                message: "failed to factor Gaussian-process kernel matrix".into(),
            });
        };
        let alpha = cholesky_solve(&chol_l, &y_norm);

        Ok(Self {
            x: x.to_vec(),
            y_mean,
            y_scale,
            alpha,
            chol_l,
            lengthscales: lengthscales.to_vec(),
            kernel_variance,
        })
    }

    fn predict(&self, x: &Array1<f64>) -> (f64, f64) {
        let n = self.x.len();
        let mut k_star = DVector::zeros(n);
        for i in 0..n {
            k_star[i] = matern52(x, &self.x[i], &self.lengthscales, self.kernel_variance);
        }
        let mean_norm = k_star.dot(&self.alpha);
        let v = solve_lower(&self.chol_l, &k_star);
        let var_norm = (self.kernel_variance - v.dot(&v)).max(1e-14);
        let mean = self.y_mean + self.y_scale * mean_norm;
        let std = self.y_scale * var_norm.sqrt();
        (mean, std.max(1e-12))
    }

    fn predict_joint(&self, xs: &[Array1<f64>]) -> (Vec<f64>, DMatrix<f64>) {
        let n_train = self.x.len();
        let q = xs.len();
        let mut k_train_star = DMatrix::zeros(n_train, q);
        for i in 0..n_train {
            for j in 0..q {
                k_train_star[(i, j)] =
                    matern52(&self.x[i], &xs[j], &self.lengthscales, self.kernel_variance);
            }
        }

        let means = (0..q)
            .map(|j| {
                let mean_norm = k_train_star.column(j).dot(&self.alpha);
                self.y_mean + self.y_scale * mean_norm
            })
            .collect::<Vec<_>>();

        let v = solve_lower_matrix(&self.chol_l, &k_train_star);
        let mut cov = DMatrix::zeros(q, q);
        for i in 0..q {
            for j in i..q {
                let prior = matern52(&xs[i], &xs[j], &self.lengthscales, self.kernel_variance);
                let reduction = v.column(i).dot(&v.column(j));
                let value = (prior - reduction) * self.y_scale * self.y_scale;
                cov[(i, j)] = value;
                cov[(j, i)] = value;
            }
        }
        for i in 0..q {
            cov[(i, i)] = cov[(i, i)].max(1e-14);
        }

        (means, cov)
    }
}

/// Minimise `f` with Gaussian-process Bayesian optimisation.
///
/// The objective receives parameters in the original coordinate system.
/// Internally the surrogate is fit on normalised `[0, 1]^n` coordinates.
pub fn bayesian_optimization<F>(f: &F, mut config: BayesOptConfig) -> Result<BayesOptReport>
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    validate_config(&config)?;
    let n = config.bounds.len();
    let batch_size = config.batch_size.max(1);
    let initial_samples = derive_initial_samples(n, &config).min(config.maxeval);
    let candidate_pool_size = derive_candidate_pool_size(n, &config);
    let lengthscales = derive_lengthscales(n, &config)?;
    let fixed_lengthscales = config.lengthscales.is_some();
    let mut rng = make_rng(config.seed);

    let mut samples = initial_design(&config, initial_samples);
    let mut xs_norm = Vec::with_capacity(config.maxeval);
    let mut ys = Vec::with_capacity(config.maxeval);
    let mut best_x = Array1::zeros(n);
    let mut best_y = f64::INFINITY;

    evaluate_and_store(
        f,
        &mut samples,
        &config,
        &mut xs_norm,
        &mut ys,
        &mut best_x,
        &mut best_y,
    );

    let mut nit = 0usize;
    let mut final_std = f64::INFINITY;
    let mut message = String::from("evaluation budget reached");
    let mut success = false;

    while ys.len() < config.maxeval {
        let gp = fit_gp(&xs_norm, &ys, &lengthscales, fixed_lengthscales, &config)?;
        let candidates = candidate_pool(&config, candidate_pool_size, &xs_norm, &mut rng);
        if candidates.is_empty() {
            message = String::from("candidate pool exhausted");
            break;
        }
        final_std = max_posterior_std(&gp, &candidates, &config.bounds, &config.parallel);

        if config.posterior_std_threshold > 0.0
            && final_std <= config.posterior_std_threshold
            && ys.len() >= initial_samples
        {
            success = true;
            message = format!(
                "posterior std {:.3e} below threshold {:.3e}",
                final_std, config.posterior_std_threshold
            );
            break;
        }

        let remaining = config.maxeval - ys.len();
        let q = batch_size.min(remaining);
        let mut selected = select_batch(&gp, &candidates, best_y, q, &config, &mut rng);
        evaluate_and_store(
            f,
            &mut selected,
            &config,
            &mut xs_norm,
            &mut ys,
            &mut best_x,
            &mut best_y,
        );

        nit += 1;
        if let Some(callback) = config.callback.as_mut() {
            let payload = BayesOptIntermediate {
                x: best_x.clone(),
                fun: best_y,
                iter: nit,
                nfev: ys.len(),
                posterior_std: final_std,
            };
            if matches!(callback(&payload), CallbackAction::Stop) {
                success = true;
                message = String::from("stopped by callback");
                break;
            }
        }
    }

    if nit > 0 && !success {
        success = true;
    }

    Ok(BayesOptReport {
        x: best_x,
        fun: best_y,
        success,
        message,
        nfev: ys.len(),
        nit,
        posterior_std: final_std,
    })
}

/// Minimise a vector objective with Monte-Carlo EHVI.
///
/// This uses one independent GP per objective, samples the joint posterior
/// over each proposed batch, and scores candidates by expected hypervolume
/// improvement. All objectives are minimised.
pub fn bayesian_multi_objective<F>(f: &F, config: BayesOptConfig) -> Result<BayesOptParetoReport>
where
    F: Fn(&Array1<f64>) -> Vec<f64> + Sync,
{
    validate_config(&config)?;
    let n = config.bounds.len();
    let batch_size = config.batch_size.max(1);
    let initial_samples = derive_initial_samples(n, &config).min(config.maxeval);
    let candidate_pool_size = derive_candidate_pool_size(n, &config).min(256).max(32);
    let lengthscales = derive_lengthscales(n, &config)?;
    let fixed_lengthscales = config.lengthscales.is_some();
    let mut rng = make_rng(config.seed);

    let mut initial = initial_design(&config, initial_samples);
    let mut xs_norm = Vec::with_capacity(config.maxeval);
    let mut values = Vec::with_capacity(config.maxeval);
    evaluate_multi_and_store(f, &mut initial, &config, &mut xs_norm, &mut values);

    let mut nit = 0usize;
    while values.len() < config.maxeval {
        let m = values.first().map(|v| v.len()).unwrap_or(0);
        if m == 0 {
            return Err(DEError::InvalidConfig {
                message: "multi-objective function returned an empty objective vector".into(),
            });
        }

        let gps = (0..m)
            .map(|j| {
                let yj = values.iter().map(|v| v[j]).collect::<Vec<_>>();
                fit_gp(&xs_norm, &yj, &lengthscales, fixed_lengthscales, &config)
            })
            .collect::<Result<Vec<_>>>()?;

        let candidates = candidate_pool(&config, candidate_pool_size, &xs_norm, &mut rng);
        if candidates.is_empty() {
            break;
        }
        let current_front = pareto_values(&values);
        let reference = reference_point(&values);
        let remaining = config.maxeval - values.len();
        let q = batch_size.min(remaining);
        let mut selected = select_ehvi_batch(
            &gps,
            &candidates,
            &current_front,
            &reference,
            q,
            &config,
            &mut rng,
        );
        evaluate_multi_and_store(f, &mut selected, &config, &mut xs_norm, &mut values);
        nit += 1;
    }

    let population = xs_norm
        .iter()
        .zip(values.iter())
        .map(|(x, objectives)| BayesParetoSolution {
            x: denormalize(x, &config.bounds),
            objectives: objectives.clone(),
        })
        .collect::<Vec<_>>();
    let pareto_front = pareto_solutions(&population);

    Ok(BayesOptParetoReport {
        pareto_front,
        population,
        nfev: values.len(),
        nit,
        success: nit > 0,
        message: if nit > 0 {
            String::from("evaluation budget reached")
        } else {
            String::from("initial design consumed evaluation budget")
        },
    })
}

fn validate_config(config: &BayesOptConfig) -> Result<()> {
    if config.bounds.is_empty() {
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
        && x0.len() != config.bounds.len()
    {
        return Err(DEError::X0DimensionMismatch {
            expected: config.bounds.len(),
            got: x0.len(),
        });
    }
    if config.maxeval == 0 {
        return Err(DEError::InvalidConfig {
            message: "maxeval must be greater than zero".into(),
        });
    }
    Ok(())
}

fn derive_initial_samples(n: usize, config: &BayesOptConfig) -> usize {
    if config.initial_samples == 0 {
        (2 * n + 1).max(8)
    } else {
        config.initial_samples
    }
}

fn derive_candidate_pool_size(n: usize, config: &BayesOptConfig) -> usize {
    if config.candidate_pool_size == 0 {
        (64 * n).max(512)
    } else {
        config.candidate_pool_size.max(16)
    }
}

fn derive_lengthscales(n: usize, config: &BayesOptConfig) -> Result<Vec<f64>> {
    if let Some(ref ls) = config.lengthscales {
        if ls.len() != n {
            return Err(DEError::InvalidConfig {
                message: format!(
                    "lengthscales dimension mismatch: expected {}, got {}",
                    n,
                    ls.len()
                ),
            });
        }
        Ok(ls
            .iter()
            .map(|v| v.clamp(MIN_LENGTHSCALE, MAX_LENGTHSCALE))
            .collect())
    } else {
        Ok(vec![0.25; n])
    }
}

fn fit_gp(
    xs_norm: &[Array1<f64>],
    ys: &[f64],
    initial_lengthscales: &[f64],
    fixed_lengthscales: bool,
    config: &BayesOptConfig,
) -> Result<GaussianProcess> {
    let kernel_variance = config.kernel_variance.max(1e-12);
    let lengthscales = if fixed_lengthscales {
        initial_lengthscales.to_vec()
    } else {
        learn_lengthscales(
            xs_norm,
            ys,
            initial_lengthscales,
            kernel_variance,
            config.noise,
        )?
    };
    GaussianProcess::fit(xs_norm, ys, &lengthscales, kernel_variance, config.noise)
}

fn learn_lengthscales(
    x: &[Array1<f64>],
    y: &[f64],
    initial: &[f64],
    kernel_variance: f64,
    noise: f64,
) -> Result<Vec<f64>> {
    if x.len() < 3 || initial.is_empty() {
        return Ok(initial.to_vec());
    }

    let log_min = MIN_LENGTHSCALE.ln();
    let log_max = MAX_LENGTHSCALE.ln();
    let mut log_ls = initial
        .iter()
        .map(|v| v.clamp(MIN_LENGTHSCALE, MAX_LENGTHSCALE).ln())
        .collect::<Vec<_>>();
    let mut best =
        negative_log_marginal_likelihood(x, y, &log_lengthscales(&log_ls), kernel_variance, noise)
            .unwrap_or(f64::INFINITY);

    let mut step = 0.7_f64;
    for _ in 0..LENGTHSCALE_SEARCH_PASSES {
        let mut improved = false;
        for d in 0..log_ls.len() {
            let base = log_ls[d];
            let mut best_for_dim = best;
            let mut best_log_value = base;
            for direction in [-1.0, 1.0] {
                let mut trial = log_ls.clone();
                trial[d] = (base + direction * step).clamp(log_min, log_max);
                if (trial[d] - base).abs() < 1e-12 {
                    continue;
                }
                let ls = log_lengthscales(&trial);
                if let Some(score) =
                    negative_log_marginal_likelihood(x, y, &ls, kernel_variance, noise)
                    && score < best_for_dim
                {
                    best_for_dim = score;
                    best_log_value = trial[d];
                }
            }
            if best_for_dim < best - 1e-7 {
                log_ls[d] = best_log_value;
                best = best_for_dim;
                improved = true;
            }
        }
        if !improved {
            step *= 0.5;
            if step < 0.05 {
                break;
            }
        }
    }

    Ok(log_lengthscales(&log_ls))
}

fn log_lengthscales(log_ls: &[f64]) -> Vec<f64> {
    log_ls
        .iter()
        .map(|v| v.exp().clamp(MIN_LENGTHSCALE, MAX_LENGTHSCALE))
        .collect()
}

fn make_rng(seed: Option<u64>) -> StdRng {
    match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => {
            let mut thread_rng = rand::rng();
            StdRng::from_rng(&mut thread_rng)
        }
    }
}

fn initial_design(config: &BayesOptConfig, initial_samples: usize) -> Vec<Array1<f64>> {
    let mut samples = Vec::with_capacity(initial_samples);
    if let Some(ref x0) = config.x0 {
        samples.push(clip_to_bounds(x0, &config.bounds));
    }

    let needed = initial_samples.saturating_sub(samples.len());
    for s in init_sobol(config.bounds.len(), needed, &config.bounds) {
        samples.push(Array1::from(s));
    }
    samples
}

fn evaluate_and_store<F>(
    f: &F,
    candidates: &mut [Array1<f64>],
    config: &BayesOptConfig,
    xs_norm: &mut Vec<Array1<f64>>,
    ys: &mut Vec<f64>,
    best_x: &mut Array1<f64>,
    best_y: &mut f64,
) where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    let values = if config.parallel.enabled && candidates.len() >= 4 {
        candidates.par_iter().map(f).collect::<Vec<_>>()
    } else {
        candidates.iter().map(f).collect::<Vec<_>>()
    };

    for (x, y) in candidates.iter().zip(values) {
        xs_norm.push(normalize(x, &config.bounds));
        ys.push(y);
        if y < *best_y {
            *best_y = y;
            *best_x = x.clone();
        }
    }
}

fn evaluate_multi_and_store<F>(
    f: &F,
    candidates: &mut [Array1<f64>],
    config: &BayesOptConfig,
    xs_norm: &mut Vec<Array1<f64>>,
    values: &mut Vec<Vec<f64>>,
) where
    F: Fn(&Array1<f64>) -> Vec<f64> + Sync,
{
    let ys = if config.parallel.enabled && candidates.len() >= 4 {
        candidates.par_iter().map(f).collect::<Vec<_>>()
    } else {
        candidates.iter().map(f).collect::<Vec<_>>()
    };

    for (x, y) in candidates.iter().zip(ys) {
        xs_norm.push(normalize(x, &config.bounds));
        values.push(y);
    }
}

fn candidate_pool(
    config: &BayesOptConfig,
    size: usize,
    existing_norm: &[Array1<f64>],
    rng: &mut StdRng,
) -> Vec<Array1<f64>> {
    let mut candidates = Vec::with_capacity(size);
    let sobol_count = size / 2;
    for s in init_sobol(config.bounds.len(), sobol_count, &config.bounds) {
        let x = Array1::from(s);
        if is_new_point(
            &normalize(&x, &config.bounds),
            existing_norm,
            &candidates,
            &config.bounds,
        ) {
            candidates.push(x);
        }
    }
    let mut attempts = 0usize;
    while candidates.len() < size && attempts < size.saturating_mul(20).max(1000) {
        let x = random_point(&config.bounds, rng);
        if is_new_point(
            &normalize(&x, &config.bounds),
            existing_norm,
            &candidates,
            &config.bounds,
        ) {
            candidates.push(x);
        }
        attempts += 1;
    }
    candidates
}

fn is_new_point(
    x_norm: &Array1<f64>,
    existing_norm: &[Array1<f64>],
    pending_original: &[Array1<f64>],
    bounds: &[(f64, f64)],
) -> bool {
    let min_dist2 = 1e-18;
    if existing_norm
        .iter()
        .any(|x| squared_distance(x_norm, x) < min_dist2)
    {
        return false;
    }
    !pending_original
        .iter()
        .map(|x| normalize(x, bounds))
        .any(|x| squared_distance(x_norm, &x) < min_dist2)
}

fn select_batch(
    gp: &GaussianProcess,
    candidates: &[Array1<f64>],
    best_y: f64,
    q: usize,
    config: &BayesOptConfig,
    rng: &mut StdRng,
) -> Vec<Array1<f64>> {
    if matches!(config.acquisition, BayesAcquisition::QExpectedImprovement) {
        return select_qei_batch(gp, candidates, best_y, q, config, rng);
    }

    let acquisition = config.acquisition;
    let exploration = config.exploration;
    let bounds = &config.bounds;
    let mut scored = if config.parallel.enabled && candidates.len() >= 4 {
        candidates
            .par_iter()
            .map(|x| score_candidate(gp, x, best_y, acquisition, exploration, bounds, None))
            .collect::<Vec<_>>()
    } else {
        candidates
            .iter()
            .map(|x| score_candidate(gp, x, best_y, acquisition, exploration, bounds, None))
            .collect::<Vec<_>>()
    };

    if matches!(config.acquisition, BayesAcquisition::Thompson) {
        for item in &mut scored {
            item.0 = score_candidate(
                gp,
                &item.1,
                best_y,
                acquisition,
                exploration,
                bounds,
                Some(rng),
            )
            .0;
        }
        scored.sort_by(|a, b| a.0.total_cmp(&b.0));
    } else {
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    }

    let mut selected = Vec::with_capacity(q);
    for (_, x) in scored {
        let x_norm = normalize(&x, &config.bounds);
        let diverse = selected.iter().all(|s| {
            let s_norm = normalize(s, &config.bounds);
            squared_distance(&x_norm, &s_norm) > 1e-8
        });
        if diverse {
            selected.push(x);
        }
        if selected.len() == q {
            break;
        }
    }
    selected
}

fn score_candidate(
    gp: &GaussianProcess,
    x: &Array1<f64>,
    best_y: f64,
    acquisition: BayesAcquisition,
    exploration: f64,
    bounds: &[(f64, f64)],
    rng: Option<&mut StdRng>,
) -> (f64, Array1<f64>) {
    let x_norm = normalize(x, bounds);
    let (mean, std) = gp.predict(&x_norm);
    let score = match (acquisition, rng) {
        (BayesAcquisition::ExpectedImprovement, _) => {
            expected_improvement(best_y, mean, std, exploration)
        }
        (BayesAcquisition::Thompson, Some(rng)) => mean + std * standard_normal(rng),
        (BayesAcquisition::QExpectedImprovement | BayesAcquisition::Thompson, None)
        | (BayesAcquisition::QExpectedImprovement, Some(_)) => mean,
    };
    (score, x.clone())
}

fn select_qei_batch(
    gp: &GaussianProcess,
    candidates: &[Array1<f64>],
    best_y: f64,
    q: usize,
    config: &BayesOptConfig,
    rng: &mut StdRng,
) -> Vec<Array1<f64>> {
    let candidate_norms = candidates
        .iter()
        .map(|x| normalize(x, &config.bounds))
        .collect::<Vec<_>>();
    let mut available = (0..candidates.len()).collect::<Vec<_>>();
    let mut selected_indices: Vec<usize> = Vec::with_capacity(q);

    while selected_indices.len() < q && !available.is_empty() {
        let batch_width = selected_indices.len() + 1;
        let common_normals = normal_draws(QEI_MC_DRAWS, batch_width, rng);
        let mut best_candidate = available[0];
        let mut best_score = f64::NEG_INFINITY;

        for &idx in &available {
            let mut batch = selected_indices
                .iter()
                .map(|&i| candidate_norms[i].clone())
                .collect::<Vec<_>>();
            batch.push(candidate_norms[idx].clone());
            let score =
                q_expected_improvement_mc(gp, &batch, best_y, config.exploration, &common_normals);
            if score > best_score {
                best_score = score;
                best_candidate = idx;
            }
        }

        selected_indices.push(best_candidate);
        available.retain(|&idx| idx != best_candidate);
    }

    selected_indices
        .into_iter()
        .map(|idx| candidates[idx].clone())
        .collect()
}

fn q_expected_improvement_mc(
    gp: &GaussianProcess,
    batch_norm: &[Array1<f64>],
    best_y: f64,
    xi: f64,
    normals: &[Vec<f64>],
) -> f64 {
    if batch_norm.is_empty() || normals.is_empty() {
        return 0.0;
    }
    let (means, cov) = gp.predict_joint(batch_norm);
    let mut total = 0.0;
    for z in normals {
        let sample = sample_multivariate_normal(&means, &cov, z);
        let batch_min = sample.into_iter().fold(f64::INFINITY, f64::min);
        total += (best_y - batch_min - xi.max(0.0)).max(0.0);
    }
    total / normals.len() as f64
}

fn max_posterior_std(
    gp: &GaussianProcess,
    candidates: &[Array1<f64>],
    bounds: &[(f64, f64)],
    parallel: &ParallelConfig,
) -> f64 {
    if parallel.enabled && candidates.len() >= 4 {
        candidates
            .par_iter()
            .map(|x| gp.predict(&normalize(x, bounds)).1)
            .reduce(|| 0.0, f64::max)
    } else {
        candidates
            .iter()
            .map(|x| gp.predict(&normalize(x, bounds)).1)
            .fold(0.0, f64::max)
    }
}

fn select_ehvi_batch(
    gps: &[GaussianProcess],
    candidates: &[Array1<f64>],
    current_front: &[Vec<f64>],
    reference: &[f64],
    q: usize,
    config: &BayesOptConfig,
    rng: &mut StdRng,
) -> Vec<Array1<f64>> {
    let candidate_norms = candidates
        .iter()
        .map(|x| normalize(x, &config.bounds))
        .collect::<Vec<_>>();
    let lower = hypervolume_integration_lower(current_front, gps, &candidate_norms, reference);
    let hv_samples = HypervolumeSamples::new(current_front, &lower, reference, EHVI_HV_DRAWS, rng);
    if hv_samples.points.is_empty() {
        return candidates.iter().take(q).cloned().collect();
    }

    let mut available = (0..candidates.len()).collect::<Vec<_>>();
    let mut selected_indices: Vec<usize> = Vec::with_capacity(q);
    while selected_indices.len() < q && !available.is_empty() {
        let batch_width = selected_indices.len() + 1;
        let common_normals = normal_tensor(EHVI_POSTERIOR_DRAWS, gps.len(), batch_width, rng);
        let mut best_candidate = available[0];
        let mut best_score = f64::NEG_INFINITY;

        for &idx in &available {
            let mut batch = selected_indices
                .iter()
                .map(|&i| candidate_norms[i].clone())
                .collect::<Vec<_>>();
            batch.push(candidate_norms[idx].clone());
            let score = expected_hypervolume_improvement_mc(
                gps,
                &batch,
                reference,
                &hv_samples,
                &common_normals,
            );
            if score > best_score {
                best_score = score;
                best_candidate = idx;
            }
        }

        selected_indices.push(best_candidate);
        available.retain(|&idx| idx != best_candidate);
    }

    selected_indices
        .into_iter()
        .map(|idx| candidates[idx].clone())
        .collect()
}

fn matern52(a: &Array1<f64>, b: &Array1<f64>, lengthscales: &[f64], variance: f64) -> f64 {
    let mut r2 = 0.0;
    for i in 0..a.len() {
        let ell = lengthscales[i].max(1e-6);
        r2 += ((a[i] - b[i]) / ell).powi(2);
    }
    let r = r2.sqrt();
    let sqrt5r = 5.0_f64.sqrt() * r;
    variance * (1.0 + sqrt5r + 5.0 * r * r / 3.0) * (-sqrt5r).exp()
}

fn kernel_matrix(x: &[Array1<f64>], lengthscales: &[f64], kernel_variance: f64) -> DMatrix<f64> {
    let n = x.len();
    let mut k = DMatrix::zeros(n, n);
    for i in 0..n {
        for j in i..n {
            let kij = matern52(&x[i], &x[j], lengthscales, kernel_variance);
            k[(i, j)] = kij;
            k[(j, i)] = kij;
        }
    }
    k
}

fn negative_log_marginal_likelihood(
    x: &[Array1<f64>],
    y: &[f64],
    lengthscales: &[f64],
    kernel_variance: f64,
    noise: f64,
) -> Option<f64> {
    if x.is_empty() || x.len() != y.len() {
        return None;
    }
    let n = y.len();
    let y_mean = y.iter().sum::<f64>() / n as f64;
    let variance = y.iter().map(|v| (v - y_mean).powi(2)).sum::<f64>() / n as f64;
    let y_scale = variance.sqrt().max(1e-12);
    let y_norm = DVector::from_iterator(n, y.iter().map(|v| (v - y_mean) / y_scale));
    let k = kernel_matrix(x, lengthscales, kernel_variance);
    let (chol_l, _) = cholesky_with_jitter(&k, noise.max(1e-12))?;
    let alpha = cholesky_solve(&chol_l, &y_norm);
    let data_fit = 0.5 * y_norm.dot(&alpha);
    let complexity = (0..n).map(|i| chol_l[(i, i)].ln()).sum::<f64>();
    let normalizer = 0.5 * n as f64 * (2.0 * std::f64::consts::PI).ln();
    let score = data_fit + complexity + normalizer;
    score.is_finite().then_some(score)
}

fn cholesky_with_jitter(matrix: &DMatrix<f64>, base_jitter: f64) -> Option<(DMatrix<f64>, f64)> {
    for attempt in 0..CHOLESKY_ATTEMPTS {
        let jitter = base_jitter.max(1e-14) * 10f64.powi(attempt as i32);
        let mut shifted = matrix.clone();
        for i in 0..shifted.nrows() {
            shifted[(i, i)] += jitter;
        }
        if let Some(l) = cholesky_lower(&shifted) {
            return Some((l, jitter));
        }
    }
    None
}

fn cholesky_lower(matrix: &DMatrix<f64>) -> Option<DMatrix<f64>> {
    let n = matrix.nrows();
    if n != matrix.ncols() {
        return None;
    }
    let mut l = DMatrix::zeros(n, n);
    for i in 0..n {
        for j in 0..=i {
            let mut sum = matrix[(i, j)];
            for k in 0..j {
                sum -= l[(i, k)] * l[(j, k)];
            }
            if i == j {
                if !sum.is_finite() || sum <= 0.0 {
                    return None;
                }
                l[(i, j)] = sum.sqrt();
            } else {
                let diag = l[(j, j)];
                if diag <= 0.0 {
                    return None;
                }
                l[(i, j)] = sum / diag;
            }
        }
    }
    Some(l)
}

fn cholesky_solve(l: &DMatrix<f64>, b: &DVector<f64>) -> DVector<f64> {
    let y = solve_lower(l, b);
    solve_upper_from_lower(l, &y)
}

fn solve_lower(l: &DMatrix<f64>, b: &DVector<f64>) -> DVector<f64> {
    let n = l.nrows();
    let mut x = DVector::zeros(n);
    for i in 0..n {
        let mut sum = b[i];
        for j in 0..i {
            sum -= l[(i, j)] * x[j];
        }
        x[i] = sum / l[(i, i)];
    }
    x
}

fn solve_upper_from_lower(l: &DMatrix<f64>, b: &DVector<f64>) -> DVector<f64> {
    let n = l.nrows();
    let mut x = DVector::zeros(n);
    for i in (0..n).rev() {
        let mut sum = b[i];
        for j in (i + 1)..n {
            sum -= l[(j, i)] * x[j];
        }
        x[i] = sum / l[(i, i)];
    }
    x
}

fn solve_lower_matrix(l: &DMatrix<f64>, b: &DMatrix<f64>) -> DMatrix<f64> {
    let mut out = DMatrix::zeros(b.nrows(), b.ncols());
    for col in 0..b.ncols() {
        let rhs = DVector::from_iterator(b.nrows(), (0..b.nrows()).map(|row| b[(row, col)]));
        let solved = solve_lower(l, &rhs);
        for row in 0..b.nrows() {
            out[(row, col)] = solved[row];
        }
    }
    out
}

fn expected_improvement(best: f64, mean: f64, std: f64, xi: f64) -> f64 {
    if std <= 1e-12 {
        return 0.0;
    }
    let improvement = best - mean - xi.max(0.0);
    let z = improvement / std;
    improvement * normal_cdf(z) + std * normal_pdf(z)
}

fn normal_pdf(z: f64) -> f64 {
    (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

fn normal_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf(z / 2.0_f64.sqrt()))
}

fn erf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
            + 0.254829592)
            * t
            * (-x * x).exp();
    sign * y
}

fn standard_normal(rng: &mut StdRng) -> f64 {
    let u1 = rng.random::<f64>().clamp(1e-12, 1.0);
    let u2 = rng.random::<f64>();
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

fn normal_draws(draws: usize, dim: usize, rng: &mut StdRng) -> Vec<Vec<f64>> {
    (0..draws.max(1))
        .map(|_| (0..dim).map(|_| standard_normal(rng)).collect())
        .collect()
}

fn normal_tensor(
    draws: usize,
    objectives: usize,
    dim: usize,
    rng: &mut StdRng,
) -> Vec<Vec<Vec<f64>>> {
    (0..draws.max(1))
        .map(|_| {
            (0..objectives)
                .map(|_| (0..dim).map(|_| standard_normal(rng)).collect())
                .collect()
        })
        .collect()
}

fn sample_multivariate_normal(mean: &[f64], cov: &DMatrix<f64>, normals: &[f64]) -> Vec<f64> {
    let q = mean.len();
    if q == 0 {
        return Vec::new();
    }
    if normals.len() != q || cov.nrows() != q || cov.ncols() != q {
        return mean.to_vec();
    }

    let mut sym = DMatrix::zeros(q, q);
    for i in 0..q {
        for j in 0..q {
            sym[(i, j)] = 0.5 * (cov[(i, j)] + cov[(j, i)]);
        }
    }
    if let Some((l, _)) = cholesky_with_jitter(&sym, 1e-12) {
        let z = DVector::from_column_slice(normals);
        let correlated = l * z;
        return mean
            .iter()
            .enumerate()
            .map(|(i, m)| m + correlated[i])
            .collect();
    }

    mean.iter()
        .enumerate()
        .map(|(i, m)| m + cov[(i, i)].max(1e-14).sqrt() * normals[i])
        .collect()
}

fn normalize(x: &Array1<f64>, bounds: &[(f64, f64)]) -> Array1<f64> {
    if bounds.is_empty() {
        return x.clone();
    }
    Array1::from_iter(x.iter().zip(bounds.iter()).map(|(&v, &(lo, hi))| {
        let span = (hi - lo).max(1e-12);
        ((v - lo) / span).clamp(0.0, 1.0)
    }))
}

fn denormalize(x: &Array1<f64>, bounds: &[(f64, f64)]) -> Array1<f64> {
    Array1::from_iter(
        x.iter()
            .zip(bounds.iter())
            .map(|(&v, &(lo, hi))| lo + v.clamp(0.0, 1.0) * (hi - lo)),
    )
}

fn clip_to_bounds(x: &Array1<f64>, bounds: &[(f64, f64)]) -> Array1<f64> {
    Array1::from_iter(
        x.iter()
            .zip(bounds.iter())
            .map(|(&v, &(lo, hi))| v.clamp(lo, hi)),
    )
}

fn random_point(bounds: &[(f64, f64)], rng: &mut StdRng) -> Array1<f64> {
    Array1::from_iter(bounds.iter().map(|&(lo, hi)| {
        if hi <= lo {
            lo
        } else {
            rng.random_range(lo..=hi)
        }
    }))
}

fn squared_distance(a: &Array1<f64>, b: &Array1<f64>) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| (x - y).powi(2)).sum()
}

fn pareto_solutions(population: &[BayesParetoSolution]) -> Vec<BayesParetoSolution> {
    let mut front = Vec::new();
    for (i, solution) in population.iter().enumerate() {
        let dominated = population
            .iter()
            .enumerate()
            .any(|(j, other)| i != j && dominates(&other.objectives, &solution.objectives));
        if !dominated {
            front.push(solution.clone());
        }
    }
    front
}

fn pareto_values(values: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let mut front = Vec::new();
    for (i, value) in values.iter().enumerate() {
        let dominated = values
            .iter()
            .enumerate()
            .any(|(j, other)| i != j && dominates(other, value));
        if !dominated {
            front.push(value.clone());
        }
    }
    front
}

fn dominates(a: &[f64], b: &[f64]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| x <= y) && a.iter().zip(b.iter()).any(|(x, y)| x < y)
}

fn reference_point(values: &[Vec<f64>]) -> Vec<f64> {
    let m = values.first().map(|v| v.len()).unwrap_or(0);
    let mut ideal = vec![f64::INFINITY; m];
    let mut nadir = vec![f64::NEG_INFINITY; m];
    for v in values {
        for j in 0..m {
            ideal[j] = ideal[j].min(v[j]);
            nadir[j] = nadir[j].max(v[j]);
        }
    }
    (0..m)
        .map(|j| {
            let span = (nadir[j] - ideal[j]).abs().max(1.0);
            nadir[j] + 0.2 * span
        })
        .collect()
}

struct HypervolumeSamples {
    points: Vec<Vec<f64>>,
    front_dominated: Vec<bool>,
    volume: f64,
}

impl HypervolumeSamples {
    fn new(
        front: &[Vec<f64>],
        lower: &[f64],
        reference: &[f64],
        draws: usize,
        rng: &mut StdRng,
    ) -> Self {
        if lower.len() != reference.len() || lower.iter().zip(reference).any(|(lo, hi)| lo >= hi) {
            return Self {
                points: Vec::new(),
                front_dominated: Vec::new(),
                volume: 0.0,
            };
        }

        let volume = lower
            .iter()
            .zip(reference.iter())
            .map(|(lo, hi)| hi - lo)
            .product::<f64>();
        let mut points = Vec::with_capacity(draws.max(1));
        let mut front_dominated = Vec::with_capacity(draws.max(1));
        for _ in 0..draws.max(1) {
            let point = lower
                .iter()
                .zip(reference.iter())
                .map(|(lo, hi)| lo + rng.random::<f64>() * (hi - lo))
                .collect::<Vec<_>>();
            front_dominated.push(front.iter().any(|p| dominates_or_equal(p, &point)));
            points.push(point);
        }

        Self {
            points,
            front_dominated,
            volume,
        }
    }
}

fn hypervolume_integration_lower(
    front: &[Vec<f64>],
    gps: &[GaussianProcess],
    candidates_norm: &[Array1<f64>],
    reference: &[f64],
) -> Vec<f64> {
    let m = reference.len();
    let mut lower = vec![f64::INFINITY; m];
    for point in front {
        for j in 0..m {
            lower[j] = lower[j].min(point[j]);
        }
    }
    for (j, gp) in gps.iter().enumerate().take(m) {
        for x in candidates_norm {
            let (mean, std) = gp.predict(x);
            lower[j] = lower[j].min(mean - 4.0 * std);
        }
    }
    for j in 0..m {
        if !lower[j].is_finite() {
            lower[j] = reference[j] - 1.0;
        }
        if lower[j] >= reference[j] {
            lower[j] = reference[j] - 1.0;
        }
    }
    lower
}

fn expected_hypervolume_improvement_mc(
    gps: &[GaussianProcess],
    batch_norm: &[Array1<f64>],
    reference: &[f64],
    hv_samples: &HypervolumeSamples,
    normals: &[Vec<Vec<f64>>],
) -> f64 {
    if gps.is_empty() || batch_norm.is_empty() || normals.is_empty() {
        return 0.0;
    }

    let joint = gps
        .iter()
        .map(|gp| gp.predict_joint(batch_norm))
        .collect::<Vec<_>>();
    let mut total = 0.0;
    for draw in normals {
        let mut sampled_points = vec![vec![0.0; gps.len()]; batch_norm.len()];
        for (objective, (means, cov)) in joint.iter().enumerate() {
            let sample = sample_multivariate_normal(means, cov, &draw[objective]);
            for i in 0..batch_norm.len() {
                sampled_points[i][objective] = sample[i];
            }
        }
        total += hypervolume_improvement_from_samples(&sampled_points, reference, hv_samples);
    }
    total / normals.len() as f64
}

fn hypervolume_improvement_from_samples(
    candidates: &[Vec<f64>],
    reference: &[f64],
    hv_samples: &HypervolumeSamples,
) -> f64 {
    if candidates
        .iter()
        .any(|point| point.len() != reference.len())
    {
        return 0.0;
    }

    let mut improved = 0usize;
    for (point, front_dominated) in hv_samples
        .points
        .iter()
        .zip(hv_samples.front_dominated.iter())
    {
        if *front_dominated {
            continue;
        }
        if candidates
            .iter()
            .any(|candidate| dominates_or_equal(candidate, point))
        {
            improved += 1;
        }
    }
    hv_samples.volume * improved as f64 / hv_samples.points.len().max(1) as f64
}

fn dominates_or_equal(a: &[f64], b: &[f64]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| x <= y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_improvement_prefers_better_mean() {
        let better = expected_improvement(1.0, 0.5, 0.1, 0.0);
        let worse = expected_improvement(1.0, 1.5, 0.1, 0.0);
        assert!(better > worse);
    }

    #[test]
    fn learned_lengthscales_detect_irrelevant_dimension() {
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for i in 0..5 {
            for j in 0..5 {
                let x0 = i as f64 / 4.0;
                let x1 = j as f64 / 4.0;
                xs.push(Array1::from(vec![x0, x1]));
                ys.push((x0 - 0.35).powi(2));
            }
        }
        let learned = learn_lengthscales(&xs, &ys, &[0.25, 0.25], 1.0, 1e-8).unwrap();
        assert!(
            learned[1] > learned[0],
            "irrelevant dimension should get longer lengthscale: {learned:?}"
        );
    }

    #[test]
    fn gp_prediction_reports_latent_not_observation_variance() {
        let xs = vec![Array1::from(vec![0.0]), Array1::from(vec![1.0])];
        let ys = vec![0.0, 1.0];
        let gp = GaussianProcess::fit(&xs, &ys, &[0.4], 1.0, 1e-2).unwrap();
        let (_, std) = gp.predict(&Array1::from(vec![0.0]));
        assert!(
            std < 0.08,
            "latent posterior std should not include observation noise: {std}"
        );
    }

    #[test]
    fn qei_batch_value_includes_joint_improvement() {
        let xs = vec![Array1::from(vec![0.0]), Array1::from(vec![1.0])];
        let ys = vec![0.0, 1.0];
        let gp = GaussianProcess::fit(&xs, &ys, &[0.5], 1.0, 1e-8).unwrap();
        let batch = vec![Array1::from(vec![0.25]), Array1::from(vec![0.75])];
        let normals = vec![vec![0.0, 0.0], vec![-1.0, 1.0], vec![1.0, -1.0]];
        let score = q_expected_improvement_mc(&gp, &batch, 0.5, 0.0, &normals);
        assert!(score.is_finite() && score > 0.0);
    }

    #[test]
    fn bayes_opt_converges_on_quadratic() {
        let f = |x: &Array1<f64>| -> f64 { (x[0] - 0.25).powi(2) + (x[1] + 0.5).powi(2) };
        let cfg = BayesOptConfig {
            bounds: vec![(-1.0, 1.0), (-1.0, 1.0)],
            maxeval: 60,
            initial_samples: 8,
            batch_size: 2,
            candidate_pool_size: 128,
            seed: Some(7),
            ..Default::default()
        };
        let report = bayesian_optimization(&f, cfg).unwrap();
        assert!(report.fun < 0.05, "fun={}", report.fun);
    }

    #[test]
    fn multi_objective_returns_non_dominated_front() {
        let f = |x: &Array1<f64>| -> Vec<f64> { vec![x[0].powi(2), (x[0] - 1.0).powi(2)] };
        let cfg = BayesOptConfig {
            bounds: vec![(0.0, 1.0)],
            maxeval: 20,
            initial_samples: 6,
            batch_size: 2,
            candidate_pool_size: 64,
            seed: Some(3),
            ..Default::default()
        };
        let report = bayesian_multi_objective(&f, cfg).unwrap();
        assert!(!report.pareto_front.is_empty());
    }
}
