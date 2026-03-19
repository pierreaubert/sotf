//! Spectral channel alignment via shelf filters + gain.
//!
//! After independent per-channel room EQ optimization, L and R corrected curves
//! can differ in both overall level and spectral tilt. This module replaces the
//! flat-gain-only alignment with a 3-parameter spectral model:
//!   **low-shelf + high-shelf + flat gain**
//! fitted via weighted linear least squares.
//!
//! The shelf response in dB is nonlinear in `db_gain` (the transition band shape
//! changes with gain). The fitting uses Gauss-Newton iteration to handle this:
//! a linear solve provides the initial estimate, then 3 iterations refine it
//! using the actual shelf response and a finite-difference Jacobian.

use crate::Curve;
use log::info;
use math_audio_iir_fir::{Biquad, BiquadFilterType, DEFAULT_Q_HIGH_LOW_SHELF};
use math_audio_optimisation::{levenberg_marquardt, LMConfigBuilder};
use ndarray::Array1;
use std::collections::HashMap;

use super::output;
use super::types::PluginConfigWrapper;

/// Low-shelf center frequency for alignment basis
pub const LOWSHELF_FREQ: f64 = 200.0;

/// High-shelf center frequency for alignment basis
pub const HIGHSHELF_FREQ: f64 = 4000.0;

/// Maximum allowed shelf gain magnitude (dB)
const MAX_SHELF_GAIN_DB: f64 = 6.0;

/// Maximum allowed flat gain magnitude (dB) — prevents solver divergence on narrow-band data
const MAX_FLAT_GAIN_DB: f64 = 12.0;

/// Minimum correction magnitude to bother applying (dB)
const MIN_CORRECTION_DB: f64 = 0.3;

/// Result of spectral alignment for a single channel.
#[derive(Debug, Clone)]
pub struct SpectralAlignmentResult {
    /// Low-shelf correction gain in dB (applied at `LOWSHELF_FREQ` Hz)
    pub lowshelf_gain_db: f64,
    /// High-shelf correction gain in dB (applied at `HIGHSHELF_FREQ` Hz)
    pub highshelf_gain_db: f64,
    /// Broadband flat gain correction in dB
    pub flat_gain_db: f64,
    /// RMS of the weighted residual after fitting (dB) — fit quality metric
    pub residual_rms_db: f64,
}

/// Compute spectral alignment corrections for all channels.
///
/// 1. Computes a pointwise-average reference curve.
/// 2. For each channel, fits `lowshelf + highshelf + flat_gain` to the
///    difference `channel - reference` via weighted least squares.
/// 3. Clamps shelf gains to ±`MAX_SHELF_GAIN_DB` and skips corrections
///    smaller than `MIN_CORRECTION_DB`.
/// 4. Renormalizes flat gains so the mean across channels is zero.
///
/// Returns an empty map when there is only one channel (nothing to align).
pub fn compute_spectral_alignment(
    curves: &HashMap<String, Curve>,
    sample_rate: f64,
    min_freq: f64,
    max_freq: f64,
) -> HashMap<String, SpectralAlignmentResult> {
    if curves.len() <= 1 {
        return HashMap::new();
    }

    // All curves must share the same frequency grid (they come from the same
    // optimization pipeline). Use the first curve's freq as the canonical grid.
    let first_curve = curves.values().next().unwrap();
    let freq = &first_curve.freq;

    // Build mask: only consider frequencies within [min_freq, max_freq]
    let mask: Vec<bool> = freq
        .iter()
        .map(|&f| f >= min_freq && f <= max_freq)
        .collect();
    let n_active: usize = mask.iter().filter(|m| **m).count();
    if n_active < 3 {
        return HashMap::new();
    }

    // Extract active frequencies
    let active_freq: Array1<f64> = Array1::from(
        freq.iter()
            .zip(mask.iter())
            .filter(|(_, m)| **m)
            .map(|(f, _)| *f)
            .collect::<Vec<_>>(),
    );

    // Compute reference curve (pointwise average of all channels' SPL, masked)
    let reference_spl = compute_reference_curve(curves, &mask, n_active);

    // Compute octave-spaced weights
    let weights = compute_octave_weights(&active_freq);

    // Fit each channel using iterative Gauss-Newton solver
    let mut results: HashMap<String, SpectralAlignmentResult> = HashMap::new();

    for (name, curve) in curves {
        // Extract active SPL for this channel
        let channel_spl: Array1<f64> = Array1::from(
            curve
                .spl
                .iter()
                .zip(mask.iter())
                .filter(|(_, m)| **m)
                .map(|(s, _)| *s)
                .collect::<Vec<_>>(),
        );

        // diff = channel - reference
        let diff = &channel_spl - &reference_spl;

        // Iteratively fit shelf + gain to the difference. The fit tells us what
        // the channel IS relative to reference; the correction is the negative.
        let (ls_fit, hs_fit, flat_fit, residual_rms) =
            fit_shelf_gain_iterative(&diff, &active_freq, sample_rate, &weights);

        // Negate to get corrections, then clamp gains
        let ls_gain = (-ls_fit).clamp(-MAX_SHELF_GAIN_DB, MAX_SHELF_GAIN_DB);
        let hs_gain = (-hs_fit).clamp(-MAX_SHELF_GAIN_DB, MAX_SHELF_GAIN_DB);
        let flat_gain = (-flat_fit).clamp(-MAX_FLAT_GAIN_DB, MAX_FLAT_GAIN_DB);

        results.insert(
            name.clone(),
            SpectralAlignmentResult {
                lowshelf_gain_db: ls_gain,
                highshelf_gain_db: hs_gain,
                flat_gain_db: flat_gain,
                residual_rms_db: residual_rms,
            },
        );
    }

    // Renormalize: subtract mean flat_gain so net system level doesn't shift
    let mean_flat: f64 =
        results.values().map(|r| r.flat_gain_db).sum::<f64>() / results.len() as f64;
    for result in results.values_mut() {
        result.flat_gain_db -= mean_flat;
    }

    // Zero out corrections that are too small
    for result in results.values_mut() {
        if result.lowshelf_gain_db.abs() < MIN_CORRECTION_DB {
            result.lowshelf_gain_db = 0.0;
        }
        if result.highshelf_gain_db.abs() < MIN_CORRECTION_DB {
            result.highshelf_gain_db = 0.0;
        }
        if result.flat_gain_db.abs() < MIN_CORRECTION_DB {
            result.flat_gain_db = 0.0;
        }
    }

    results
}

/// Create alignment plugins (EQ with shelves + gain) from an alignment result.
///
/// Returns `(Option<EQ plugin with shelves>, Option<gain plugin>)`.
/// Either or both may be `None` if the corresponding corrections are zero.
pub fn create_alignment_plugins(
    result: &SpectralAlignmentResult,
    sample_rate: f64,
) -> (Option<PluginConfigWrapper>, Option<PluginConfigWrapper>) {
    // Build shelf filters
    let mut shelf_filters = Vec::new();

    if result.lowshelf_gain_db.abs() >= MIN_CORRECTION_DB {
        shelf_filters.push(Biquad::new(
            BiquadFilterType::Lowshelf,
            LOWSHELF_FREQ,
            sample_rate,
            DEFAULT_Q_HIGH_LOW_SHELF,
            result.lowshelf_gain_db,
        ));
    }

    if result.highshelf_gain_db.abs() >= MIN_CORRECTION_DB {
        shelf_filters.push(Biquad::new(
            BiquadFilterType::Highshelf,
            HIGHSHELF_FREQ,
            sample_rate,
            DEFAULT_Q_HIGH_LOW_SHELF,
            result.highshelf_gain_db,
        ));
    }

    let eq_plugin = if shelf_filters.is_empty() {
        None
    } else {
        Some(output::create_eq_plugin(&shelf_filters))
    };

    let gain_plugin = if result.flat_gain_db.abs() >= MIN_CORRECTION_DB {
        Some(output::create_gain_plugin(result.flat_gain_db))
    } else {
        None
    };

    (eq_plugin, gain_plugin)
}

// ============================================================================
// Public helpers (exposed for broadband target matching)
// ============================================================================

/// Compute broadband alignment corrections to match a specific target curve.
///
/// Unlike `compute_spectral_alignment` which matches channels to their average,
/// this function matches a single channel to an explicit target curve.
/// This is used for "Broadband Target Matching" before fine EQ.
pub fn compute_target_alignment(
    curve: &Curve,
    target: &Curve,
    min_freq: f64,
    max_freq: f64,
    sample_rate: f64,
) -> Option<SpectralAlignmentResult> {
    // Build mask: only consider frequencies within [min_freq, max_freq]
    // where both curve and target have data (assuming same freq grid)
    let freq = &curve.freq;
    let mask: Vec<bool> = freq
        .iter()
        .map(|&f| f >= min_freq && f <= max_freq)
        .collect();
    let n_active: usize = mask.iter().filter(|m| **m).count();

    if n_active < 3 {
        return None;
    }

    // Extract active frequencies
    let active_freq: Array1<f64> = Array1::from(
        freq.iter()
            .zip(mask.iter())
            .filter(|(_, m)| **m)
            .map(|(f, _)| *f)
            .collect::<Vec<_>>(),
    );

    // Extract active SPL for channel and target
    let channel_spl: Array1<f64> = Array1::from(
        curve
            .spl
            .iter()
            .zip(mask.iter())
            .filter(|(_, m)| **m)
            .map(|(s, _)| *s)
            .collect::<Vec<_>>(),
    );

    let target_spl: Array1<f64> = Array1::from(
        target
            .spl
            .iter()
            .zip(mask.iter())
            .filter(|(_, m)| **m)
            .map(|(s, _)| *s)
            .collect::<Vec<_>>(),
    );

    // diff = channel - target (positive diff means channel is too loud)
    let diff = &channel_spl - &target_spl;

    // Compute weights
    let weights = compute_octave_weights(&active_freq);

    // Iteratively fit shelf + gain to the difference
    // Results are what the channel *has* relative to target
    let (ls_fit, hs_fit, flat_fit, residual_rms) =
        fit_shelf_gain_iterative(&diff, &active_freq, sample_rate, &weights);

    // Determine corrections (negative of fit)
    let ls_gain = (-ls_fit).clamp(-MAX_SHELF_GAIN_DB, MAX_SHELF_GAIN_DB);
    let hs_gain = (-hs_fit).clamp(-MAX_SHELF_GAIN_DB, MAX_SHELF_GAIN_DB);
    let flat_gain = (-flat_fit).clamp(-MAX_FLAT_GAIN_DB, MAX_FLAT_GAIN_DB);

    // If corrections are negligible, return None
    if ls_gain.abs() < MIN_CORRECTION_DB
        && hs_gain.abs() < MIN_CORRECTION_DB
        && flat_gain.abs() < MIN_CORRECTION_DB
    {
        return None;
    }

    Some(SpectralAlignmentResult {
        lowshelf_gain_db: ls_gain,
        highshelf_gain_db: hs_gain,
        flat_gain_db: flat_gain,
        residual_rms_db: residual_rms,
    })
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Compute the pointwise-average reference curve across all channels (masked).
fn compute_reference_curve(
    curves: &HashMap<String, Curve>,
    mask: &[bool],
    n_active: usize,
) -> Array1<f64> {
    let n_channels = curves.len() as f64;
    let mut sum = Array1::zeros(n_active);

    for curve in curves.values() {
        let active_spl: Vec<f64> = curve
            .spl
            .iter()
            .zip(mask.iter())
            .filter(|(_, m)| **m)
            .map(|(s, _)| *s)
            .collect();
        sum += &Array1::from(active_spl);
    }

    sum / n_channels
}

/// Build basis vectors for the low-shelf and high-shelf at 1 dB gain.
///
/// Evaluates the dB response of each shelf filter at the given frequencies.
fn build_basis_vectors(freq: &Array1<f64>, sample_rate: f64) -> (Array1<f64>, Array1<f64>) {
    let ls = Biquad::new(
        BiquadFilterType::Lowshelf,
        LOWSHELF_FREQ,
        sample_rate,
        DEFAULT_Q_HIGH_LOW_SHELF,
        1.0, // 1 dB basis
    );

    let hs = Biquad::new(
        BiquadFilterType::Highshelf,
        HIGHSHELF_FREQ,
        sample_rate,
        DEFAULT_Q_HIGH_LOW_SHELF,
        1.0, // 1 dB basis
    );

    (ls.np_log_result(freq), hs.np_log_result(freq))
}

/// Compute octave-spaced weights for log-frequency weighting.
///
/// Weight_i = log2(f_{i+1}) - log2(f_{i-1}), normalized so Σw = n.
/// This gives equal weight per octave of frequency range.
fn compute_octave_weights(freq: &Array1<f64>) -> Array1<f64> {
    let n = freq.len();
    let mut weights = Array1::zeros(n);

    let log2_freq: Vec<f64> = freq.iter().map(|&f| f.log2()).collect();

    // Interior points: half the span between neighbors
    for i in 1..n - 1 {
        weights[i] = (log2_freq[i + 1] - log2_freq[i - 1]) / 2.0;
    }
    // Boundary points
    if n >= 2 {
        weights[0] = log2_freq[1] - log2_freq[0];
        weights[n - 1] = log2_freq[n - 1] - log2_freq[n - 2];
    }

    // Normalize so sum = n (preserves scale of the residual)
    let total: f64 = weights.sum();
    if total > 0.0 {
        weights *= n as f64 / total;
    }

    weights
}

/// Evaluate the actual dB response of low-shelf + high-shelf + flat gain.
fn evaluate_shelf_response(
    freq: &Array1<f64>,
    sample_rate: f64,
    ls_gain: f64,
    hs_gain: f64,
    flat_gain: f64,
) -> Array1<f64> {
    let n = freq.len();
    let mut response = Array1::from_elem(n, flat_gain);

    if ls_gain.abs() > 1e-12 {
        let ls = Biquad::new(
            BiquadFilterType::Lowshelf,
            LOWSHELF_FREQ,
            sample_rate,
            DEFAULT_Q_HIGH_LOW_SHELF,
            ls_gain,
        );
        response += &ls.np_log_result(freq);
    }

    if hs_gain.abs() > 1e-12 {
        let hs = Biquad::new(
            BiquadFilterType::Highshelf,
            HIGHSHELF_FREQ,
            sample_rate,
            DEFAULT_Q_HIGH_LOW_SHELF,
            hs_gain,
        );
        response += &hs.np_log_result(freq);
    }

    response
}

/// Fit low-shelf + high-shelf + flat gain to a target difference curve using
/// Levenberg-Marquardt bounded nonlinear least squares.
///
/// The LM solver handles the nonlinear relationship between shelf gain (dB) and
/// shelf frequency response shape, with damping to prevent divergence when basis
/// vectors become nearly collinear (e.g., narrow-band data).
fn fit_shelf_gain_iterative(
    diff: &Array1<f64>,
    freq: &Array1<f64>,
    sample_rate: f64,
    weights: &Array1<f64>,
) -> (f64, f64, f64, f64) {
    let n = freq.len();
    let flat_basis = Array1::ones(n);

    // Initial linear estimate using 1 dB basis (provides a good starting point)
    let (ls_basis, hs_basis) = build_basis_vectors(freq, sample_rate);
    let (ls_init, hs_init, flat_init, _) =
        solve_3x3_wls(diff, &ls_basis, &hs_basis, &flat_basis, weights);

    // NaN guard on initial solve (ill-conditioned matrix)
    let x0 = if ls_init.is_finite() && hs_init.is_finite() && flat_init.is_finite() {
        ndarray::array![ls_init, hs_init, flat_init]
    } else {
        ndarray::array![0.0, 0.0, 0.0]
    };

    // Capture references for the residual closure
    let diff = diff.clone();
    let freq = freq.clone();
    let weights = weights.clone();

    let residual_fn = |x: &Array1<f64>| -> Array1<f64> {
        let response = evaluate_shelf_response(&freq, sample_rate, x[0], x[1], x[2]);
        let r = &diff - &response;
        // Bake sqrt(weights) into residuals so LM minimizes sum(w_i * r_i^2)
        &r * &weights.mapv(f64::sqrt)
    };

    let bounds = [
        (-MAX_SHELF_GAIN_DB * 2.0, MAX_SHELF_GAIN_DB * 2.0), // ls_gain
        (-MAX_SHELF_GAIN_DB * 2.0, MAX_SHELF_GAIN_DB * 2.0), // hs_gain
        (-MAX_FLAT_GAIN_DB, MAX_FLAT_GAIN_DB),                // flat_gain
    ];

    let config = LMConfigBuilder::new()
        .x0(x0)
        .maxiter(10)
        .tol(1e-10)
        .jacobian_epsilon(0.1)
        .build();

    let report = match levenberg_marquardt(&residual_fn, &bounds, config) {
        Ok(r) => r,
        Err(_) => {
            // Fallback to the linear estimate (clamped); guard NaN (clamp(NaN) = NaN)
            let ls = if ls_init.is_finite() {
                ls_init.clamp(-MAX_SHELF_GAIN_DB * 2.0, MAX_SHELF_GAIN_DB * 2.0)
            } else {
                0.0
            };
            let hs = if hs_init.is_finite() {
                hs_init.clamp(-MAX_SHELF_GAIN_DB * 2.0, MAX_SHELF_GAIN_DB * 2.0)
            } else {
                0.0
            };
            let flat = if flat_init.is_finite() {
                flat_init.clamp(-MAX_FLAT_GAIN_DB, MAX_FLAT_GAIN_DB)
            } else {
                0.0
            };
            let actual = evaluate_shelf_response(&freq, sample_rate, ls, hs, flat);
            let residual = &diff - &actual;
            let rms = (residual
                .iter()
                .zip(weights.iter())
                .map(|(&r, &w)| w * r * r)
                .sum::<f64>()
                / n as f64)
                .sqrt();
            return (ls, hs, flat, rms);
        }
    };

    // Compute final residual RMS (in unweighted dB space)
    let actual = evaluate_shelf_response(&freq, sample_rate, report.x[0], report.x[1], report.x[2]);
    let residual = &diff - &actual;
    let weighted_sq: f64 = residual
        .iter()
        .zip(weights.iter())
        .map(|(&r, &w)| w * r * r)
        .sum();
    let residual_rms = (weighted_sq / n as f64).sqrt();

    (report.x[0], report.x[1], report.x[2], residual_rms)
}

/// Solve the 3×3 weighted least squares problem:
///
///   minimize Σ w_i · (diff_i - a·ls_i - b·hs_i - c)²
///
/// via the normal equations  (B^T W B) x = B^T W d
///
/// Returns `(ls_gain, hs_gain, flat_gain, residual_rms)`.
fn solve_3x3_wls(
    diff: &Array1<f64>,
    ls_basis: &Array1<f64>,
    hs_basis: &Array1<f64>,
    flat_basis: &Array1<f64>,
    weights: &Array1<f64>,
) -> (f64, f64, f64, f64) {
    let n = diff.len();

    // B^T W B  (3×3 symmetric matrix)
    //   [ls·w·ls  ls·w·hs  ls·w·1]
    //   [hs·w·ls  hs·w·hs  hs·w·1]
    //   [ 1·w·ls   1·w·hs   1·w·1]
    let wls = weights * ls_basis;
    let whs = weights * hs_basis;
    let w1 = weights * flat_basis;

    let a00 = ls_basis.dot(&wls);
    let a01 = ls_basis.dot(&whs);
    let a02 = ls_basis.dot(&w1);
    let a11 = hs_basis.dot(&whs);
    let a12 = hs_basis.dot(&w1);
    let a22 = flat_basis.dot(&w1);

    // B^T W d  (3-vector)
    let wd = weights * diff;
    let b0 = ls_basis.dot(&wd);
    let b1 = hs_basis.dot(&wd);
    let b2 = flat_basis.dot(&wd);

    // Solve 3×3 symmetric system via Cramer's rule
    // A = [[a00 a01 a02], [a01 a11 a12], [a02 a12 a22]]
    let det = a00 * (a11 * a22 - a12 * a12) - a01 * (a01 * a22 - a12 * a02)
        + a02 * (a01 * a12 - a11 * a02);

    if det.abs() < 1e-30 {
        // Singular matrix — fall back to flat gain only
        let flat_gain = if a22.abs() > 1e-30 { b2 / a22 } else { 0.0 };
        return (0.0, 0.0, flat_gain, 0.0);
    }

    let inv_det = 1.0 / det;

    // Cofactor / adjugate for symmetric 3×3
    let x0 = ((a11 * a22 - a12 * a12) * b0
        + (a02 * a12 - a01 * a22) * b1
        + (a01 * a12 - a02 * a11) * b2)
        * inv_det;

    let x1 = ((a02 * a12 - a01 * a22) * b0
        + (a00 * a22 - a02 * a02) * b1
        + (a01 * a02 - a00 * a12) * b2)
        * inv_det;

    let x2 = ((a01 * a12 - a02 * a11) * b0
        + (a01 * a02 - a00 * a12) * b1
        + (a00 * a11 - a01 * a01) * b2)
        * inv_det;

    // Compute residual RMS
    let fitted = ls_basis * x0 + hs_basis * x1 + flat_basis * x2;
    let residual = diff - &fitted;
    let weighted_sq: f64 = residual
        .iter()
        .zip(weights.iter())
        .map(|(&r, &w)| w * r * r)
        .sum();
    let residual_rms = (weighted_sq / n as f64).sqrt();

    (x0, x1, x2, residual_rms)
}

/// Log spectral alignment results for all channels.
pub fn log_spectral_alignment(results: &HashMap<String, SpectralAlignmentResult>) {
    for (name, result) in results {
        let has_shelves = result.lowshelf_gain_db.abs() >= MIN_CORRECTION_DB
            || result.highshelf_gain_db.abs() >= MIN_CORRECTION_DB;
        let has_gain = result.flat_gain_db.abs() >= MIN_CORRECTION_DB;

        if has_shelves || has_gain {
            info!(
                "  Channel '{}': spectral alignment LS={:+.2} dB @ {} Hz, \
                 HS={:+.2} dB @ {} Hz, gain={:+.2} dB (residual {:.2} dB RMS)",
                name,
                result.lowshelf_gain_db,
                LOWSHELF_FREQ,
                result.highshelf_gain_db,
                HIGHSHELF_FREQ,
                result.flat_gain_db,
                result.residual_rms_db,
            );
        } else {
            info!(
                "  Channel '{}': no spectral alignment needed (residual {:.2} dB RMS)",
                name, result.residual_rms_db,
            );
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f64 = 48000.0;

    /// Build a simple Curve at log-spaced frequencies from 20 Hz to 20 kHz
    fn make_curve(spl_fn: impl Fn(f64) -> f64) -> Curve {
        let n = 200;
        let log_start = 20f64.log10();
        let log_end = 20000f64.log10();
        let freq: Vec<f64> = (0..n)
            .map(|i| 10f64.powf(log_start + (log_end - log_start) * i as f64 / (n - 1) as f64))
            .collect();
        let spl: Vec<f64> = freq.iter().map(|&f| spl_fn(f)).collect();
        Curve {
            freq: Array1::from(freq),
            spl: Array1::from(spl),
            phase: None,
        }
    }

    /// Build a narrow-band Curve at log-spaced frequencies within [min_freq, max_freq]
    fn make_narrow_curve(spl_fn: impl Fn(f64) -> f64, min_freq: f64, max_freq: f64) -> Curve {
        let n = 50;
        let log_start = min_freq.log10();
        let log_end = max_freq.log10();
        let freq: Vec<f64> = (0..n)
            .map(|i| 10f64.powf(log_start + (log_end - log_start) * i as f64 / (n - 1) as f64))
            .collect();
        let spl: Vec<f64> = freq.iter().map(|&f| spl_fn(f)).collect();
        Curve {
            freq: Array1::from(freq),
            spl: Array1::from(spl),
            phase: None,
        }
    }

    #[test]
    fn test_flat_offset() {
        // L is 2 dB above flat, R is 0 dB. Reference = 1 dB.
        // Expected: L gets -1 dB flat, R gets +1 dB flat, shelves ≈ 0.
        let mut curves = HashMap::new();
        curves.insert("L".to_string(), make_curve(|_| 2.0));
        curves.insert("R".to_string(), make_curve(|_| 0.0));

        let results = compute_spectral_alignment(&curves, SAMPLE_RATE, 20.0, 20000.0);

        let l = &results["L"];
        let r = &results["R"];

        // Shelves should be near zero
        assert!(
            l.lowshelf_gain_db.abs() < 0.3,
            "L lowshelf should be ~0, got {}",
            l.lowshelf_gain_db
        );
        assert!(
            l.highshelf_gain_db.abs() < 0.3,
            "L highshelf should be ~0, got {}",
            l.highshelf_gain_db
        );
        assert!(
            r.lowshelf_gain_db.abs() < 0.3,
            "R lowshelf should be ~0, got {}",
            r.lowshelf_gain_db
        );
        assert!(
            r.highshelf_gain_db.abs() < 0.3,
            "R highshelf should be ~0, got {}",
            r.highshelf_gain_db
        );

        // Flat gains should be opposite and sum to 0 (after renormalization)
        assert!(
            (l.flat_gain_db + r.flat_gain_db).abs() < 0.01,
            "flat gains should sum to 0"
        );
        // L should get negative correction, R positive
        assert!(
            l.flat_gain_db < -0.5,
            "L flat should be negative, got {}",
            l.flat_gain_db
        );
        assert!(
            r.flat_gain_db > 0.5,
            "R flat should be positive, got {}",
            r.flat_gain_db
        );
    }

    #[test]
    fn test_bass_tilt() {
        // L has 3 dB extra bass (low frequencies boosted), R is flat.
        // This should produce a lowshelf correction on L.
        let mut curves = HashMap::new();
        // L: +3 dB below 200 Hz, tapering to 0 above
        curves.insert(
            "L".to_string(),
            make_curve(|f| if f < 200.0 { 3.0 } else { 0.0 }),
        );
        curves.insert("R".to_string(), make_curve(|_| 0.0));

        let results = compute_spectral_alignment(&curves, SAMPLE_RATE, 20.0, 20000.0);

        let l = &results["L"];
        // L should have negative lowshelf correction (cut bass to match reference)
        assert!(
            l.lowshelf_gain_db < -0.3,
            "L should need LS cut, got {}",
            l.lowshelf_gain_db
        );
        // Highshelf should be small
        assert!(
            l.highshelf_gain_db.abs() < 1.5,
            "L HS should be small, got {}",
            l.highshelf_gain_db
        );
    }

    #[test]
    fn test_treble_tilt() {
        // L has 3 dB extra treble, R is flat.
        let mut curves = HashMap::new();
        curves.insert(
            "L".to_string(),
            make_curve(|f| if f > 4000.0 { 3.0 } else { 0.0 }),
        );
        curves.insert("R".to_string(), make_curve(|_| 0.0));

        let results = compute_spectral_alignment(&curves, SAMPLE_RATE, 20.0, 20000.0);

        let l = &results["L"];
        // L should have negative highshelf correction (cut treble)
        assert!(
            l.highshelf_gain_db < -0.3,
            "L should need HS cut, got {}",
            l.highshelf_gain_db
        );
        // Lowshelf should be small relative to highshelf
        assert!(
            l.lowshelf_gain_db.abs() < l.highshelf_gain_db.abs(),
            "LS ({}) should be smaller than HS ({})",
            l.lowshelf_gain_db,
            l.highshelf_gain_db
        );
    }

    #[test]
    fn test_clamping() {
        // L is 20 dB above R — shelves should be clamped to ±6 dB
        let mut curves = HashMap::new();
        curves.insert(
            "L".to_string(),
            make_curve(|f| if f < 200.0 { 20.0 } else { 0.0 }),
        );
        curves.insert("R".to_string(), make_curve(|_| 0.0));

        let results = compute_spectral_alignment(&curves, SAMPLE_RATE, 20.0, 20000.0);

        for result in results.values() {
            assert!(
                result.lowshelf_gain_db.abs() <= MAX_SHELF_GAIN_DB + 0.01,
                "LS gain {} exceeds max ±{}",
                result.lowshelf_gain_db,
                MAX_SHELF_GAIN_DB
            );
            assert!(
                result.highshelf_gain_db.abs() <= MAX_SHELF_GAIN_DB + 0.01,
                "HS gain {} exceeds max ±{}",
                result.highshelf_gain_db,
                MAX_SHELF_GAIN_DB
            );
        }
    }

    #[test]
    fn test_single_channel() {
        let mut curves = HashMap::new();
        curves.insert("L".to_string(), make_curve(|_| 0.0));

        let results = compute_spectral_alignment(&curves, SAMPLE_RATE, 20.0, 20000.0);
        assert!(
            results.is_empty(),
            "Single channel should produce no alignment"
        );
    }

    #[test]
    fn test_solver_identity() {
        // If diff is exactly 2·ls_basis + 3·hs_basis + 1·flat, solver should recover those coefficients.
        let n = 100;
        let freq = Array1::linspace(20.0, 20000.0, n);
        let (ls_basis, hs_basis) = build_basis_vectors(&freq, SAMPLE_RATE);
        let flat_basis = Array1::ones(n);
        let weights = compute_octave_weights(&freq);

        let diff = &ls_basis * 2.0 + &hs_basis * 3.0 + &flat_basis * 1.0;

        let (ls, hs, flat, residual) =
            solve_3x3_wls(&diff, &ls_basis, &hs_basis, &flat_basis, &weights);

        assert!((ls - 2.0).abs() < 0.01, "LS should be 2.0, got {}", ls);
        assert!((hs - 3.0).abs() < 0.01, "HS should be 3.0, got {}", hs);
        assert!(
            (flat - 1.0).abs() < 0.01,
            "flat should be 1.0, got {}",
            flat
        );
        assert!(residual < 0.01, "residual should be ~0, got {}", residual);
    }

    #[test]
    fn test_create_alignment_plugins_shelves_and_gain() {
        let result = SpectralAlignmentResult {
            lowshelf_gain_db: -2.0,
            highshelf_gain_db: 1.5,
            flat_gain_db: -1.0,
            residual_rms_db: 0.5,
        };

        let (eq, gain) = create_alignment_plugins(&result, SAMPLE_RATE);

        assert!(eq.is_some(), "should have EQ plugin for shelves");
        let eq = eq.unwrap();
        assert_eq!(eq.plugin_type, "eq");
        let filters = eq.parameters["filters"].as_array().unwrap();
        assert_eq!(filters.len(), 2, "should have LS + HS");

        assert!(gain.is_some(), "should have gain plugin");
        let gain = gain.unwrap();
        assert_eq!(gain.plugin_type, "gain");
    }

    #[test]
    fn test_create_alignment_plugins_gain_only() {
        let result = SpectralAlignmentResult {
            lowshelf_gain_db: 0.0,
            highshelf_gain_db: 0.0,
            flat_gain_db: -2.0,
            residual_rms_db: 0.3,
        };

        let (eq, gain) = create_alignment_plugins(&result, SAMPLE_RATE);

        assert!(eq.is_none(), "no shelves → no EQ plugin");
        assert!(gain.is_some(), "should have gain plugin");
    }

    #[test]
    fn test_create_alignment_plugins_none() {
        let result = SpectralAlignmentResult {
            lowshelf_gain_db: 0.0,
            highshelf_gain_db: 0.0,
            flat_gain_db: 0.0,
            residual_rms_db: 0.1,
        };

        let (eq, gain) = create_alignment_plugins(&result, SAMPLE_RATE);

        assert!(eq.is_none());
        assert!(gain.is_none());
    }

    #[test]
    fn test_iterative_improves_large_gain_accuracy() {
        // At 5-6 dB shelf gains, the nonlinear shelf shape diverges from the
        // linear 1 dB basis. The iterative solver should produce a lower residual
        // than a single linear solve.
        //
        // We construct a difference curve that IS exactly the shelf response at
        // 5 dB, then verify the iterative solver recovers it accurately.
        let n = 200;
        let log_start = 20f64.log10();
        let log_end = 20000f64.log10();
        let freq: Array1<f64> = Array1::from(
            (0..n)
                .map(|i| 10f64.powf(log_start + (log_end - log_start) * i as f64 / (n - 1) as f64))
                .collect::<Vec<_>>(),
        );

        // Ground truth: lowshelf at +5 dB, highshelf at -4 dB, flat +1 dB
        let true_ls = 5.0;
        let true_hs = -4.0;
        let true_flat = 1.0;
        let diff = evaluate_shelf_response(&freq, SAMPLE_RATE, true_ls, true_hs, true_flat);

        let weights = compute_octave_weights(&freq);

        // Iterative solver
        let (ls, hs, flat, residual) =
            fit_shelf_gain_iterative(&diff, &freq, SAMPLE_RATE, &weights);

        assert!(
            (ls - true_ls).abs() < 0.05,
            "LS should be {}, got {} (error {})",
            true_ls,
            ls,
            (ls - true_ls).abs()
        );
        assert!(
            (hs - true_hs).abs() < 0.05,
            "HS should be {}, got {} (error {})",
            true_hs,
            hs,
            (hs - true_hs).abs()
        );
        assert!(
            (flat - true_flat).abs() < 0.05,
            "flat should be {}, got {} (error {})",
            true_flat,
            flat,
            (flat - true_flat).abs()
        );
        assert!(residual < 0.01, "residual should be ~0, got {}", residual);

        // Compare: linear-only solve should have higher residual
        let flat_basis = Array1::ones(n);
        let (ls_basis, hs_basis) = build_basis_vectors(&freq, SAMPLE_RATE);
        let (lin_ls, lin_hs, lin_flat, _) =
            solve_3x3_wls(&diff, &ls_basis, &hs_basis, &flat_basis, &weights);
        let lin_response = evaluate_shelf_response(&freq, SAMPLE_RATE, lin_ls, lin_hs, lin_flat);
        let lin_residual_vec = &diff - &lin_response;
        let lin_rms = (lin_residual_vec
            .iter()
            .zip(weights.iter())
            .map(|(&r, &w)| w * r * r)
            .sum::<f64>()
            / n as f64)
            .sqrt();

        assert!(
            residual < lin_rms,
            "Iterative residual ({:.4}) should be less than linear-only ({:.4})",
            residual,
            lin_rms
        );
    }

    #[test]
    fn test_three_channels() {
        // Three channels: L boosted bass, C flat, R boosted treble
        let mut curves = HashMap::new();
        curves.insert(
            "L".to_string(),
            make_curve(|f| if f < 200.0 { 2.0 } else { 0.0 }),
        );
        curves.insert("C".to_string(), make_curve(|_| 0.0));
        curves.insert(
            "R".to_string(),
            make_curve(|f| if f > 4000.0 { 2.0 } else { 0.0 }),
        );

        let results = compute_spectral_alignment(&curves, SAMPLE_RATE, 20.0, 20000.0);

        assert_eq!(results.len(), 3);
        // Sum of flat gains should be ~0 after renormalization
        let flat_sum: f64 = results.values().map(|r| r.flat_gain_db).sum();
        assert!(
            flat_sum.abs() < 0.1,
            "flat gains should sum to ~0, got {}",
            flat_sum
        );
    }

    #[test]
    fn test_narrow_band_no_divergence() {
        // Narrow frequency range 100-400 Hz: lowshelf (200 Hz) and flat basis
        // become nearly collinear. The old Gauss-Newton solver diverged here
        // with flat_gain exploding to ±60+ dB. LM damping prevents this.
        let mut curves = HashMap::new();
        curves.insert(
            "L".to_string(),
            make_narrow_curve(|_| -30.0, 100.0, 400.0),
        );
        curves.insert(
            "R".to_string(),
            make_narrow_curve(|_| -32.0, 100.0, 400.0),
        );

        let results = compute_spectral_alignment(&curves, SAMPLE_RATE, 100.0, 400.0);

        for (name, r) in &results {
            assert!(
                r.flat_gain_db.abs() <= MAX_FLAT_GAIN_DB + 0.01,
                "Channel '{}' flat_gain {:.2} dB exceeds ±{} dB",
                name,
                r.flat_gain_db,
                MAX_FLAT_GAIN_DB
            );
            assert!(
                r.flat_gain_db.is_finite(),
                "Channel '{}' flat_gain is not finite",
                name
            );
            assert!(
                r.lowshelf_gain_db.is_finite(),
                "Channel '{}' lowshelf_gain is not finite",
                name
            );
            assert!(
                r.highshelf_gain_db.is_finite(),
                "Channel '{}' highshelf_gain is not finite",
                name
            );
        }
    }

    #[test]
    fn test_identical_channels_zero_correction() {
        // Two identical channels should produce zero corrections
        let mut curves = HashMap::new();
        curves.insert("L".to_string(), make_curve(|_| 0.0));
        curves.insert("R".to_string(), make_curve(|_| 0.0));

        let results = compute_spectral_alignment(&curves, SAMPLE_RATE, 20.0, 20000.0);

        for (name, r) in &results {
            assert!(
                r.flat_gain_db.abs() < MIN_CORRECTION_DB,
                "Channel '{}' flat_gain should be ~0, got {:.4}",
                name,
                r.flat_gain_db
            );
            assert!(
                r.lowshelf_gain_db.abs() < MIN_CORRECTION_DB,
                "Channel '{}' lowshelf should be ~0, got {:.4}",
                name,
                r.lowshelf_gain_db
            );
            assert!(
                r.highshelf_gain_db.abs() < MIN_CORRECTION_DB,
                "Channel '{}' highshelf should be ~0, got {:.4}",
                name,
                r.highshelf_gain_db
            );
        }
    }

    /// Regression test: broadband target matching must not produce large corrections
    /// when the measurement is level-shifted relative to the target.
    ///
    /// Before the fix, `compute_target_alignment` compared a measurement at +5dB mean
    /// against a 0dB-centered target, producing a catastrophic -5dB flat_gain that
    /// cascaded into +20dB EQ boosts. The caller must level-align the target to the
    /// measurement's mean before calling this function.
    #[test]
    fn test_target_alignment_level_offset_must_not_cause_large_correction() {
        // Simulate a measurement at ~5 dB mean (typical room measurement)
        let measurement = make_curve(|_| 5.0);
        // Target: level-aligned to measurement mean (5.0) + small tilt (-0.8 dB/oct)
        let target = make_curve(|f| 5.0 + (-0.8) * (f / 1000.0).log2());

        let result = compute_target_alignment(&measurement, &target, 20.0, 20000.0, SAMPLE_RATE);

        // With a level-aligned target, the flat gain should be small (only tilt mismatch)
        if let Some(r) = &result {
            assert!(
                r.flat_gain_db.abs() < 3.0,
                "flat_gain should be small when target is level-aligned, got {:.2}dB",
                r.flat_gain_db
            );
        }

        // Now test the BAD case: target NOT level-aligned (centered at 0dB).
        // This is what caused the catastrophic bug. The correction should be large.
        let bad_target = make_curve(|f| (-0.8) * (f / 1000.0).log2());
        let bad_result =
            compute_target_alignment(&measurement, &bad_target, 20.0, 20000.0, SAMPLE_RATE);

        if let Some(r) = &bad_result {
            // The flat_gain will be clamped to MAX_FLAT_GAIN_DB, but it's still large
            assert!(
                r.flat_gain_db.abs() > 3.0,
                "un-aligned target should produce large flat_gain, got {:.2}dB",
                r.flat_gain_db
            );
        }
    }

    /// Test that target alignment with a flat measurement and flat target at same level
    /// produces negligible corrections.
    #[test]
    fn test_target_alignment_same_level_flat() {
        let mean_level = 7.0; // arbitrary absolute SPL
        let measurement = make_curve(|_| mean_level);
        let target = make_curve(|_| mean_level);

        let result = compute_target_alignment(&measurement, &target, 20.0, 20000.0, SAMPLE_RATE);

        // Should be None (negligible corrections) or have very small values
        if let Some(r) = &result {
            assert!(
                r.flat_gain_db.abs() < MIN_CORRECTION_DB,
                "flat_gain should be negligible, got {:.4}dB",
                r.flat_gain_db
            );
            assert!(
                r.lowshelf_gain_db.abs() < MIN_CORRECTION_DB,
                "lowshelf should be negligible, got {:.4}dB",
                r.lowshelf_gain_db
            );
            assert!(
                r.highshelf_gain_db.abs() < MIN_CORRECTION_DB,
                "highshelf should be negligible, got {:.4}dB",
                r.highshelf_gain_db
            );
        }
    }

    /// Test that target alignment with tilt produces shelf corrections, not flat gain.
    #[test]
    fn test_target_alignment_tilt_produces_shelf_not_flat() {
        let mean_level = 5.0;
        // Flat measurement
        let measurement = make_curve(|_| mean_level);
        // Tilted target at same mean level
        let target = make_curve(|f| mean_level + (-0.8) * (f / 1000.0).log2());

        let result = compute_target_alignment(&measurement, &target, 20.0, 20000.0, SAMPLE_RATE);

        if let Some(r) = result {
            // Should have shelf corrections (the tilt shape) but small flat gain
            assert!(
                r.flat_gain_db.abs() < 2.0,
                "flat_gain should be small for pure tilt, got {:.2}dB",
                r.flat_gain_db
            );
            // At least one shelf should be non-trivial to correct the tilt
            let has_shelf = r.lowshelf_gain_db.abs() > MIN_CORRECTION_DB
                || r.highshelf_gain_db.abs() > MIN_CORRECTION_DB;
            assert!(has_shelf, "tilt should produce shelf corrections");
        }
    }

    /// Regression test: broadband target matching must use a FLAT target
    /// (at the measurement's mean level), NOT a tilted target.
    ///
    /// If the target includes the tilt, the broadband shelves push the measurement
    /// toward the tilt, and then the EQ optimizer subtracts the tilt again — double-
    /// applying it. The correct pattern is:
    ///   broadband target = flat at mean_spl  (only corrects broadband shape)
    ///   EQ optimizer target = measurement - tilt_curve  (handles tilt)
    ///
    /// With a tilted target, the broadband shelves add the tilt shape.
    /// Then `optimization_curve = curve_for_optim - tilt_curve` subtracts it
    /// again, but the shelf approximation doesn't perfectly cancel, leaving
    /// artifacts that the EQ fights against. We verify the flat target
    /// does NOT include tilt-shaped shelves.
    #[test]
    fn test_broadband_must_use_flat_target_not_tilted() {
        // Flat measurement at uniform level
        let measurement = make_curve(|_| 5.0);

        // CORRECT: flat target at measurement level → negligible corrections
        let flat_target = make_curve(|_| 5.0);
        let flat_result =
            compute_target_alignment(&measurement, &flat_target, 20.0, 20000.0, SAMPLE_RATE);

        // BAD: tilted target → shelves try to impose the tilt shape
        let tilted_target = make_curve(|f| 5.0 + (-0.8) * (f / 1000.0).log2());
        let tilted_result =
            compute_target_alignment(&measurement, &tilted_target, 20.0, 20000.0, SAMPLE_RATE);

        // With flat measurement + flat target: corrections should be negligible
        let flat_total = flat_result
            .as_ref()
            .map(|r| r.flat_gain_db.abs() + r.lowshelf_gain_db.abs() + r.highshelf_gain_db.abs())
            .unwrap_or(0.0);
        assert!(
            flat_total < 1.0,
            "flat measurement + flat target should need negligible correction, got {:.2}dB",
            flat_total
        );

        // With flat measurement + tilted target: shelves must be non-trivial
        // (the alignment tries to impose a tilt that doesn't exist in the data)
        if let Some(r) = &tilted_result {
            let tilted_total =
                r.flat_gain_db.abs() + r.lowshelf_gain_db.abs() + r.highshelf_gain_db.abs();
            assert!(
                tilted_total > 1.0,
                "flat measurement + tilted target should produce shelf corrections, got {:.2}dB",
                tilted_total
            );
        }
    }

    /// Test that broadband alignment only produces low-Q (gentle) corrections.
    /// The shelf filters have fixed frequencies (200Hz, 4000Hz) and the
    /// gains are clamped to MAX_SHELF_GAIN_DB (6dB). This ensures the
    /// broadband stage never produces aggressive narrow corrections.
    #[test]
    fn test_broadband_corrections_are_gentle() {
        // Measurement with a 10dB peak at 300Hz (aggressive room mode)
        let measurement = make_curve(|f| {
            let peak = 10.0 * (-((f.log2() - 300.0_f64.log2()).powi(2)) / 0.5).exp();
            5.0 + peak
        });
        let target = make_curve(|_| 5.0);

        let result =
            compute_target_alignment(&measurement, &target, 20.0, 20000.0, SAMPLE_RATE);

        if let Some(r) = result {
            // Shelf gains must be within the clamped limits
            assert!(
                r.lowshelf_gain_db.abs() <= MAX_SHELF_GAIN_DB + 0.01,
                "lowshelf {:.2}dB exceeds limit {:.1}dB",
                r.lowshelf_gain_db,
                MAX_SHELF_GAIN_DB
            );
            assert!(
                r.highshelf_gain_db.abs() <= MAX_SHELF_GAIN_DB + 0.01,
                "highshelf {:.2}dB exceeds limit {:.1}dB",
                r.highshelf_gain_db,
                MAX_SHELF_GAIN_DB
            );
            assert!(
                r.flat_gain_db.abs() <= MAX_FLAT_GAIN_DB + 0.01,
                "flat_gain {:.2}dB exceeds limit {:.1}dB",
                r.flat_gain_db,
                MAX_FLAT_GAIN_DB
            );
        }
    }
}
