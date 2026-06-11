//! Pure-Rust COBYLA local optimizer.
//!
//! Wraps the [`cobyla`](https://crates.io/crates/cobyla) crate (a faithful
//! pure-Rust port of NLopt's `cobyla.c`, itself a port of M.J.D. Powell's
//! 1994 FORTRAN implementation) behind an API consistent with this crate's
//! [`differential_evolution`](crate::differential_evolution) and
//! [`levenberg_marquardt`](crate::levenberg_marquardt) entry points.
//!
//! # Constraint convention
//!
//! Inequality constraints are passed in the **autoeq/NLopt convention**:
//! `g_i(x) <= 0` is feasible. Internally this module flips the sign before
//! handing them to the `cobyla` crate, which uses the opposite convention
//! (`fc(x) >= 0` is feasible).
//!
//! # Example
//!
//! ```rust
//! use math_audio_optimisation::cobyla::{cobyla, CobylaConfig, CobylaConstraint, CobylaRhoBegin};
//! use ndarray::Array1;
//! use std::sync::Arc;
//!
//! // Minimize 10*(x0+1)^2 + x1^2 subject to x0 >= 0
//! // (i.e. constraint  -x0 <= 0  in our convention)
//! let f = |x: &Array1<f64>| 10.0 * (x[0] + 1.0).powi(2) + x[1].powi(2);
//! let constraints = vec![CobylaConstraint {
//!     fun: Arc::new(|x: &Array1<f64>| -x[0]),
//! }];
//! let cfg = CobylaConfig {
//!     x0: Array1::from(vec![1.0, 1.0]),
//!     bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
//!     maxeval: 200,
//!     rho_begin: CobylaRhoBegin::All(0.5),
//!     ..Default::default()
//! };
//! let report = cobyla(&f, &constraints, cfg).expect("cobyla should run");
//! assert!(report.x[0] >= -1e-3, "x0 must be roughly >= 0");
//! assert!(report.fun < 10.1, "constrained minimum is at (0, 0) with value 10");
//! ```

use ndarray::Array1;
use std::sync::Arc;

use crate::error::{DEError, Result};

/// Erased inequality-constraint closure. Returns `<= 0` when feasible
/// (NLopt convention; the wrapper flips the sign for the `cobyla` crate).
pub type CobylaConstraintFn = Arc<dyn Fn(&Array1<f64>) -> f64 + Send + Sync>;

/// A single inequality constraint `fun(x) <= 0`.
#[derive(Clone)]
pub struct CobylaConstraint {
    /// Constraint function. Feasible when `<= 0`.
    pub fun: CobylaConstraintFn,
}

/// Termination tolerances passed through to NLopt's `nlopt_stopping`.
#[derive(Debug, Clone, Copy)]
pub struct CobylaStopTols {
    /// Stop when objective drops at or below `stopval`. Disabled if
    /// non-positive (mapped to `-INFINITY` internally). The upstream
    /// `cobyla` crate's `minimize()` hardcoded this to `-INFINITY` —
    /// re-exposing it as a tuning knob.
    pub stopval: f64,
    /// Stop when |Δf| < `ftol_abs`. Disabled if non-positive.
    pub ftol_abs: f64,
    /// Stop when |Δf|/|f| < `ftol_rel`. Disabled if non-positive.
    pub ftol_rel: f64,
    /// Stop when ||Δx||_inf < `xtol_abs`. Disabled if non-positive.
    pub xtol_abs: f64,
    /// Stop when ||Δx||/||x|| < `xtol_rel`. Disabled if non-positive.
    pub xtol_rel: f64,
}

impl Default for CobylaStopTols {
    fn default() -> Self {
        // Match NLopt-via-autoeq settings (the previous `optim/nlopt.rs`
        // before the C-FFI dep was dropped):
        //   set_stopval(1e-4); set_ftol_rel(1e-6); set_xtol_rel(1e-4);
        Self {
            stopval: 1e-4,
            ftol_abs: 0.0,
            ftol_rel: 1e-6,
            xtol_abs: 0.0,
            xtol_rel: 1e-4,
        }
    }
}

/// Initial trust-region radius spec.
///
/// Powell's COBYLA accepts a single `rhobeg` applied to all variables, or
/// per-dimension initial steps. For autoeq's PEQ problem the parameters
/// differ in scale by orders of magnitude (log10-frequency span ~0.5,
/// gain span ~24 dB), so per-dimension steps are recommended.
#[derive(Debug, Clone)]
pub enum CobylaRhoBegin {
    /// Same initial step for every dimension.
    All(f64),
    /// Per-dimension initial steps. Length must equal `x0.len()`.
    PerDim(Vec<f64>),
}

impl Default for CobylaRhoBegin {
    fn default() -> Self {
        Self::All(0.5)
    }
}

/// Configuration for [`cobyla`].
#[derive(Clone)]
pub struct CobylaConfig {
    /// Initial guess. Must lie within `bounds`.
    pub x0: Array1<f64>,
    /// `(lower, upper)` per dimension.
    pub bounds: Vec<(f64, f64)>,
    /// Initial trust-region radius. See [`CobylaRhoBegin`].
    pub rho_begin: CobylaRhoBegin,
    /// Maximum number of objective evaluations.
    pub maxeval: usize,
    /// Termination tolerances.
    pub stop_tol: CobylaStopTols,
}

impl Default for CobylaConfig {
    fn default() -> Self {
        Self {
            x0: Array1::zeros(0),
            bounds: Vec::new(),
            rho_begin: CobylaRhoBegin::default(),
            maxeval: 1000,
            stop_tol: CobylaStopTols::default(),
        }
    }
}

/// Result of a [`cobyla`] run.
#[derive(Clone)]
pub struct CobylaReport {
    /// Best parameter vector found.
    pub x: Array1<f64>,
    /// Objective value at `x`.
    pub fun: f64,
    /// Whether the underlying solver returned a success status.
    pub success: bool,
    /// Human-readable status (Debug-formatted underlying status).
    pub message: String,
    /// Number of function evaluations consumed (best-effort — the
    /// underlying crate does not expose an exact count, so this is the
    /// configured `maxeval` budget when unknown).
    pub nfev: usize,
}

impl std::fmt::Debug for CobylaReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CobylaReport")
            .field("x_len", &self.x.len())
            .field("fun", &self.fun)
            .field("success", &self.success)
            .field("message", &self.message)
            .field("nfev", &self.nfev)
            .finish()
    }
}

/// Minimize `f` subject to bounds and inequality constraints `g_i(x) <= 0`.
///
/// On success returns the best point found. On a non-success termination
/// the report is still populated with the last point evaluated so callers
/// can decide whether the result is usable.
pub fn cobyla<F>(
    f: &F,
    constraints: &[CobylaConstraint],
    config: CobylaConfig,
) -> Result<CobylaReport>
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    let n = config.x0.len();
    if n == 0 {
        return Err(DEError::BoundsMismatch {
            lower_len: 0,
            upper_len: 0,
        });
    }
    if config.bounds.len() != n {
        return Err(DEError::BoundsMismatch {
            lower_len: config.bounds.len(),
            upper_len: n,
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

    // Translate config into the `cobyla_native` API (hand-translated
    // Rust port of NLopt 2.7.1's cobyla.c, replacing the c2rust output).
    use crate::cobyla_native as native;

    // Build per-dim initial steps (rhobeg).
    let dx: Vec<f64> = match &config.rho_begin {
        CobylaRhoBegin::All(r) => vec![*r; n],
        CobylaRhoBegin::PerDim(v) => {
            if v.len() != n {
                return Err(DEError::BoundsMismatch {
                    lower_len: v.len(),
                    upper_len: n,
                });
            }
            v.clone()
        }
    };

    // Stop criteria — hand-port honours these directly (no wrapper bugs to
    // work around).
    let stopval = if config.stop_tol.stopval > 0.0 {
        config.stop_tol.stopval
    } else {
        f64::NEG_INFINITY
    };
    let stop = native::StopCriteria {
        stopval,
        ftol_abs: config.stop_tol.ftol_abs,
        ftol_rel: config.stop_tol.ftol_rel,
        xtol_abs: config.stop_tol.xtol_abs,
        xtol_rel: config.stop_tol.xtol_rel,
        maxeval: config.maxeval,
    };

    // Wrap user-facing closures (Array1) as &[f64] for the native API.
    // Constraint convention is identical: feasible when `g(x) <= 0`.
    let f_native = |x: &[f64]| -> f64 {
        let arr = Array1::from(x.to_vec());
        f(&arr)
    };
    type SliceObj = Box<dyn Fn(&[f64]) -> f64>;
    let cons_native: Vec<SliceObj> = constraints
        .iter()
        .map(|c| {
            let g = c.fun.clone();
            let b: SliceObj = Box::new(move |x: &[f64]| {
                let arr = Array1::from(x.to_vec());
                g(&arr)
            });
            b
        })
        .collect();

    let mut x_buf: Vec<f64> = config.x0.to_vec();
    let report = native::cobyla_native(
        n,
        f_native,
        &cons_native,
        &config.bounds,
        &mut x_buf,
        &dx,
        &stop,
    )?;

    Ok(CobylaReport {
        x: Array1::from(report.x),
        fun: report.fun,
        success: report.success,
        message: report.message.to_string(),
        nfev: report.nfev,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paraboloid_unconstrained() {
        // f(x) = (x0+1)^2 + x1^2, minimum at (-1, 0) with value 0.
        let f = |x: &Array1<f64>| (x[0] + 1.0).powi(2) + x[1].powi(2);
        let cfg = CobylaConfig {
            x0: Array1::from(vec![1.0, 1.0]),
            bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
            rho_begin: CobylaRhoBegin::All(0.5),
            maxeval: 500,
            // Disable stopval so the test can drive convergence below 1e-4.
            stop_tol: CobylaStopTols {
                stopval: 0.0,
                ..CobylaStopTols::default()
            },
        };
        let report = cobyla(&f, &[], cfg).expect("cobyla failed");
        assert!(report.fun < 1e-6, "fun = {} should be ~0", report.fun);
        assert!((report.x[0] - (-1.0)).abs() < 1e-3);
        assert!(report.x[1].abs() < 1e-3);
    }

    #[test]
    fn paraboloid_with_inequality() {
        // Minimize (x0+1)^2 + x1^2 subject to x0 >= 0 (i.e. -x0 <= 0).
        // Constrained optimum at (0, 0) with value 1.
        let f = |x: &Array1<f64>| (x[0] + 1.0).powi(2) + x[1].powi(2);
        let constraints = vec![CobylaConstraint {
            fun: Arc::new(|x: &Array1<f64>| -x[0]),
        }];
        let cfg = CobylaConfig {
            x0: Array1::from(vec![1.0, 1.0]),
            bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
            rho_begin: CobylaRhoBegin::All(0.5),
            maxeval: 500,
            ..Default::default()
        };
        let report = cobyla(&f, &constraints, cfg).expect("cobyla failed");
        // Active constraint at x0 = 0 ⇒ f = 1.
        assert!(
            (report.fun - 1.0).abs() < 1e-3,
            "fun = {} should be ~1",
            report.fun
        );
        assert!(
            report.x[0] >= -1e-3,
            "x0 = {} should respect x0 >= 0",
            report.x[0]
        );
    }

    #[test]
    fn rejects_bad_bounds() {
        let f = |x: &Array1<f64>| x[0] * x[0];
        let cfg = CobylaConfig {
            x0: Array1::from(vec![0.0]),
            bounds: vec![(1.0, -1.0)], // lo > hi
            rho_begin: CobylaRhoBegin::All(0.1),
            maxeval: 100,
            ..Default::default()
        };
        let err = cobyla(&f, &[], cfg).unwrap_err();
        matches!(err, DEError::InvalidBounds { .. });
    }

    #[test]
    fn test_per_dim_rho_begin() {
        let f = |x: &Array1<f64>| (x[0] + 1.0).powi(2) + (x[1]).powi(2);
        let cfg = CobylaConfig {
            x0: Array1::from(vec![1.0, 1.0]),
            bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
            rho_begin: CobylaRhoBegin::PerDim(vec![0.5, 1.0]),
            maxeval: 500,
            stop_tol: CobylaStopTols {
                stopval: 0.0,
                ..CobylaStopTols::default()
            },
        };
        let report = cobyla(&f, &[], cfg).expect("cobyla failed");
        assert!(report.fun < 1e-3, "fun = {} should be ~0", report.fun);
    }

    #[test]
    fn test_per_dim_rho_begin_mismatch() {
        let f = |x: &Array1<f64>| x[0] * x[0];
        let cfg = CobylaConfig {
            x0: Array1::from(vec![1.0, 1.0]),
            bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
            rho_begin: CobylaRhoBegin::PerDim(vec![0.5]), // wrong length
            maxeval: 100,
            ..Default::default()
        };
        let err = cobyla(&f, &[], cfg).unwrap_err();
        assert!(matches!(err, DEError::BoundsMismatch { .. }));
    }

    #[test]
    fn test_multiple_constraints() {
        let f = |x: &Array1<f64>| x[0].powi(2) + x[1].powi(2);
        let constraints = vec![
            CobylaConstraint {
                fun: Arc::new(|x: &Array1<f64>| x[0] - 1.0), // x0 <= 1
            },
            CobylaConstraint {
                fun: Arc::new(|x: &Array1<f64>| -x[1] - 1.0), // x1 >= -1
            },
        ];
        let cfg = CobylaConfig {
            x0: Array1::from(vec![2.0, -2.0]),
            bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
            rho_begin: CobylaRhoBegin::All(0.5),
            maxeval: 1000,
            ..Default::default()
        };
        let report = cobyla(&f, &constraints, cfg).expect("cobyla failed");
        assert!(
            report.x[0] <= 1.01,
            "x0 = {} should respect <= 1",
            report.x[0]
        );
        assert!(
            report.x[1] >= -1.01,
            "x1 = {} should respect >= -1",
            report.x[1]
        );
    }

    #[test]
    fn test_dimension_mismatch_x0_bounds() {
        let f = |x: &Array1<f64>| x[0] * x[0];
        let cfg = CobylaConfig {
            x0: Array1::from(vec![0.0, 0.0]),
            bounds: vec![(-10.0, 10.0)], // mismatch
            rho_begin: CobylaRhoBegin::All(0.5),
            maxeval: 100,
            ..Default::default()
        };
        let err = cobyla(&f, &[], cfg).unwrap_err();
        assert!(matches!(err, DEError::BoundsMismatch { .. }));
    }

    #[test]
    fn test_empty_x0_rejected() {
        let f = |x: &Array1<f64>| x[0] * x[0];
        let cfg = CobylaConfig {
            x0: Array1::zeros(0),
            bounds: vec![(-10.0, 10.0)],
            rho_begin: CobylaRhoBegin::All(0.5),
            maxeval: 100,
            ..Default::default()
        };
        let err = cobyla(&f, &[], cfg).unwrap_err();
        assert!(matches!(err, DEError::BoundsMismatch { .. }));
    }

    #[test]
    fn test_stopval_reached() {
        let f = |x: &Array1<f64>| (x[0] + 1.0).powi(2) + x[1].powi(2);
        let cfg = CobylaConfig {
            x0: Array1::from(vec![1.0, 1.0]),
            bounds: vec![(-10.0, 10.0), (-10.0, 10.0)],
            rho_begin: CobylaRhoBegin::All(0.5),
            maxeval: 1000,
            stop_tol: CobylaStopTols {
                stopval: 0.5, // stop once f <= 0.5
                ..CobylaStopTols::default()
            },
        };
        let report = cobyla(&f, &[], cfg).expect("cobyla failed");
        assert!(
            report.fun <= 0.6,
            "fun = {} should be <= 0.5+eps",
            report.fun
        );
    }
}
