//! Group delay optimization for subwoofer-main speaker alignment.
//!
//! This module provides algorithms for aligning speakers in the time domain:
//! - `optimize_group_delay`: Finds optimal delay to minimize GD variance
//! - `optimize_gd_iir`: Generates All-Pass filters to match GD slopes

use crate::error::Result;
use crate::Curve;
use log::debug;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use ndarray::Array1;
use num_complex::Complex64;
use std::f64::consts::PI;

/// Configuration for group delay optimization
#[derive(Debug, Clone)]
pub struct GroupDelayConfig {
    /// Maximum delay to search (ms)
    pub max_delay_ms: f64,
    /// Tolerance for Brent's method (ms)
    pub tolerance_ms: f64,
    /// Maximum iterations for Brent's method
    pub max_iterations: usize,
}

impl Default for GroupDelayConfig {
    fn default() -> Self {
        Self {
            max_delay_ms: 30.0,
            tolerance_ms: 0.01,
            max_iterations: 100,
        }
    }
}

/// Configuration for All-Pass filter optimization
#[derive(Debug, Clone)]
pub struct ApOptimizerConfig {
    /// Maximum number of AP filters to use (1-3)
    pub max_filters: usize,
    /// Minimum Q for AP filters
    pub min_q: f64,
    /// Maximum Q for AP filters
    pub max_q: f64,
    /// Grid resolution for initial search
    pub grid_resolution: usize,
    /// Fine-tune with local optimization
    pub fine_tune: bool,
}

impl Default for ApOptimizerConfig {
    fn default() -> Self {
        Self {
            max_filters: 2,
            min_q: 0.3,
            max_q: 4.0,
            grid_resolution: 15,
            fine_tune: true,
        }
    }
}

/// Optimize group delay alignment between a subwoofer and a speaker using Brent's method.
///
/// Returns the optimal delay (in ms) to apply to the speaker to align it with the subwoofer.
/// A positive value means the speaker should be delayed.
/// A negative value means the speaker is "too late" and the subwoofer should be delayed.
///
/// # Algorithm
/// Uses Brent's method for efficient 1D minimization, which combines:
/// - Bisection (robust)
/// - Secant method (fast convergence)
/// - Inverse quadratic interpolation (superlinear convergence)
///
/// This typically converges in 10-15 iterations vs 120+ for grid search.
pub fn optimize_group_delay(
    sub: &Curve,
    speaker: &Curve,
    min_freq: f64,
    max_freq: f64,
) -> Result<f64> {
    optimize_group_delay_with_config(
        sub,
        speaker,
        min_freq,
        max_freq,
        GroupDelayConfig::default(),
    )
}

/// Optimize group delay with custom configuration.
pub fn optimize_group_delay_with_config(
    sub: &Curve,
    speaker: &Curve,
    min_freq: f64,
    max_freq: f64,
    config: GroupDelayConfig,
) -> Result<f64> {
    let freq = &sub.freq;
    let speaker_interp = interpolate_curve(speaker, freq);

    let sub_complex = curve_to_complex(sub);
    let speaker_complex = curve_to_complex(&speaker_interp);

    // Pre-compute indices in frequency range for efficiency
    let range_indices: Vec<usize> = freq
        .iter()
        .enumerate()
        .filter(|&(_, &f)| f >= min_freq && f <= max_freq)
        .map(|(i, _)| i)
        .collect();

    // Use Brent's method for minimization
    let result = brent_minimize(
        |delay_ms| {
            evaluate_delay_fast(
                delay_ms,
                freq,
                &sub_complex,
                &speaker_complex,
                &range_indices,
            )
        },
        -config.max_delay_ms,
        config.max_delay_ms,
        config.tolerance_ms,
        config.max_iterations,
    );

    Ok(result)
}

/// Brent's method for 1D minimization.
///
/// Combines bisection, secant method, and inverse quadratic interpolation
/// for robust and efficient optimization.
fn brent_minimize<F>(mut f: F, mut a: f64, mut b: f64, tol: f64, max_iter: usize) -> f64
where
    F: FnMut(f64) -> f64,
{
    const GOLDEN_RATIO: f64 = 0.3819660112501051; // (3 - sqrt(5)) / 2

    let mut x = a + GOLDEN_RATIO * (b - a);
    let mut w = x;
    let mut v = x;

    let mut fx = f(x);
    let mut fw = fx;
    let mut fv = fx;

    let mut d: f64 = 0.0;
    let mut e: f64 = 0.0;

    for _ in 0..max_iter {
        let xm = 0.5 * (a + b);
        let tol1 = tol; // Use absolute tolerance (ms units)

        // Check convergence
        if (x - xm).abs() <= tol1 && (b - a) < 4.0 * tol1 {
            return x;
        }

        // Fit parabola
        let mut u: f64;
        let fu: f64;

        if e.abs() > tol1 {
            // Parabolic interpolation
            let r = (x - w) * (fx - fv);
            let q = (x - v) * (fx - fw);
            let p = (x - v) * q - (x - w) * r;
            let q = 2.0 * (q - r);

            let p = if q > 0.0 { -p } else { -p };
            let q = q.abs();

            if q.abs() < 1e-10 {
                u = x + GOLDEN_RATIO * e;
            } else {
                let etemp = e;
                e = d;
                if (p.abs() < 0.5 * q * etemp) && p > q * (a - x) && p < q * (b - x) {
                    d = p / q;
                    u = x + d;
                    if (u - a) < tol1 || (b - u) < tol1 {
                        d = if x < xm { tol1 } else { -tol1 };
                    }
                } else {
                    e = if x >= xm { a - x } else { b - x };
                    d = GOLDEN_RATIO * e;
                }
            }
        } else {
            e = if x >= xm { a - x } else { b - x };
            d = GOLDEN_RATIO * e;
        }

        u = if d.abs() >= tol1 {
            x + d
        } else {
            x + if d > 0.0 { tol1 } else { -tol1 }
        };
        fu = f(u);

        if fu <= fx {
            if u >= x {
                a = x;
            } else {
                b = x;
            }
            v = w;
            fv = fw;
            w = x;
            fw = fx;
            x = u;
            fx = fu;
        } else {
            if u < x {
                a = u;
            } else {
                b = u;
            }
            if fu <= fw || (w - x).abs() < 1e-10 {
                v = w;
                fv = fw;
                w = u;
                fw = fu;
            } else if fu <= fv || (v - x).abs() < 1e-10 || (v - w).abs() < 1e-10 {
                v = u;
                fv = fu;
            }
        }
    }

    x
}

/// Fast delay evaluation using pre-computed range indices.
fn evaluate_delay_fast(
    delay_ms: f64,
    freq: &Array1<f64>,
    sub: &Array1<Complex64>,
    speaker: &Array1<Complex64>,
    range_indices: &[usize],
) -> f64 {
    if range_indices.is_empty() {
        return f64::INFINITY;
    }

    let delay_s = delay_ms / 1000.0;

    // Calculate combined phase at each frequency in range
    let mut phases = Vec::with_capacity(range_indices.len());
    for &i in range_indices {
        let f = freq[i];
        let w = 2.0 * PI * f;
        let phase_shift = -w * delay_s;
        let rot = Complex64::from_polar(1.0, phase_shift);

        let combined = sub[i] + speaker[i] * rot;
        phases.push(combined.arg());
    }

    // Compute group delay variance from phases
    let unwrapped = unwrap_phase(&phases);

    // Calculate GD from finite differences
    let mut gd_values = Vec::with_capacity(unwrapped.len() - 1);
    for i in 0..unwrapped.len() - 1 {
        let idx1 = range_indices[i];
        let idx2 = range_indices[i + 1];

        let d_phi = unwrapped[i + 1] - unwrapped[i];
        let d_f = freq[idx2] - freq[idx1];
        let d_w = 2.0 * PI * d_f;

        if d_w.abs() > 1e-9 {
            let gd = -d_phi / d_w * 1000.0; // Convert to ms
            if gd.is_finite() {
                gd_values.push(gd);
            }
        }
    }

    if gd_values.is_empty() {
        return f64::INFINITY;
    }

    // Standard deviation of GD
    let mean = gd_values.iter().sum::<f64>() / gd_values.len() as f64;
    let variance =
        gd_values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / gd_values.len() as f64;
    variance.sqrt()
}

/// Optimize All-Pass filters for Main speakers to match Subwoofer group delay (IIR Mode).
///
/// Returns a list of Biquad filters (All-Pass) to be applied to the Mains.
/// Uses multiple AP filters for better matching of complex GD curves.
pub fn optimize_gd_iir(
    sub: &Curve,
    speaker: &Curve,
    min_freq: f64,
    max_freq: f64,
    sample_rate: f64,
) -> Result<Vec<Biquad>> {
    optimize_gd_iir_with_config(
        sub,
        speaker,
        min_freq,
        max_freq,
        sample_rate,
        ApOptimizerConfig::default(),
    )
}

/// Optimize All-Pass filters with custom configuration.
pub fn optimize_gd_iir_with_config(
    sub: &Curve,
    speaker: &Curve,
    min_freq: f64,
    max_freq: f64,
    sample_rate: f64,
    config: ApOptimizerConfig,
) -> Result<Vec<Biquad>> {
    let freq = &sub.freq;
    let speaker_interp = interpolate_curve(speaker, freq);

    let sub_complex = curve_to_complex(sub);
    let spk_complex = curve_to_complex(&speaker_interp);

    let sub_gd = calculate_group_delay(freq, sub_complex.as_slice().unwrap());
    let spk_gd = calculate_group_delay(freq, spk_complex.as_slice().unwrap());

    // Compute target GD (difference that AP filters need to add)
    let target_gd: Vec<f64> = sub_gd
        .iter()
        .zip(spk_gd.iter())
        .map(|(&s, &p)| s - p)
        .collect();

    // Pre-compute indices in frequency range
    let range_indices: Vec<usize> = freq
        .iter()
        .enumerate()
        .filter(|&(_, &f)| f >= min_freq && f <= max_freq)
        .map(|(i, _)| i)
        .collect();

    // Try different numbers of filters and pick the best
    let mut best_filters = Vec::new();
    let mut best_error = f64::INFINITY;

    for n_filters in 1..=config.max_filters {
        let (filters, error) = optimize_ap_filters_n(
            freq,
            &target_gd,
            &spk_gd,
            &sub_gd,
            &range_indices,
            sample_rate,
            min_freq,
            max_freq,
            n_filters,
            &config,
        );

        // Only accept if improvement is significant (> 10%)
        if error < best_error * 0.9 || best_filters.is_empty() {
            best_error = error;
            best_filters = filters;
        } else {
            // No significant improvement, stop adding filters
            break;
        }
    }

    if !best_filters.is_empty() {
        debug!(
            "GD-Opt: {} AP filters, error={:.3}ms RMS",
            best_filters.len(),
            best_error
        );
    }

    Ok(best_filters)
}

/// Optimize N All-Pass filters.
#[allow(clippy::too_many_arguments)]
fn optimize_ap_filters_n(
    freq: &Array1<f64>,
    _target_gd: &[f64],
    spk_gd: &[f64],
    sub_gd: &[f64],
    range_indices: &[usize],
    sample_rate: f64,
    min_freq: f64,
    max_freq: f64,
    n_filters: usize,
    config: &ApOptimizerConfig,
) -> (Vec<Biquad>, f64) {
    let _n_params = 2 * n_filters; // (freq, Q) for each filter
    let grid_res = config.grid_resolution;

    // Bounds for each parameter
    let log_min = min_freq.ln();
    let log_max = max_freq.ln();

    // Multi-dimensional grid search with local refinement
    // Initialize with grid search
    let grid_size = grid_res.min(10); // Limit for performance
    let mut best_params = vec![0.0f64; 2 * n_filters];

    // Iterative optimization: optimize one filter at a time
    let mut current_gd = spk_gd.to_vec();

    for filter_idx in 0..n_filters {
        let mut best_f = (log_min + log_max) / 2.0;
        let mut best_q = 1.0;
        let mut filter_best_error = f64::INFINITY;

        // Grid search for this filter
        for fi in 0..grid_size {
            let t = fi as f64 / (grid_size - 1).max(1) as f64;
            let f = (log_min + t * (log_max - log_min)).exp();

            for qi in 0..grid_size {
                let q = config.min_q
                    + (qi as f64 / (grid_size - 1).max(1) as f64) * (config.max_q - config.min_q);

                let error = evaluate_single_ap_filter(
                    f,
                    q,
                    freq,
                    &current_gd,
                    sub_gd,
                    range_indices,
                    sample_rate,
                );

                if error < filter_best_error {
                    filter_best_error = error;
                    best_f = f;
                    best_q = q;
                }
            }
        }

        // Fine-tune with golden section search on frequency
        if config.fine_tune {
            let (f_refined, _) = golden_section_search(
                |f| {
                    evaluate_single_ap_filter(
                        f,
                        best_q,
                        freq,
                        &current_gd,
                        sub_gd,
                        range_indices,
                        sample_rate,
                    )
                },
                best_f * 0.8,
                best_f * 1.2,
                1.0,
                20,
            );
            best_f = f_refined;

            // Fine-tune Q
            let (q_refined, _) = golden_section_search(
                |q| {
                    evaluate_single_ap_filter(
                        best_f,
                        q,
                        freq,
                        &current_gd,
                        sub_gd,
                        range_indices,
                        sample_rate,
                    )
                },
                config.min_q,
                config.max_q,
                0.05,
                20,
            );
            best_q = q_refined;
        }

        // Store parameters
        best_params[filter_idx * 2] = best_f;
        best_params[filter_idx * 2 + 1] = best_q;

        // Update current GD for next filter
        let filter = Biquad::new(BiquadFilterType::AllPass, best_f, sample_rate, best_q, 0.0);
        for &i in range_indices {
            let ap_gd = compute_ap_gd_analytic(&filter, freq[i]);
            current_gd[i] += ap_gd;
        }
    }

    // Build filters and compute final error
    let filters: Vec<Biquad> = (0..n_filters)
        .map(|i| {
            Biquad::new(
                BiquadFilterType::AllPass,
                best_params[i * 2],
                sample_rate,
                best_params[i * 2 + 1],
                0.0,
            )
        })
        .collect();

    let final_error =
        evaluate_ap_filters(&filters, freq, spk_gd, sub_gd, range_indices, sample_rate);

    (filters, final_error)
}

/// Evaluate a single AP filter's contribution to GD matching.
fn evaluate_single_ap_filter(
    ap_freq: f64,
    ap_q: f64,
    freqs: &Array1<f64>,
    current_gd: &[f64],
    target_gd: &[f64],
    range_indices: &[usize],
    sample_rate: f64,
) -> f64 {
    let filter = Biquad::new(BiquadFilterType::AllPass, ap_freq, sample_rate, ap_q, 0.0);

    let mut total_error = 0.0;
    let mut count = 0;

    for &i in range_indices {
        let ap_gd = compute_ap_gd_analytic(&filter, freqs[i]);
        let combined_gd = current_gd[i] + ap_gd;
        let diff = combined_gd - target_gd[i];
        total_error += diff * diff;
        count += 1;
    }

    if count == 0 {
        f64::INFINITY
    } else {
        (total_error / count as f64).sqrt()
    }
}

/// Evaluate multiple AP filters.
fn evaluate_ap_filters(
    filters: &[Biquad],
    freqs: &Array1<f64>,
    spk_gd: &[f64],
    sub_gd: &[f64],
    range_indices: &[usize],
    _sample_rate: f64,
) -> f64 {
    let mut total_error = 0.0;
    let mut count = 0;

    for &i in range_indices {
        let mut ap_gd_total = 0.0;
        for filter in filters {
            ap_gd_total += compute_ap_gd_analytic(filter, freqs[i]);
        }

        let combined_gd = spk_gd[i] + ap_gd_total;
        let diff = combined_gd - sub_gd[i];
        total_error += diff * diff;
        count += 1;
    }

    if count == 0 {
        f64::INFINITY
    } else {
        (total_error / count as f64).sqrt()
    }
}

/// Analytic group delay for a 2nd-order All-Pass filter.
///
/// GD(ω) = (2/Q) * (ω₀ * ω² + ω₀³) / ((ω₀² - ω²)² + (ω₀ * ω / Q)²)
///
/// This is faster and more accurate than numerical differentiation.
fn compute_ap_gd_analytic(filter: &Biquad, freq: f64) -> f64 {
    let w0 = 2.0 * PI * filter.freq;
    let w = 2.0 * PI * freq;
    let q = filter.q;

    let w0_sq = w0 * w0;
    let w_sq = w * w;

    let numerator = (2.0 / q) * (w0 * w_sq + w0_sq * w0);
    let denominator = (w0_sq - w_sq).powi(2) + (w0 * w / q).powi(2);

    if denominator < 1e-20 {
        return 0.0;
    }

    // Result in seconds, convert to ms
    (numerator / denominator) * 1000.0
}

/// Golden section search for 1D minimization.
fn golden_section_search<F>(f: F, a: f64, b: f64, tol: f64, max_iter: usize) -> (f64, f64)
where
    F: Fn(f64) -> f64,
{
    const PHI: f64 = 1.618033988749895; // Golden ratio
    const RESPHI: f64 = 2.0 - PHI; // 1 / PHI^2

    let mut a = a;
    let mut b = b;
    let mut c = b - RESPHI * (b - a);
    let mut fc = f(c);

    for _ in 0..max_iter {
        if (b - a).abs() < tol {
            break;
        }

        let d;
        if (b - c) > (c - a) {
            d = c + RESPHI * (b - c);
        } else {
            d = c - RESPHI * (c - a);
        }

        let fd = f(d);

        if fd < fc {
            if (b - c) > (c - a) {
                a = c;
            } else {
                b = c;
            }
            c = d;
            fc = fd;
        } else {
            if (b - c) > (c - a) {
                b = d;
            } else {
                a = d;
            }
        }
    }

    (c, fc)
}

/// Original delay evaluation (kept for compatibility).
fn evaluate_delay(
    delay_ms: f64,
    freq: &Array1<f64>,
    sub: &Array1<Complex64>,
    speaker: &Array1<Complex64>,
    min_freq: f64,
    max_freq: f64,
) -> f64 {
    let delay_s = delay_ms / 1000.0;
    let mut combined_complex = vec![Complex64::new(0.0, 0.0); freq.len()];

    for (i, ((&f, &sub_val), &speaker_val)) in
        freq.iter().zip(sub.iter()).zip(speaker.iter()).enumerate()
    {
        let w = 2.0 * PI * f;
        let phase_shift = -w * delay_s;
        let rot = Complex64::from_polar(1.0, phase_shift);
        combined_complex[i] = sub_val + speaker_val * rot;
    }

    let gd = calculate_group_delay(freq, &combined_complex);

    let mut values = Vec::new();
    for i in 0..freq.len() {
        if freq[i] >= min_freq && freq[i] <= max_freq && gd[i].is_finite() {
            values.push(gd[i]);
        }
    }

    if values.is_empty() {
        return f64::INFINITY;
    }

    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt()
}

fn calculate_group_delay(freq: &Array1<f64>, complex: &[Complex64]) -> Vec<f64> {
    let mut phases = Vec::with_capacity(complex.len());
    for c in complex {
        phases.push(c.arg());
    }

    let unwrapped = unwrap_phase(&phases);
    let mut gd = vec![0.0; freq.len()];

    for i in 0..freq.len() - 1 {
        let d_phi = unwrapped[i + 1] - unwrapped[i];
        let d_f = freq[i + 1] - freq[i];
        let d_w = 2.0 * PI * d_f;

        if d_w.abs() > 1e-9 {
            gd[i] = -d_phi / d_w;
        }
    }

    if freq.len() > 1 {
        gd[freq.len() - 1] = gd[freq.len() - 2];
    }

    gd.iter().map(|v| v * 1000.0).collect()
}

fn unwrap_phase(phase: &[f64]) -> Vec<f64> {
    let mut unwrapped = Vec::with_capacity(phase.len());
    if phase.is_empty() {
        return unwrapped;
    }

    unwrapped.push(phase[0]);
    let mut offset = 0.0;

    for i in 1..phase.len() {
        let diff = phase[i] - phase[i - 1];
        // Handle jumps of arbitrary multiples of 2π (not just single wraps).
        // This is equivalent to NumPy's np.unwrap: round the jump to the
        // nearest multiple of 2π and subtract it.
        let wraps = (diff / (2.0 * PI)).round();
        offset -= wraps * 2.0 * PI;
        unwrapped.push(phase[i] + offset);
    }
    unwrapped
}

fn curve_to_complex(curve: &Curve) -> Array1<Complex64> {
    let mut out = Array1::default(curve.spl.len());
    for i in 0..curve.spl.len() {
        let mag = 10.0_f64.powf(curve.spl[i] / 20.0);
        let phase_deg = curve.phase.as_ref().map(|p| p[i]).unwrap_or(0.0);
        let phase_rad = phase_deg.to_radians();
        out[i] = Complex64::from_polar(mag, phase_rad);
    }
    out
}

fn interpolate_curve(curve: &Curve, target_freq: &Array1<f64>) -> Curve {
    let complex_in = curve_to_complex(curve);

    let mut spl = Array1::zeros(target_freq.len());
    let mut phase = Array1::zeros(target_freq.len());
    let has_phase = curve.phase.is_some();

    for (i, &f) in target_freq.iter().enumerate() {
        let re = interp_linear_complex(&curve.freq, &complex_in, f, |c| c.re);
        let im = interp_linear_complex(&curve.freq, &complex_in, f, |c| c.im);
        let c = Complex64::new(re, im);

        spl[i] = 20.0 * c.norm().max(1e-12).log10();
        if has_phase {
            phase[i] = c.arg().to_degrees();
        }
    }

    Curve {
        freq: target_freq.clone(),
        spl,
        phase: if has_phase { Some(phase) } else { None },
    }
}

fn interp_linear_complex<F>(
    x: &Array1<f64>,
    y: &Array1<Complex64>,
    target: f64,
    extractor: F,
) -> f64
where
    F: Fn(&Complex64) -> f64,
{
    if target <= x[0] {
        return extractor(&y[0]);
    }
    if target >= x[x.len() - 1] {
        return extractor(&y[y.len() - 1]);
    }

    let idx = match x
        .as_slice()
        .unwrap()
        .binary_search_by(|v| v.partial_cmp(&target).unwrap())
    {
        Ok(i) => i,
        Err(i) => i - 1,
    };

    let x0 = x[idx];
    let x1 = x[idx + 1];
    let y0 = extractor(&y[idx]);
    let y1 = extractor(&y[idx + 1]);

    let t = (target - x0) / (x1 - x0);
    y0 + t * (y1 - y0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unwrap_phase() {
        let phase = vec![
            -170.0_f64.to_radians(),
            -175.0_f64.to_radians(),
            175.0_f64.to_radians(),
            170.0_f64.to_radians(),
        ];
        let unwrapped = unwrap_phase(&phase);

        let expected = vec![-170.0, -175.0, -185.0, -190.0];
        for (u, e) in unwrapped.iter().zip(expected.iter()) {
            assert!(
                (u.to_degrees() - e).abs() < 1e-5,
                "Got {}, expected {}",
                u.to_degrees(),
                e
            );
        }
    }

    #[test]
    fn test_unwrap_phase_multi_wrap() {
        // Phase values with jumps exceeding 2π between adjacent samples.
        // This can happen when phase data comes from sources other than .arg()
        // (e.g., accumulated computation, interpolated data, or scaled values).
        //
        // The old single-wrap code only corrected by ±2π, leaving a residual
        // jump of ~3π after correction. The round()-based code correctly
        // identifies the nearest multiple and eliminates the jump.
        let phase = vec![
            0.0,
            0.1,
            0.1 + 5.0 * PI, // raw jump of 5π (~15.7 rad), needs 2×2π correction
            0.2 + 5.0 * PI, // smooth continuation
        ];
        let unwrapped = unwrap_phase(&phase);

        // After unwrapping, no adjacent pair should have a jump > π
        for i in 1..unwrapped.len() {
            let jump = (unwrapped[i] - unwrapped[i - 1]).abs();
            assert!(
                jump < PI + 0.01,
                "Jump between samples {} and {} is {:.3} rad (> π), unwrapping failed",
                i - 1,
                i,
                jump
            );
        }
    }

    #[test]
    fn test_calculate_group_delay_constant() {
        let delay_s = 0.010;
        let freqs = Array1::linspace(20.0, 100.0, 10);
        let mut complex = Vec::new();

        for &f in &freqs {
            let w = 2.0 * PI * f;
            let phi = -w * delay_s;
            complex.push(Complex64::from_polar(1.0, phi));
        }

        let gd = calculate_group_delay(&freqs, &complex);

        for &d in &gd {
            assert!((d - 10.0).abs() < 0.1, "Expected 10ms, got {}", d);
        }
    }

    #[test]
    fn test_optimize_group_delay_alignment() {
        let fc = 80.0;
        let freqs = Array1::linspace(20.0, 200.0, 100);
        let mut sub_spl = Array1::zeros(freqs.len());
        let mut sub_phase = Array1::zeros(freqs.len());
        let mut spk_spl = Array1::zeros(freqs.len());
        let mut spk_phase = Array1::zeros(freqs.len());

        let sub_extra_delay_s = 0.005;

        for i in 0..freqs.len() {
            let f = freqs[i];
            let w = 2.0 * PI * f;
            let s = Complex64::new(0.0, f / fc);

            let lp = Complex64::new(1.0, 0.0) / (Complex64::new(1.0, 0.0) + s);
            let sub_rot = Complex64::from_polar(1.0, -w * sub_extra_delay_s);
            let sub_final = lp * sub_rot;

            sub_spl[i] = 20.0 * sub_final.norm().log10();
            sub_phase[i] = sub_final.arg().to_degrees();

            let hp = s / (Complex64::new(1.0, 0.0) + s);
            spk_spl[i] = 20.0 * hp.norm().log10();
            spk_phase[i] = hp.arg().to_degrees();
        }

        let sub = Curve {
            freq: freqs.clone(),
            spl: sub_spl,
            phase: Some(sub_phase),
        };
        let spk = Curve {
            freq: freqs.clone(),
            spl: spk_spl,
            phase: Some(spk_phase),
        };

        let delay = optimize_group_delay(&sub, &spk, 40.0, 160.0).unwrap();

        assert!((delay - 5.0).abs() < 0.1, "Expected 5.0ms, got {}", delay);
    }

    #[test]
    fn test_brent_minimization() {
        // Minimize (x - 3)^2
        let result = brent_minimize(|x| (x - 3.0).powi(2), -10.0, 10.0, 1e-6, 100);
        assert!((result - 3.0).abs() < 1e-5, "Expected 3.0, got {}", result);
    }

    #[test]
    fn test_golden_section_search() {
        // Minimize (x - 5)^2
        let (x, _) = golden_section_search(|x| (x - 5.0).powi(2), 0.0, 10.0, 1e-6, 50);
        assert!((x - 5.0).abs() < 1e-5, "Expected 5.0, got {}", x);
    }

    #[test]
    fn test_ap_gd_analytic() {
        // Test that analytic GD is positive and reasonable
        let filter = Biquad::new(BiquadFilterType::AllPass, 100.0, 48000.0, 1.0, 0.0);

        // At resonance frequency, GD should be maximum
        let gd_at_resonance = compute_ap_gd_analytic(&filter, 100.0);
        let gd_below = compute_ap_gd_analytic(&filter, 50.0);
        let gd_above = compute_ap_gd_analytic(&filter, 200.0);

        assert!(gd_at_resonance > 0.0, "GD at resonance should be positive");
        assert!(
            gd_at_resonance > gd_below,
            "GD at resonance should be higher than below"
        );
        assert!(
            gd_at_resonance > gd_above,
            "GD at resonance should be higher than above"
        );
    }

    #[test]
    fn test_multi_ap_optimization() {
        // Create a complex GD curve that requires multiple AP filters
        let freqs = Array1::linspace(20.0, 500.0, 200);

        // Simulate subwoofer with steeper rolloff (higher GD)
        let mut sub_gd = vec![0.0; freqs.len()];
        let mut spk_gd = vec![0.0; freqs.len()];

        for i in 0..freqs.len() {
            let f = freqs[i];
            // Sub has 24dB/oct rolloff at 80Hz
            if f < 80.0 {
                sub_gd[i] = 10.0_f64 * (80.0_f64 / f).sqrt();
            } else {
                sub_gd[i] = 10.0_f64 * (80.0_f64 / f).sqrt() * 0.5;
            }
            // Speaker has 12dB/oct rolloff at 80Hz
            spk_gd[i] = sub_gd[i] * 0.7;
        }

        let range_indices: Vec<usize> = freqs
            .iter()
            .enumerate()
            .filter(|&(_, &f)| f >= 30.0 && f <= 200.0)
            .map(|(i, _)| i)
            .collect();

        // Test with 2 AP filters
        let config = ApOptimizerConfig {
            max_filters: 2,
            ..Default::default()
        };

        let filters = optimize_ap_filters_n(
            &freqs,
            &sub_gd,
            &spk_gd,
            &sub_gd,
            &range_indices,
            48000.0,
            30.0,
            200.0,
            2,
            &config,
        );

        assert!(filters.0.len() <= 2, "Should use at most 2 filters");
        assert!(filters.1 < f64::INFINITY, "Error should be finite");
    }
}
