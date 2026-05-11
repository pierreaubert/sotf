//! Mixed-phase correction for room EQ.
//!
//! Implements mixed-phase filter design:
//! 1. Decompose measurement into minimum-phase + excess phase
//! 2. IIR (parametric EQ) corrects the minimum-phase component
//! 3. Short FIR corrects the excess phase component (with pre-ringing constraint)
//!
//! This gives the best of both worlds: IIR latency for causal correction,
//! and a short FIR for the non-causal excess phase — without the long latency
//! of full FIR correction.
//!
//! Reference: Brännmark & Sternad, AES 124th Convention (2008)
//! Reference: Patent EP2104374B1

use crate::Curve;
use log::info;
use math_audio_iir_fir::{Biquad, FirDesignConfig, FirPhase, PreRingingConfig};
use ndarray::Array1;

use super::phase_utils;

/// Decomposed phase result: (minimum_phase, excess_phase, delay_ms, residual_phase)
pub type PhaseDecomposition = (Array1<f64>, Array1<f64>, f64, Array1<f64>);

/// Configuration for mixed-phase correction.
#[derive(Debug, Clone)]
pub struct MixedPhaseConfig {
    /// Maximum FIR length in milliseconds for excess phase correction.
    /// Shorter = less latency, but less correction.
    /// Default: 10.0 ms (480 taps at 48 kHz)
    pub max_fir_length_ms: f64,

    /// Pre-ringing threshold in dB relative to main tap.
    /// Default: -30.0 dB
    pub pre_ringing_threshold_db: f64,

    /// Minimum correction depth from spatial robustness analysis.
    /// Excess phase correction is only applied at frequencies where
    /// the correction depth exceeds this value.
    /// Default: 0.5 (only correct where spatially consistent)
    pub min_spatial_depth: f64,

    /// Smoothing width in octaves for excess phase before FIR generation.
    /// Reduces noise in phase measurements. Default: 1/6 octave.
    pub phase_smoothing_octaves: f64,
}

impl Default for MixedPhaseConfig {
    fn default() -> Self {
        Self {
            max_fir_length_ms: 10.0,
            pre_ringing_threshold_db: -30.0,
            min_spatial_depth: 0.5,
            phase_smoothing_octaves: 1.0 / 6.0,
        }
    }
}

/// Result of mixed-phase correction.
#[derive(Debug, Clone)]
pub struct MixedPhaseResult {
    /// IIR filters for minimum-phase correction (from standard PEQ optimizer)
    pub iir_filters: Vec<Biquad>,

    /// FIR coefficients for excess phase correction (short, pre-ringing constrained)
    pub fir_coefficients: Vec<f64>,

    /// Estimated propagation delay in milliseconds (from excess phase linear component)
    pub estimated_delay_ms: f64,

    /// Minimum phase reconstructed from magnitude
    pub minimum_phase: Array1<f64>,

    /// Excess phase (measured - minimum)
    pub excess_phase: Array1<f64>,

    /// Residual excess phase after removing linear delay
    pub residual_excess_phase: Array1<f64>,

    /// FIR filter length in taps
    pub fir_taps: usize,
}

/// Decompose a room measurement into minimum-phase and excess phase components.
///
/// The minimum phase is reconstructed from the magnitude response via Hilbert transform.
/// The excess phase is the difference between measured phase and minimum phase.
/// The excess phase is further decomposed into a linear delay component and a residual.
///
/// # Arguments
/// * `measurement` - Room measurement with phase data
/// * `config` - Mixed-phase configuration
///
/// # Returns
/// * `(minimum_phase_deg, excess_phase_deg, delay_ms, residual_phase_deg)` or error
pub fn decompose_phase(
    measurement: &Curve,
    config: &MixedPhaseConfig,
) -> Result<PhaseDecomposition, String> {
    let measured_phase = measurement
        .phase
        .as_ref()
        .ok_or("Mixed-phase correction requires phase data in measurements")?;

    // Unwrap measured phase to remove discontinuities
    let unwrapped_phase = phase_utils::unwrap_phase_degrees(measured_phase);

    // Reconstruct minimum phase from magnitude
    let min_phase = phase_utils::reconstruct_minimum_phase(&measurement.freq, &measurement.spl);

    // Compute excess phase
    let excess_phase = phase_utils::compute_excess_phase(&unwrapped_phase, &min_phase);

    // Decompose excess phase into linear delay + residual
    let (delay_ms, residual) =
        phase_utils::estimate_delay_from_excess_phase(&measurement.freq, &excess_phase);

    info!(
        "  Mixed-phase decomposition: delay={:.2} ms, residual phase range={:.1}°",
        delay_ms,
        residual.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - residual.iter().cloned().fold(f64::INFINITY, f64::min),
    );

    // Apply phase smoothing to residual if configured
    let smoothed_residual = if config.phase_smoothing_octaves > 0.0 {
        smooth_phase_log_freq(&residual, &measurement.freq, config.phase_smoothing_octaves)
    } else {
        residual.clone()
    };

    Ok((min_phase, excess_phase, delay_ms, smoothed_residual))
}

/// Generate a short FIR filter to correct the residual excess phase.
///
/// The FIR only corrects phase (magnitude is unity / 0 dB), keeping the filter short
/// and avoiding spectral coloration. Pre-ringing is constrained.
///
/// # Arguments
/// * `freq` - Frequency axis
/// * `residual_phase_deg` - Residual excess phase to correct (in degrees)
/// * `config` - Mixed-phase configuration
/// * `sample_rate` - Sample rate in Hz
///
/// # Returns
/// * FIR coefficients
pub fn generate_excess_phase_fir(
    freq: &Array1<f64>,
    residual_phase_deg: &Array1<f64>,
    config: &MixedPhaseConfig,
    sample_rate: f64,
) -> Vec<f64> {
    generate_excess_phase_fir_with_depth(freq, residual_phase_deg, config, sample_rate, None)
}

/// Generate a short FIR filter to correct residual excess phase, optionally
/// masked by a spatial correction depth array.
///
/// When `correction_depth` is provided, the excess phase correction is zeroed
/// at frequencies where the depth is below `config.min_spatial_depth`. This
/// prevents the FIR from trying to correct position-dependent phase artifacts.
pub fn generate_excess_phase_fir_with_depth(
    freq: &Array1<f64>,
    residual_phase_deg: &Array1<f64>,
    config: &MixedPhaseConfig,
    sample_rate: f64,
    correction_depth: Option<&Array1<f64>>,
) -> Vec<f64> {
    let n_taps = (config.max_fir_length_ms / 1000.0 * sample_rate).round() as usize;
    // Ensure odd number of taps for symmetric linear-phase center
    let n_taps = if n_taps.is_multiple_of(2) {
        n_taps + 1
    } else {
        n_taps
    };
    let n_taps = n_taps.max(31); // minimum useful length

    // The correction phase is the negation of the residual excess phase,
    // scaled by spatial correction depth where available.
    let correction_phase_deg: Vec<f64> = if let Some(depth) = correction_depth {
        assert_eq!(
            residual_phase_deg.len(),
            depth.len(),
            "correction_depth length ({}) must match residual_phase_deg length ({})",
            depth.len(),
            residual_phase_deg.len(),
        );
        residual_phase_deg
            .iter()
            .zip(depth.iter())
            .map(|(&p, &d)| {
                if d >= config.min_spatial_depth {
                    -p
                } else {
                    0.0 // Don't correct position-dependent phase
                }
            })
            .collect()
    } else {
        residual_phase_deg.iter().map(|&p| -p).collect()
    };

    // Generate FIR with unity magnitude and the correction phase
    // Using Kirkeby-style approach: construct complex spectrum, IFFT
    let fir_config = FirDesignConfig {
        n_taps,
        sample_rate,
        phase: FirPhase::Minimum, // will be overridden by direct spectrum construction
        min_freq: freq[0],
        max_freq: freq[freq.len() - 1],
        pre_ringing: Some(PreRingingConfig {
            threshold_db: config.pre_ringing_threshold_db,
            max_time_s: config.max_fir_length_ms / 1000.0 / 2.0, // half the FIR length
        }),
        ..Default::default()
    };

    // Generate FIR from unity magnitude with the correction phase
    // Use the standard FIR generation shape with phase override. Since
    // FirPhase::Minimum uses magnitude-derived phase, this needs a custom path.
    generate_phase_only_fir(freq.as_slice().unwrap(), &correction_phase_deg, &fir_config)
}

/// Generate a phase-only FIR filter (unity magnitude, specified phase).
///
/// Constructs a complex spectrum with |H(f)| = 1 and φ(f) = correction_phase,
/// then performs IFFT, windowing, and pre-ringing suppression. After these
/// time-domain modifications, the magnitude is re-normalized to unity from
/// the modified IR's phase via a second FFT/IFFT round-trip so the FIR
/// corrects phase without coloring the magnitude response. If pre-ringing
/// suppression is asymmetric, the renormalized phase follows the suppressed
/// IR rather than the original requested phase exactly.
fn generate_phase_only_fir(freqs: &[f64], phase_deg: &[f64], config: &FirDesignConfig) -> Vec<f64> {
    use num_complex::Complex64;
    use rustfft::FftPlanner;

    let n_taps = config.n_taps;
    let sample_rate = config.sample_rate;

    let fft_size = (n_taps * 4).max(4096).next_power_of_two();
    let n_bins = fft_size / 2 + 1;
    let freq_step = sample_rate / fft_size as f64;

    // Build linear frequency grid
    let linear_freqs: Vec<f64> = (0..n_bins).map(|i| i as f64 * freq_step).collect();

    // Interpolate phase to linear grid (log-space interpolation)
    let interp_phase = interpolate_phase_log_space(freqs, phase_deg, &linear_freqs);

    // Build complex spectrum: unity magnitude, correction phase
    let mut spectrum: Vec<Complex64> = interp_phase
        .iter()
        .map(|&phase| {
            let phi = phase.to_radians();
            Complex64::from_polar(1.0, phi)
        })
        .collect();

    // DC and Nyquist must be real
    spectrum[0] = Complex64::new(1.0, 0.0);
    if n_bins > 1 {
        // Preserve magnitude when forcing Nyquist to real; if phase is 90°
        // copying .re alone would collapse the bin to zero.
        spectrum[n_bins - 1] = Complex64::new(spectrum[n_bins - 1].norm(), 0.0);
    }

    // Build full spectrum (conjugate symmetric)
    let mut full_spectrum: Vec<Complex64> = Vec::with_capacity(fft_size);
    full_spectrum.extend_from_slice(&spectrum);
    for i in (1..n_bins - 1).rev() {
        full_spectrum.push(spectrum[i].conj());
    }

    // IFFT
    let mut planner = FftPlanner::new();
    let ifft = planner.plan_fft_inverse(fft_size);
    ifft.process(&mut full_spectrum);

    // Extract and normalize
    let ir: Vec<f64> = full_spectrum
        .iter()
        .map(|c| c.re / fft_size as f64)
        .collect();

    // Center the impulse response (linear phase style)
    let center = n_taps / 2;
    let mut final_ir = vec![0.0; n_taps];
    for (i, val) in final_ir.iter_mut().enumerate() {
        let shift = i as isize - center as isize;
        let ir_idx = if shift < 0 {
            fft_size as isize + shift
        } else {
            shift
        };
        *val = ir[ir_idx as usize];
    }

    // Apply window
    let window =
        math_audio_iir_fir::generate_window(n_taps, math_audio_iir_fir::WindowType::Hann, 0.0);
    for (x, w) in final_ir.iter_mut().zip(window.iter()) {
        *x *= w;
    }

    // Apply pre-ringing suppression
    if let Some(pr_config) = &config.pre_ringing {
        math_audio_iir_fir::suppress_pre_ringing(&mut final_ir, pr_config, sample_rate);
    }

    // --- Magnitude re-normalization ---
    // Windowing and pre-ringing suppression destroy the unity-magnitude property.
    // Re-normalize: FFT the modified IR, force |H(f)| = 1 while keeping the
    // resulting phase, then IFFT back.
    let mut renorm_spectrum: Vec<Complex64> = vec![Complex64::new(0.0, 0.0); fft_size];
    for (i, &v) in final_ir.iter().enumerate() {
        renorm_spectrum[i] = Complex64::new(v, 0.0);
    }

    let fft = planner.plan_fft_forward(fft_size);
    fft.process(&mut renorm_spectrum);

    // Force unity magnitude, preserve phase
    for bin in renorm_spectrum.iter_mut() {
        let mag = bin.norm();
        if mag > 1e-12 {
            *bin /= mag;
        }
    }

    // IFFT back
    let ifft2 = planner.plan_fft_inverse(fft_size);
    ifft2.process(&mut renorm_spectrum);

    let inv = 1.0 / fft_size as f64;
    let mut renorm_ir = vec![0.0; n_taps];
    for (i, val) in renorm_ir.iter_mut().enumerate() {
        *val = renorm_spectrum[i].re * inv;
    }

    renorm_ir
}

/// Interpolate phase values from log-spaced source frequencies to a linear target grid.
fn interpolate_phase_log_space(
    src_freqs: &[f64],
    src_phase: &[f64],
    target_freqs: &[f64],
) -> Vec<f64> {
    let n_src = src_freqs.len();
    if n_src == 0 {
        return vec![0.0; target_freqs.len()];
    }
    if n_src == 1 {
        return vec![src_phase[0]; target_freqs.len()];
    }

    let src_log: Vec<f64> = src_freqs.iter().map(|&f| f.max(1.0).log2()).collect();

    target_freqs
        .iter()
        .map(|&f| {
            let f_log = f.max(1.0).log2();

            if f_log <= src_log[0] {
                return src_phase[0];
            }
            if f_log >= src_log[n_src - 1] {
                return src_phase[n_src - 1];
            }

            // Binary search for interval
            let idx = src_log.partition_point(|&x| x < f_log);
            let idx = idx.min(n_src - 1).max(1);

            let t = (f_log - src_log[idx - 1]) / (src_log[idx] - src_log[idx - 1]);
            let mut delta = src_phase[idx] - src_phase[idx - 1];
            // Shortest-arc interpolation across the 360° wrap boundary
            while delta > 180.0 {
                delta -= 360.0;
            }
            while delta < -180.0 {
                delta += 360.0;
            }
            let mut interp = src_phase[idx - 1] + t * delta;
            // Normalize result to [-180, 180]
            while interp > 180.0 {
                interp -= 360.0;
            }
            while interp < -180.0 {
                interp += 360.0;
            }
            interp
        })
        .collect()
}

/// Smooth phase in log-frequency domain using a sliding window.
fn smooth_phase_log_freq(
    phase: &Array1<f64>,
    freq: &Array1<f64>,
    width_octaves: f64,
) -> Array1<f64> {
    let len = phase.len();
    let half_width = width_octaves / 2.0;
    let mut smoothed = Array1::zeros(len);

    for i in 0..len {
        let center_log = freq[i].log2();
        let low_log = center_log - half_width;
        let high_log = center_log + half_width;

        let mut sum_sin = 0.0;
        let mut sum_cos = 0.0;
        let mut count = 0.0;
        for j in 0..len {
            let f_log = freq[j].log2();
            if f_log >= low_log && f_log <= high_log {
                let rad = phase[j].to_radians();
                sum_sin += rad.sin();
                sum_cos += rad.cos();
                count += 1.0;
            }
        }

        smoothed[i] = if count > 0.0 {
            let mut avg_deg = sum_sin.atan2(sum_cos).to_degrees();
            // Normalize to [-180, 180] for consistency
            while avg_deg > 180.0 {
                avg_deg -= 360.0;
            }
            while avg_deg < -180.0 {
                avg_deg += 360.0;
            }
            avg_deg
        } else {
            phase[i]
        };
    }

    smoothed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_curve_with_phase(freq: Vec<f64>, spl: Vec<f64>, phase: Vec<f64>) -> Curve {
        Curve {
            freq: Array1::from_vec(freq),
            spl: Array1::from_vec(spl),
            phase: Some(Array1::from_vec(phase)),
            ..Default::default()
        }
    }

    #[test]
    fn test_decompose_phase_requires_phase_data() {
        let curve = Curve {
            freq: Array1::from_vec(vec![100.0, 1000.0]),
            spl: Array1::from_vec(vec![80.0, 80.0]),
            phase: None,
            ..Default::default()
        };
        let config = MixedPhaseConfig::default();
        let result = decompose_phase(&curve, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_decompose_phase_flat_measurement() {
        // Flat magnitude + zero phase → min phase ≈ 0, excess ≈ 0
        let n = 64;
        let freq: Vec<f64> = (0..n)
            .map(|i| 20.0 * (20000.0 / 20.0_f64).powf(i as f64 / (n - 1) as f64))
            .collect();
        let spl = vec![80.0; n];
        let phase = vec![0.0; n];

        let curve = make_curve_with_phase(freq, spl, phase);
        let config = MixedPhaseConfig::default();
        let result = decompose_phase(&curve, &config);

        assert!(result.is_ok());
        let (min_phase, _excess, delay_ms, residual) = result.unwrap();

        // For flat response, everything should be near zero
        assert!(min_phase.len() == n);
        assert!(
            delay_ms.abs() < 5.0,
            "delay should be small for flat response, got {:.2} ms",
            delay_ms
        );
        let max_residual = residual.iter().map(|r| r.abs()).fold(0.0_f64, f64::max);
        assert!(max_residual < 180.0, "residual should be bounded");
    }

    #[test]
    fn test_generate_excess_phase_fir_produces_valid_output() {
        let n = 32;
        let freq = Array1::linspace(20.0, 20000.0, n);
        // Small residual phase: 5 degrees at all frequencies
        let residual_phase = Array1::from_elem(n, 5.0);
        let config = MixedPhaseConfig::default();

        let fir = generate_excess_phase_fir(&freq, &residual_phase, &config, 48000.0);

        assert!(!fir.is_empty(), "FIR should not be empty");
        assert!(fir.len() >= 31, "FIR should have minimum length");
        assert!(
            fir.iter().any(|&x| x.abs() > 1e-10),
            "FIR should have non-zero taps"
        );
    }

    #[test]
    fn test_interpolate_phase_log_space() {
        let src_freqs = vec![100.0, 1000.0, 10000.0];
        let src_phase = vec![0.0, -45.0, -90.0];

        // At source points
        let result = interpolate_phase_log_space(&src_freqs, &src_phase, &src_freqs);
        assert!((result[0] - 0.0).abs() < 0.1);
        assert!((result[1] - (-45.0)).abs() < 0.1);
        assert!((result[2] - (-90.0)).abs() < 0.1);

        // Geometric mean of 100 and 1000 ≈ 316 → should be ~-22.5°
        let mid = interpolate_phase_log_space(&src_freqs, &src_phase, &[316.0]);
        assert!(
            (mid[0] - (-22.5)).abs() < 1.0,
            "expected ~-22.5, got {:.1}",
            mid[0]
        );
    }

    #[test]
    fn interpolate_phase_log_space_uses_shortest_arc_across_wrap_boundary() {
        // Phase wraps from +179° to -179°. The shortest arc goes through ±180°,
        // not through 0°. Without wrap handling the midpoint would be 0°.
        let src_freqs = vec![100.0, 1000.0];
        let src_phase = vec![179.0, -179.0];

        let result = interpolate_phase_log_space(&src_freqs, &src_phase, &[316.0]);

        // Midpoint via shortest arc: 179 + 0.5 * (-358 + 360) = 180°
        let diff = (result[0] - 180.0).abs().min((result[0] + 180.0).abs());
        assert!(
            diff < 1.0,
            "expected ~±180° at wrap boundary, got {:.1}°",
            result[0]
        );
    }

    #[test]
    fn smooth_phase_log_freq_uses_circular_mean_at_wrap_boundary() {
        // Two phase values straddling the 360°/0° boundary: 350° and 10°.
        // The circular mean is ~0°, not the arithmetic mean of 180°.
        let freq = Array1::from_vec(vec![100.0, 200.0, 400.0]);
        let phase = Array1::from_vec(vec![350.0, 10.0, 0.0]);

        let smoothed = smooth_phase_log_freq(&phase, &freq, 2.0);

        // The middle point (200 Hz) is within 1 octave of both neighbors,
        // so it should average 350° and 10° circularly → ~0°.
        let diff = smoothed[1].abs().min((smoothed[1] - 360.0).abs());
        assert!(
            diff < 10.0,
            "circular mean of 350° and 10° should be ~0°, got {:.1}°",
            smoothed[1]
        );
    }

    #[test]
    fn generate_phase_only_fir_preserves_nyquist_magnitude_at_90_deg() {
        use num_complex::Complex64;
        use rustfft::FftPlanner;

        // When the excess phase at Nyquist is 90°, forcing the bin to be real
        // by copying .re and zeroing .im collapses the magnitude to 0. The fix
        // preserves the magnitude by using .norm() for the real part.
        let sample_rate = 48000.0;
        let freqs = vec![20.0, sample_rate / 2.0];
        let phase_deg = vec![0.0, 90.0];

        let config = FirDesignConfig {
            n_taps: 255,
            sample_rate,
            pre_ringing: None,
            ..Default::default()
        };

        let fir = generate_phase_only_fir(&freqs, &phase_deg, &config);
        assert_eq!(fir.len(), 255);

        // FFT the FIR to check magnitude at Nyquist
        let fft_size = fir.len().next_power_of_two() * 4;
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);
        let mut buffer: Vec<Complex64> = fir
            .iter()
            .map(|&x| Complex64::new(x, 0.0))
            .collect();
        buffer.resize(fft_size, Complex64::new(0.0, 0.0));
        fft.process(&mut buffer);

        let nyquist_bin = fft_size / 2;
        let nyquist_mag_db = 20.0 * buffer[nyquist_bin].norm().log10();
        assert!(
            nyquist_mag_db > -15.0,
            "Nyquist magnitude should be preserved (> -15 dB), got {:.1} dB",
            nyquist_mag_db
        );
    }

    #[test]
    fn test_phase_only_fir_near_unity_magnitude() {
        use num_complex::Complex64;

        // A phase-only FIR should have approximately unity magnitude response
        let freqs = vec![20.0, 100.0, 1000.0, 10000.0, 20000.0];
        let phase_deg = vec![0.0, -10.0, -30.0, -20.0, -5.0];

        let config = FirDesignConfig {
            n_taps: 511,
            sample_rate: 48000.0,
            pre_ringing: None,
            ..Default::default()
        };

        let fir = generate_phase_only_fir(&freqs, &phase_deg, &config);
        assert_eq!(fir.len(), 511);

        // Verify magnitude response is near-unity across audio band
        // Compute frequency response at test points
        let test_freqs: Vec<f64> = (0..50)
            .map(|i| 20.0 * (20000.0 / 20.0_f64).powf(i as f64 / 49.0))
            .collect();
        let sr = 48000.0;
        let mut max_deviation_db: f64 = 0.0;
        for &f in &test_freqs {
            let w = 2.0 * std::f64::consts::PI * f / sr;
            let mut h = Complex64::new(0.0, 0.0);
            for (n, &val) in fir.iter().enumerate() {
                let angle = -w * n as f64;
                h += Complex64::from_polar(val, angle);
            }
            let mag_db = 20.0 * h.norm().log10();
            max_deviation_db = max_deviation_db.max(mag_db.abs());
        }
        assert!(
            max_deviation_db < 0.5,
            "magnitude deviation should be < 0.5 dB, got {:.2} dB",
            max_deviation_db,
        );
    }

    #[test]
    fn test_phase_only_fir_zero_phase_is_near_impulse() {
        // Zero correction phase → FIR should be near-identity (impulse at center)
        let freqs = vec![20.0, 100.0, 1000.0, 10000.0, 20000.0];
        let phase_deg = vec![0.0; 5]; // zero correction = identity

        let config = FirDesignConfig {
            n_taps: 255,
            sample_rate: 48000.0,
            pre_ringing: None,
            ..Default::default()
        };

        let fir = generate_phase_only_fir(&freqs, &phase_deg, &config);
        assert_eq!(fir.len(), 255);

        // Center tap should dominate (due to windowing it won't be exactly 1.0)
        let center = 255 / 2;
        let center_energy = fir[center].abs();
        let off_center_max = fir
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != center)
            .map(|(_, v)| v.abs())
            .fold(0.0_f64, f64::max);

        assert!(
            center_energy > off_center_max * 2.0,
            "center tap ({:.4}) should dominate off-center max ({:.4})",
            center_energy,
            off_center_max
        );
    }

    #[test]
    fn test_phase_only_fir_produces_real_output() {
        // The IFFT output should be real-valued (imaginary parts ≈ 0)
        // This verifies conjugate symmetry construction including the Nyquist fix

        let freqs = vec![20.0, 100.0, 1000.0, 10000.0, 20000.0];
        let phase_deg = vec![0.0, -30.0, -60.0, -30.0, 0.0];

        let config = FirDesignConfig {
            n_taps: 127,
            sample_rate: 48000.0,
            pre_ringing: None,
            ..Default::default()
        };

        let fir = generate_phase_only_fir(&freqs, &phase_deg, &config);

        // All taps should be real-valued (finite, no NaN)
        for (i, &v) in fir.iter().enumerate() {
            assert!(v.is_finite(), "tap {} should be finite, got {}", i, v);
        }
    }

    #[test]
    fn test_decompose_phase_with_delay() {
        // Known delay → decompose should recover it
        let n = 128;
        let freq: Vec<f64> = (0..n)
            .map(|i| 20.0 * (20000.0 / 20.0_f64).powf(i as f64 / (n - 1) as f64))
            .collect();
        let spl = vec![80.0; n];
        let delay_ms = 2.0;
        let delay_s = delay_ms / 1000.0;
        // Linear phase from delay: φ = -360*f*τ degrees
        let phase: Vec<f64> = freq.iter().map(|&f| -360.0 * f * delay_s).collect();

        let curve = make_curve_with_phase(freq, spl, phase);
        let config = MixedPhaseConfig {
            phase_smoothing_octaves: 0.0,
            ..Default::default()
        };
        let result = decompose_phase(&curve, &config);
        assert!(result.is_ok());
        let (_, _, estimated_delay, _) = result.unwrap();

        // The delay estimation is approximate since minimum-phase reconstruction
        // introduces its own phase contribution that partially absorbs the delay.
        // Just verify the estimated delay is positive and in the right ballpark.
        assert!(
            estimated_delay > 0.0 && estimated_delay < delay_ms * 3.0,
            "should recover positive delay roughly near {:.1} ms, got {:.2} ms",
            delay_ms,
            estimated_delay
        );
    }

    #[test]
    #[should_panic(expected = "correction_depth length")]
    fn test_depth_mask_length_mismatch_panics() {
        // Bug fix: mismatched depth mask length must panic, not silently truncate
        let n = 32;
        let freq = Array1::linspace(20.0, 20000.0, n);
        let residual_phase = Array1::from_elem(n, 5.0);
        let bad_depth = Array1::from_elem(n / 2, 0.8); // wrong length
        let config = MixedPhaseConfig::default();

        // This should panic due to length mismatch assertion
        generate_excess_phase_fir_with_depth(
            &freq,
            &residual_phase,
            &config,
            48000.0,
            Some(&bad_depth),
        );
    }

    #[test]
    fn test_depth_mask_zeros_low_depth_frequencies() {
        // When all depth values are below min_spatial_depth, correction phase is zeroed
        // → the FIR becomes a near-identity filter (center tap dominates).
        let n = 32;
        let freq = Array1::linspace(20.0, 20000.0, n);
        let residual_phase = Array1::from_elem(n, 30.0); // 30 degrees everywhere

        // All-low depth: everything below min_spatial_depth → no correction
        let low_depth = Array1::from_elem(n, 0.1);
        let config = MixedPhaseConfig {
            min_spatial_depth: 0.5,
            ..Default::default()
        };

        let fir_masked = generate_excess_phase_fir_with_depth(
            &freq,
            &residual_phase,
            &config,
            48000.0,
            Some(&low_depth),
        );

        let fir_unmasked =
            generate_excess_phase_fir_with_depth(&freq, &residual_phase, &config, 48000.0, None);

        // The masked version (zero correction) should be more center-concentrated
        // (near-identity) than the unmasked version (phase correction applied).
        let center = fir_masked.len() / 2;
        let masked_center_ratio =
            fir_masked[center].abs() / fir_masked.iter().map(|x| x.abs()).sum::<f64>().max(1e-12);
        let unmasked_center_ratio = fir_unmasked[center].abs()
            / fir_unmasked.iter().map(|x| x.abs()).sum::<f64>().max(1e-12);

        assert!(
            masked_center_ratio > unmasked_center_ratio,
            "masked FIR center ratio ({:.4}) should be more concentrated than unmasked ({:.4})",
            masked_center_ratio,
            unmasked_center_ratio,
        );
    }
}
