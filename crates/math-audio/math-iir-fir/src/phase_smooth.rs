//! Phase smoothing via group delay
//!
//! This module provides functions for smoothing noisy phase measurements
//! by converting to group delay, applying octave smoothing, and integrating
//! back to phase.
//!
//! # Approach
//!
//! Group delay smoothing is preferred over direct phase smoothing because:
//! - Group delay (derivative of phase) has no wrapping discontinuities
//! - Preserves linear delay component naturally
//! - Consistent with how audio engineers think about phase
//! - Existing octave-smoothing pattern can be reused
//!
//! # Pipeline
//!
//! ```text
//! Measured Phase → Unwrap → Differentiate to Group Delay → Smooth (1/N octave) → Integrate → Smoothed Phase
//! ```

use std::f64::consts::PI;

/// Smooth phase via group delay smoothing
///
/// This function takes measured phase data and smooths it by:
/// 1. Unwrapping the phase to remove 2π discontinuities
/// 2. Computing group delay (negative derivative of phase vs frequency)
/// 3. Smoothing the group delay using octave-based averaging
/// 4. Integrating back to get smoothed phase
///
/// # Arguments
/// * `freqs` - Frequency points in Hz (must be positive and sorted ascending)
/// * `phase_rad` - Phase values in radians at each frequency point
/// * `smoothing_octaves` - Smoothing width in octaves (e.g., 0.167 for 1/6 octave)
///
/// # Returns
/// * Smoothed phase values in radians
///
/// # Example
/// ```
/// use math_audio_iir_fir::smooth_phase_via_group_delay;
///
/// let freqs = vec![100.0, 200.0, 400.0, 800.0, 1600.0];
/// let phase_rad = vec![0.0, -0.5, -1.0, -1.5, -2.0];
/// let smoothed = smooth_phase_via_group_delay(&freqs, &phase_rad, 0.167);
/// assert_eq!(smoothed.len(), phase_rad.len());
/// ```
pub fn smooth_phase_via_group_delay(
    freqs: &[f64],
    phase_rad: &[f64],
    smoothing_octaves: f64,
) -> Vec<f64> {
    if freqs.is_empty() || phase_rad.is_empty() {
        return Vec::new();
    }

    if freqs.len() != phase_rad.len() {
        return phase_rad.to_vec(); // Return unchanged if mismatched
    }

    if freqs.len() < 3 {
        return phase_rad.to_vec(); // Need at least 3 points for derivatives
    }

    // Step 1: Unwrap phase
    let unwrapped = unwrap_phase(phase_rad);

    // Step 2: Compute group delay
    let group_delay = compute_group_delay(freqs, &unwrapped);

    // Step 3: Smooth group delay using octave averaging
    let smoothed_gd = smooth_octave(freqs, &group_delay, smoothing_octaves);

    // Step 4: Integrate back to phase
    integrate_group_delay(freqs, &smoothed_gd, unwrapped[0])
}

/// Interpolate phase in the complex domain to avoid wrap artifacts
///
/// This function interpolates phase by converting to unit complex numbers,
/// interpolating in the complex plane, and converting back to phase.
/// This avoids artifacts that occur when linearly interpolating near ±π.
///
/// # Arguments
/// * `src_freqs` - Source frequency points in Hz
/// * `src_phase_rad` - Source phase values in radians
/// * `target_freqs` - Target frequency points for interpolation
///
/// # Returns
/// * Interpolated phase values in radians at target frequencies
pub fn interpolate_phase_complex(
    src_freqs: &[f64],
    src_phase_rad: &[f64],
    target_freqs: &[f64],
) -> Vec<f64> {
    if src_freqs.is_empty() || src_phase_rad.is_empty() {
        return vec![0.0; target_freqs.len()];
    }

    target_freqs
        .iter()
        .map(|&f| {
            // Find bracketing indices
            let (lower_idx, upper_idx, t) = find_interpolation_indices(src_freqs, f);

            if lower_idx == upper_idx {
                return src_phase_rad[lower_idx];
            }

            // Convert to unit complex numbers
            let phase_low = src_phase_rad[lower_idx];
            let phase_high = src_phase_rad[upper_idx];

            let z_low = (phase_low.cos(), phase_low.sin());
            let z_high = (phase_high.cos(), phase_high.sin());

            // Interpolate in complex plane
            let z_interp = (
                z_low.0 + t * (z_high.0 - z_low.0),
                z_low.1 + t * (z_high.1 - z_low.1),
            );

            // Convert back to phase
            z_interp.1.atan2(z_interp.0)
        })
        .collect()
}

/// Unwrap phase by removing 2π discontinuities
///
/// This function detects jumps greater than π and adjusts subsequent
/// values to create a continuous phase curve.
///
/// # Arguments
/// * `phase_rad` - Phase values in radians (may have wrapping)
///
/// # Returns
/// * Unwrapped phase values (continuous)
pub fn unwrap_phase(phase_rad: &[f64]) -> Vec<f64> {
    if phase_rad.is_empty() {
        return Vec::new();
    }

    let two_pi = 2.0 * PI;
    let mut unwrapped = Vec::with_capacity(phase_rad.len());
    unwrapped.push(phase_rad[0]);

    for i in 1..phase_rad.len() {
        let diff = phase_rad[i] - phase_rad[i - 1];
        // Wrap diff to [-π, π], handling arbitrarily large jumps
        let wrapped_diff = diff - two_pi * (diff / two_pi).round();
        unwrapped.push(unwrapped[i - 1] + wrapped_diff);
    }

    unwrapped
}

/// Compute group delay from unwrapped phase
///
/// Group delay is defined as: τ_g(ω) = -dφ/dω = -dφ/df * (1/2π)
///
/// This function uses central differences for interior points and
/// forward/backward differences at the boundaries.
///
/// # Arguments
/// * `freqs` - Frequency points in Hz
/// * `phase_rad` - Unwrapped phase values in radians
///
/// # Returns
/// * Group delay in seconds at each frequency point
fn compute_group_delay(freqs: &[f64], phase_rad: &[f64]) -> Vec<f64> {
    let n = freqs.len();
    if n < 2 {
        return vec![0.0; n];
    }

    let mut group_delay = Vec::with_capacity(n);

    // Forward difference at start
    let df_start = freqs[1] - freqs[0];
    let dphi_start = phase_rad[1] - phase_rad[0];
    group_delay.push(-dphi_start / (2.0 * PI * df_start));

    // Central differences for interior points
    for i in 1..n - 1 {
        let df = freqs[i + 1] - freqs[i - 1];
        let dphi = phase_rad[i + 1] - phase_rad[i - 1];
        group_delay.push(-dphi / (2.0 * PI * df));
    }

    // Backward difference at end
    let df_end = freqs[n - 1] - freqs[n - 2];
    let dphi_end = phase_rad[n - 1] - phase_rad[n - 2];
    group_delay.push(-dphi_end / (2.0 * PI * df_end));

    group_delay
}

/// Smooth data using octave-based averaging
///
/// For each frequency point, averages all values within ±half_octaves
/// of that frequency (in log space).
///
/// # Arguments
/// * `freqs` - Frequency points in Hz
/// * `values` - Values to smooth
/// * `smoothing_octaves` - Total smoothing width in octaves
///
/// # Returns
/// * Smoothed values
fn smooth_octave(freqs: &[f64], values: &[f64], smoothing_octaves: f64) -> Vec<f64> {
    if freqs.is_empty() || values.is_empty() || smoothing_octaves <= 0.0 {
        return values.to_vec();
    }

    let half_octaves = smoothing_octaves / 2.0;
    let ratio = 2.0_f64.powf(half_octaves);

    freqs
        .iter()
        .enumerate()
        .map(|(i, &f)| {
            if f <= 0.0 {
                return values[i];
            }

            let f_low = f / ratio;
            let f_high = f * ratio;

            let mut sum = 0.0;
            let mut count = 0;

            for (j, &freq) in freqs.iter().enumerate() {
                if freq >= f_low && freq <= f_high {
                    sum += values[j];
                    count += 1;
                }
            }

            if count > 0 {
                sum / count as f64
            } else {
                values[i]
            }
        })
        .collect()
}

/// Integrate group delay back to phase
///
/// Uses trapezoidal integration: φ(f) = φ₀ - 2π ∫ τ_g(f) df
///
/// # Arguments
/// * `freqs` - Frequency points in Hz
/// * `group_delay` - Group delay values in seconds
/// * `initial_phase` - Initial phase value (at first frequency)
///
/// # Returns
/// * Reconstructed phase values in radians
fn integrate_group_delay(freqs: &[f64], group_delay: &[f64], initial_phase: f64) -> Vec<f64> {
    if freqs.is_empty() || group_delay.is_empty() {
        return Vec::new();
    }

    let mut phase = Vec::with_capacity(freqs.len());
    phase.push(initial_phase);

    for i in 1..freqs.len() {
        let df = freqs[i] - freqs[i - 1];
        // Trapezoidal integration
        let avg_gd = (group_delay[i] + group_delay[i - 1]) / 2.0;
        // φ = φ_prev - 2π * τ_g * Δf
        let new_phase = phase[i - 1] - 2.0 * PI * avg_gd * df;
        phase.push(new_phase);
    }

    phase
}

/// Find interpolation indices and parameter for a target frequency
///
/// Returns (lower_idx, upper_idx, t) where t is the interpolation parameter [0, 1]
/// in log space.
fn find_interpolation_indices(freqs: &[f64], target_freq: f64) -> (usize, usize, f64) {
    if freqs.is_empty() {
        return (0, 0, 0.0);
    }

    if target_freq <= freqs[0] || target_freq <= 0.0 {
        return (0, 0, 0.0);
    }

    if target_freq >= freqs[freqs.len() - 1] {
        let last = freqs.len() - 1;
        return (last, last, 0.0);
    }

    // Binary search for the interval
    let mut lower = 0;
    let mut upper = freqs.len() - 1;

    while upper - lower > 1 {
        let mid = (lower + upper) / 2;
        if freqs[mid] <= target_freq {
            lower = mid;
        } else {
            upper = mid;
        }
    }

    // Compute log-space interpolation parameter
    let log_target = target_freq.ln();
    let log_low = freqs[lower].ln();
    let log_high = freqs[upper].ln();

    let t = if (log_high - log_low).abs() > 1e-10 {
        (log_target - log_low) / (log_high - log_low)
    } else {
        0.0
    };

    (lower, upper, t.clamp(0.0, 1.0))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn test_unwrap_phase_no_wrapping() {
        let phase = vec![0.0, 0.5, 1.0, 1.5, 2.0];
        let unwrapped = unwrap_phase(&phase);
        assert_eq!(unwrapped, phase);
    }

    #[test]
    fn test_unwrap_phase_with_positive_wrap() {
        // Phase jumps from near π to near -π (should add 2π)
        let phase = vec![0.0, 1.0, 2.0, 3.0, -3.0, -2.0];
        let unwrapped = unwrap_phase(&phase);

        // After unwrapping, the sequence should be continuous
        assert!(approx_eq(unwrapped[0], 0.0, 1e-10));
        assert!(approx_eq(unwrapped[4], -3.0 + 2.0 * PI, 1e-10));
        assert!(approx_eq(unwrapped[5], -2.0 + 2.0 * PI, 1e-10));
    }

    #[test]
    fn test_unwrap_phase_with_negative_wrap() {
        // Phase jumps from near -π to near π (should subtract 2π)
        let phase = vec![0.0, -1.0, -2.0, -3.0, 3.0, 2.0];
        let unwrapped = unwrap_phase(&phase);

        assert!(approx_eq(unwrapped[0], 0.0, 1e-10));
        assert!(approx_eq(unwrapped[4], 3.0 - 2.0 * PI, 1e-10));
        assert!(approx_eq(unwrapped[5], 2.0 - 2.0 * PI, 1e-10));
    }

    #[test]
    fn test_unwrap_phase_empty() {
        let phase: Vec<f64> = vec![];
        let unwrapped = unwrap_phase(&phase);
        assert!(unwrapped.is_empty());
    }

    #[test]
    fn test_compute_group_delay_constant_slope() {
        // Linear phase with constant slope should give constant group delay
        // φ(f) = -2π * τ * f => τ_g = τ
        let delay = 0.001; // 1ms delay
        let freqs: Vec<f64> = (1..=10).map(|i| i as f64 * 100.0).collect(); // 100-1000 Hz
        let phase: Vec<f64> = freqs.iter().map(|&f| -2.0 * PI * delay * f).collect();

        let gd = compute_group_delay(&freqs, &phase);

        // All group delay values should be approximately 1ms
        for &g in &gd {
            assert!(
                approx_eq(g, delay, 1e-6),
                "Group delay {} should be close to {}",
                g,
                delay
            );
        }
    }

    #[test]
    fn test_smooth_octave_flat_data() {
        let freqs: Vec<f64> = (1..=10)
            .map(|i| 100.0 * 2.0_f64.powf(i as f64 / 3.0))
            .collect();
        let values = vec![1.0; freqs.len()];

        let smoothed = smooth_octave(&freqs, &values, 0.5);

        // Flat data should remain flat after smoothing
        for &v in &smoothed {
            assert!(approx_eq(v, 1.0, 1e-10));
        }
    }

    #[test]
    fn test_smooth_octave_reduces_noise() {
        let freqs: Vec<f64> = (0..100)
            .map(|i| 20.0 * 2.0_f64.powf(i as f64 * 0.1))
            .collect();
        let values: Vec<f64> = freqs
            .iter()
            .enumerate()
            .map(|(i, _)| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();

        let smoothed = smooth_octave(&freqs, &values, 0.5);

        // Smoothed values should have lower variance than original
        let orig_var: f64 = values.iter().map(|v| v * v).sum::<f64>() / values.len() as f64;
        let smooth_var: f64 = smoothed.iter().map(|v| v * v).sum::<f64>() / smoothed.len() as f64;

        assert!(
            smooth_var < orig_var,
            "Smoothed variance {} should be less than original {}",
            smooth_var,
            orig_var
        );
    }

    #[test]
    fn test_integrate_group_delay_roundtrip() {
        // Create a phase curve, compute group delay, integrate back
        let delay = 0.002; // 2ms
        let freqs: Vec<f64> = (1..=20).map(|i| i as f64 * 50.0).collect(); // 50-1000 Hz
        let original_phase: Vec<f64> = freqs.iter().map(|&f| -2.0 * PI * delay * f).collect();

        let gd = compute_group_delay(&freqs, &original_phase);
        let reconstructed = integrate_group_delay(&freqs, &gd, original_phase[0]);

        // Reconstructed phase should match original (within numerical precision)
        for i in 0..freqs.len() {
            assert!(
                approx_eq(reconstructed[i], original_phase[i], 0.1),
                "At freq {}: reconstructed {} != original {}",
                freqs[i],
                reconstructed[i],
                original_phase[i]
            );
        }
    }

    #[test]
    fn test_smooth_phase_via_group_delay_linear_phase() {
        // Linear phase (constant group delay) should be preserved after smoothing
        let delay = 0.001;
        let freqs: Vec<f64> = (1..=50)
            .map(|i| 20.0 * 2.0_f64.powf(i as f64 * 0.2))
            .collect();
        let phase: Vec<f64> = freqs.iter().map(|&f| -2.0 * PI * delay * f).collect();

        let smoothed = smooth_phase_via_group_delay(&freqs, &phase, 0.167);

        // For a constant delay system, the group delay should be constant
        // After smoothing and integration, the phase should still be approximately linear
        // We check that the phase values are correlated with frequency
        let n = freqs.len();

        // Calculate the average group delay from the smoothed phase
        // If phase is linear with frequency, group delay is constant
        let gd_samples: Vec<f64> = (1..n)
            .map(|i| {
                let dphi = smoothed[i] - smoothed[i - 1];
                let df = freqs[i] - freqs[i - 1];
                -dphi / (2.0 * PI * df)
            })
            .collect();

        let avg_gd: f64 = gd_samples.iter().sum::<f64>() / gd_samples.len() as f64;

        // The average group delay should be close to original delay
        assert!(
            approx_eq(avg_gd, delay, delay * 0.5),
            "Average group delay {} should be close to expected {}",
            avg_gd,
            delay
        );
    }

    #[test]
    fn test_interpolate_phase_complex_basic() {
        let src_freqs = vec![100.0, 1000.0, 10000.0];
        let src_phase = vec![0.0, -PI / 2.0, -PI];

        let target_freqs = vec![100.0, 316.0, 1000.0, 3160.0, 10000.0];
        let result = interpolate_phase_complex(&src_freqs, &src_phase, &target_freqs);

        // Known points should match
        assert!(approx_eq(result[0], 0.0, 0.1));
        assert!(approx_eq(result[2], -PI / 2.0, 0.1));
        assert!(approx_eq(result[4], -PI, 0.1));
    }

    #[test]
    fn test_interpolate_phase_complex_near_wrap() {
        // Test interpolation near the wrap point (±π)
        let src_freqs = vec![100.0, 200.0];
        let src_phase = vec![PI * 0.9, -PI * 0.9]; // Near wrap point

        let target_freqs = vec![150.0];
        let result = interpolate_phase_complex(&src_freqs, &src_phase, &target_freqs);

        // Should interpolate through the "short way" (near π)
        // not through 0
        assert!(
            result[0].abs() > PI / 2.0,
            "Interpolated phase {} should be near ±π",
            result[0]
        );
    }

    #[test]
    fn test_smooth_phase_via_group_delay_empty() {
        let freqs: Vec<f64> = vec![];
        let phase: Vec<f64> = vec![];
        let result = smooth_phase_via_group_delay(&freqs, &phase, 0.167);
        assert!(result.is_empty());
    }

    #[test]
    fn test_smooth_phase_via_group_delay_short() {
        // With < 3 points, should return unchanged
        let freqs = vec![100.0, 200.0];
        let phase = vec![0.0, -0.5];
        let result = smooth_phase_via_group_delay(&freqs, &phase, 0.167);
        assert_eq!(result, phase);
    }

    #[test]
    fn test_find_interpolation_indices() {
        let freqs = vec![100.0, 200.0, 400.0, 800.0];

        // Below range
        let (l, u, _t) = find_interpolation_indices(&freqs, 50.0);
        assert_eq!(l, 0);
        assert_eq!(u, 0);

        // Above range
        let (l, u, _t) = find_interpolation_indices(&freqs, 1000.0);
        assert_eq!(l, 3);
        assert_eq!(u, 3);

        // At exact point
        let (l, u, _t) = find_interpolation_indices(&freqs, 200.0);
        assert_eq!(l, 1);
        assert_eq!(u, 2); // Due to binary search, upper will be next

        // Between points
        let (l, u, t) = find_interpolation_indices(&freqs, 300.0);
        assert_eq!(l, 1);
        assert_eq!(u, 2);
        assert!(t > 0.0 && t < 1.0);
    }
}
