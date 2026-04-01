//! Group delay optimization for subwoofer-main speaker alignment.
//!
//! This module provides algorithms for aligning speakers in the time domain:
//! - `optimize_gd_iir`: Generates All-Pass filters to match GD slopes

use crate::Curve;
use crate::error::Result;
use log::debug;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use ndarray::Array1;
use num_complex::Complex64;
use std::f64::consts::PI;

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

        let d = if (b - c) > (c - a) {
            c + RESPHI * (b - c)
        } else {
            c - RESPHI * (c - a)
        };

        let fd = f(d);

        if fd < fc {
            if (b - c) > (c - a) {
                a = c;
            } else {
                b = c;
            }
            c = d;
            fc = fd;
        } else if (b - c) > (c - a) {
            b = d;
        } else {
            a = d;
        }
    }

    (c, fc)
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

        let expected = [-170.0, -175.0, -185.0, -190.0];
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

    /// Helper: build a synthetic Curve with phase from a constant delay.
    fn make_synthetic_curve_with_phase(
        freqs: &Array1<f64>,
        spl_fn: impl Fn(f64) -> f64,
        delay_ms: f64,
        _sample_rate: f64,
    ) -> Curve {
        let spl = freqs.map(|&f| spl_fn(f));
        let delay_s = delay_ms / 1000.0;
        let phase = freqs.map(|&f| (-2.0 * PI * f * delay_s).to_degrees());
        Curve {
            freq: freqs.clone(),
            spl,
            phase: Some(phase),
        }
    }

    #[test]
    fn test_optimize_gd_iir_basic() {
        // Sub with steep GD slope at 80Hz, speaker with shallow slope
        let n = 200;
        let freqs = Array1::linspace(20.0, 500.0, n);
        let sub = make_synthetic_curve_with_phase(&freqs, |_| 85.0, 10.0, 48000.0);
        let speaker = make_synthetic_curve_with_phase(&freqs, |_| 85.0, 5.0, 48000.0);

        let result = optimize_gd_iir(&sub, &speaker, 30.0, 200.0, 48000.0);
        assert!(result.is_ok(), "optimize_gd_iir should succeed");

        let filters = result.unwrap();
        assert!(!filters.is_empty(), "Should produce at least 1 AP filter");
        assert!(filters.len() <= 2, "Should produce at most 2 AP filters");

        for f in &filters {
            assert!(
                f.freq >= 20.0 && f.freq <= 500.0,
                "Filter freq {} out of range",
                f.freq
            );
            assert!(f.q >= 0.3 && f.q <= 4.0, "Filter Q {} out of range", f.q);
        }
    }

    #[test]
    fn test_optimize_gd_iir_identical_curves() {
        let freqs = Array1::linspace(20.0, 500.0, 200);
        let curve = make_synthetic_curve_with_phase(&freqs, |_| 85.0, 5.0, 48000.0);

        let result = optimize_gd_iir(&curve, &curve, 30.0, 200.0, 48000.0);
        assert!(result.is_ok());

        let filters = result.unwrap();
        // Identical curves → AP filters should have small GD contribution.
        // The optimizer may still produce filters (it always tries at least 1),
        // but the resulting AP group delay should be modest.
        let mut max_ap_gd_ms = 0.0_f64;
        for &f in freqs.iter().filter(|&&f| (30.0..=200.0).contains(&f)) {
            let mut total = 0.0;
            for filter in &filters {
                total += compute_ap_gd_analytic(filter, f);
            }
            max_ap_gd_ms = max_ap_gd_ms.max(total.abs());
        }
        // With identical curves the target GD mismatch is ~0, so AP contribution
        // should be small (optimizer found a low-impact filter). Allow up to 5ms.
        assert!(
            max_ap_gd_ms < 5.0,
            "AP GD contribution should be modest for identical curves, got {:.2}ms",
            max_ap_gd_ms
        );
    }

    #[test]
    fn test_optimize_gd_iir_max_filters_1() {
        let freqs = Array1::linspace(20.0, 500.0, 200);
        let sub = make_synthetic_curve_with_phase(&freqs, |_| 85.0, 10.0, 48000.0);
        let speaker = make_synthetic_curve_with_phase(&freqs, |_| 85.0, 5.0, 48000.0);

        let config = ApOptimizerConfig {
            max_filters: 1,
            ..Default::default()
        };
        let result = optimize_gd_iir_with_config(&sub, &speaker, 30.0, 200.0, 48000.0, config);
        assert!(result.is_ok());
        assert!(result.unwrap().len() <= 1, "Should use at most 1 filter");
    }

    #[test]
    fn test_optimize_gd_iir_max_filters_3() {
        let freqs = Array1::linspace(20.0, 500.0, 200);
        let sub = make_synthetic_curve_with_phase(&freqs, |_| 85.0, 10.0, 48000.0);
        let speaker = make_synthetic_curve_with_phase(&freqs, |_| 85.0, 5.0, 48000.0);

        // Test with max_filters=3
        let config3 = ApOptimizerConfig {
            max_filters: 3,
            ..Default::default()
        };
        let result3 = optimize_gd_iir_with_config(&sub, &speaker, 30.0, 200.0, 48000.0, config3);
        assert!(result3.is_ok());
        let filters3 = result3.unwrap();
        assert!(filters3.len() <= 3, "Should use at most 3 filters");

        // Test with max_filters=1 for comparison
        let config1 = ApOptimizerConfig {
            max_filters: 1,
            ..Default::default()
        };
        let result1 = optimize_gd_iir_with_config(&sub, &speaker, 30.0, 200.0, 48000.0, config1);
        assert!(result1.is_ok());
        let filters1 = result1.unwrap();

        // More filters should give equal or better error
        let range_indices: Vec<usize> = freqs
            .iter()
            .enumerate()
            .filter(|&(_, &f)| (30.0..=200.0).contains(&f))
            .map(|(i, _)| i)
            .collect();

        let sub_complex = curve_to_complex(&sub);
        let spk_complex = curve_to_complex(&speaker);
        let sub_gd = calculate_group_delay(&freqs, sub_complex.as_slice().unwrap());
        let spk_gd = calculate_group_delay(&freqs, spk_complex.as_slice().unwrap());

        let err1 =
            evaluate_ap_filters(&filters1, &freqs, &spk_gd, &sub_gd, &range_indices, 48000.0);
        let err3 =
            evaluate_ap_filters(&filters3, &freqs, &spk_gd, &sub_gd, &range_indices, 48000.0);
        assert!(
            err3 <= err1 * 1.01,
            "3 filters error ({}) should be <= 1 filter error ({})",
            err3,
            err1
        );
    }

    #[test]
    fn test_compute_ap_gd_analytic_known_values() {
        // AllPass at f0=100Hz, Q=0.707, sr=48000
        // At resonance, theoretical GD = 2*Q/(2*pi*f0) seconds = Q/(pi*f0)
        let f0 = 100.0;
        let q = 0.707;
        let filter = Biquad::new(BiquadFilterType::AllPass, f0, 48000.0, q, 0.0);

        let gd_at_resonance = compute_ap_gd_analytic(&filter, f0);
        // Theoretical: at resonance w=w0, GD = 2*Q/(pi*f0) * 1000 ms
        let theoretical_ms = 2.0 * q / (PI * f0) * 1000.0;
        let rel_error = (gd_at_resonance - theoretical_ms).abs() / theoretical_ms;
        assert!(
            rel_error < 0.05,
            "GD at resonance should be ~{:.4}ms (Q/(pi*f0)), got {:.4}ms (error {:.1}%)",
            theoretical_ms,
            gd_at_resonance,
            rel_error * 100.0
        );
    }

    #[test]
    fn test_interpolate_curve_identity() {
        // Interpolate a curve onto its own frequency grid → should be identity
        let freqs = Array1::linspace(20.0, 20000.0, 100);
        let spl = freqs.map(|&f| 85.0 - 10.0 * (f / 1000.0_f64).log10());
        let phase = freqs.map(|&f| -180.0 * f / 10000.0);
        let curve = Curve {
            freq: freqs.clone(),
            spl: spl.clone(),
            phase: Some(phase.clone()),
        };

        let interpolated = interpolate_curve(&curve, &freqs);

        for i in 0..freqs.len() {
            assert!(
                (interpolated.spl[i] - curve.spl[i]).abs() < 0.01,
                "SPL mismatch at index {}: {} vs {}",
                i,
                interpolated.spl[i],
                curve.spl[i]
            );
        }
    }

    #[test]
    fn test_evaluate_ap_filters_empty() {
        // Empty filter list should return raw GD mismatch error
        let freqs = Array1::linspace(20.0, 500.0, 100);
        let sub = make_synthetic_curve_with_phase(&freqs, |_| 85.0, 10.0, 48000.0);
        let speaker = make_synthetic_curve_with_phase(&freqs, |_| 85.0, 5.0, 48000.0);

        let sub_complex = curve_to_complex(&sub);
        let spk_complex = curve_to_complex(&speaker);
        let sub_gd = calculate_group_delay(&freqs, sub_complex.as_slice().unwrap());
        let spk_gd = calculate_group_delay(&freqs, spk_complex.as_slice().unwrap());

        let range_indices: Vec<usize> = (0..freqs.len()).collect();
        let empty_filters: Vec<Biquad> = vec![];

        let error = evaluate_ap_filters(
            &empty_filters,
            &freqs,
            &spk_gd,
            &sub_gd,
            &range_indices,
            48000.0,
        );
        assert!(
            error > 0.0,
            "Empty filters should give non-zero error for mismatched curves"
        );
        assert!(error < f64::INFINITY, "Error should be finite");
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
            .filter(|&(_, &f)| (30.0..=200.0).contains(&f))
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
