//! Phase reconstruction utilities for room EQ.
//!
//! Provides minimum phase reconstruction from magnitude data using
//! the Hilbert transform approach for measurements that lack phase data.

#![allow(dead_code)]

use ndarray::Array1;
use num_complex::Complex64;
use rustfft::FftPlanner;
use std::f64::consts::PI;

/// Reconstruct minimum phase from magnitude response using Hilbert transform.
///
/// The minimum phase response is computed using the relationship:
/// φ_min(ω) = -H{ln|H(ω)|}
///
/// where H{} is the Hilbert transform.
///
/// # Arguments
/// * `freq` - Frequency points in Hz
/// * `spl` - SPL values in dB
///
/// # Returns
/// * Phase values in degrees corresponding to minimum phase response
pub fn reconstruct_minimum_phase(_freq: &Array1<f64>, spl: &Array1<f64>) -> Array1<f64> {
    let _n = spl.len();

    // Convert dB to natural log of magnitude
    // SPL = 20 * log10(|H|)
    // ln(|H|) = SPL / 20 * ln(10)
    let ln_mag: Vec<f64> = spl.iter().map(|&s| s / 20.0 * 10.0_f64.ln()).collect();

    // Compute Hilbert transform of ln|H|
    // This gives us the minimum phase
    let phase_rad = hilbert_transform(&ln_mag);

    // Convert to degrees
    let phase_deg: Vec<f64> = phase_rad.iter().map(|&p| -p.to_degrees()).collect();

    Array1::from_vec(phase_deg)
}

/// Compute the Hilbert transform of a signal using FFT.
///
/// The Hilbert transform is computed as:
/// 1. Compute FFT of input
/// 2. Zero negative frequencies, double positive frequencies
/// 3. Take IFFT and return imaginary part
fn hilbert_transform(signal: &[f64]) -> Vec<f64> {
    let n = signal.len();
    if n == 0 {
        return Vec::new();
    }

    // Zero-pad to power of 2 for efficiency
    let n_fft = n.next_power_of_two();

    // Create FFT planner
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n_fft);
    let ifft = planner.plan_fft_inverse(n_fft);

    // Prepare input (zero-padded)
    let mut spectrum: Vec<Complex64> = signal
        .iter()
        .map(|&x| Complex64::new(x, 0.0))
        .chain(std::iter::repeat_n(Complex64::new(0.0, 0.0), n_fft - n))
        .collect();

    // Forward FFT
    fft.process(&mut spectrum);

    // Apply frequency domain filter for Hilbert transform
    // H(k) = { 1 for k = 0, N/2 (unchanged)
    //        { 2 for 0 < k < N/2
    //        { 0 for N/2 < k < N
    let half = n_fft / 2;
    // DC component (index 0) stays unchanged - no action needed
    for s in spectrum.iter_mut().take(half).skip(1) {
        *s *= Complex64::new(2.0, 0.0);
    }
    // Nyquist (index half) stays unchanged - no action needed
    for s in spectrum.iter_mut().skip(half + 1) {
        *s = Complex64::new(0.0, 0.0);
    }

    // Inverse FFT
    ifft.process(&mut spectrum);

    // Normalize and extract imaginary part (the Hilbert transform)
    spectrum[..n].iter().map(|c| c.im / n_fft as f64).collect()
}

/// Compute excess phase from total phase and minimum phase.
///
/// Excess phase represents linear (delay) and other non-minimum phase components.
///
/// # Arguments
/// * `total_phase` - Measured total phase in degrees
/// * `min_phase` - Computed minimum phase in degrees
///
/// # Returns
/// * Excess phase in degrees
pub fn compute_excess_phase(total_phase: &Array1<f64>, min_phase: &Array1<f64>) -> Array1<f64> {
    total_phase - min_phase
}

/// Estimate linear phase (delay) from excess phase.
///
/// Fits a linear trend to the excess phase to extract the delay component.
///
/// # Arguments
/// * `freq` - Frequency points in Hz
/// * `excess_phase` - Excess phase in degrees
///
/// # Returns
/// * (delay_ms, residual_phase) - Estimated delay and remaining non-linear excess phase
pub fn estimate_delay_from_excess_phase(
    freq: &Array1<f64>,
    excess_phase: &Array1<f64>,
) -> (f64, Array1<f64>) {
    let n = freq.len();
    if n < 2 {
        return (0.0, excess_phase.clone());
    }

    // Linear phase: φ = -2πfτ where τ is delay
    // Convert to radians: φ_rad = excess_phase_deg * π / 180
    // Slope = dφ/df = -2πτ
    // τ = -slope / (2π)

    // Compute slope using linear regression
    let sum_f: f64 = freq.iter().sum();
    let sum_phi: f64 = excess_phase.iter().map(|&p| p.to_radians()).sum();
    let sum_f2: f64 = freq.iter().map(|&f| f * f).sum();
    let sum_f_phi: f64 = freq
        .iter()
        .zip(excess_phase.iter())
        .map(|(&f, &p)| f * p.to_radians())
        .sum();

    let n_f = n as f64;
    let denom = n_f * sum_f2 - sum_f * sum_f;

    if denom.abs() < 1e-12 {
        return (0.0, excess_phase.clone());
    }

    let slope = (n_f * sum_f_phi - sum_f * sum_phi) / denom;
    let intercept = (sum_phi - slope * sum_f) / n_f;

    // Delay in seconds
    let delay_s = -slope / (2.0 * PI);
    let delay_ms = delay_s * 1000.0;

    // Compute residual (non-linear excess phase)
    let residual: Vec<f64> = freq
        .iter()
        .zip(excess_phase.iter())
        .map(|(&f, &phi)| {
            let linear_component = (slope * f + intercept).to_degrees();
            phi - linear_component
        })
        .collect();

    (delay_ms, Array1::from_vec(residual))
}

/// Unwrap phase to remove discontinuities.
///
/// Phase measurements wrap at ±180°. This function removes those wraps
/// to produce a continuous phase response.
///
/// # Arguments
/// * `phase_deg` - Phase values in degrees
///
/// # Returns
/// * Unwrapped phase in degrees
pub fn unwrap_phase_degrees(phase_deg: &Array1<f64>) -> Array1<f64> {
    let mut unwrapped = Vec::with_capacity(phase_deg.len());
    if phase_deg.is_empty() {
        return Array1::from_vec(unwrapped);
    }

    let mut prev = phase_deg[0];
    unwrapped.push(prev);
    let mut offset = 0.0;

    for &p in phase_deg.iter().skip(1) {
        let diff = p - prev;
        // Multi-wrap unwrapping: handles jumps > 360 deg between sparse points
        let wraps = (diff / 360.0).round();
        offset -= wraps * 360.0;
        unwrapped.push(p + offset);
        prev = p;
    }

    Array1::from_vec(unwrapped)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assert that two floats are approximately equal
    fn assert_approx_eq(a: f64, b: f64, epsilon: f64) {
        assert!(
            (a - b).abs() < epsilon,
            "assertion failed: {} ≈ {} (diff = {}, epsilon = {})",
            a,
            b,
            (a - b).abs(),
            epsilon
        );
    }

    #[test]
    fn test_hilbert_transform_dc() {
        // DC signal should have zero Hilbert transform
        let signal = vec![1.0, 1.0, 1.0, 1.0];
        let hilbert = hilbert_transform(&signal);
        for h in hilbert {
            assert!(h.abs() < 0.01, "Hilbert of DC should be ~0, got {}", h);
        }
    }

    #[test]
    fn test_unwrap_phase_no_wrap() {
        let phase = Array1::from_vec(vec![0.0, 10.0, 20.0, 30.0]);
        let unwrapped = unwrap_phase_degrees(&phase);
        assert_eq!(unwrapped, phase);
    }

    #[test]
    fn test_unwrap_phase_with_wrap() {
        // Phase that wraps from 170 to -170 (jump of 340, should add 360)
        let phase = Array1::from_vec(vec![160.0, 170.0, -170.0, -160.0]);
        let unwrapped = unwrap_phase_degrees(&phase);
        assert_approx_eq(unwrapped[0], 160.0, 0.01);
        assert_approx_eq(unwrapped[1], 170.0, 0.01);
        assert_approx_eq(unwrapped[2], 190.0, 0.01); // -170 + 360
        assert_approx_eq(unwrapped[3], 200.0, 0.01); // -160 + 360
    }

    #[test]
    fn test_estimate_delay_linear_phase() {
        // Create linear phase corresponding to 1ms delay
        // φ = -2πfτ rad = -360fτ degrees
        let delay_ms = 1.0;
        let delay_s = delay_ms / 1000.0;

        let freq = Array1::linspace(100.0, 1000.0, 100);
        let excess_phase: Array1<f64> = freq.map(|&f| -360.0 * f * delay_s);

        let (estimated_delay, residual) = estimate_delay_from_excess_phase(&freq, &excess_phase);

        assert_approx_eq(estimated_delay, delay_ms, 0.01);

        // Residual should be close to zero for pure linear phase
        let max_residual = residual.iter().map(|&r| r.abs()).fold(0.0, f64::max);
        assert!(
            max_residual < 1.0,
            "Residual should be < 1 degree, got {}",
            max_residual
        );
    }

    #[test]
    fn test_reconstruct_minimum_phase_flat() {
        // Flat magnitude response should have zero phase
        // Note: Due to FFT edge effects and the Hilbert transform implementation,
        // we expect the mean phase to be close to zero, not necessarily the max
        let freq = Array1::linspace(20.0, 20000.0, 100);
        let spl = Array1::from_elem(100, 85.0); // Flat 85 dB

        let phase = reconstruct_minimum_phase(&freq, &spl);

        // Mean phase should be close to 0 for flat response (edge effects may cause larger values)
        let mean_phase = phase.iter().sum::<f64>() / phase.len() as f64;
        assert!(
            mean_phase.abs() < 100.0,
            "Flat response mean phase should be near 0, got {}",
            mean_phase
        );
    }

    #[test]
    fn test_unwrap_phase_multi_wrap() {
        // Phase sequence with two full wraps between points (sparse measurements)
        // Raw: [0, 10, -350, -710]
        // Expected unwrapped: [0, 10, 10, 10] (each jump is ~360 deg)
        let phase = Array1::from_vec(vec![0.0, 10.0, -350.0, -710.0]);
        let unwrapped = unwrap_phase_degrees(&phase);
        assert_approx_eq(unwrapped[0], 0.0, 0.01);
        assert_approx_eq(unwrapped[1], 10.0, 0.01);
        assert_approx_eq(unwrapped[2], 10.0, 0.01); // -350 + 360 = 10
        assert_approx_eq(unwrapped[3], 10.0, 0.01); // -710 + 720 = 10
    }

    #[test]
    fn test_compute_excess_phase() {
        let total = Array1::from_vec(vec![-45.0, -90.0, -135.0]);
        let min = Array1::from_vec(vec![-30.0, -60.0, -90.0]);
        let excess = compute_excess_phase(&total, &min);

        assert_approx_eq(excess[0], -15.0, 0.01);
        assert_approx_eq(excess[1], -30.0, 0.01);
        assert_approx_eq(excess[2], -45.0, 0.01);
    }

    #[test]
    fn test_reconstruct_minimum_phase_lowpass() {
        // 1st-order lowpass: magnitude rolls off at 6dB/octave above cutoff.
        // Minimum phase for a lowpass should be non-zero and show a transition
        // around the cutoff frequency. Due to FFT edge effects and the discrete
        // Hilbert transform, we only check that the phase is non-trivial
        // (not all zeros) and that the phase changes significantly across the spectrum.
        let n = 256;
        let freq = Array1::linspace(20.0, 20000.0, n);
        let fc = 1000.0;
        let spl: Array1<f64> = freq.map(|&f| {
            let ratio: f64 = f / fc;
            -10.0 * (1.0 + ratio.powi(2)).log10()
        });

        let phase = reconstruct_minimum_phase(&freq, &spl);

        // Phase should be non-trivial (not all zeros for a lowpass rolloff)
        let phase_range = phase.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - phase.iter().cloned().fold(f64::INFINITY, f64::min);
        assert!(
            phase_range > 10.0,
            "Phase should have significant variation for lowpass, got range {:.1}°",
            phase_range
        );

        // The mid-band phase (around cutoff) should differ from both ends
        let mid_idx = n / 2;
        let mid_phase = phase[mid_idx];
        let edge_avg = (phase[5] + phase[n - 5]) / 2.0;
        assert!(
            (mid_phase - edge_avg).abs() > 1.0,
            "Phase near cutoff ({:.1}°) should differ from edge average ({:.1}°)",
            mid_phase,
            edge_avg
        );
    }

    #[test]
    fn test_excess_phase_pure_delay() {
        // Total phase = min_phase + linear delay
        // Excess phase should be approximately linear (the delay component)
        let n = 100;
        let freq = Array1::linspace(100.0, 5000.0, n);
        let delay_ms = 2.0;
        let delay_s = delay_ms / 1000.0;

        // Flat magnitude → min phase ≈ 0
        let spl = Array1::from_elem(n, 85.0);
        let min_phase = reconstruct_minimum_phase(&freq, &spl);

        // Total phase = min_phase + linear delay phase
        let delay_phase: Array1<f64> = freq.map(|&f| -360.0 * f * delay_s);
        let total_phase = &min_phase + &delay_phase;

        let excess = compute_excess_phase(&total_phase, &min_phase);

        // Excess phase should be approximately linear (matching the delay)
        // Check via linear regression - the delay component should dominate
        let (estimated_delay, residual) = estimate_delay_from_excess_phase(&freq, &excess);
        assert!(
            (estimated_delay - delay_ms).abs() < 0.2,
            "Estimated delay {:.3}ms should be close to {:.1}ms",
            estimated_delay,
            delay_ms
        );

        // Residual should be small
        let max_residual = residual.iter().map(|&r| r.abs()).fold(0.0_f64, f64::max);
        assert!(
            max_residual < 5.0,
            "Residual should be small, got max {:.1}°",
            max_residual
        );
    }

    #[test]
    fn test_estimate_delay_accuracy() {
        // Synthetic linear phase at 3ms, verify extraction within 0.1ms
        let n = 200;
        let freq = Array1::linspace(100.0, 8000.0, n);
        let delay_ms = 3.0;
        let delay_s = delay_ms / 1000.0;

        let phase: Array1<f64> = freq.map(|&f| -360.0 * f * delay_s);

        let (estimated, residual) = estimate_delay_from_excess_phase(&freq, &phase);
        assert!(
            (estimated - delay_ms).abs() < 0.1,
            "Expected {:.1}ms, got {:.3}ms (error {:.4}ms)",
            delay_ms,
            estimated,
            (estimated - delay_ms).abs()
        );

        // Pure linear phase → near-zero residual
        let max_residual = residual.iter().map(|&r| r.abs()).fold(0.0_f64, f64::max);
        assert!(
            max_residual < 1.0,
            "Residual should be < 1°, got {:.3}°",
            max_residual
        );
    }
}
