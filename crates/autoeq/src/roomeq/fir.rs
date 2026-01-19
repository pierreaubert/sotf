//! FIR filter optimization for room correction

use crate::Curve;
use crate::fir::{FirPhase, generate_fir_from_response};
use ndarray::Array1;
use num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::collections::HashMap;
use std::error::Error;
use std::f64::consts::PI;
use std::sync::{Arc, Mutex, OnceLock};

/// Global FFT planner cache for reuse across calls
/// The planner caches plans internally, so reusing it improves performance
static FFT_PLANNER: OnceLock<Mutex<FftPlanner<f64>>> = OnceLock::new();

/// Type alias for the FFT cache to reduce complexity
type FftCache = HashMap<usize, Arc<dyn Fft<f64>>>;

/// Cache for planned FFTs by size
static FFT_CACHE: OnceLock<Mutex<FftCache>> = OnceLock::new();

/// Get or create an inverse FFT plan for the given size
fn get_inverse_fft(fft_len: usize) -> Arc<dyn Fft<f64>> {
    let cache = FFT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache_guard = cache.lock().unwrap();

    if let Some(fft) = cache_guard.get(&fft_len) {
        return Arc::clone(fft);
    }

    let planner = FFT_PLANNER.get_or_init(|| Mutex::new(FftPlanner::new()));
    let mut planner_guard = planner.lock().unwrap();
    let fft = planner_guard.plan_fft_inverse(fft_len);
    cache_guard.insert(fft_len, Arc::clone(&fft));
    fft
}

use super::types::{OptimizerConfig, TargetCurveConfig};

/// Generate an FIR correction filter for a single channel
pub fn generate_fir_correction(
    measurement: &Curve,
    config: &OptimizerConfig,
    target_config: Option<&TargetCurveConfig>,
    sample_rate: f64,
) -> Result<Vec<f64>, Box<dyn Error>> {
    // 1. Determine Target Curve
    let target_curve = match target_config {
        Some(TargetCurveConfig::Path(path)) => {
            let target = crate::read::read_curve_from_csv(path)?;
            crate::read::normalize_and_interpolate_response(&measurement.freq, &target)
        }
        Some(TargetCurveConfig::Predefined(name)) => {
            use crate::cli::Args;
            use clap::Parser;
            let dummy_args = Args::parse_from(["autoeq", "--curve-name", name]);
            crate::workflow::build_target_curve(&dummy_args, &measurement.freq, measurement)?
        }
        None => Curve {
            freq: measurement.freq.clone(),
            spl: Array1::zeros(measurement.freq.len()),
            phase: None,
        },
    };

    let fir_config = config.fir.as_ref().ok_or("FIR configuration missing")?;
    let n_taps = fir_config.taps;

    if fir_config.phase.to_lowercase() == "kirkeby" {
        generate_kirkeby_correction(
            measurement,
            &target_curve,
            sample_rate,
            n_taps,
            config.min_freq,
            config.max_freq,
        )
    } else {
        // Standard magnitude-based generation
        let correction_spl = &target_curve.spl - &measurement.spl;
        let correction_curve = Curve {
            freq: measurement.freq.clone(),
            spl: correction_spl,
            phase: None,
        };

        let phase_type = match fir_config.phase.to_lowercase().as_str() {
            "linear" => FirPhase::Linear,
            "minimum" => FirPhase::Minimum,
            _ => return Err(format!("Unknown FIR phase type: {}", fir_config.phase).into()),
        };

        let coeffs = generate_fir_from_response(&correction_curve, sample_rate, n_taps, phase_type);
        Ok(coeffs)
    }
}

/// Generate mixed-phase inversion using Kirkeby regularization
fn generate_kirkeby_correction(
    measurement: &Curve,
    target: &Curve,
    sample_rate: f64,
    n_taps: usize,
    min_freq: f64,
    max_freq: f64,
) -> Result<Vec<f64>, Box<dyn Error>> {
    // FFT size - use next power of 2 above n_taps, but at least 65536 for good low freq resolution
    let fft_len = (n_taps * 4).max(65536).next_power_of_two();
    let num_bins = fft_len / 2 + 1;
    let freq_step = sample_rate / fft_len as f64;

    // Linear frequency grid
    let mut freqs = Vec::with_capacity(num_bins);
    for i in 0..num_bins {
        freqs.push(i as f64 * freq_step);
    }
    let freqs_arr = Array1::from(freqs);

    // Interpolate measurement and target to linear grid
    // Note: We interpolate magnitude (dB) and phase separately
    let meas_interp = crate::read::interpolate_log_space(&freqs_arr, measurement);
    let target_interp = crate::read::interpolate_log_space(&freqs_arr, target);

    // Regularization parameters
    // In-band regularization: keeps inversion stable (e.g. don't boost nulls infinitely)
    let in_band_reg = 10.0_f64.powf(-30.0 / 10.0); // -30dB regularization relative to peak
    // Out-of-band regularization: prevents boosting noise (high regularization)
    let out_band_reg = 1.0; // 0dB (don't boost)

    let mut h_inv = Vec::with_capacity(num_bins);

    for i in 0..num_bins {
        let f = freqs_arr[i];

        // 1. Reconstruct Measurement H(f)
        let m_spl = meas_interp.spl[i];
        // Use phase if available, else assume minimum phase or 0 (Kirkeby works best with phase)
        // If measurement has no phase, we really should generate MP phase first, but 0 is a fallback.
        // Usually room measurements have phase.
        let m_phase_deg = meas_interp.phase.as_ref().map(|p| p[i]).unwrap_or(0.0);

        let m_mag = 10.0_f64.powf(m_spl / 20.0);
        let m_phase_rad = m_phase_deg.to_radians();
        let h = Complex::from_polar(m_mag, m_phase_rad);

        // 2. Reconstruct Target T(f)
        let t_spl = target_interp.spl[i];
        // Target usually 0 phase (linear phase target) or min phase
        // For Kirkeby, we usually want the target to be the magnitude response we aim for, phase 0.
        // The resulting filter will have the phase required to match T from H.
        let t_mag = 10.0_f64.powf(t_spl / 20.0);
        let t = Complex::new(t_mag, 0.0);

        // 3. Compute Regularization e(f)
        // Transition width for regularization
        let width = 10.0; // Hz
        let transition = if f < min_freq {
            // Below min_freq: transition from out to in
            // f going 0 -> min_freq means val going 0 -> 1
            ((f - (min_freq - width)) / width).clamp(0.0, 1.0)
        } else if f > max_freq {
            // Above max_freq: transition from in to out
            // f going max_freq -> inf means val going 1 -> 0
            1.0 - ((f - max_freq) / width).clamp(0.0, 1.0)
        } else {
            1.0
        };

        // Linear interpolation of log regularization?
        // Simple blend:
        let epsilon = out_band_reg + (in_band_reg - out_band_reg) * transition;

        // 4. Compute Inverse: C = (H* . T) / (|H|^2 + epsilon)
        // This is the regularized least squares solution
        let numerator = h.conj() * t;
        let denominator = h.norm_sqr() + epsilon;

        // Avoid div by zero (epsilon ensures this, but safety check)
        let c = if denominator > 1e-12 {
            numerator / denominator
        } else {
            Complex::new(0.0, 0.0)
        };

        h_inv.push(c);
    }

    // 5. IFFT to get impulse response
    // Construct full spectrum (positive + negative freq)
    let mut spectrum = vec![Complex::new(0.0, 0.0); fft_len];

    // DC and Nyquist
    spectrum[0] = h_inv[0];
    spectrum[fft_len / 2] = h_inv[num_bins - 1]; // Nyquist is real for real signal

    // Fill positive freqs
    for i in 1..fft_len / 2 {
        spectrum[i] = h_inv[i];
        // Conjugate symmetry for real IFFT
        spectrum[fft_len - i] = h_inv[i].conj();
    }

    // Perform IFFT using cached planner for better performance
    let fft = get_inverse_fft(fft_len);
    fft.process(&mut spectrum);

    // Normalize
    let mut impulse: Vec<f64> = spectrum.iter().map(|c| c.re / fft_len as f64).collect();

    // 6. Cyclic Shift (Center the impulse)
    // Kirkeby inversion usually results in a non-causal filter (peak at t=0).
    // We need to shift it to the center of our window.
    // However, the FFT result wraps around. The peak is likely at index 0.
    // We want to shift index 0 to index n_taps/2.
    // But we are working with fft_len >> n_taps.

    // Find the peak to verify (it should be near 0 if minimum phase, but mixed phase might be elsewhere)
    // Actually, regularized inversion of mixed phase room response tends to put the main spike near 0 (acausal correction)
    // or distributed.
    // We generally perform a cyclic shift of fft_len/2 to center it in the FFT buffer.

    let shift = fft_len / 2;
    impulse.rotate_right(shift);

    // Now the "main energy" should be around `shift`.
    // We want to extract `n_taps` centered around `shift`.
    let start_idx = shift - n_taps / 2;

    // Apply windowing to the extracted segment to smooth edges
    let window = hann_window(n_taps);
    let mut coeffs = vec![0.0; n_taps];
    for (i, coeff) in coeffs.iter_mut().enumerate() {
        let src_idx = start_idx + i;
        if src_idx < impulse.len() {
            *coeff = impulse[src_idx] * window[i];
        }
    }

    Ok(coeffs)
}

/// FIR window function type
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[allow(dead_code)]
pub enum WindowType {
    /// Rectangular window (no windowing)
    Rectangular,
    /// Hann window - good general purpose
    #[default]
    Hann,
    /// Hamming window - slightly less sidelobe than Hann
    Hamming,
    /// Blackman window - even lower sidelobes
    Blackman,
    /// Kaiser window with parameter beta
    Kaiser(f64),
}

/// Generate a rectangular window (no windowing)
#[allow(dead_code)]
pub fn rectangular_window(n: usize) -> Vec<f64> {
    vec![1.0; n]
}

/// Generate a Hann window
pub fn hann_window(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f64 / (n - 1) as f64).cos()))
        .collect()
}

/// Generate a Hamming window
#[allow(dead_code)]
pub fn hamming_window(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| 0.54 - 0.46 * (2.0 * PI * i as f64 / (n - 1) as f64).cos())
        .collect()
}

/// Generate a Blackman window
#[allow(dead_code)]
pub fn blackman_window(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            let x = 2.0 * PI * i as f64 / (n - 1) as f64;
            0.42 - 0.5 * x.cos() + 0.08 * (2.0 * x).cos()
        })
        .collect()
}

/// Generate a Kaiser window
///
/// # Arguments
/// * `n` - Window length
/// * `beta` - Shape parameter (higher = narrower main lobe, higher sidelobes)
///   - beta = 0: rectangular window
///   - beta ≈ 5.0: similar to Hamming
///   - beta ≈ 6.0: similar to Hann
///   - beta ≈ 8.6: similar to Blackman
#[allow(dead_code)]
pub fn kaiser_window(n: usize, beta: f64) -> Vec<f64> {
    let denom = bessel_i0(beta);
    (0..n)
        .map(|i| {
            let x = 2.0 * i as f64 / (n - 1) as f64 - 1.0; // -1 to +1
            let arg = beta * (1.0 - x * x).max(0.0).sqrt();
            bessel_i0(arg) / denom
        })
        .collect()
}

/// Modified Bessel function of the first kind, order 0
/// Approximation using series expansion
#[allow(dead_code)]
fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let x_sq_4 = x * x / 4.0;

    for k in 1..50 {
        term *= x_sq_4 / (k * k) as f64;
        sum += term;
        if term.abs() < 1e-15 * sum.abs() {
            break;
        }
    }

    sum
}

/// Generate a window of the specified type
#[allow(dead_code)]
pub fn generate_window(window_type: WindowType, n: usize) -> Vec<f64> {
    match window_type {
        WindowType::Rectangular => rectangular_window(n),
        WindowType::Hann => hann_window(n),
        WindowType::Hamming => hamming_window(n),
        WindowType::Blackman => blackman_window(n),
        WindowType::Kaiser(beta) => kaiser_window(n, beta),
    }
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
    fn test_hann_window_symmetry() {
        let window = hann_window(8);
        assert_approx_eq(window[0], window[7], 0.01);
        assert_approx_eq(window[1], window[6], 0.01);
        assert_approx_eq(window[2], window[5], 0.01);
        assert_approx_eq(window[3], window[4], 0.01);
    }

    #[test]
    fn test_hann_window_endpoints() {
        let window = hann_window(128);
        // Hann should be 0 at endpoints
        assert!(window[0] < 0.01);
        assert!(window[127] < 0.01);
        // Maximum should be at center
        assert!(window[64] > 0.99);
    }

    #[test]
    fn test_hamming_window_endpoints() {
        let window = hamming_window(128);
        // Hamming has non-zero endpoints (~0.08)
        assert!(window[0] > 0.07 && window[0] < 0.09);
        // Maximum at center
        assert!(window[64] > 0.99);
    }

    #[test]
    fn test_blackman_window_endpoints() {
        let window = blackman_window(128);
        // Blackman should be very close to 0 at endpoints
        assert!(window[0] < 0.01);
        // Maximum at center
        assert!(window[64] > 0.99);
    }

    #[test]
    fn test_kaiser_window_beta_0() {
        // beta = 0 should give rectangular window
        let window = kaiser_window(8, 0.0);
        for w in window {
            assert_approx_eq(w, 1.0, 0.01);
        }
    }

    #[test]
    fn test_rectangular_window() {
        let window = rectangular_window(10);
        assert_eq!(window.len(), 10);
        for w in window {
            assert_eq!(w, 1.0);
        }
    }

    #[test]
    fn test_bessel_i0() {
        // Known values
        assert_approx_eq(bessel_i0(0.0), 1.0, 1e-10);
        assert_approx_eq(bessel_i0(1.0), 1.266065877752, 1e-6);
        assert_approx_eq(bessel_i0(2.0), 2.279585302336, 1e-6);
    }

    #[test]
    fn test_generate_window() {
        let hann = generate_window(WindowType::Hann, 64);
        let hamming = generate_window(WindowType::Hamming, 64);

        assert_eq!(hann.len(), 64);
        assert_eq!(hamming.len(), 64);

        // Hann should have lower sidelobes (higher rolloff)
        // This is a property test - Hann endpoints should be lower
        assert!(hann[0] < hamming[0]);
    }
}
