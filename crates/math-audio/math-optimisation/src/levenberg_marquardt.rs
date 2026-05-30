//! Levenberg-Marquardt bounded nonlinear least-squares solver.
//!
//! Solves `min_x  ||r(x)||^2_W` subject to `lb <= x <= ub`, where `r(x)` is a
//! vector-valued residual function and `W` is a diagonal weight matrix.
//!
//! The solver interpolates between Gauss-Newton (fast near the minimum) and
//! gradient descent (robust far from it) via an adaptive damping parameter `lambda`.
//! Bounds are enforced by projecting the trial step.

use ndarray::{Array1, Array2};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from the Levenberg-Marquardt solver.
#[derive(Debug, Error)]
pub enum LMError {
    /// Lower and upper bounds have different lengths, or don't match x0.
    #[error("bounds/x0 dimension mismatch: x0 has {x0_len}, bounds has {bounds_len}")]
    DimensionMismatch {
        /// Length of x0
        x0_len: usize,
        /// Number of bound pairs
        bounds_len: usize,
    },

    /// A lower bound exceeds its upper bound.
    #[error("invalid bounds at index {index}: lower ({lower}) > upper ({upper})")]
    InvalidBounds {
        /// Index of the bad pair
        index: usize,
        /// Lower bound
        lower: f64,
        /// Upper bound
        upper: f64,
    },

    /// Residual function returned different sizes on successive calls.
    #[error("residual dimension changed: was {expected}, now {got}")]
    ResidualDimensionChanged {
        /// First observed size
        expected: usize,
        /// New size
        got: usize,
    },

    /// The Jacobian-derived system is singular (QR rank-deficient).
    #[error("singular Jacobian — cannot compute step")]
    SingularJacobian,
}

/// Result alias for LM operations.
pub type LMResult<T> = std::result::Result<T, LMError>;

// ---------------------------------------------------------------------------
// Callback
// ---------------------------------------------------------------------------

/// Action returned by a progress callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LMCallbackAction {
    /// Keep iterating.
    Continue,
    /// Stop early.
    Stop,
}

/// Snapshot of solver state passed to the callback.
pub struct LMIntermediate {
    /// Current parameter vector.
    pub x: Array1<f64>,
    /// Current objective value (weighted sum of squared residuals).
    pub fun: f64,
    /// Current damping parameter.
    pub lambda: f64,
    /// Iteration number.
    pub iter: usize,
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Progress callback type for the LM solver.
pub type LMCallback = Box<dyn FnMut(&LMIntermediate) -> LMCallbackAction>;

/// Configuration for the LM solver.
pub struct LMConfig {
    /// Maximum iterations (default 100).
    pub maxiter: usize,
    /// Relative convergence tolerance on the objective.
    pub tol: f64,
    /// Absolute convergence tolerance on the objective.
    pub atol: f64,
    /// Initial damping parameter (default 1.0).
    pub lambda_init: f64,
    /// Finite-difference step for Jacobian approximation (default 1e-8).
    pub jacobian_epsilon: f64,
    /// Initial guess (required).
    pub x0: Array1<f64>,
    /// Per-residual weights (optional; default = uniform).
    pub weights: Option<Array1<f64>>,
    /// Print progress to stderr.
    pub disp: bool,
    /// Optional progress callback.
    pub callback: Option<LMCallback>,
}

/// Fluent builder for [`LMConfig`].
pub struct LMConfigBuilder {
    maxiter: usize,
    tol: f64,
    atol: f64,
    lambda_init: f64,
    jacobian_epsilon: f64,
    x0: Option<Array1<f64>>,
    weights: Option<Array1<f64>>,
    disp: bool,
    callback: Option<LMCallback>,
}

impl LMConfigBuilder {
    /// Create a new builder with defaults.
    pub fn new() -> Self {
        Self {
            maxiter: 100,
            tol: 1e-10,
            atol: 1e-14,
            lambda_init: 1.0,
            jacobian_epsilon: 1e-8,
            x0: None,
            weights: None,
            disp: false,
            callback: None,
        }
    }

    /// Set the initial guess (required).
    pub fn x0(mut self, x0: Array1<f64>) -> Self {
        self.x0 = Some(x0);
        self
    }

    /// Maximum number of iterations.
    pub fn maxiter(mut self, n: usize) -> Self {
        self.maxiter = n;
        self
    }

    /// Relative convergence tolerance.
    pub fn tol(mut self, t: f64) -> Self {
        self.tol = t;
        self
    }

    /// Absolute convergence tolerance.
    pub fn atol(mut self, t: f64) -> Self {
        self.atol = t;
        self
    }

    /// Initial damping factor.
    pub fn lambda_init(mut self, l: f64) -> Self {
        self.lambda_init = l;
        self
    }

    /// Finite-difference epsilon for Jacobian.
    pub fn jacobian_epsilon(mut self, eps: f64) -> Self {
        self.jacobian_epsilon = eps;
        self
    }

    /// Per-residual weights.
    pub fn weights(mut self, w: Array1<f64>) -> Self {
        self.weights = Some(w);
        self
    }

    /// Print progress.
    pub fn disp(mut self, d: bool) -> Self {
        self.disp = d;
        self
    }

    /// Progress callback.
    pub fn callback(mut self, cb: Box<dyn FnMut(&LMIntermediate) -> LMCallbackAction>) -> Self {
        self.callback = Some(cb);
        self
    }

    /// Build the config. Panics if `x0` was not set.
    pub fn build(self) -> LMConfig {
        LMConfig {
            maxiter: self.maxiter,
            tol: self.tol,
            atol: self.atol,
            lambda_init: self.lambda_init,
            jacobian_epsilon: self.jacobian_epsilon,
            x0: self.x0.expect("LMConfigBuilder: x0 is required"),
            weights: self.weights,
            disp: self.disp,
            callback: self.callback,
        }
    }
}

impl Default for LMConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// Result of a Levenberg-Marquardt optimization.
#[derive(Debug)]
pub struct LMReport {
    /// Optimal parameter vector.
    pub x: Array1<f64>,
    /// Final objective value (weighted sum of squared residuals).
    pub fun: f64,
    /// Final residual vector.
    pub residuals: Array1<f64>,
    /// Whether the solver converged.
    pub success: bool,
    /// Human-readable termination message.
    pub message: String,
    /// Number of iterations performed.
    pub nit: usize,
    /// Number of residual function evaluations.
    pub nfev: usize,
}

// ---------------------------------------------------------------------------
// Solver
// ---------------------------------------------------------------------------

/// Bounded Levenberg-Marquardt nonlinear least-squares solver.
///
/// Minimizes `sum_i  w_i * r_i(x)^2` subject to `bounds[j].0 <= x[j] <= bounds[j].1`.
///
/// # Arguments
/// * `residual_fn` — returns the residual vector `r(x)`.
/// * `bounds` — per-parameter `(lower, upper)` pairs.
/// * `config` — solver configuration (must include `x0`).
pub fn levenberg_marquardt<F>(
    residual_fn: &F,
    bounds: &[(f64, f64)],
    config: LMConfig,
) -> LMResult<LMReport>
where
    F: Fn(&Array1<f64>) -> Array1<f64>,
{
    let n_params = config.x0.len();

    // --- Validate inputs ---
    if bounds.len() != n_params {
        return Err(LMError::DimensionMismatch {
            x0_len: n_params,
            bounds_len: bounds.len(),
        });
    }
    for (i, &(lo, hi)) in bounds.iter().enumerate() {
        if lo > hi {
            return Err(LMError::InvalidBounds {
                index: i,
                lower: lo,
                upper: hi,
            });
        }
    }

    // --- Initialize ---
    let mut x = project(&config.x0, bounds);
    let mut r = residual_fn(&x);
    let n_residuals = r.len();
    let mut nfev: usize = 1;

    let w = config.weights.unwrap_or_else(|| Array1::ones(n_residuals));
    let mut f_val = weighted_sos(&r, &w);
    let mut lambda = config.lambda_init;
    let eps = config.jacobian_epsilon;

    let mut success = false;
    let mut message = format!("max iterations ({}) reached", config.maxiter);
    let mut callback = config.callback;
    let mut nit: usize = 0;

    for iter in 0..config.maxiter {
        nit = iter + 1;
        // --- Callback ---
        if let Some(ref mut cb) = callback {
            let intermediate = LMIntermediate {
                x: x.clone(),
                fun: f_val,
                lambda,
                iter,
            };
            if cb(&intermediate) == LMCallbackAction::Stop {
                message = "stopped by callback".to_string();
                break;
            }
        }

        // --- Check for already-converged ---
        if f_val <= config.atol {
            success = true;
            message = format!(
                "converged: objective {:.2e} <= atol {:.2e}",
                f_val, config.atol
            );
            break;
        }

        // --- Compute Jacobian via central finite differences ---
        let jac = compute_jacobian(residual_fn, &x, &r, n_residuals, eps, &mut nfev)?;

        // --- Form damped normal equations ---
        // LM step solves: (J^T W J + λ·D) δ = −J^T W r
        // where D = diag(J^T W J) (Marquardt scaling).
        let jtwj = jtw_j(&jac, &w);
        let jtwr = jtw_r(&jac, &w, &r); // gradient direction

        // Marquardt diagonal scaling
        let diag: Array1<f64> = (0..n_params).map(|j| jtwj[[j, j]].max(1e-20)).collect();

        // H = J^T W J + λ·diag(D)
        let mut h = jtwj.clone();
        for j in 0..n_params {
            h[[j, j]] += lambda * diag[j];
        }

        // RHS = −J^T W r  (descent direction)
        let neg_jtwr = jtwr.mapv(|v| -v);

        // Solve H · δ = −J^T W r
        let delta = match solve_linear_system(&h, &neg_jtwr) {
            Some(d) => d,
            None => {
                // Increase damping and retry
                lambda *= 4.0;
                continue;
            }
        };

        // --- Trial step with bounds projection ---
        let x_trial = project(&(&x + &delta), bounds);
        let r_trial = residual_fn(&x_trial);
        nfev += 1;

        if r_trial.len() != n_residuals {
            return Err(LMError::ResidualDimensionChanged {
                expected: n_residuals,
                got: r_trial.len(),
            });
        }

        let f_trial = weighted_sos(&r_trial, &w);

        // --- Gain ratio ---
        // Model reduction for f(x) = r^T W r with step δ solving (JTWJ + λD)δ = −jtwr:
        //   pred = δ^T JTWJ δ + 2λ δ^T D δ
        let pred: f64 = {
            let mut p = 0.0;
            for j in 0..n_params {
                let mut jtwj_delta_j = 0.0;
                for k in 0..n_params {
                    jtwj_delta_j += jtwj[[j, k]] * delta[k];
                }
                p += delta[j] * jtwj_delta_j + 2.0 * lambda * diag[j] * delta[j] * delta[j];
            }
            p
        };

        let actual = f_val - f_trial;
        let rho = if pred.abs() > 1e-30 {
            actual / pred
        } else {
            0.0
        };

        if rho > 0.0 && f_trial < f_val {
            // Accept step
            let f_old = f_val;
            x = x_trial;
            r = r_trial;
            f_val = f_trial;
            lambda = (lambda / 2.0).max(1e-15);

            // Convergence check
            if (f_old - f_val).abs() < config.tol * f_old + config.atol {
                success = true;
                message = format!(
                    "converged: |df|={:.2e} < tol*f+atol={:.2e}",
                    (f_old - f_val).abs(),
                    config.tol * f_old + config.atol
                );
                break;
            }
        } else {
            // Reject step — increase damping
            lambda = (lambda * 2.0).min(1e15);
        }

        if config.disp && iter % 10 == 0 {
            eprintln!(
                "LM iter {}: f={:.6e}, lambda={:.2e}, rho={:.3}",
                iter, f_val, lambda, rho
            );
        }
    }

    Ok(LMReport {
        x,
        fun: f_val,
        residuals: r,
        success,
        message,
        nit,
        nfev,
    })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Project `x` onto the box `[lb, ub]`.
fn project(x: &Array1<f64>, bounds: &[(f64, f64)]) -> Array1<f64> {
    Array1::from(
        x.iter()
            .zip(bounds.iter())
            .map(|(&xi, &(lo, hi))| xi.clamp(lo, hi))
            .collect::<Vec<_>>(),
    )
}

/// Weighted sum of squares: sum(w_i * r_i^2).
fn weighted_sos(r: &Array1<f64>, w: &Array1<f64>) -> f64 {
    r.iter().zip(w.iter()).map(|(&ri, &wi)| wi * ri * ri).sum()
}

/// Compute Jacobian via central finite differences.
fn compute_jacobian<F>(
    residual_fn: &F,
    x: &Array1<f64>,
    _r0: &Array1<f64>,
    n_residuals: usize,
    eps: f64,
    nfev: &mut usize,
) -> LMResult<Array2<f64>>
where
    F: Fn(&Array1<f64>) -> Array1<f64>,
{
    let n_params = x.len();
    let mut jac = Array2::zeros((n_residuals, n_params));

    for j in 0..n_params {
        let mut x_plus = x.clone();
        let mut x_minus = x.clone();
        let h = jacobian_step(eps, x[j]);
        x_plus[j] += h;
        x_minus[j] -= h;

        let r_plus = residual_fn(&x_plus);
        let r_minus = residual_fn(&x_minus);
        *nfev += 2;

        if r_plus.len() != n_residuals {
            return Err(LMError::ResidualDimensionChanged {
                expected: n_residuals,
                got: r_plus.len(),
            });
        }
        if r_minus.len() != n_residuals {
            return Err(LMError::ResidualDimensionChanged {
                expected: n_residuals,
                got: r_minus.len(),
            });
        }

        let inv_2h = 1.0 / (2.0 * h);
        for i in 0..n_residuals {
            jac[[i, j]] = (r_plus[i] - r_minus[i]) * inv_2h;
        }
    }

    Ok(jac)
}

/// Finite-difference step size scaled to the parameter magnitude.
///
/// Uses the standard relative rule `eps * (1 + |x|)` with a lower bound of
/// `eps`, so coordinates near zero still receive a resolvable central
/// difference step.
fn jacobian_step(eps: f64, x: f64) -> f64 {
    (eps * (1.0 + x.abs())).max(eps)
}

/// Compute J^T W J (n_params x n_params).
fn jtw_j(jac: &Array2<f64>, w: &Array1<f64>) -> Array2<f64> {
    let n_params = jac.ncols();
    let n_res = jac.nrows();
    let mut result = Array2::zeros((n_params, n_params));

    for j in 0..n_params {
        for k in j..n_params {
            let mut val = 0.0;
            for i in 0..n_res {
                val += w[i] * jac[[i, j]] * jac[[i, k]];
            }
            result[[j, k]] = val;
            result[[k, j]] = val;
        }
    }

    result
}

/// Compute J^T W r (n_params vector).
fn jtw_r(jac: &Array2<f64>, w: &Array1<f64>, r: &Array1<f64>) -> Array1<f64> {
    let n_params = jac.ncols();
    let n_res = jac.nrows();
    let mut result = Array1::zeros(n_params);

    for j in 0..n_params {
        let mut val = 0.0;
        for i in 0..n_res {
            val += w[i] * jac[[i, j]] * r[i];
        }
        result[j] = val;
    }

    result
}

/// Solve a dense symmetric positive-definite system Ax = b via Gaussian
/// elimination with partial pivoting. Returns `None` if the matrix is singular.
fn solve_linear_system(a: &Array2<f64>, b: &Array1<f64>) -> Option<Array1<f64>> {
    let n = b.len();
    debug_assert_eq!(a.nrows(), n);
    debug_assert_eq!(a.ncols(), n);

    // Augmented matrix [A | b]
    let mut aug = Array2::zeros((n, n + 1));
    for i in 0..n {
        for j in 0..n {
            aug[[i, j]] = a[[i, j]];
        }
        aug[[i, n]] = b[i];
    }

    // Forward elimination with partial pivoting
    for col in 0..n {
        // Find pivot
        let mut max_val = aug[[col, col]].abs();
        let mut max_row = col;
        for row in (col + 1)..n {
            let val = aug[[row, col]].abs();
            if val > max_val {
                max_val = val;
                max_row = row;
            }
        }

        if max_val < 1e-12 {
            return None; // Singular
        }

        // Swap rows
        if max_row != col {
            for j in 0..=n {
                let tmp = aug[[col, j]];
                aug[[col, j]] = aug[[max_row, j]];
                aug[[max_row, j]] = tmp;
            }
        }

        // Eliminate below
        let pivot = aug[[col, col]];
        for row in (col + 1)..n {
            let factor = aug[[row, col]] / pivot;
            for j in col..=n {
                aug[[row, j]] -= factor * aug[[col, j]];
            }
        }
    }

    // Back substitution
    let mut x = Array1::zeros(n);
    for col in (0..n).rev() {
        let mut sum = aug[[col, n]];
        for j in (col + 1)..n {
            sum -= aug[[col, j]] * x[j];
        }
        x[col] = sum / aug[[col, col]];
    }

    // NaN check
    if x.iter().any(|v| !v.is_finite()) {
        return None;
    }

    Some(x)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_sphere() {
        // r_i = x_i, minimum at origin
        let residual = |x: &Array1<f64>| x.clone();
        let bounds = vec![(-10.0, 10.0); 3];
        let config = LMConfigBuilder::new()
            .x0(array![3.0, -2.0, 1.0])
            .maxiter(50)
            .build();

        let report = levenberg_marquardt(&residual, &bounds, config).unwrap();
        assert!(report.success, "should converge: {}", report.message);
        assert!(
            report.fun < 1e-12,
            "objective should be ~0, got {}",
            report.fun
        );
        for &xi in report.x.iter() {
            assert!(xi.abs() < 1e-6, "x should be ~0, got {}", xi);
        }
    }

    #[test]
    fn test_rosenbrock_residual() {
        // r = [10*(x1 - x0^2), 1 - x0]  =>  min at (1, 1)
        let residual = |x: &Array1<f64>| array![10.0 * (x[1] - x[0] * x[0]), 1.0 - x[0]];
        let bounds = vec![(-5.0, 5.0); 2];
        let config = LMConfigBuilder::new()
            .x0(array![-1.0, 1.0])
            .maxiter(200)
            .tol(1e-12)
            .build();

        let report = levenberg_marquardt(&residual, &bounds, config).unwrap();
        assert!(report.success, "should converge: {}", report.message);
        assert!(
            (report.x[0] - 1.0).abs() < 1e-4,
            "x0 should be ~1, got {}",
            report.x[0]
        );
        assert!(
            (report.x[1] - 1.0).abs() < 1e-4,
            "x1 should be ~1, got {}",
            report.x[1]
        );
    }

    #[test]
    fn test_bounded_solution() {
        // r = [x - 5]  =>  unconstrained min at x=5, but bound x <= 3
        let residual = |x: &Array1<f64>| array![x[0] - 5.0];
        let bounds = vec![(-10.0, 3.0)];
        let config = LMConfigBuilder::new().x0(array![0.0]).maxiter(50).build();

        let report = levenberg_marquardt(&residual, &bounds, config).unwrap();
        assert!(
            (report.x[0] - 3.0).abs() < 1e-6,
            "x should be at bound 3.0, got {}",
            report.x[0]
        );
    }

    #[test]
    fn test_nan_residual_handled() {
        // Residual that returns NaN for large x
        let residual = |x: &Array1<f64>| {
            if x[0].abs() > 100.0 {
                array![f64::NAN]
            } else {
                array![x[0] - 1.0]
            }
        };
        let bounds = vec![(-200.0, 200.0)];
        let config = LMConfigBuilder::new().x0(array![0.0]).maxiter(50).build();

        // Should not panic
        let result = levenberg_marquardt(&residual, &bounds, config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_zero_residual() {
        // Already at the optimum
        let residual = |x: &Array1<f64>| array![x[0], x[1]];
        let bounds = vec![(-10.0, 10.0); 2];
        let config = LMConfigBuilder::new()
            .x0(array![0.0, 0.0])
            .maxiter(10)
            .build();

        let report = levenberg_marquardt(&residual, &bounds, config).unwrap();
        assert!(report.success, "already at optimum: {}", report.message);
        assert!(report.fun < 1e-14);
    }

    #[test]
    fn test_callback_stop() {
        let residual = |x: &Array1<f64>| x.clone();
        let bounds = vec![(-10.0, 10.0); 2];
        let config = LMConfigBuilder::new()
            .x0(array![5.0, 5.0])
            .maxiter(1000)
            .callback(Box::new(|inter| {
                if inter.iter >= 3 {
                    LMCallbackAction::Stop
                } else {
                    LMCallbackAction::Continue
                }
            }))
            .build();

        let report = levenberg_marquardt(&residual, &bounds, config).unwrap();
        assert_eq!(report.message, "stopped by callback");
    }

    #[test]
    fn test_weighted_residuals() {
        // r = [x - 1, x - 3], unweighted min at x=2
        // With w = [10, 1], should pull x closer to 1
        let residual = |x: &Array1<f64>| array![x[0] - 1.0, x[0] - 3.0];
        let bounds = vec![(-10.0, 10.0)];

        // Unweighted
        let config_unw = LMConfigBuilder::new().x0(array![0.0]).maxiter(50).build();
        let report_unw = levenberg_marquardt(&residual, &bounds, config_unw).unwrap();

        // Weighted (10x weight on first residual)
        let config_w = LMConfigBuilder::new()
            .x0(array![0.0])
            .maxiter(50)
            .weights(array![10.0, 1.0])
            .build();
        let report_w = levenberg_marquardt(&residual, &bounds, config_w).unwrap();

        // Unweighted should land at ~2.0
        assert!(
            (report_unw.x[0] - 2.0).abs() < 0.01,
            "unweighted x should be ~2, got {}",
            report_unw.x[0]
        );
        // Weighted should be closer to 1.0
        assert!(
            report_w.x[0] < report_unw.x[0],
            "weighted x ({}) should be less than unweighted ({})",
            report_w.x[0],
            report_unw.x[0]
        );
        assert!(
            (report_w.x[0] - 1.0).abs() < 0.5,
            "weighted x should be near 1.0, got {}",
            report_w.x[0]
        );
    }

    #[test]
    fn test_dimension_mismatch() {
        let residual = |x: &Array1<f64>| x.clone();
        let bounds = vec![(-1.0, 1.0); 3]; // 3 bounds
        let config = LMConfigBuilder::new()
            .x0(array![0.0, 0.0]) // 2 params
            .build();

        let err = levenberg_marquardt(&residual, &bounds, config).unwrap_err();
        assert!(matches!(err, LMError::DimensionMismatch { .. }));
    }

    #[test]
    fn test_nit_tracks_iterations_not_nfev() {
        // nit should count outer iterations, nfev counts residual evaluations
        // For a 2-param problem, each iteration does 1 trial + 4 FD evals = 5+ nfev
        let residual = |x: &Array1<f64>| x.clone();
        let bounds = vec![(-10.0, 10.0); 2];
        let config = LMConfigBuilder::new()
            .x0(array![5.0, 5.0])
            .maxiter(5)
            .tol(1e-20) // don't converge early
            .atol(0.0)
            .build();

        let report = levenberg_marquardt(&residual, &bounds, config).unwrap();
        assert_eq!(
            report.nit, 5,
            "nit should be maxiter (5), got {}",
            report.nit
        );
        assert!(
            report.nfev > report.nit,
            "nfev ({}) should be much larger than nit ({})",
            report.nfev,
            report.nit
        );
    }

    #[test]
    fn test_invalid_bounds() {
        let residual = |x: &Array1<f64>| x.clone();
        let bounds = vec![(5.0, 1.0)]; // lower > upper
        let config = LMConfigBuilder::new().x0(array![0.0]).build();

        let err = levenberg_marquardt(&residual, &bounds, config).unwrap_err();
        assert!(matches!(err, LMError::InvalidBounds { .. }));
    }

    #[test]
    fn test_jacobian_step_size_uses_relative_scale() {
        let eps = 1e-8;
        assert!((jacobian_step(eps, 0.0) - eps).abs() < 1e-15);
        assert!((jacobian_step(eps, 1.0) - 2.0e-8).abs() < 1e-15);
        let h_large = jacobian_step(eps, 1.0e12);
        assert!(h_large >= eps);
        assert!((h_large - eps * (1.0 + 1.0e12)).abs() < 1.0);
    }

    #[test]
    fn test_large_x0_lm_converges() {
        // Verify LM still converges from a large x0 after the step-size fix.
        let residual = |x: &Array1<f64>| array![x[0] - 1.0e6];
        let bounds = vec![(0.0, 2.0e6)];
        let config = LMConfigBuilder::new()
            .x0(array![1.0e8])
            .maxiter(100)
            .build();
        let report = levenberg_marquardt(&residual, &bounds, config).unwrap();
        assert!(report.success, "should converge: {}", report.message);
        assert!(
            (report.x[0] - 1.0e6).abs() < 1.0,
            "x should be ~1e6, got {}",
            report.x[0]
        );
    }

    #[test]
    fn test_linear_solver_rejects_near_singular() {
        // A matrix with a very small pivot (~1e-15) should be treated as singular
        // rather than trusted to produce a meaningful step.
        let a = Array2::from_shape_vec((2, 2), vec![1.0, 1.0, 1.0, 1.0 + 1e-15]).unwrap();
        let b = Array1::from(vec![1.0, 1.0]);
        assert!(
            solve_linear_system(&a, &b).is_none(),
            "near-singular matrix should be rejected"
        );
    }
}
