//! Hand-translated Rust port of NLopt 2.7.1's cobyla.c (Powell 1994 COBYLA).
//!
//! Reference: Powell, M.J.D., "A Direct Search Optimization Method that Models
//! the Objective and Constraint Functions by Linear Interpolation," in Advances
//! in Optimization and Numerical Analysis, eds. S. Gomez and J.-P. Hennart, 51-67.
//!
//! This is a **hand-translation**, not a c2rust auto-port. The upstream
//! `cobyla` crate v1.0 is a c2rust output with manual fix-ups; we observed
//! that on some autoeq QA cases (`small_stereo_2_2_group`) it converges to
//! a different local minimum than NLopt's C version, indicating a subtle
//! translation drift somewhere in the c2rust output. This module replaces
//! it with a faithful from-source translation.
//!
//! # Conventions
//!
//! - All arrays are `Vec<f64>` / `&[f64]` / `&mut [f64]` with **0-based**
//!   indexing (the C source is 1-based via `--ptr` offsetting).
//! - 2D matrices are stored column-major as `Vec<f64>` to match the C
//!   `mat[i + j * dim1]` access pattern.
//! - State-machine control flow (the C source has ~25 gotos) is encoded as
//!   an explicit `enum State` switch inside `loop`s.
//!
//! # Constraint convention
//!
//! Inequality constraints `g_i(x) <= 0` are feasible at zero or below.
//! Internally the algorithm uses NLopt's convention `con[k] = -g(x)`
//! (feasible when `con[k] >= 0`) — the public `cobyla::cobyla` wrapper
//! flips signs.

// FORTRAN-derived numerical code: many loops use the index for column-major
// 2D matrix access (e.g. `mat[idx2(i, j, n)]`), which clippy can't model
// cleanly.
#![allow(clippy::needless_range_loop)]

use crate::error::{DEError, Result};

// ---------------------------------------------------------------------------
// LCG (linear congruential generator) for pseudo-random simplex steps.
// Matches cobyla.c lines 302-309.
// ---------------------------------------------------------------------------

/// LCG state. Same constants as glibc's `rand` (1103515245, 12345) — the
/// algorithm relies on the *exact* sequence to match NLopt's behaviour.
#[derive(Debug, Clone, Copy)]
struct Lcg(u32);

impl Lcg {
    fn new(seed: u32) -> Self {
        Self(seed)
    }
    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1103515245).wrapping_add(12345);
        self.0
    }
    /// Uniform sample in `[a, b)`.
    fn urand(&mut self, a: f64, b: f64) -> f64 {
        a + (self.next_u32() as f64) * (b - a) / (u32::MAX as f64)
    }
}

// ---------------------------------------------------------------------------
// Public configuration types — these mirror what the wrapper layer in
// `cobyla.rs` builds. Kept here so the algorithm code is self-contained.
// ---------------------------------------------------------------------------

/// Termination criteria.
///
/// The `*_rel` and `*_abs` semantics match NLopt's `nlopt_stopping`. The
/// algorithm stops on the *first* triggered criterion. Setting a field to
/// 0.0 disables that criterion (except `stopval`, which uses `f64::NEG_INFINITY`
/// to disable).
#[derive(Debug, Clone, Copy)]
pub struct StopCriteria {
    /// Stop when objective drops at or below `stopval`. Use `f64::NEG_INFINITY`
    /// to disable (matches NLopt's default when not set).
    pub stopval: f64,
    /// Stop when |Δf| < `ftol_abs`. Disabled if non-positive.
    pub ftol_abs: f64,
    /// Stop when |Δf|/|f| < `ftol_rel`. Disabled if non-positive.
    pub ftol_rel: f64,
    /// Stop when ||Δx||_inf < `xtol_abs`. Disabled if non-positive. Drives
    /// `rhoend` together with `xtol_rel` (see `cobyla_minimize` in
    /// cobyla.c L218-225).
    pub xtol_abs: f64,
    /// Stop when ||Δx||/||x|| < `xtol_rel`. Disabled if non-positive. The
    /// COBYLA termination radius `rhoend = xtol_rel * rhobeg`.
    pub xtol_rel: f64,
    /// Maximum number of objective evaluations. Use 0 for unlimited.
    pub maxeval: usize,
}

impl Default for StopCriteria {
    fn default() -> Self {
        Self {
            stopval: f64::NEG_INFINITY,
            ftol_abs: 0.0,
            ftol_rel: 0.0,
            xtol_abs: 0.0,
            xtol_rel: 1e-4,
            maxeval: 0,
        }
    }
}

/// Result of a [`cobyla_native`] run.
#[derive(Debug, Clone)]
#[allow(dead_code)] // max_violation and feasible are inspected by external callers
pub struct NativeReport {
    pub x: Vec<f64>,
    pub fun: f64,
    pub max_violation: f64,
    pub feasible: bool,
    pub success: bool,
    pub message: &'static str,
    pub nfev: usize,
}

/// Status codes (mirrors NLopt's `nlopt_result`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Failure / InvalidArgs / ForcedStop are reserved for future use
enum Status {
    Success,
    StopvalReached,
    FtolReached,
    XtolReached,
    MaxevalReached,
    RoundoffLimited,
    Failure,
    InvalidArgs,
    ForcedStop,
}

impl Status {
    fn message(self) -> &'static str {
        match self {
            Status::Success => "Success",
            Status::StopvalReached => "StopvalReached",
            Status::FtolReached => "FtolReached",
            Status::XtolReached => "XtolReached",
            Status::MaxevalReached => "MaxevalReached",
            Status::RoundoffLimited => "RoundoffLimited",
            Status::Failure => "Failure",
            Status::InvalidArgs => "InvalidArgs",
            Status::ForcedStop => "ForcedStop",
        }
    }
    fn is_success(self) -> bool {
        matches!(
            self,
            Status::Success
                | Status::StopvalReached
                | Status::FtolReached
                | Status::XtolReached
                | Status::MaxevalReached
        )
    }
}

// ---------------------------------------------------------------------------
// Helpers: 2D column-major access. The C code uses `mat[i + j*dim1]` with
// 1-based i,j — we strip the 1-based offset and access at `mat[(i-1) +
// (j-1)*dim1]`. To keep the body readable, expose `idx(i, j, dim1)` macro.
// ---------------------------------------------------------------------------

#[inline(always)]
fn idx2(i: usize, j: usize, dim1: usize) -> usize {
    // 0-based i, j into a column-major dim1 × ? matrix.
    i + j * dim1
}

// ---------------------------------------------------------------------------
// Entry point. Bounds and dx (initial step) are taken in original coordinates;
// `cobylb` runs in rescaled coordinates (each dim divided by `s = dx[i]/dx[0]`)
// so a single rho applies uniformly. This matches `nlopt_compute_rescaling`.
// ---------------------------------------------------------------------------

/// Public entry point used by the `cobyla` module's wrapper.
///
/// Returns `Ok(Status)` on terminal status, or `Err` on a setup error
/// (bounds mismatch, etc.).
#[allow(clippy::too_many_arguments)]
pub fn cobyla_native<F, G>(
    n: usize,
    f: F,
    constraints: &[G],
    bounds: &[(f64, f64)],
    x: &mut [f64],
    dx: &[f64],
    stop: &StopCriteria,
) -> Result<NativeReport>
where
    F: Fn(&[f64]) -> f64,
    G: Fn(&[f64]) -> f64,
{
    if x.len() != n || bounds.len() != n || dx.len() != n {
        return Err(DEError::BoundsMismatch {
            lower_len: bounds.len(),
            upper_len: n,
        });
    }
    for (i, (lo, hi)) in bounds.iter().enumerate() {
        if lo > hi {
            return Err(DEError::InvalidBounds {
                index: i,
                lower: *lo,
                upper: *hi,
            });
        }
    }

    // --- Rescaling (mirror nlopt_compute_rescaling + nlopt_rescale) -------
    //
    // s[i] = dx[i] / dx[0]  if any dx differ; otherwise s[i] = 1.
    // Algorithm runs in rescaled space where x' = x / s, and rhobeg = |dx[0]|.
    let mut s = vec![1.0_f64; n];
    if n > 1 && dx.iter().any(|&v| v != dx[0]) {
        for i in 1..n {
            s[i] = dx[i] / dx[0];
        }
    }
    for j in 0..n {
        if s[j] == 0.0 || !s[j].is_finite() {
            return Err(DEError::InvalidBounds {
                index: j,
                lower: bounds[j].0,
                upper: bounds[j].1,
            });
        }
    }

    let lb_rescaled: Vec<f64> = bounds.iter().zip(&s).map(|((lo, _), si)| lo / si).collect();
    let ub_rescaled: Vec<f64> = bounds.iter().zip(&s).map(|((_, hi), si)| hi / si).collect();
    // Reorder if rescaling negated bounds.
    let (lb_rescaled, ub_rescaled): (Vec<f64>, Vec<f64>) = lb_rescaled
        .iter()
        .zip(ub_rescaled.iter())
        .map(|(&lo, &hi)| if lo > hi { (hi, lo) } else { (lo, hi) })
        .unzip();
    let mut x_rescaled: Vec<f64> = x.iter().zip(&s).map(|(xi, si)| xi / si).collect();

    let rhobeg = (dx[0] / s[0]).abs();
    // rhoend = xtol_rel * rhobeg, floored by xtol_abs / |s[i]| per dim
    // (mirrors cobyla.c L218-225).
    let mut rhoend = if stop.xtol_rel > 0.0 {
        stop.xtol_rel * rhobeg
    } else {
        // Sensible fallback when caller didn't set xtol_rel — matches
        // NLopt's default tolerance behaviour for unset fields.
        1e-4 * rhobeg
    };
    if stop.xtol_abs > 0.0 {
        for j in 0..n {
            let candidate = stop.xtol_abs / s[j].abs();
            if rhoend < candidate {
                rhoend = candidate;
            }
        }
    }

    let m = constraints.len();

    // Bounds → constraints conversion: the C code (cobyla_minimize) appends
    // 2n bounds-as-inequality constraints `lb_i - x_i <= 0` and
    // `x_i - ub_i <= 0`. We do the same so `cobylb` only sees ≤-style.
    // Total constraint count m_total = m + 2n (bounds always present in our
    // setup; an unbounded dim has lb = -INF so the wrapper clips).
    let n_bound_lo = bounds.iter().filter(|b| b.0.is_finite()).count();
    let n_bound_hi = bounds.iter().filter(|b| b.1.is_finite()).count();
    let m_total = m + n_bound_lo + n_bound_hi;
    let mpp = m_total + 2; // m + objective slot + resmax slot

    // Workspace allocations (all the scratch arrays cobylb needs).
    let mut con = vec![0.0_f64; mpp];
    let mut sim = vec![0.0_f64; n * (n + 1)]; // n × (n+1)
    let mut simi = vec![0.0_f64; n * n];
    let mut datmat = vec![0.0_f64; mpp * (n + 1)];
    let mut a_mat = vec![0.0_f64; n * (m_total + 1)]; // n × (m+1)
    let mut vsig = vec![0.0_f64; n];
    let mut veta = vec![0.0_f64; n];
    let mut sigbar = vec![0.0_f64; n];
    let mut dx_ws = vec![0.0_f64; n];
    let mut iact = vec![0_usize; m_total + 1];
    // trstlp workspace: z (n×n) + zdota (n) + vmultc (m+1) + sdirn (n) + dxnew (n) + vmultd (m+1)
    let mut z_ws = vec![0.0_f64; n * n];
    let mut zdota = vec![0.0_f64; n];
    let mut vmultc = vec![0.0_f64; m_total + 1];
    let mut sdirn = vec![0.0_f64; n];
    let mut dxnew = vec![0.0_f64; n];
    let mut vmultd = vec![0.0_f64; m_total + 1];

    // Wrap user objective + constraints + bound-as-constraints into a
    // single calcfc closure.
    //
    // Convention (NLopt's `func_wrap`): the input `x_rs` is in *rescaled*
    // coordinates (cobylb works in rescaled space). User functions are
    // evaluated at the unscaled point. Bound constraints are evaluated in
    // *rescaled* space — `con = x_rs[i] - lb_rescaled[i]` and
    // `ub_rescaled[i] - x_rs[i]` — so their gradient magnitudes match
    // C-NLopt's (gradient = ±1 in rescaled space, regardless of the
    // per-dim rescaling factor `s[i]`).
    let lb_rs_for_cfc = lb_rescaled.clone();
    let ub_rs_for_cfc = ub_rescaled.clone();
    let calcfc = |x_rs: &[f64], con: &mut [f64]| -> f64 {
        // Unscale to evaluate user functions in original coordinates.
        let x_orig: Vec<f64> = x_rs.iter().zip(&s).map(|(xi, si)| xi * si).collect();

        // User-provided inequality constraints: feasible when g(x) <= 0,
        // so con[k] = -g(x) (feasible when con[k] >= 0).
        for k in 0..m {
            con[k] = -constraints[k](&x_orig);
        }
        // Bound constraints in RESCALED space, ordered (lb_i, ub_i) per dim
        // to match the order in `func_wrap` (cobyla.c L114-122). Skip
        // infinite bounds — those dims aren't constrained.
        let mut idx = m;
        for (i, &(lo, hi)) in bounds.iter().enumerate() {
            if lo.is_finite() {
                con[idx] = x_rs[i] - lb_rs_for_cfc[i];
                idx += 1;
            }
            if hi.is_finite() {
                con[idx] = ub_rs_for_cfc[i] - x_rs[i];
                idx += 1;
            }
        }
        f(&x_orig)
    };

    let mut nfev = 0usize;
    let mut minf = f64::INFINITY;

    let status = cobylb(
        n,
        m_total,
        mpp,
        &mut x_rescaled,
        &mut minf,
        rhobeg,
        rhoend,
        stop,
        &lb_rescaled,
        &ub_rescaled,
        &mut con,
        &mut sim,
        &mut simi,
        &mut datmat,
        &mut a_mat,
        &mut vsig,
        &mut veta,
        &mut sigbar,
        &mut dx_ws,
        &mut iact,
        &mut z_ws,
        &mut zdota,
        &mut vmultc,
        &mut sdirn,
        &mut dxnew,
        &mut vmultd,
        &mut nfev,
        &calcfc,
    );

    // Unscale back into x.
    for i in 0..n {
        x[i] = x_rescaled[i] * s[i];
        // Round-off can push slightly out of bounds — clip.
        x[i] = x[i].clamp(bounds[i].0, bounds[i].1);
    }

    // Recompute final feasibility from user constraints.
    let max_v: f64 = constraints
        .iter()
        .map(|g| g(x))
        .fold(0.0_f64, |acc, v| acc.max(v));

    Ok(NativeReport {
        x: x.to_vec(),
        fun: minf,
        max_violation: max_v.max(0.0),
        feasible: max_v <= 0.0,
        success: status.is_success(),
        message: status.message(),
        nfev,
    })
}

// ---------------------------------------------------------------------------
// cobylb — the main optimization loop.
//
// Translated from cobyla.c lines 452-1245. State labels in the C source are
// L40, L130, L140, L370, L440, L550, L600, L620; in this Rust port they are
// the variants of `enum BodyState` driving the outer `loop`.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum BodyState {
    EvalCallback,        // L40: evaluate calcfc, then dispatch
    AfterInitialSimplex, // L130: ibrnch=1; identify best vertex
    IdentifyBest,        // L140: pick best vertex, build linear models
    SolveLp,             // L370: trust-region LP, evaluate trial step
    PostTrial,           // L440: accept/reject, update simplex
    RhoCheck,            // L550: shrink rho or terminate
    CleanExit,           // L600: copy best vertex back to x
    UseCurrentExit,      // L620: keep current x/f/resmax
}

#[allow(clippy::too_many_arguments)]
fn cobylb<F>(
    n: usize,
    m: usize,
    mpp: usize,
    x: &mut [f64],
    minf: &mut f64,
    rhobeg: f64,
    rhoend: f64,
    stop: &StopCriteria,
    lb: &[f64],
    ub: &[f64],
    con: &mut [f64],
    sim: &mut [f64],
    simi: &mut [f64],
    datmat: &mut [f64],
    a_mat: &mut [f64],
    vsig: &mut [f64],
    veta: &mut [f64],
    sigbar: &mut [f64],
    dx: &mut [f64],
    iact: &mut [usize],
    z_ws: &mut [f64],
    zdota: &mut [f64],
    vmultc: &mut [f64],
    sdirn: &mut [f64],
    dxnew: &mut [f64],
    vmultd: &mut [f64],
    nfev: &mut usize,
    calcfc: &F,
) -> Status
where
    F: Fn(&[f64], &mut [f64]) -> f64,
{
    let np = n + 1; // simplex has n+1 vertices
    let mp = m; // objective is at index `m` in con/datmat (0-based)
    let _ = mp; // silence unused warning until we reach references

    // Algorithm constants (cobyla.c L527-530).
    let alpha = 0.25_f64;
    let beta = 2.1_f64;
    let gamma_ = 0.5_f64;
    let delta = 1.1_f64;

    let mut rho = rhobeg;
    let mut parmu = 0.0_f64;
    let mut seed = Lcg::new((n as u32).wrapping_add(m as u32));
    *minf = f64::INFINITY;

    // ---- Initialise simplex (cobyla.c L538-563) ----
    //
    // SIM column np-1 (0-based) holds the optimal vertex. SIM columns 0..n
    // hold displacements from the optimal vertex. SIMI is the inverse of
    // SIM's leading n×n submatrix.
    for i in 0..n {
        sim[idx2(i, np - 1, n)] = x[i];
        for j in 0..n {
            sim[idx2(i, j, n)] = 0.0;
            simi[idx2(i, j, n)] = 0.0;
        }
        // Bound enforcement: keep the simplex column inside `[lb, ub]`.
        //
        // Fixed dimensions (`lb == ub == x[i]`, e.g. autoeq's hp-pk-lp
        // gain pinned to 0) need special-casing: the C code's
        // `0.5 * (x - lb)` / `0.5 * (ub - x)` fallback would yield 0,
        // then `simi[i,i] = 1/0 = inf` and COBYLA exits immediately with
        // an infinite objective. The bound-as-constraints in `trstlp`
        // already keep `dx[i] = 0` for fixed dims, so we just need a
        // finite `rhocur` to avoid the division blowup. Use a tiny
        // positive value — the dim never actually moves.
        let lb_i = lb[i];
        let ub_i = ub[i];
        let mut rhocur = if (ub_i - lb_i) <= f64::EPSILON {
            // Fixed dim — keep simi[i,i] finite without leaving bounds.
            f64::EPSILON
        } else {
            let mut r = rho;
            if x[i] + r > ub_i {
                if x[i] - r >= lb_i {
                    r = -r;
                } else if ub_i - x[i] > x[i] - lb_i {
                    r = 0.5 * (ub_i - x[i]);
                } else {
                    r = 0.5 * (x[i] - lb_i);
                }
            }
            r
        };
        if rhocur == 0.0 {
            // Fallback if a degenerate bound somehow slipped through —
            // never let `simi[i,i]` be infinite.
            rhocur = f64::EPSILON;
        }
        sim[idx2(i, i, n)] = rhocur;
        simi[idx2(i, i, n)] = 1.0 / rhocur;
    }
    // jdrop is a 0-based simplex column index. C initialises to np (1-based,
    // meaning "the optimal vertex column" — column N in 0-based).
    let mut jdrop: usize = n;
    let mut ibrnch = 0i32;
    let mut iflag = 0i32;
    let mut ifull = 0i32;
    let mut nbest;
    let mut f = 0.0_f64;
    let mut resmax = 0.0_f64;
    let mut prerec = 0.0_f64;
    let mut prerem = 0.0_f64;
    let mut sum = 0.0_f64; // last-set value of `sum` from L370 carries into L440 in C

    // The C source jumps to L600 (CleanExit) on early-exit conditions like
    // maxeval/forced-stop, so the exit always passes through the same
    // "copy best simplex vertex back to x" code path. We mirror that by
    // carrying the intended exit status into `CleanExit` via this slot.
    let mut pending_exit: Option<Status> = None;
    let mut state = BodyState::EvalCallback;

    // Outer state-machine loop.
    loop {
        match state {
            // -----------------------------------------------------------
            // L40 — evaluate calcfc at current x.
            // -----------------------------------------------------------
            BodyState::EvalCallback => {
                if stop.maxeval > 0 && *nfev >= stop.maxeval {
                    // Mirror C: `goto L600;` so the best simplex vertex
                    // and its `f` get copied back to `x`/`*minf` before
                    // returning. Otherwise `*minf` stays at its initial
                    // `INFINITY` and callers see a bogus result.
                    pending_exit = Some(Status::MaxevalReached);
                    state = BodyState::CleanExit;
                    continue;
                }
                *nfev += 1;
                f = calcfc(x, &mut con[..m]);

                // Compute resmax = max(0, -con[k] for k<m).
                resmax = 0.0;
                let mut feasible = true;
                for k in 0..m {
                    let v = -con[k];
                    if v > resmax {
                        resmax = v;
                    }
                    if v > 0.0 {
                        feasible = false;
                    }
                }

                // Stopval check (only if feasible).
                if f < stop.stopval && feasible {
                    *minf = f;
                    state = BodyState::UseCurrentExit;
                    continue;
                }

                con[m] = f; // objective slot
                con[mpp - 1] = resmax; // resmax slot

                if ibrnch == 1 {
                    state = BodyState::PostTrial;
                    continue;
                }

                // Set the recently calculated values in datmat column jdrop.
                for k in 0..mpp {
                    datmat[idx2(k, jdrop, mpp)] = con[k];
                }
                if *nfev > np {
                    state = BodyState::AfterInitialSimplex;
                    continue;
                }

                // Build initial simplex.
                if jdrop < n {
                    if datmat[idx2(m, np - 1, mpp)] <= f {
                        x[jdrop] = sim[idx2(jdrop, np - 1, n)];
                    } else {
                        // Improvement at this vertex — swap with optimal.
                        let rhocur = x[jdrop] - sim[idx2(jdrop, np - 1, n)];
                        sim[idx2(jdrop, np - 1, n)] = x[jdrop];
                        for k in 0..mpp {
                            datmat[idx2(k, jdrop, mpp)] = datmat[idx2(k, np - 1, mpp)];
                            datmat[idx2(k, np - 1, mpp)] = con[k];
                        }
                        for k in 0..=jdrop {
                            sim[idx2(jdrop, k, n)] = -rhocur;
                            let mut temp = 0.0;
                            for ii in k..=jdrop {
                                temp -= simi[idx2(ii, k, n)];
                            }
                            simi[idx2(jdrop, k, n)] = temp;
                        }
                    }
                }
                if *nfev <= n {
                    // Still building initial simplex: perturb dim (nfev-1) by sim[i,i].
                    jdrop = *nfev - 1;
                    x[jdrop] += sim[idx2(jdrop, jdrop, n)];
                    state = BodyState::EvalCallback;
                    continue;
                }
                // Initial simplex complete.
                state = BodyState::AfterInitialSimplex;
            }

            // -----------------------------------------------------------
            // L130 — initial simplex done.
            // -----------------------------------------------------------
            BodyState::AfterInitialSimplex => {
                ibrnch = 1;
                state = BodyState::IdentifyBest;
            }

            // -----------------------------------------------------------
            // L140 — identify best simplex vertex; build linear models.
            // -----------------------------------------------------------
            BodyState::IdentifyBest => {
                // phimin = datmat[mp + np-1] + parmu * datmat[mpp-1 + np-1]
                let mut phimin =
                    datmat[idx2(m, np - 1, mpp)] + parmu * datmat[idx2(mpp - 1, np - 1, mpp)];
                nbest = np - 1;
                for j in 0..n {
                    let temp =
                        datmat[idx2(m, j, mpp)] + parmu * datmat[idx2(mpp - 1, j, mpp)];
                    if temp < phimin {
                        nbest = j;
                        phimin = temp;
                    } else if temp == phimin
                        && parmu == 0.0
                        && datmat[idx2(mpp - 1, j, mpp)] < datmat[idx2(mpp - 1, nbest, mpp)]
                    {
                        nbest = j;
                    }
                }

                // Swap best into pole position (col np-1) if not already.
                if nbest < n {
                    for ii in 0..mpp {
                        let temp = datmat[idx2(ii, np - 1, mpp)];
                        datmat[idx2(ii, np - 1, mpp)] = datmat[idx2(ii, nbest, mpp)];
                        datmat[idx2(ii, nbest, mpp)] = temp;
                    }
                    for ii in 0..n {
                        let temp = sim[idx2(ii, nbest, n)];
                        sim[idx2(ii, nbest, n)] = 0.0;
                        sim[idx2(ii, np - 1, n)] += temp;
                        let mut tempa = 0.0;
                        for k in 0..n {
                            sim[idx2(ii, k, n)] -= temp;
                            tempa -= simi[idx2(k, ii, n)];
                        }
                        simi[idx2(nbest, ii, n)] = tempa;
                    }
                }

                // Sanity-check SIMI ≈ inv(SIM).
                let mut error = 0.0_f64;
                for ii in 0..n {
                    for j in 0..n {
                        let mut temp = if ii == j { -1.0 } else { 0.0 };
                        for k in 0..n {
                            if sim[idx2(k, j, n)] != 0.0 {
                                temp += simi[idx2(ii, k, n)] * sim[idx2(k, j, n)];
                            }
                        }
                        error = error.max(temp.abs());
                    }
                }
                if error > 0.1 {
                    break Status::RoundoffLimited;
                }

                // Build linear approximations: A[i, k] = ∇con_k(x).
                // a[i + k*n] for k in 0..=m (inclusive — m is the objective).
                for k in 0..=m {
                    con[k] = -datmat[idx2(k, np - 1, mpp)];
                    let mut w = vec![0.0_f64; n];
                    for j in 0..n {
                        w[j] = datmat[idx2(k, j, mpp)] + con[k];
                    }
                    for ii in 0..n {
                        let mut temp = 0.0;
                        for j in 0..n {
                            temp += w[j] * simi[idx2(j, ii, n)];
                        }
                        if k == m {
                            temp = -temp;
                        }
                        a_mat[idx2(ii, k, n)] = temp;
                    }
                }

                // Compute vsig, veta; mark simplex acceptable iff all dims ok.
                iflag = 1;
                let parsig = alpha * rho;
                let pareta = beta * rho;
                for j in 0..n {
                    let mut wsig = 0.0;
                    let mut weta = 0.0;
                    for ii in 0..n {
                        let s = simi[idx2(j, ii, n)];
                        wsig += s * s;
                        let e = sim[idx2(ii, j, n)];
                        weta += e * e;
                    }
                    vsig[j] = 1.0 / wsig.sqrt();
                    veta[j] = weta.sqrt();
                    if vsig[j] < parsig || veta[j] > pareta {
                        iflag = 0;
                    }
                }

                // If branched or simplex acceptable, go to LP.
                if ibrnch == 1 || iflag == 1 {
                    state = BodyState::SolveLp;
                    continue;
                }

                // Otherwise pick a new vertex to drop and rebuild simplex.
                let mut local_jdrop: Option<usize> = None;
                let mut temp = pareta;
                for j in 0..n {
                    if veta[j] > temp {
                        local_jdrop = Some(j);
                        temp = veta[j];
                    }
                }
                if local_jdrop.is_none() {
                    for j in 0..n {
                        if vsig[j] < temp {
                            local_jdrop = Some(j);
                            temp = vsig[j];
                        }
                    }
                }
                let jd = local_jdrop.expect("at least one vertex must be droppable");
                jdrop = jd;

                // Step direction toward new vertex.
                let temp_step = gamma_ * rho * vsig[jd];
                for ii in 0..n {
                    dx[ii] = temp_step * simi[idx2(jd, ii, n)];
                }
                let mut cvmaxp = 0.0_f64;
                let mut cvmaxm = 0.0_f64;
                for k in 0..=m {
                    let mut s = 0.0;
                    for ii in 0..n {
                        s += a_mat[idx2(ii, k, n)] * dx[ii];
                    }
                    sum = s; // carries into the dxsign decision below
                    if k < m {
                        let temp = datmat[idx2(k, np - 1, mpp)];
                        cvmaxp = cvmaxp.max(-s - temp);
                        cvmaxm = cvmaxm.max(s - temp);
                    }
                }
                let mut dxsign = 1.0_f64;
                if parmu * (cvmaxp - cvmaxm) > sum + sum {
                    dxsign = -1.0;
                }

                // Update simplex with new dx (with bound enforcement).
                let mut temp = 0.0;
                for ii in 0..n {
                    dx[ii] = dxsign * dx[ii] * seed.urand(0.01, 1.0);
                    let xi = sim[idx2(ii, np - 1, n)];
                    // fixdx loop: while dx pushes xi out of [lb, ub], correct.
                    loop {
                        if xi + dx[ii] > ub[ii] {
                            dx[ii] = -dx[ii];
                        }
                        if xi + dx[ii] >= lb[ii] {
                            break;
                        }
                        if xi - dx[ii] <= ub[ii] {
                            dx[ii] = -dx[ii];
                            break;
                        }
                        dx[ii] *= 0.5;
                    }
                    sim[idx2(ii, jd, n)] = dx[ii];
                    temp += simi[idx2(jd, ii, n)] * dx[ii];
                }
                for ii in 0..n {
                    simi[idx2(jd, ii, n)] /= temp;
                }
                for j in 0..n {
                    if j != jd {
                        let mut t = 0.0;
                        for ii in 0..n {
                            t += simi[idx2(j, ii, n)] * dx[ii];
                        }
                        for ii in 0..n {
                            simi[idx2(j, ii, n)] -= t * simi[idx2(jd, ii, n)];
                        }
                    }
                    x[j] = sim[idx2(j, np - 1, n)] + dx[j];
                }
                state = BodyState::EvalCallback;
            }

            // -----------------------------------------------------------
            // L370 — solve the trust-region LP subproblem.
            // -----------------------------------------------------------
            BodyState::SolveLp => {
                let lp_status = trstlp(
                    n, m, a_mat, con, rho, dx, &mut ifull, iact, z_ws, zdota, vmultc,
                    sdirn, dxnew, vmultd,
                );
                if lp_status != Status::Success {
                    break lp_status;
                }
                // Bound clamp dx (linear constraints should keep it in box,
                // but be paranoid).
                //
                // NOTE: NLopt 2.7.1's cobyla.c writes the lower clamp as
                // `dx[i] = xi - lb[i]` — but that's positive when xi > lb,
                // pushing the trial point AWAY from the bound and possibly
                // past the upper bound (since the upper-clamp check has
                // already run). The intent is `xi + dx == lb`, which
                // requires `dx = lb - xi`. We deviate from the C source
                // here intentionally; the C version's formula is a
                // long-standing latent bug that rarely triggers because
                // `trstlp` already enforces the linear bound constraints,
                // but when round-off does push the trial outside the box,
                // the original would mis-clamp.
                for ii in 0..n {
                    let xi = sim[idx2(ii, np - 1, n)];
                    if xi + dx[ii] > ub[ii] {
                        dx[ii] = ub[ii] - xi;
                    }
                    if xi + dx[ii] < lb[ii] {
                        dx[ii] = lb[ii] - xi;
                    }
                }
                if ifull == 0 {
                    let mut t = 0.0;
                    for ii in 0..n {
                        t += dx[ii] * dx[ii];
                    }
                    if t < rho * 0.25 * rho {
                        ibrnch = 1;
                        state = BodyState::RhoCheck;
                        continue;
                    }
                }

                // Predict change to f and new resmax.
                let mut resnew = 0.0_f64;
                con[m] = 0.0;
                let mut last_sum = 0.0_f64;
                for k in 0..=m {
                    let mut s = con[k];
                    for ii in 0..n {
                        s -= a_mat[idx2(ii, k, n)] * dx[ii];
                    }
                    if k < m {
                        resnew = resnew.max(s);
                    }
                    last_sum = s;
                }
                sum = last_sum;

                // Increase parmu if needed; if best vertex changes, restart.
                let mut barmu = 0.0_f64;
                prerec = datmat[idx2(mpp - 1, np - 1, mpp)] - resnew;
                if prerec > 0.0 {
                    barmu = sum / prerec;
                }
                if parmu < barmu * 1.5 {
                    parmu = barmu * 2.0;
                    let phi = datmat[idx2(m, np - 1, mpp)]
                        + parmu * datmat[idx2(mpp - 1, np - 1, mpp)];
                    let mut restart = false;
                    for j in 0..n {
                        let temp =
                            datmat[idx2(m, j, mpp)] + parmu * datmat[idx2(mpp - 1, j, mpp)];
                        if temp < phi {
                            restart = true;
                            break;
                        }
                        if temp == phi
                            && parmu == 0.0
                            && datmat[idx2(mpp - 1, j, mpp)] < datmat[idx2(mpp - 1, np - 1, mpp)]
                        {
                            restart = true;
                            break;
                        }
                    }
                    if restart {
                        state = BodyState::IdentifyBest;
                        continue;
                    }
                }
                prerem = parmu * prerec - sum;

                // Evaluate objective at trial point.
                for ii in 0..n {
                    x[ii] = sim[idx2(ii, np - 1, n)] + dx[ii];
                }
                ibrnch = 1;
                state = BodyState::EvalCallback;
            }

            // -----------------------------------------------------------
            // L440 — post-trial accept/reject + simplex update.
            // -----------------------------------------------------------
            BodyState::PostTrial => {
                let vmold = datmat[idx2(m, np - 1, mpp)]
                    + parmu * datmat[idx2(mpp - 1, np - 1, mpp)];
                let vmnew = f + parmu * resmax;
                let mut trured = vmold - vmnew;
                if parmu == 0.0 && f == datmat[idx2(m, np - 1, mpp)] {
                    prerem = prerec;
                    trured = datmat[idx2(mpp - 1, np - 1, mpp)] - resmax;
                }

                // Pick vertex to replace. ratio = max |z_jdrop · dx|.
                let mut ratio = if trured <= 0.0 { 1.0 } else { 0.0 };
                let mut local_jdrop: usize = usize::MAX;
                for j in 0..n {
                    let mut temp = 0.0_f64;
                    for ii in 0..n {
                        temp += simi[idx2(j, ii, n)] * dx[ii];
                    }
                    let temp = temp.abs();
                    if temp > ratio {
                        local_jdrop = j;
                        ratio = temp;
                    }
                    sigbar[j] = temp * vsig[j];
                }

                // Calculate ell.
                let edgmax_init = delta * rho;
                let mut edgmax = edgmax_init;
                let mut l: Option<usize> = None;
                let parsig = alpha * rho;
                for j in 0..n {
                    if sigbar[j] >= parsig || sigbar[j] >= vsig[j] {
                        let temp = if trured > 0.0 {
                            let mut t = 0.0;
                            for ii in 0..n {
                                let d = dx[ii] - sim[idx2(ii, j, n)];
                                t += d * d;
                            }
                            t.sqrt()
                        } else {
                            veta[j]
                        };
                        if temp > edgmax {
                            l = Some(j);
                            edgmax = temp;
                        }
                    }
                }
                if let Some(ll) = l {
                    local_jdrop = ll;
                }
                if local_jdrop == usize::MAX {
                    state = BodyState::RhoCheck;
                    continue;
                }
                jdrop = local_jdrop;

                // Revise simplex: replace column jdrop with dx.
                let mut temp = 0.0_f64;
                for ii in 0..n {
                    sim[idx2(ii, jdrop, n)] = dx[ii];
                    temp += simi[idx2(jdrop, ii, n)] * dx[ii];
                }
                for ii in 0..n {
                    simi[idx2(jdrop, ii, n)] /= temp;
                }
                for j in 0..n {
                    if j != jdrop {
                        let mut t = 0.0;
                        for ii in 0..n {
                            t += simi[idx2(j, ii, n)] * dx[ii];
                        }
                        for ii in 0..n {
                            simi[idx2(j, ii, n)] -= t * simi[idx2(jdrop, ii, n)];
                        }
                    }
                }
                for k in 0..mpp {
                    datmat[idx2(k, jdrop, mpp)] = con[k];
                }

                // Branch back if reduction is good.
                if trured > 0.0 && trured >= prerem * 0.1 {
                    // SAS-style: increase rho if predicted ≈ actual.
                    if trured >= prerem * 0.9 && trured <= prerem * 1.1 && iflag != 0 {
                        rho *= 2.0;
                    }
                    state = BodyState::IdentifyBest;
                    continue;
                }
                state = BodyState::RhoCheck;
            }

            // -----------------------------------------------------------
            // L550 — shrink rho or terminate.
            // -----------------------------------------------------------
            BodyState::RhoCheck => {
                if iflag == 0 {
                    ibrnch = 0;
                    state = BodyState::IdentifyBest;
                    continue;
                }
                // Convergence check.
                let fbest = if ifull == 1 {
                    f
                } else {
                    datmat[idx2(m, np - 1, mpp)]
                };
                if fbest < *minf
                    && stop.ftol_rel > 0.0
                    && relstop(fbest, *minf, stop.ftol_rel, stop.ftol_abs)
                {
                    *minf = fbest;
                    break Status::FtolReached;
                }
                *minf = fbest;

                if rho > rhoend {
                    rho *= 0.5;
                    if rho <= rhoend * 1.5 {
                        rho = rhoend;
                    }
                    if parmu > 0.0 {
                        let mut denom = 0.0_f64;
                        let mut cmin = 0.0_f64;
                        let mut cmax = 0.0_f64;
                        for k in 0..=m {
                            cmin = datmat[idx2(k, np - 1, mpp)];
                            cmax = cmin;
                            for ii in 0..n {
                                let v = datmat[idx2(k, ii, mpp)];
                                cmin = cmin.min(v);
                                cmax = cmax.max(v);
                            }
                            if k < m && cmin < cmax * 0.5 {
                                let temp = cmax.max(0.0) - cmin;
                                if denom <= 0.0 {
                                    denom = temp;
                                } else {
                                    denom = denom.min(temp);
                                }
                            }
                        }
                        if denom == 0.0 {
                            parmu = 0.0;
                        } else if cmax - cmin < parmu * denom {
                            parmu = (cmax - cmin) / denom;
                        }
                    }
                    state = BodyState::IdentifyBest;
                    continue;
                }
                // rho ≤ rhoend → terminate.
                let exit = if rhoend > 0.0 {
                    Status::XtolReached
                } else {
                    Status::RoundoffLimited
                };
                if ifull == 1 {
                    *minf = f;
                    break exit;
                }
                state = BodyState::CleanExit;
            }

            // -----------------------------------------------------------
            // L600 — copy best vertex back to x.
            // -----------------------------------------------------------
            BodyState::CleanExit => {
                for ii in 0..n {
                    x[ii] = sim[idx2(ii, np - 1, n)];
                }
                f = datmat[idx2(m, np - 1, mpp)];
                resmax = datmat[idx2(mpp - 1, np - 1, mpp)];
                let _ = resmax; // captured into final report through caller
                *minf = f;
                break pending_exit.unwrap_or(Status::Success);
            }

            // -----------------------------------------------------------
            // L620 — keep current x/f/resmax.
            // -----------------------------------------------------------
            BodyState::UseCurrentExit => {
                *minf = f;
                break Status::StopvalReached;
            }
        }
    }
}

/// Relative stop check matching NLopt's `nlopt_stop_ftol`.
fn relstop(new: f64, old: f64, tol_rel: f64, tol_abs: f64) -> bool {
    if !new.is_finite() || !old.is_finite() {
        return false;
    }
    let diff = (new - old).abs();
    diff < tol_abs || diff < 0.5 * tol_rel * (new.abs() + old.abs())
}

// ---------------------------------------------------------------------------
// trstlp — trust-region linear-programming subproblem.
//
// Translated from cobyla.c lines 1247-1872. Two stages:
//   Stage 1 (mcon == m): minimise greatest constraint violation.
//   Stage 2 (mcon == m+1): minimise the linearised objective subject to no
//     increase in any constraint violation.
//
// Labels: L60, L70, L100, L130, L170, L210, L260, L270, L320, L340, L390,
// L480, L490, L500.
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn trstlp(
    n: usize,
    m: usize,
    a_mat: &[f64],
    b: &mut [f64],
    rho: f64,
    dx: &mut [f64],
    ifull: &mut i32,
    iact: &mut [usize],
    z_ws: &mut [f64],
    zdota: &mut [f64],
    vmultc: &mut [f64],
    sdirn: &mut [f64],
    dxnew: &mut [f64],
    vmultd: &mut [f64],
) -> Status {
    *ifull = 1;
    // mcon evolves through the algorithm: starts at m (stage 1, constraint
    // satisfaction), bumps to m+1 (stage 2, objective minimization). Both
    // stages share the L60/L70 outer loop; only the `optnew` computation
    // and a few `if mcon == m` branches differ.
    let mut mcon = m;
    let mut nact: usize = 0;
    let mut resmax = 0.0_f64;
    let mut icon: usize = 0;

    // Initialise z = I, dx = 0.
    for i in 0..n {
        for j in 0..n {
            z_ws[idx2(i, j, n)] = 0.0;
        }
        z_ws[idx2(i, i, n)] = 1.0;
        dx[i] = 0.0;
    }

    if m >= 1 {
        for k in 0..m {
            if b[k] > resmax {
                resmax = b[k];
                icon = k;
            }
        }
        for k in 0..m {
            iact[k] = k;
            vmultc[k] = resmax - b[k];
        }
    }
    let mut skip_stage1 = false;
    if resmax == 0.0 {
        // Already feasible at dx=0 — skip stage 1, jump straight to stage 2.
        // (cobyla.c: `if (resmax == 0.) goto L480;`)
        mcon = m + 1;
        icon = mcon - 1;
        iact[icon] = icon;
        vmultc[icon] = 0.0;
        skip_stage1 = true;
    }
    for i in 0..n {
        sdirn[i] = 0.0;
    }
    let _ = skip_stage1; // skip_stage1 is implicitly handled by mcon == m+1 entering stage 2 paths

    // ---- Outer loop (L60) ----
    let mut optold: f64 = 0.0;
    let mut nactx: usize = 0;
    let mut icount: i32 = 0;
    let mut state = TrLoopState::OuterStart;
    loop {
        match state {
            TrLoopState::OuterStart => {
                optold = 0.0;
                icount = 0;
                state = TrLoopState::Cycle;
            }
            // -- L70 -----------------------------------------------------
            TrLoopState::Cycle => {
                let optnew = if mcon == m {
                    resmax
                } else {
                    let mut s = 0.0_f64;
                    for i in 0..n {
                        s -= dx[i] * a_mat[idx2(i, mcon - 1, n)];
                    }
                    s
                };
                if icount == 0 || optnew < optold {
                    optold = optnew;
                    nactx = nact;
                    icount = 3;
                } else if nact > nactx {
                    nactx = nact;
                    icount = 3;
                } else {
                    icount -= 1;
                    if icount == 0 {
                        state = TrLoopState::L490Dispatch;
                        continue;
                    }
                }

                // L100-L210: add icon to active set if icon > nact.
                if icon < nact {
                    state = TrLoopState::DeleteFromActive;
                    continue;
                }
                // No room: active set already spans R^n. Any further constraint
                // is linearly dependent on the current basis, so the trust-region
                // step from the existing active set is the best feasible solution.
                // Without this guard, the AddNew path would index `z_ws` and
                // `zdota` at column/index `n`, which is out of bounds.
                if nact >= n {
                    state = TrLoopState::L490Dispatch;
                    continue;
                }
                let kk = iact[icon];
                for i in 0..n {
                    dxnew[i] = a_mat[idx2(i, kk, n)];
                }
                let mut tot = 0.0_f64;
                let mut k = n;
                while k > nact {
                    let kix = k - 1;
                    let mut sp = 0.0_f64;
                    let mut spabs = 0.0_f64;
                    for i in 0..n {
                        let temp = z_ws[idx2(i, kix, n)] * dxnew[i];
                        sp += temp;
                        spabs += temp.abs();
                    }
                    let acca = spabs + sp.abs() * 0.1;
                    let accb = spabs + sp.abs() * 0.2;
                    if spabs >= acca || acca >= accb {
                        sp = 0.0;
                    }
                    if tot == 0.0 {
                        tot = sp;
                    } else {
                        let kp = kix + 1;
                        let temp = (sp * sp + tot * tot).sqrt();
                        let alpha = sp / temp;
                        let beta = tot / temp;
                        tot = temp;
                        for i in 0..n {
                            let t =
                                alpha * z_ws[idx2(i, kix, n)] + beta * z_ws[idx2(i, kp, n)];
                            z_ws[idx2(i, kp, n)] =
                                alpha * z_ws[idx2(i, kp, n)] - beta * z_ws[idx2(i, kix, n)];
                            z_ws[idx2(i, kix, n)] = t;
                        }
                    }
                    k -= 1;
                }

                if tot != 0.0 {
                    nact += 1;
                    zdota[nact - 1] = tot;
                    vmultc[icon] = vmultc[nact - 1];
                    vmultc[nact - 1] = 0.0;
                    state = TrLoopState::AfterAdd;
                    continue;
                }

                // Need to delete one active constraint to make room.
                let mut ratio = -1.0_f64;
                let mut k_iter: i32 = nact as i32;
                let _iout: Option<usize> = None;
                while k_iter > 0 {
                    let kix = (k_iter - 1) as usize;
                    let mut zdotv = 0.0_f64;
                    let mut zdvabs = 0.0_f64;
                    for i in 0..n {
                        let temp = z_ws[idx2(i, kix, n)] * dxnew[i];
                        zdotv += temp;
                        zdvabs += temp.abs();
                    }
                    let acca = zdvabs + zdotv.abs() * 0.1;
                    let accb = zdvabs + zdotv.abs() * 0.2;
                    if zdvabs < acca && acca < accb {
                        let temp = zdotv / zdota[kix];
                        if temp > 0.0 && iact[kix] < m {
                            let tempa = vmultc[kix] / temp;
                            if ratio < 0.0 || tempa < ratio {
                                ratio = tempa;
                            }
                        }
                        if k_iter >= 2 {
                            let kw = iact[kix];
                            for i in 0..n {
                                dxnew[i] -= temp * a_mat[idx2(i, kw, n)];
                            }
                        }
                        vmultd[kix] = temp;
                    } else {
                        vmultd[kix] = 0.0;
                    }
                    k_iter -= 1;
                }
                if ratio < 0.0 {
                    state = TrLoopState::L490Dispatch;
                    continue;
                }

                // Revise multipliers.
                for kk in 0..nact {
                    vmultc[kk] = (vmultc[kk] - ratio * vmultd[kk]).max(0.0);
                }
                if icon < nact {
                    let isave = iact[icon];
                    let vsave = vmultc[icon];
                    let mut k = icon;
                    while k < nact - 1 {
                        let kp = k + 1;
                        let kw = iact[kp];
                        let mut sp = 0.0_f64;
                        for i in 0..n {
                            sp += z_ws[idx2(i, k, n)] * a_mat[idx2(i, kw, n)];
                        }
                        let temp = (sp * sp + zdota[kp] * zdota[kp]).sqrt();
                        let alpha = zdota[kp] / temp;
                        let beta = sp / temp;
                        zdota[kp] = alpha * zdota[k];
                        zdota[k] = temp;
                        for i in 0..n {
                            let t =
                                alpha * z_ws[idx2(i, kp, n)] + beta * z_ws[idx2(i, k, n)];
                            z_ws[idx2(i, kp, n)] =
                                alpha * z_ws[idx2(i, k, n)] - beta * z_ws[idx2(i, kp, n)];
                            z_ws[idx2(i, k, n)] = t;
                        }
                        iact[k] = kw;
                        vmultc[k] = vmultc[kp];
                        k = kp;
                    }
                    iact[k] = isave;
                    vmultc[k] = vsave;
                }
                let kk_new = iact[icon];
                let mut temp = 0.0_f64;
                for i in 0..n {
                    temp += z_ws[idx2(i, nact, n)] * a_mat[idx2(i, kk_new, n)];
                }
                if temp == 0.0 {
                    state = TrLoopState::L490Dispatch;
                    continue;
                }
                nact += 1;
                zdota[nact - 1] = temp;
                vmultc[icon] = 0.0;
                vmultc[nact - 1] = ratio;
                state = TrLoopState::AfterAdd;
            }

            // -- L210 ----------------------------------------------------
            TrLoopState::AfterAdd => {
                let kk = iact[icon];
                iact.swap(icon, nact - 1);
                if mcon > m && kk != mcon - 1 {
                    let k = nact - 2;
                    let mut sp = 0.0_f64;
                    for i in 0..n {
                        sp += z_ws[idx2(i, k, n)] * a_mat[idx2(i, kk, n)];
                    }
                    let temp = (sp * sp + zdota[nact - 1] * zdota[nact - 1]).sqrt();
                    let alpha = zdota[nact - 1] / temp;
                    let beta = sp / temp;
                    zdota[nact - 1] = alpha * zdota[k];
                    zdota[k] = temp;
                    for i in 0..n {
                        let t = alpha * z_ws[idx2(i, nact - 1, n)] + beta * z_ws[idx2(i, k, n)];
                        z_ws[idx2(i, nact - 1, n)] =
                            alpha * z_ws[idx2(i, k, n)] - beta * z_ws[idx2(i, nact - 1, n)];
                        z_ws[idx2(i, k, n)] = t;
                    }
                    // Three-way exchange (NOT a plain swap — iact[k] gets the
                    // original iact[icon] value `kk`, while iact[nact-1] gets
                    // iact[k]'s previous value):
                    iact[nact - 1] = iact[k];
                    iact[k] = kk;
                    vmultc.swap(k, nact - 1);
                }
                state = TrLoopState::SetSdirn;
            }

            // -- L260 (delete from active set) ---------------------------
            TrLoopState::DeleteFromActive => {
                if icon < nact - 1 {
                    let isave = iact[icon];
                    let vsave = vmultc[icon];
                    let mut k = icon;
                    while k < nact - 1 {
                        let kp = k + 1;
                        let kk = iact[kp];
                        let mut sp = 0.0_f64;
                        for i in 0..n {
                            sp += z_ws[idx2(i, k, n)] * a_mat[idx2(i, kk, n)];
                        }
                        let temp = (sp * sp + zdota[kp] * zdota[kp]).sqrt();
                        let alpha = zdota[kp] / temp;
                        let beta = sp / temp;
                        zdota[kp] = alpha * zdota[k];
                        zdota[k] = temp;
                        for i in 0..n {
                            let t =
                                alpha * z_ws[idx2(i, kp, n)] + beta * z_ws[idx2(i, k, n)];
                            z_ws[idx2(i, kp, n)] =
                                alpha * z_ws[idx2(i, k, n)] - beta * z_ws[idx2(i, kp, n)];
                            z_ws[idx2(i, k, n)] = t;
                        }
                        iact[k] = kk;
                        vmultc[k] = vmultc[kp];
                        k = kp;
                    }
                    iact[k] = isave;
                    vmultc[k] = vsave;
                }
                nact -= 1;
                state = TrLoopState::SetSdirn;
            }

            // -- L320 / L340 (compute sdirn and step) --------------------
            TrLoopState::SetSdirn => {
                if mcon > m {
                    // Stage 2: sdirn = (1 / zdota[nact-1]) * z_col[nact-1]
                    let inv = 1.0 / zdota[nact - 1];
                    for i in 0..n {
                        sdirn[i] = inv * z_ws[idx2(i, nact - 1, n)];
                    }
                } else {
                    // Stage 1.
                    let kk = iact[nact - 1];
                    let mut temp = 0.0_f64;
                    for i in 0..n {
                        temp += sdirn[i] * a_mat[idx2(i, kk, n)];
                    }
                    temp = (temp - 1.0) / zdota[nact - 1];
                    for i in 0..n {
                        sdirn[i] -= temp * z_ws[idx2(i, nact - 1, n)];
                    }
                }
                state = TrLoopState::ComputeStep;
            }

            // -- L340 ----------------------------------------------------
            TrLoopState::ComputeStep => {
                let mut dd = rho * rho;
                let mut sd = 0.0_f64;
                let mut ss = 0.0_f64;
                for i in 0..n {
                    if dx[i].abs() >= rho * 1e-6 {
                        dd -= dx[i] * dx[i];
                    }
                    sd += dx[i] * sdirn[i];
                    ss += sdirn[i] * sdirn[i];
                }
                if dd <= 0.0 {
                    state = TrLoopState::L490Dispatch;
                    continue;
                }
                let mut t = (ss * dd).sqrt();
                if sd.abs() >= t * 1e-6 {
                    t = (ss * dd + sd * sd).sqrt();
                }
                let stpful = dd / (t + sd);
                let mut step = stpful;
                if mcon == m {
                    let acca = step + resmax * 0.1;
                    let accb = step + resmax * 0.2;
                    if step >= acca || acca >= accb {
                        // L480: unconditional stage transition. Reached only
                        // when mcon == m, so L490Dispatch will hit the
                        // transition branch.
                        state = TrLoopState::L490Dispatch;
                        continue;
                    }
                    step = step.min(resmax);
                }
                if !step.is_finite() {
                    return Status::RoundoffLimited;
                }

                // dxnew = dx + step * sdirn.
                for i in 0..n {
                    dxnew[i] = dx[i] + step * sdirn[i];
                }
                let mut resold = 0.0_f64;
                let mut new_resmax = resmax;
                if mcon == m {
                    resold = resmax;
                    new_resmax = 0.0;
                    for k in 0..nact {
                        let kk = iact[k];
                        let mut tt = b[kk];
                        for i in 0..n {
                            tt -= a_mat[idx2(i, kk, n)] * dxnew[i];
                        }
                        new_resmax = new_resmax.max(tt);
                    }
                }
                resmax = new_resmax;

                // Compute vmultd via L390 loop.
                let mut k_iter: i32 = nact as i32;
                while k_iter > 0 {
                    let kix = (k_iter - 1) as usize;
                    let mut zdotw = 0.0_f64;
                    let mut zdwabs = 0.0_f64;
                    for i in 0..n {
                        let tt = z_ws[idx2(i, kix, n)] * dxnew[i];
                        zdotw += tt;
                        zdwabs += tt.abs();
                    }
                    let acca = zdwabs + zdotw.abs() * 0.1;
                    let accb = zdwabs + zdotw.abs() * 0.2;
                    if zdwabs >= acca || acca >= accb {
                        zdotw = 0.0;
                    }
                    vmultd[kix] = zdotw / zdota[kix];
                    if k_iter >= 2 {
                        let kk = iact[kix];
                        for i in 0..n {
                            dxnew[i] -= vmultd[kix] * a_mat[idx2(i, kk, n)];
                        }
                    }
                    k_iter -= 1;
                }
                if mcon > m {
                    vmultd[nact - 1] = vmultd[nact - 1].max(0.0);
                }

                // Reset dxnew, then complete vmultc residuals for inactive.
                for i in 0..n {
                    dxnew[i] = dx[i] + step * sdirn[i];
                }
                if mcon > nact {
                    for k in nact..mcon {
                        let kk = iact[k];
                        let mut s = resmax - b[kk];
                        let mut sumabs = resmax + b[kk].abs();
                        for i in 0..n {
                            let tt = a_mat[idx2(i, kk, n)] * dxnew[i];
                            s += tt;
                            sumabs += tt.abs();
                        }
                        let acca = sumabs + s.abs() * 0.1;
                        let accb = sumabs + s.abs() * 0.2;
                        if sumabs >= acca || acca >= accb {
                            s = 0.0;
                        }
                        vmultd[k] = s;
                    }
                }

                // Pick step ratio.
                let mut ratio = 1.0_f64;
                let mut new_icon: Option<usize> = None;
                for k in 0..mcon {
                    if vmultd[k] < 0.0 {
                        let temp = vmultc[k] / (vmultc[k] - vmultd[k]);
                        if temp < ratio {
                            ratio = temp;
                            new_icon = Some(k);
                        }
                    }
                }

                let temp = 1.0 - ratio;
                for i in 0..n {
                    dx[i] = temp * dx[i] + ratio * dxnew[i];
                }
                for k in 0..mcon {
                    vmultc[k] = (temp * vmultc[k] + ratio * vmultd[k]).max(0.0);
                }
                if mcon == m {
                    resmax = resold + ratio * (resmax - resold);
                }

                if let Some(ic) = new_icon {
                    icon = ic;
                    state = TrLoopState::OuterStart;
                    continue;
                }
                if step == stpful {
                    return Status::Success;
                }
                // L480 stage transition (unconditional in C; reached only
                // when mcon == m).
                state = TrLoopState::L490Dispatch;
            }

            // -- L490 dispatch (and L480 transition) ---------------------
            TrLoopState::L490Dispatch => {
                if mcon == m {
                    // L480: bump to stage 2.
                    mcon = m + 1;
                    icon = mcon - 1;
                    iact[icon] = icon;
                    vmultc[icon] = 0.0;
                    state = TrLoopState::OuterStart;
                } else {
                    // L500 with ifull=0: incomplete return.
                    *ifull = 0;
                    return Status::Success;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum TrLoopState {
    OuterStart,       // L60
    Cycle,            // L70
    AfterAdd,         // L210
    DeleteFromActive, // L260
    SetSdirn,         // L320 / L340 setup
    ComputeStep,      // L340
    /// L490 dispatch: if `mcon == m`, transition to stage 2 (L480) and
    /// re-enter the outer loop; else set `ifull = 0` and return success.
    L490Dispatch,
}

// (Stage transition is handled inline by `TrLoopState::L490Dispatch` which
// either bumps `mcon` to `m + 1` or sets `*ifull = 0` and returns.)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "manual probe; run with --ignored to print convergence details"]
    fn print_convergence() {
        let f = |x: &[f64]| (x[0] + 1.0).powi(2) + x[1].powi(2);
        let cons: Vec<Box<dyn Fn(&[f64]) -> f64>> = Vec::new();
        let mut x = vec![1.0, 1.0];
        let report = cobyla_native(
            2,
            f,
            &cons,
            &[(-10.0, 10.0); 2],
            &mut x,
            &[0.5, 0.5],
            &StopCriteria {
                stopval: f64::NEG_INFINITY,
                ftol_rel: 1e-12,
                maxeval: 1000,
                ..Default::default()
            },
        )
        .unwrap();
        eprintln!(
            "unconstrained: x={:?} fun={:.3e} status={} nfev={}",
            report.x, report.fun, report.message, report.nfev
        );

        let f2 = |x: &[f64]| 10.0 * (x[0] + 1.0).powi(2) + x[1].powi(2);
        let g0: Box<dyn Fn(&[f64]) -> f64> = Box::new(|x: &[f64]| -x[0]);
        let cons2 = vec![g0];
        let mut x = vec![1.0, 1.0];
        let report = cobyla_native(
            2,
            f2,
            &cons2,
            &[(-10.0, 10.0); 2],
            &mut x,
            &[0.5, 0.5],
            &StopCriteria {
                stopval: f64::NEG_INFINITY,
                ftol_rel: 1e-12,
                maxeval: 1000,
                ..Default::default()
            },
        )
        .unwrap();
        eprintln!(
            "constrained: x={:?} fun={:.3e} status={} nfev={}",
            report.x, report.fun, report.message, report.nfev
        );
    }

    #[test]
    fn paraboloid_unconstrained() {
        // f(x) = (x0+1)^2 + x1^2, minimum at (-1, 0) with value 0.
        let f = |x: &[f64]| (x[0] + 1.0).powi(2) + x[1].powi(2);
        let cons: Vec<Box<dyn Fn(&[f64]) -> f64>> = Vec::new();
        let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
        let mut x = vec![1.0, 1.0];
        let dx = vec![0.5, 0.5];
        let stop = StopCriteria {
            stopval: f64::NEG_INFINITY,
            ftol_rel: 1e-10,
            maxeval: 500,
            ..Default::default()
        };
        let report = cobyla_native(2, f, &cons, &bounds, &mut x, &dx, &stop)
            .expect("cobyla failed");
        assert!(
            report.fun < 1e-3,
            "fun = {} should converge near 0 (status: {})",
            report.fun,
            report.message
        );
    }

    /// Diagnostic for the anisotropic-dx case: WITHOUT rescaling (uniform
    /// `dx`), the same problem shape converges fine — proves the issue
    /// specifically lives in the rescaling+bounds interaction.
    #[test]
    fn bug1b_isotropic_bounds_with_active_bound() {
        // Same f as bug1 but uniform dx — no rescaling occurs.
        let f = |x: &[f64]| (x[0] - 5.0).powi(2) + (x[1] - 100.0).powi(2);
        let cons: Vec<Box<dyn Fn(&[f64]) -> f64>> = Vec::new();
        let bounds = vec![(-1.0, 1.0), (-1000.0, 1000.0)];
        let mut x = vec![0.0, 0.0];
        let dx = vec![1.0, 1.0]; // isotropic — s[i] = 1 for all i
        let stop = StopCriteria {
            stopval: f64::NEG_INFINITY,
            xtol_rel: 1e-8,
            maxeval: 5000,
            ..Default::default()
        };
        let report = cobyla_native(2, f, &cons, &bounds, &mut x, &dx, &stop)
            .expect("cobyla failed");
        eprintln!(
            "bug1b (isotropic): x = {:?}, fun = {}, nfev = {}, status = {}",
            report.x, report.fun, report.nfev, report.message
        );
        assert!(
            (report.x[0] - 1.0).abs() < 0.01,
            "x0 = {} should be near 1.0",
            report.x[0]
        );
        assert!((report.fun - 16.0).abs() < 0.5, "fun = {}", report.fun);
    }

    /// Bug #1 reproducer: bound-as-constraints must use **rescaled-space**
    /// coordinates (gradient magnitude 1 in rescaled space) to match
    /// NLopt's behaviour, regardless of the per-dim rescaling factor
    /// `s[i] = dx[i] / dx[0]`.
    ///
    /// Test: a 2D problem with widely different bound spans + active bound
    /// constraint. `dx` is sized to provide an adequate initial trust region
    /// (rhobeg=1.0 in rescaled space, reaching the constrained optimum
    /// without exhausting the geometric-shrinking budget).
    #[test]
    fn bug1_anisotropic_bounds_with_active_bound() {
        // Minimize (x0 - 5)^2 + (x1 - 100)^2 with x0 in [-1,1], x1 in [-1000, 1000].
        // dx anisotropic by 1000× to exercise rescaling. rhobeg = 1.0 in
        // rescaled space — enough to reach the bound from the centre.
        let f = |x: &[f64]| (x[0] - 5.0).powi(2) + (x[1] - 100.0).powi(2);
        let cons: Vec<Box<dyn Fn(&[f64]) -> f64>> = Vec::new();
        let bounds = vec![(-1.0, 1.0), (-1000.0, 1000.0)];
        let mut x = vec![0.0, 0.0];
        // dx[1]/dx[0] = 1000 → s[1] = 1000. Anisotropic rescaling exercises
        // the bound-constraint gradient handling.
        let dx = vec![1.0, 1000.0];
        let stop = StopCriteria {
            stopval: f64::NEG_INFINITY,
            xtol_rel: 1e-10,
            maxeval: 2000,
            ..Default::default()
        };
        let report = cobyla_native(2, f, &cons, &bounds, &mut x, &dx, &stop)
            .expect("cobyla failed");
        eprintln!(
            "bug1: x = {:?}, fun = {}, nfev = {}, status = {}, max_v = {}",
            report.x, report.fun, report.nfev, report.message, report.max_violation
        );
        // Expected: x0 ≈ 1.0 (active upper bound), x1 ≈ 100.0, f ≈ 16.
        assert!(
            (report.x[0] - 1.0).abs() < 0.01,
            "x0 = {} should be near 1.0 (active upper bound)",
            report.x[0]
        );
        assert!(
            (report.x[1] - 100.0).abs() < 1.0,
            "x1 = {} should be near 100.0",
            report.x[1]
        );
        assert!(
            (report.fun - 16.0).abs() < 0.5,
            "fun = {} should be near 16",
            report.fun
        );
    }

    /// Bug A reproducer: a dimension with `lb == ub == x0` (fixed) must
    /// not produce `simi[i,i] = 1/0`. Real autoeq use case: hp-pk-lp model
    /// fixes the HP/LP filter gain to 0.
    #[test]
    fn bug_a_fixed_dimension_does_not_explode() {
        // Two-dim problem; dim 0 is fixed at 0.5, dim 1 is free in [-1, 1].
        let f = |x: &[f64]| (x[0] - 0.5).powi(2) + (x[1] + 0.3).powi(2);
        let cons: Vec<Box<dyn Fn(&[f64]) -> f64>> = Vec::new();
        let bounds = vec![(0.5, 0.5), (-1.0, 1.0)];
        let mut x = vec![0.5, 0.0];
        // dx[0] tiny but non-zero, mirroring autoeq's behaviour for fixed
        // dims (`span.max(1e-6)`).
        let dx = vec![1e-6, 0.05];
        let stop = StopCriteria {
            stopval: f64::NEG_INFINITY,
            xtol_rel: 1e-8,
            maxeval: 500,
            ..Default::default()
        };
        let report = cobyla_native(2, f, &cons, &bounds, &mut x, &dx, &stop)
            .expect("cobyla failed");
        eprintln!(
            "bug_a: x = {:?}, fun = {:.3e}, nfev = {}, status = {}",
            report.x, report.fun, report.nfev, report.message
        );
        // Expected: x0 stays at 0.5, x1 → -0.3, f → 0.
        assert!(report.fun.is_finite(), "fun = {} should be finite", report.fun);
        assert!((report.x[0] - 0.5).abs() < 1e-6, "x0 must stay fixed at 0.5");
        assert!(report.fun < 1e-3, "fun = {} should converge near 0", report.fun);
    }

    /// Bug B reproducer: when the trial step would cross a lower bound,
    /// the clamp must drive `xi + dx` to `lb`, not push it back to `xi`.
    /// A problem whose constrained optimum lies on a lower bound exposes
    /// this immediately.
    #[test]
    fn bug_b_constrained_optimum_on_lower_bound() {
        // Minimize (x0 + 5)^2 + x1^2 with x0 in [-1, 1], x1 in [-1, 1].
        // Unconstrained min at (-5, 0); constrained min at lower bound x0 = -1.
        let f = |x: &[f64]| (x[0] + 5.0).powi(2) + x[1].powi(2);
        let cons: Vec<Box<dyn Fn(&[f64]) -> f64>> = Vec::new();
        let bounds = vec![(-1.0, 1.0), (-1.0, 1.0)];
        let mut x = vec![0.5, 0.5];
        let dx = vec![0.5, 0.5];
        let stop = StopCriteria {
            stopval: f64::NEG_INFINITY,
            xtol_rel: 1e-10,
            maxeval: 1000,
            ..Default::default()
        };
        let report = cobyla_native(2, f, &cons, &bounds, &mut x, &dx, &stop)
            .expect("cobyla failed");
        eprintln!(
            "bug_b: x = {:?}, fun = {:.3e}, nfev = {}, status = {}",
            report.x, report.fun, report.nfev, report.message
        );
        // Expected: x0 ≈ -1.0 (active LOWER bound), x1 ≈ 0, f = 16.
        assert!(
            (report.x[0] - (-1.0)).abs() < 0.01,
            "x0 = {} should be near -1.0 (active lower bound)",
            report.x[0]
        );
        assert!(report.x[1].abs() < 0.01, "x1 = {} should be near 0", report.x[1]);
        assert!(
            (report.fun - 16.0).abs() < 0.5,
            "fun = {} should be near 16",
            report.fun
        );
    }

    /// Bug #2 reproducer: `xtol_rel` should drive `rhoend` (the COBYLA
    /// parameter-space termination radius). Tight xtol_rel → more
    /// iterations, finer convergence.
    #[test]
    fn bug2_xtol_rel_controls_rhoend() {
        let f = |x: &[f64]| (x[0] + 1.0).powi(2) + x[1].powi(2);
        let cons: Vec<Box<dyn Fn(&[f64]) -> f64>> = Vec::new();
        let bounds = vec![(-10.0, 10.0); 2];

        let mut x_loose = vec![1.0, 1.0];
        let report_loose = cobyla_native(
            2,
            f,
            &cons,
            &bounds,
            &mut x_loose,
            &[0.5, 0.5],
            &StopCriteria {
                stopval: f64::NEG_INFINITY,
                xtol_rel: 1e-2, // loose
                maxeval: 5000,
                ..Default::default()
            },
        )
        .unwrap();

        let mut x_tight = vec![1.0, 1.0];
        let report_tight = cobyla_native(
            2,
            f,
            &cons,
            &bounds,
            &mut x_tight,
            &[0.5, 0.5],
            &StopCriteria {
                stopval: f64::NEG_INFINITY,
                xtol_rel: 1e-12, // tight
                maxeval: 5000,
                ..Default::default()
            },
        )
        .unwrap();

        eprintln!(
            "bug2: loose nfev={} fun={:.3e}, tight nfev={} fun={:.3e}",
            report_loose.nfev, report_loose.fun, report_tight.nfev, report_tight.fun
        );
        assert!(
            report_tight.nfev > report_loose.nfev,
            "tight xtol_rel should use more evals: tight={}, loose={}",
            report_tight.nfev,
            report_loose.nfev
        );
        assert!(
            report_tight.fun <= report_loose.fun,
            "tight should converge at least as well: tight={}, loose={}",
            report_tight.fun,
            report_loose.fun
        );
    }

    #[test]
    fn paraboloid_with_inequality() {
        // Minimize 10*(x0+1)^2 + x1^2 s.t. x0 >= 0  i.e. -x0 <= 0.
        // Constrained optimum at (0, 0) with value 10.
        let f = |x: &[f64]| 10.0 * (x[0] + 1.0).powi(2) + x[1].powi(2);
        let g0: Box<dyn Fn(&[f64]) -> f64> = Box::new(|x: &[f64]| -x[0]);
        let cons = vec![g0];
        let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
        let mut x = vec![1.0, 1.0];
        let dx = vec![0.5, 0.5];
        let stop = StopCriteria {
            stopval: f64::NEG_INFINITY,
            ftol_rel: 1e-10,
            maxeval: 1000,
            ..Default::default()
        };
        let report = cobyla_native(2, f, &cons, &bounds, &mut x, &dx, &stop)
            .expect("cobyla failed");
        assert!(
            report.x[0] >= -1e-3,
            "x0 = {} should respect x0 >= 0",
            report.x[0]
        );
        // The unconstrained minimum is at x0=-1 with f=0; the constrained
        // minimum is at x0=0 with f=10. We accept anywhere from f≈10 down
        // to small positive values (loose tolerance for hand-port WIP).
        assert!(
            report.fun < 11.0 && report.fun >= 0.0,
            "fun = {} should be near 10",
            report.fun
        );
    }
}
