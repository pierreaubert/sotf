//! FIR filter design from frequency response
//!
//! This module provides functions to generate FIR filters that match a target
//! frequency response, with support for different phase types including
//! Kirkeby regularized inversion for room correction.

use num_complex::Complex64;
use rustfft::FftPlanner;
use rustfft::num_traits::Zero;
use std::path::Path;

use super::fir::{WindowType, generate_window};

/// Phase type for FIR generation
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum FirPhase {
    /// Linear phase (symmetrical impulse response, constant delay)
    #[default]
    Linear,
    /// Minimum phase (causal, minimum delay, concentrates energy at start)
    Minimum,
    /// Kirkeby regularized inversion (mixed phase, good for room correction)
    Kirkeby,
}

impl FirPhase {
    /// Returns the short string representation of the phase type.
    pub fn short_name(&self) -> &'static str {
        match self {
            FirPhase::Linear => "LIN",
            FirPhase::Minimum => "MIN",
            FirPhase::Kirkeby => "KIRK",
        }
    }

    /// Returns the long string representation of the phase type.
    pub fn long_name(&self) -> &'static str {
        match self {
            FirPhase::Linear => "Linear",
            FirPhase::Minimum => "Minimum",
            FirPhase::Kirkeby => "Kirkeby",
        }
    }
}

/// Configuration for pre-ringing suppression on FIR filters.
///
/// Pre-ringing occurs in linear-phase and Kirkeby FIR filters as energy arriving
/// before the main impulse tap. When it exceeds the audibility threshold (~-30 dB
/// relative to the main tap), it's perceived as a "ringing" artifact before transients.
///
/// Reference: Brännmark & Sternad, Patent EP2104374B1 — pre-ringing envelope constraint
#[derive(Debug, Clone)]
pub struct PreRingingConfig {
    /// Maximum pre-ringing level in dB relative to the main tap.
    /// Taps before the main tap exceeding this threshold will be attenuated.
    /// Default: -30.0 dB (psychoacoustically inaudible)
    pub threshold_db: f64,

    /// Maximum pre-ringing time in seconds. Pre-ringing energy beyond this
    /// duration before the main tap will be fully suppressed.
    /// Default: 0.005 (5 ms)
    pub max_time_s: f64,
}

impl Default for PreRingingConfig {
    fn default() -> Self {
        Self {
            threshold_db: -30.0,
            max_time_s: 0.005,
        }
    }
}

/// Configuration for FIR filter generation
#[derive(Debug, Clone)]
pub struct FirDesignConfig {
    /// Number of taps (coefficients)
    pub n_taps: usize,
    /// Sample rate in Hz
    pub sample_rate: f64,
    /// Phase type
    pub phase: FirPhase,
    /// Minimum frequency for in-band regularization (Kirkeby only)
    pub min_freq: f64,
    /// Maximum frequency for in-band regularization (Kirkeby only)
    pub max_freq: f64,
    /// Window type for final windowing
    pub window: WindowType,
    /// Whether to correct excess phase in Kirkeby mode (default: false)
    /// When true, the filter will correct both magnitude and excess phase.
    /// When false, only magnitude is corrected (produces linear-phase FIR).
    /// Note: Excess phase correction requires clean phase measurements to be effective.
    pub correct_excess_phase: bool,
    /// Phase smoothing width in octaves (default: 0.167 = 1/6 octave)
    /// Applied via group delay smoothing when excess phase correction is enabled.
    /// Set to 0.0 to disable smoothing.
    pub phase_smoothing_octaves: f64,
    /// Optional pre-ringing suppression. When set, taps before the main impulse
    /// exceeding the threshold are attenuated to reduce audible pre-ringing.
    pub pre_ringing: Option<PreRingingConfig>,
}

impl Default for FirDesignConfig {
    fn default() -> Self {
        Self {
            n_taps: 4096,
            sample_rate: 48000.0,
            phase: FirPhase::Linear,
            min_freq: 20.0,
            max_freq: 20000.0,
            window: WindowType::Blackman,
            correct_excess_phase: false, // Magnitude-only by default (more robust)
            phase_smoothing_octaves: 0.167, // 1/6 octave smoothing
            pre_ringing: None,
        }
    }
}

/// Generate an FIR filter to match a target frequency response
///
/// This function takes a target magnitude response (in dB) at specified frequencies
/// and generates FIR coefficients that approximate that response.
///
/// `FirPhase::Kirkeby` is intentionally unsupported here because Kirkeby
/// regularized inversion requires both a measurement and a target response.
/// Use `generate_kirkeby_correction` for that workflow.
///
/// # Arguments
/// * `freqs` - Frequency points in Hz (must be positive, sorted ascending)
/// * `magnitude_db` - Target magnitude in dB at each frequency point
/// * `config` - FIR design configuration
///
/// # Returns
/// * Vector of FIR coefficients
///
/// # Panics
/// Panics if `freqs` and `magnitude_db` have different lengths, if either
/// slice is empty, if any frequency or magnitude is non-finite, if any
/// frequency is non-positive, or if frequencies are not strictly increasing.
pub fn generate_fir_from_response(
    freqs: &[f64],
    magnitude_db: &[f64],
    config: &FirDesignConfig,
) -> Vec<f64> {
    assert_eq!(
        freqs.len(),
        magnitude_db.len(),
        "freqs and magnitude_db must have same length"
    );
    assert!(
        !freqs.is_empty(),
        "freqs and magnitude_db must not be empty"
    );
    assert!(
        freqs.iter().all(|f| f.is_finite() && *f > 0.0),
        "freqs must contain finite positive values"
    );
    assert!(
        freqs.windows(2).all(|w| w[0] < w[1]),
        "freqs must be strictly increasing"
    );
    assert!(
        magnitude_db.iter().all(|db| db.is_finite()),
        "magnitude_db must contain finite values"
    );
    assert!(
        config.phase != FirPhase::Kirkeby,
        "Kirkeby correction requires measurement and target responses; use generate_kirkeby_correction"
    );

    let n_taps = config.n_taps;
    let sample_rate = config.sample_rate;

    // FFT size should be at least n_taps, preferably power of 2
    let fft_size = (n_taps * 8).next_power_of_two().max(4096);
    let n_bins = fft_size / 2 + 1;

    // Create linear frequency grid (0 to Nyquist)
    let freq_step = sample_rate / fft_size as f64;
    let linear_freqs: Vec<f64> = (0..n_bins).map(|i| i as f64 * freq_step).collect();

    // Interpolate target curve to this grid (log-space interpolation)
    let interpolated_db = interpolate_log_space(freqs, magnitude_db, &linear_freqs);

    // Convert dB to linear magnitude
    let magnitude: Vec<f64> = interpolated_db
        .iter()
        .map(|db| 10.0_f64.powf(db / 20.0))
        .collect();

    // Construct complex spectrum based on phase type
    let spectrum = match config.phase {
        FirPhase::Linear => generate_linear_phase_spectrum(&magnitude),
        FirPhase::Minimum => generate_minimum_phase_spectrum(&magnitude, fft_size),
        FirPhase::Kirkeby => {
            unreachable!("Kirkeby correction must be generated with measurement data")
        }
    };

    // IFFT to get impulse response
    let ir = spectrum_to_impulse_response(&spectrum, fft_size);

    // Window and center the impulse response
    finalize_impulse_response(
        &ir,
        n_taps,
        config.phase,
        &config.window,
        config.pre_ringing.as_ref(),
        config.sample_rate,
    )
}

/// Generate Kirkeby regularized FIR correction filter
///
/// Kirkeby inversion uses frequency-dependent regularization to create a stable
/// inverse filter that doesn't over-boost deep nulls (common in room measurements).
///
/// # Arguments
/// * `meas_freqs` - Measurement frequency points in Hz
/// * `meas_magnitude_db` - Measurement magnitude in dB
/// * `meas_phase_deg` - Measurement phase in degrees (optional, uses 0 if None)
/// * `target_magnitude_db` - Target magnitude in dB at meas_freqs points
/// * `config` - FIR design configuration
///
/// # Returns
/// * Vector of FIR coefficients
pub fn generate_kirkeby_correction(
    meas_freqs: &[f64],
    meas_magnitude_db: &[f64],
    meas_phase_deg: Option<&[f64]>,
    target_magnitude_db: &[f64],
    config: &FirDesignConfig,
) -> Vec<f64> {
    assert_eq!(
        meas_freqs.len(),
        meas_magnitude_db.len(),
        "meas_freqs and meas_magnitude_db must have same length"
    );
    assert_eq!(
        meas_freqs.len(),
        target_magnitude_db.len(),
        "meas_freqs and target_magnitude_db must have same length"
    );
    if let Some(phase) = meas_phase_deg {
        assert_eq!(
            meas_freqs.len(),
            phase.len(),
            "meas_phase_deg must match measurement length"
        );
    }

    let n_taps = config.n_taps;
    let sample_rate = config.sample_rate;
    let min_freq = config.min_freq;
    let max_freq = config.max_freq;

    // FFT size - use next power of 2 above n_taps, but at least 65536 for good low freq resolution
    let fft_len = (n_taps * 4).max(65536).next_power_of_two();
    let num_bins = fft_len / 2 + 1;
    let freq_step = sample_rate / fft_len as f64;

    // Linear frequency grid
    let linear_freqs: Vec<f64> = (0..num_bins).map(|i| i as f64 * freq_step).collect();

    // Interpolate measurement and target to linear grid
    let meas_spl_interp = interpolate_log_space(meas_freqs, meas_magnitude_db, &linear_freqs);
    let target_spl_interp = interpolate_log_space(meas_freqs, target_magnitude_db, &linear_freqs);

    // Compute excess phase correction if enabled and phase data is available
    // Excess phase = measured phase - minimum phase (derived from magnitude)
    // We only want to correct excess phase, not minimum phase
    let excess_phase_correction: Option<Vec<f64>> = if config.correct_excess_phase {
        meas_phase_deg.map(|phase_deg| {
            // Convert measured phase from degrees to radians
            let meas_phase_rad: Vec<f64> = phase_deg.iter().map(|&d| d.to_radians()).collect();

            // Apply phase smoothing via group delay if enabled
            let smoothed_phase_rad = if config.phase_smoothing_octaves > 0.0 {
                super::phase_smooth::smooth_phase_via_group_delay(
                    meas_freqs,
                    &meas_phase_rad,
                    config.phase_smoothing_octaves,
                )
            } else {
                meas_phase_rad
            };

            // Interpolate smoothed phase to linear grid using complex interpolation
            // (avoids wrap artifacts)
            let meas_phase_interp = super::phase_smooth::interpolate_phase_complex(
                meas_freqs,
                &smoothed_phase_rad,
                &linear_freqs,
            );

            // Compute minimum phase from magnitude using Hilbert transform
            let min_phase = compute_minimum_phase_from_magnitude(&meas_spl_interp);

            // Excess phase = measured - minimum (both in radians for computation)
            // Correction = -excess_phase (to cancel it out)
            meas_phase_interp
                .iter()
                .zip(min_phase.iter())
                .map(|(&measured_rad, &min_rad)| {
                    let excess_rad = measured_rad - min_rad;
                    -excess_rad // Negative to invert/correct the excess phase
                })
                .collect()
        })
    } else {
        None // Magnitude-only correction (linear-phase FIR)
    };

    // Maximum boost/cut limits for room correction.
    // The Kirkeby regularization shapes the inverse before these limits are applied,
    // and the final clamp keeps pathological measurement bins from creating unsafe EQ.
    let max_boost_db = 15.0;
    let max_cut_db = 20.0;
    let max_boost_linear = 10.0_f64.powf(max_boost_db / 20.0);
    let max_cut_linear = 10.0_f64.powf(max_cut_db / 20.0);

    // Choose a regularization floor so that the peak inverse gain asymptotically
    // stays around the configured boost ceiling. For H/(|H|² + β²), the peak is
    // approximately 1/(2β), so β ~= 1/(2 * max_boost).
    let beta = 1.0 / (2.0 * max_boost_linear);

    let mut h_inv = Vec::with_capacity(num_bins);

    for i in 0..num_bins {
        let f = linear_freqs[i];

        // Relative measurement error against the target. Using the relative response keeps
        // the inversion invariant to arbitrary SPL offsets in the source curves.
        let rel_mag = 10.0_f64.powf((meas_spl_interp[i] - target_spl_interp[i]) / 20.0);

        // Determine if we're in-band or out-of-band
        let width = 10.0; // Hz transition width
        let transition = if f < min_freq {
            ((f - (min_freq - width)) / width).clamp(0.0, 1.0)
        } else if f > max_freq {
            1.0 - ((f - max_freq) / width).clamp(0.0, 1.0)
        } else {
            1.0
        };

        // Kirkeby-style regularized inverse of the relative measurement response.
        let regularized_mag = rel_mag / (rel_mag * rel_mag + beta * beta);
        let limited_mag = regularized_mag.clamp(1.0 / max_cut_linear, max_boost_linear);
        let c_mag = 1.0 + transition * (limited_mag - 1.0);

        // Apply excess phase correction if available, otherwise use zero phase (linear phase FIR)
        let c_phase = excess_phase_correction
            .as_ref()
            .map(|epc| epc[i] * transition) // Taper phase correction at band edges too
            .unwrap_or(0.0);

        let c = Complex64::from_polar(c_mag, c_phase);

        h_inv.push(c);
    }

    // IFFT to get impulse response
    let mut spectrum = vec![Complex64::new(0.0, 0.0); fft_len];

    // DC and Nyquist
    spectrum[0] = h_inv[0];
    spectrum[fft_len / 2] = h_inv[num_bins - 1];

    // Fill positive freqs and conjugate symmetry
    for i in 1..fft_len / 2 {
        spectrum[i] = h_inv[i];
        spectrum[fft_len - i] = h_inv[i].conj();
    }

    // Perform IFFT
    let mut planner = FftPlanner::new();
    let ifft = planner.plan_fft_inverse(fft_len);
    ifft.process(&mut spectrum);

    // Normalize
    let mut impulse: Vec<f64> = spectrum.iter().map(|c| c.re / fft_len as f64).collect();

    // Cyclic shift to center the impulse
    let shift = fft_len / 2;
    impulse.rotate_right(shift);

    // Extract n_taps centered around shift
    let start_idx = shift - n_taps / 2;
    let window = generate_window(n_taps, WindowType::Hann, 0.0);
    let mut coeffs = vec![0.0; n_taps];
    for (i, coeff) in coeffs.iter_mut().enumerate() {
        let src_idx = start_idx + i;
        if src_idx < impulse.len() {
            *coeff = impulse[src_idx] * window[i];
        }
    }

    // Apply pre-ringing suppression if configured
    if let Some(pr_config) = &config.pre_ringing {
        suppress_pre_ringing(&mut coeffs, pr_config, sample_rate);
    }

    coeffs
}

/// Save FIR coefficients to a WAV file (32-bit float mono)
///
/// # Arguments
/// * `coeffs` - FIR coefficients
/// * `sample_rate` - Sample rate in Hz
/// * `path` - Output file path
pub fn save_fir_to_wav(
    coeffs: &[f64],
    sample_rate: u32,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(path, spec)?;
    for &sample in coeffs {
        writer.write_sample(sample as f32)?;
    }
    writer.finalize()?;

    Ok(())
}

// ============================================================================
// Internal helper functions
// ============================================================================

/// Compute minimum phase from magnitude response using Hilbert transform.
///
/// The minimum phase is the phase response that is mathematically determined
/// by the magnitude response. It represents the "natural" phase for any passive
/// system with the given magnitude.
///
/// Formula: φ_min(ω) = -H{ln|H(ω)|} where H{} is the Hilbert transform
///
/// # Arguments
/// * `magnitude_db` - Magnitude values in dB (on linear frequency grid)
///
/// # Returns
/// * Phase values in radians (minimum phase)
fn compute_minimum_phase_from_magnitude(magnitude_db: &[f64]) -> Vec<f64> {
    let n = magnitude_db.len();
    if n == 0 {
        return Vec::new();
    }

    // Convert dB to natural log of magnitude
    // SPL = 20 * log10(|H|)
    // ln(|H|) = SPL / 20 * ln(10)
    let ln_mag: Vec<f64> = magnitude_db
        .iter()
        .map(|&db| db / 20.0 * 10.0_f64.ln())
        .collect();

    // Compute Hilbert transform of ln|H|
    // This gives us the minimum phase
    let phase_rad = hilbert_transform(&ln_mag);

    // Negate to get minimum phase (by convention)
    phase_rad.iter().map(|&p| -p).collect()
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
    let n_fft = n.next_power_of_two().max(n * 2);

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
    // DC component (index 0) stays unchanged
    for s in spectrum.iter_mut().take(half).skip(1) {
        *s *= 2.0;
    }
    // Nyquist (index half) stays unchanged
    for s in spectrum.iter_mut().skip(half + 1) {
        *s = Complex64::new(0.0, 0.0);
    }

    // Inverse FFT
    ifft.process(&mut spectrum);

    // Normalize and extract imaginary part (the Hilbert transform)
    spectrum[..n].iter().map(|c| c.im / n_fft as f64).collect()
}

/// Interpolate values from source frequencies to target frequencies using log-space
fn interpolate_log_space(src_freqs: &[f64], src_values: &[f64], target_freqs: &[f64]) -> Vec<f64> {
    let mut result = Vec::with_capacity(target_freqs.len());

    for &f in target_freqs {
        if f <= 0.0 {
            // DC: use first value or extrapolate
            result.push(src_values.first().copied().unwrap_or(0.0));
            continue;
        }

        let log_f = f.ln();

        // Find bracketing indices in source
        let mut lower_idx = 0;
        let mut upper_idx = src_freqs.len() - 1;

        // Binary search for position
        for (i, &sf) in src_freqs.iter().enumerate() {
            if sf <= f {
                lower_idx = i;
            }
            if sf >= f && i < upper_idx {
                upper_idx = i;
                break;
            }
        }

        if lower_idx == upper_idx || src_freqs[lower_idx] <= 0.0 || src_freqs[upper_idx] <= 0.0 {
            result.push(src_values[lower_idx]);
        } else {
            // Log-linear interpolation
            let log_f_low = src_freqs[lower_idx].ln();
            let log_f_high = src_freqs[upper_idx].ln();
            let t = (log_f - log_f_low) / (log_f_high - log_f_low);
            let interp_val =
                src_values[lower_idx] + t * (src_values[upper_idx] - src_values[lower_idx]);
            result.push(interp_val);
        }
    }

    result
}

/// Generate linear phase spectrum (zero phase, real-only)
fn generate_linear_phase_spectrum(magnitude: &[f64]) -> Vec<Complex64> {
    magnitude.iter().map(|&m| Complex64::new(m, 0.0)).collect()
}

/// Generate minimum phase spectrum using cepstrum method
fn generate_minimum_phase_spectrum(magnitude: &[f64], fft_size: usize) -> Vec<Complex64> {
    let n_bins = magnitude.len();

    // Step 1: Log Magnitude (avoid log(0))
    let log_mag: Vec<Complex64> = magnitude
        .iter()
        .map(|&m| Complex64::new(m.max(1e-9).ln(), 0.0))
        .collect();

    // Construct full symmetric spectrum for IFFT
    let mut full_log_mag = vec![Complex64::zero(); fft_size];
    full_log_mag[0] = log_mag[0];
    for i in 1..n_bins {
        full_log_mag[i] = log_mag[i];
        full_log_mag[fft_size - i] = log_mag[i].conj();
    }
    // Nyquist
    if fft_size.is_multiple_of(2) {
        full_log_mag[n_bins - 1] = log_mag[n_bins - 1];
    }

    // Step 2: IFFT
    let mut planner = FftPlanner::new();
    let ifft = planner.plan_fft_inverse(fft_size);
    let mut cepstrum = full_log_mag;
    ifft.process(&mut cepstrum);

    // Normalize IFFT
    for x in &mut cepstrum {
        *x /= fft_size as f64;
    }

    // Step 3: Window Cepstrum to make it causal
    let mut causal_cepstrum = vec![Complex64::zero(); fft_size];
    causal_cepstrum[0] = cepstrum[0]; // DC
    for i in 1..fft_size / 2 {
        causal_cepstrum[i] = cepstrum[i] * 2.0;
    }
    causal_cepstrum[fft_size / 2] = cepstrum[fft_size / 2]; // Nyquist

    // Step 4: FFT back
    let fft = planner.plan_fft_forward(fft_size);
    let mut analytic_log_spectrum = causal_cepstrum;
    fft.process(&mut analytic_log_spectrum);

    // Step 5: Exponentiate to get Min Phase Spectrum
    analytic_log_spectrum[..n_bins]
        .iter()
        .map(|c| c.exp())
        .collect()
}

/// Convert spectrum to impulse response via IFFT
fn spectrum_to_impulse_response(spectrum: &[Complex64], fft_size: usize) -> Vec<f64> {
    let n_bins = spectrum.len();

    // Construct full symmetric spectrum
    let mut full_spectrum = vec![Complex64::zero(); fft_size];
    full_spectrum[0] = spectrum[0]; // DC must be real
    for i in 1..n_bins {
        full_spectrum[i] = spectrum[i];
        full_spectrum[fft_size - i] = spectrum[i].conj();
    }
    // Nyquist must be real
    if fft_size.is_multiple_of(2) {
        full_spectrum[n_bins - 1] = Complex64::new(spectrum[n_bins - 1].norm(), 0.0);
    }

    let mut planner = FftPlanner::new();
    let ifft = planner.plan_fft_inverse(fft_size);
    let mut ir_complex = full_spectrum;
    ifft.process(&mut ir_complex);

    // Extract real part and normalize
    ir_complex.iter().map(|c| c.re / fft_size as f64).collect()
}

/// Finalize impulse response with windowing and centering
fn finalize_impulse_response(
    ir: &[f64],
    n_taps: usize,
    phase: FirPhase,
    window_type: &WindowType,
    pre_ringing: Option<&PreRingingConfig>,
    sample_rate: f64,
) -> Vec<f64> {
    let fft_size = ir.len();
    let mut final_ir;

    if phase == FirPhase::Linear {
        // Rotate to center for linear phase
        let center = n_taps / 2;
        final_ir = vec![0.0; n_taps];

        for (i, val) in final_ir.iter_mut().enumerate().take(n_taps) {
            let shift = i as isize - center as isize;
            let ir_idx = if shift < 0 {
                fft_size as isize + shift
            } else {
                shift
            };
            *val = ir[ir_idx as usize];
        }
    } else {
        // Minimum phase: Impulse is already at 0. Just truncate.
        final_ir = ir[..n_taps].to_vec();
    }

    // Apply window
    let window = generate_window(n_taps, *window_type, 0.0);
    for (x, w) in final_ir.iter_mut().zip(window.iter()) {
        *x *= w;
    }

    // Apply pre-ringing suppression if configured
    if let Some(config) = pre_ringing {
        suppress_pre_ringing(&mut final_ir, config, sample_rate);
    }

    final_ir
}

/// Suppress pre-ringing in an FIR impulse response.
///
/// Finds the main tap (peak absolute value), then attenuates all taps before it
/// that exceed the threshold. Also applies a time limit: taps further than
/// `max_time_s` before the main tap are fully suppressed.
///
/// The attenuation uses a smooth envelope to avoid introducing new artifacts.
pub fn suppress_pre_ringing(ir: &mut [f64], config: &PreRingingConfig, sample_rate: f64) {
    if ir.is_empty() {
        return;
    }

    // Find the main tap (peak absolute value)
    let main_tap_idx = ir
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
        .map(|(i, _)| i)
        .unwrap_or(0);

    let main_tap_abs = ir[main_tap_idx].abs();
    if main_tap_abs == 0.0 {
        return;
    }

    // Threshold in linear scale
    let threshold_linear = main_tap_abs * 10.0_f64.powf(config.threshold_db / 20.0);

    // Maximum number of samples of pre-ringing allowed
    let max_pre_samples = (config.max_time_s * sample_rate).round() as usize;

    // Process all taps before the main tap
    for (i, sample) in ir[..main_tap_idx].iter_mut().enumerate() {
        let samples_before = main_tap_idx - i;

        if max_pre_samples == 0 || samples_before > max_pre_samples {
            // Beyond time limit (or zero time limit): fully suppress
            *sample = 0.0;
        } else if sample.abs() > threshold_linear {
            // Exceeds threshold: clamp to threshold with sign preserved
            // Use smooth fade: closer to time limit → more suppression
            let time_ratio = samples_before as f64 / max_pre_samples as f64;
            // Cosine fade: 1.0 at main tap, 0.0 at time limit
            let fade = 0.5 * (1.0 + (std::f64::consts::PI * time_ratio).cos());
            let clamped = sample.signum() * threshold_linear;
            // Blend between clamped and original based on proximity to time limit
            *sample = clamped + (*sample - clamped) * fade;
            // Final clamp to threshold
            if sample.abs() > threshold_linear {
                *sample = sample.signum() * threshold_linear;
            }
        }
    }
}

/// Analyze pre-ringing in an FIR impulse response.
///
/// Returns the peak pre-ringing level in dB relative to the main tap,
/// and the time extent of pre-ringing above the threshold.
pub fn analyze_pre_ringing(ir: &[f64], sample_rate: f64) -> PreRingingAnalysis {
    if ir.is_empty() {
        return PreRingingAnalysis {
            main_tap_index: 0,
            peak_pre_ringing_db: f64::NEG_INFINITY,
            pre_ringing_time_ms: 0.0,
        };
    }

    let main_tap_idx = ir
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
        .map(|(i, _)| i)
        .unwrap_or(0);

    let main_tap_abs = ir[main_tap_idx].abs();
    if main_tap_abs == 0.0 {
        return PreRingingAnalysis {
            main_tap_index: main_tap_idx,
            peak_pre_ringing_db: f64::NEG_INFINITY,
            pre_ringing_time_ms: 0.0,
        };
    }

    let mut peak_pre_ringing_db = f64::NEG_INFINITY;
    let mut earliest_significant = main_tap_idx;
    let threshold_db = -60.0; // noise floor for analysis

    for (i, &tap) in ir[..main_tap_idx].iter().enumerate() {
        let tap_abs = tap.abs();
        if tap_abs == 0.0 {
            continue; // skip silent taps
        }
        let level_db = 20.0 * (tap_abs / main_tap_abs).log10();
        if level_db > peak_pre_ringing_db {
            peak_pre_ringing_db = level_db;
        }
        if level_db > threshold_db && i < earliest_significant {
            earliest_significant = i;
        }
    }

    let pre_ringing_samples = main_tap_idx.saturating_sub(earliest_significant);
    let pre_ringing_time_ms = pre_ringing_samples as f64 / sample_rate * 1000.0;

    PreRingingAnalysis {
        main_tap_index: main_tap_idx,
        peak_pre_ringing_db,
        pre_ringing_time_ms,
    }
}

/// Analysis results for pre-ringing in an FIR filter.
#[derive(Debug, Clone)]
pub struct PreRingingAnalysis {
    /// Index of the main (peak) tap
    pub main_tap_index: usize,
    /// Peak pre-ringing level in dB relative to main tap
    pub peak_pre_ringing_db: f64,
    /// Time extent of significant pre-ringing in milliseconds
    pub pre_ringing_time_ms: f64,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fir::Fir;
    use ndarray::array;

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn test_fir_phase_names() {
        assert_eq!(FirPhase::Linear.short_name(), "LIN");
        assert_eq!(FirPhase::Minimum.short_name(), "MIN");
        assert_eq!(FirPhase::Kirkeby.short_name(), "KIRK");

        assert_eq!(FirPhase::Linear.long_name(), "Linear");
        assert_eq!(FirPhase::Minimum.long_name(), "Minimum");
        assert_eq!(FirPhase::Kirkeby.long_name(), "Kirkeby");
    }

    #[test]
    fn test_fir_design_config_default() {
        let config = FirDesignConfig::default();
        assert_eq!(config.n_taps, 4096);
        assert_eq!(config.sample_rate, 48000.0);
        assert_eq!(config.phase, FirPhase::Linear);
    }

    #[test]
    fn test_generate_fir_from_response_flat() {
        let freqs = vec![20.0, 100.0, 1000.0, 10000.0, 20000.0];
        let magnitude_db = vec![0.0, 0.0, 0.0, 0.0, 0.0];

        let config = FirDesignConfig {
            n_taps: 256,
            sample_rate: 48000.0,
            phase: FirPhase::Linear,
            ..Default::default()
        };

        let coeffs = generate_fir_from_response(&freqs, &magnitude_db, &config);

        assert_eq!(coeffs.len(), 256);
        // Should have non-zero coefficients
        assert!(coeffs.iter().any(|&x| x.abs() > 1e-10));
    }

    #[test]
    fn test_generate_fir_minimum_phase() {
        let freqs = vec![20.0, 100.0, 1000.0, 10000.0, 20000.0];
        let magnitude_db = vec![-3.0, 0.0, 2.0, 0.0, -3.0];

        let config = FirDesignConfig {
            n_taps: 512,
            sample_rate: 48000.0,
            phase: FirPhase::Minimum,
            ..Default::default()
        };

        let coeffs = generate_fir_from_response(&freqs, &magnitude_db, &config);

        assert_eq!(coeffs.len(), 512);

        // For minimum phase, first half should have more energy than second half
        // (windowing affects the exact distribution)
        let total_energy: f64 = coeffs.iter().map(|x| x * x).sum();
        let first_half_energy: f64 = coeffs[..256].iter().map(|x| x * x).sum();
        let second_half_energy: f64 = coeffs[256..].iter().map(|x| x * x).sum();

        // First half should have more energy than second half for minimum phase
        assert!(
            first_half_energy > second_half_energy,
            "Minimum phase should have more energy in first half: first={:.4}, second={:.4}",
            first_half_energy / total_energy,
            second_half_energy / total_energy
        );
    }

    #[test]
    #[should_panic(expected = "Kirkeby correction requires measurement and target")]
    fn test_generate_fir_from_response_rejects_kirkeby_phase() {
        let freqs = vec![20.0, 100.0, 1000.0, 10000.0, 20000.0];
        let magnitude_db = vec![0.0, 0.0, 0.0, 0.0, 0.0];
        let config = FirDesignConfig {
            n_taps: 256,
            sample_rate: 48_000.0,
            phase: FirPhase::Kirkeby,
            ..Default::default()
        };

        let _ = generate_fir_from_response(&freqs, &magnitude_db, &config);
    }

    #[test]
    #[should_panic(expected = "freqs must be strictly increasing")]
    fn test_generate_fir_from_response_rejects_unsorted_freqs() {
        let freqs = vec![20.0, 1000.0, 100.0];
        let magnitude_db = vec![0.0, 0.0, 0.0];
        let config = FirDesignConfig {
            n_taps: 128,
            sample_rate: 48_000.0,
            phase: FirPhase::Linear,
            ..Default::default()
        };

        let _ = generate_fir_from_response(&freqs, &magnitude_db, &config);
    }

    #[test]
    #[should_panic(expected = "freqs must contain finite positive values")]
    fn test_generate_fir_from_response_rejects_nonpositive_freqs() {
        let freqs = vec![0.0, 100.0, 1000.0];
        let magnitude_db = vec![0.0, 0.0, 0.0];
        let config = FirDesignConfig {
            n_taps: 128,
            sample_rate: 48_000.0,
            phase: FirPhase::Linear,
            ..Default::default()
        };

        let _ = generate_fir_from_response(&freqs, &magnitude_db, &config);
    }

    #[test]
    #[should_panic(expected = "magnitude_db must contain finite values")]
    fn test_generate_fir_from_response_rejects_nonfinite_magnitude() {
        let freqs = vec![20.0, 100.0, 1000.0];
        let magnitude_db = vec![0.0, f64::NAN, 0.0];
        let config = FirDesignConfig {
            n_taps: 128,
            sample_rate: 48_000.0,
            phase: FirPhase::Linear,
            ..Default::default()
        };

        let _ = generate_fir_from_response(&freqs, &magnitude_db, &config);
    }

    #[test]
    fn test_generate_kirkeby_correction() {
        let freqs = vec![20.0, 100.0, 500.0, 1000.0, 5000.0, 20000.0];
        let meas_db = vec![75.0, 82.0, 80.0, 78.0, 72.0, 65.0];
        let target_db = vec![80.0, 80.0, 80.0, 80.0, 80.0, 80.0];

        let config = FirDesignConfig {
            n_taps: 4096,
            sample_rate: 48000.0,
            phase: FirPhase::Kirkeby,
            min_freq: 20.0,
            max_freq: 1000.0,
            ..Default::default()
        };

        let coeffs = generate_kirkeby_correction(&freqs, &meas_db, None, &target_db, &config);

        assert_eq!(coeffs.len(), 4096);
        assert!(coeffs.iter().any(|&x| x.abs() > 1e-10));
    }

    #[test]
    fn test_generate_kirkeby_correction_regularizes_deep_null() {
        let freqs = vec![
            20.0, 40.0, 80.0, 100.0, 120.0, 200.0, 1000.0, 5000.0, 20000.0,
        ];
        let meas_db = vec![0.0, 0.0, -6.0, -30.0, -6.0, 0.0, 0.0, 0.0, 0.0];
        let target_db = vec![0.0; freqs.len()];

        let config = FirDesignConfig {
            n_taps: 2048,
            sample_rate: 48_000.0,
            phase: FirPhase::Kirkeby,
            min_freq: 20.0,
            max_freq: 500.0,
            ..Default::default()
        };

        let coeffs = generate_kirkeby_correction(&freqs, &meas_db, None, &target_db, &config);
        let fir = Fir::new_custom(coeffs, config.sample_rate);
        let response = fir.np_log_result(&array![80.0, 100.0, 120.0]);

        assert!(response[1].is_finite());
        assert!(
            response[1] < 15.5,
            "deep-null correction should stay below the regularized boost ceiling, got {:.2} dB",
            response[1]
        );
        assert!(
            response[1] > response[0] && response[1] > response[2],
            "the correction should still target the null center more strongly than nearby bins"
        );
    }

    #[test]
    fn test_interpolate_log_space() {
        let src_freqs = vec![100.0, 1000.0, 10000.0];
        let src_values = vec![0.0, 10.0, 20.0];

        // Test at known points
        let target = vec![100.0, 1000.0, 10000.0];
        let result = interpolate_log_space(&src_freqs, &src_values, &target);

        assert!(approx_eq(result[0], 0.0, 0.1));
        assert!(approx_eq(result[1], 10.0, 0.1));
        assert!(approx_eq(result[2], 20.0, 0.1));

        // Test interpolated point (geometric mean of 100 and 1000 is ~316)
        let target2 = vec![316.0];
        let result2 = interpolate_log_space(&src_freqs, &src_values, &target2);
        // Should be approximately 5.0 (halfway in log space)
        assert!(result2[0] > 3.0 && result2[0] < 7.0);
    }

    #[test]
    fn test_save_fir_to_wav() {
        let coeffs: Vec<f64> = (0..256).map(|i| (i as f64 * 0.01).sin()).collect();
        let temp_dir = std::env::temp_dir();
        let wav_path = temp_dir.join("test_fir_design.wav");

        let result = save_fir_to_wav(&coeffs, 48000, &wav_path);
        assert!(result.is_ok());
        assert!(wav_path.exists());

        // Clean up
        let _ = std::fs::remove_file(&wav_path);
    }

    // Pre-ringing tests

    #[test]
    fn test_suppress_pre_ringing_basic() {
        // Create IR with pre-ringing: main tap at center, some energy before
        let mut ir = vec![0.0; 100];
        ir[50] = 1.0; // main tap
        ir[30] = 0.1; // pre-ringing: -20 dB relative to main
        ir[40] = 0.05; // pre-ringing: -26 dB relative to main

        let config = PreRingingConfig {
            threshold_db: -30.0,
            max_time_s: 0.01,
        };

        suppress_pre_ringing(&mut ir, &config, 48000.0);

        // Main tap should be unchanged
        assert_eq!(ir[50], 1.0);

        // Pre-ringing taps should be clamped
        let threshold_linear = 10.0_f64.powf(-30.0 / 20.0); // ≈ 0.0316
        assert!(
            ir[30].abs() <= threshold_linear + 1e-10,
            "tap 30 should be <= {:.4}, got {:.4}",
            threshold_linear,
            ir[30].abs()
        );
    }

    #[test]
    fn test_suppress_pre_ringing_time_limit() {
        let mut ir = vec![0.0; 1000];
        ir[500] = 1.0; // main tap
        ir[10] = 0.5; // far before main tap

        let config = PreRingingConfig {
            threshold_db: -30.0,
            max_time_s: 0.005, // 5 ms = 240 samples at 48 kHz
        };

        suppress_pre_ringing(&mut ir, &config, 48000.0);

        // Tap at index 10 is 490 samples before main tap (> 240 max)
        // Should be fully suppressed
        assert_eq!(ir[10], 0.0, "tap beyond time limit should be zeroed");
    }

    #[test]
    fn test_suppress_pre_ringing_no_effect_on_post_ringing() {
        let mut ir = vec![0.0; 100];
        ir[30] = 1.0; // main tap
        ir[60] = 0.5; // post-ringing (after main tap)

        let config = PreRingingConfig::default();
        let original_post = ir[60];

        suppress_pre_ringing(&mut ir, &config, 48000.0);

        // Post-ringing should be untouched
        assert_eq!(ir[60], original_post);
    }

    #[test]
    fn test_analyze_pre_ringing() {
        let mut ir = vec![0.0; 200];
        ir[100] = 1.0; // main tap
        ir[80] = 0.1; // -20 dB pre-ringing
        ir[90] = 0.01; // -40 dB pre-ringing

        let analysis = analyze_pre_ringing(&ir, 48000.0);

        assert_eq!(analysis.main_tap_index, 100);
        assert!(
            (analysis.peak_pre_ringing_db - (-20.0)).abs() < 0.5,
            "peak pre-ringing should be ~-20 dB, got {:.1}",
            analysis.peak_pre_ringing_db
        );
        assert!(analysis.pre_ringing_time_ms > 0.0);
    }

    #[test]
    fn test_suppress_pre_ringing_zero_max_time() {
        // Bug fix: max_time_s = 0 should suppress all pre-ringing (not divide by zero)
        let mut ir = vec![0.0; 100];
        ir[50] = 1.0;
        ir[40] = 0.1;

        let config = PreRingingConfig {
            threshold_db: -30.0,
            max_time_s: 0.0, // zero time limit
        };

        suppress_pre_ringing(&mut ir, &config, 48000.0);

        // All taps before main should be zeroed
        assert_eq!(
            ir[40], 0.0,
            "all pre-ringing should be suppressed with max_time=0"
        );
        assert_eq!(ir[50], 1.0, "main tap should be preserved");
    }

    #[test]
    fn test_analyze_pre_ringing_with_zero_taps() {
        // Bug fix: zero-valued taps before main tap should not produce NaN
        let mut ir = vec![0.0; 100];
        ir[50] = 1.0;
        // All other taps are 0.0

        let analysis = analyze_pre_ringing(&ir, 48000.0);

        assert_eq!(analysis.main_tap_index, 50);
        assert!(
            analysis.peak_pre_ringing_db.is_finite()
                || analysis.peak_pre_ringing_db == f64::NEG_INFINITY,
            "peak_pre_ringing_db should be finite or -inf, got {}",
            analysis.peak_pre_ringing_db
        );
    }

    #[test]
    fn test_pre_ringing_config_in_fir_design() {
        // Test that pre_ringing config flows through FirDesignConfig
        let freqs = vec![20.0, 100.0, 1000.0, 10000.0, 20000.0];
        let magnitude_db = vec![-3.0, 0.0, 2.0, 0.0, -3.0];

        let config_without = FirDesignConfig {
            n_taps: 512,
            sample_rate: 48000.0,
            phase: FirPhase::Linear,
            pre_ringing: None,
            ..Default::default()
        };

        let config_with = FirDesignConfig {
            pre_ringing: Some(PreRingingConfig::default()),
            ..config_without.clone()
        };

        let coeffs_without = generate_fir_from_response(&freqs, &magnitude_db, &config_without);
        let coeffs_with = generate_fir_from_response(&freqs, &magnitude_db, &config_with);

        // Both should produce valid filters
        assert_eq!(coeffs_without.len(), 512);
        assert_eq!(coeffs_with.len(), 512);

        // With pre-ringing suppression, energy before main tap should be reduced
        let analysis_without = analyze_pre_ringing(&coeffs_without, 48000.0);
        let analysis_with = analyze_pre_ringing(&coeffs_with, 48000.0);

        assert!(
            analysis_with.peak_pre_ringing_db <= analysis_without.peak_pre_ringing_db,
            "pre-ringing should be reduced: without={:.1} dB, with={:.1} dB",
            analysis_without.peak_pre_ringing_db,
            analysis_with.peak_pre_ringing_db
        );
    }

    #[test]
    fn test_suppress_pre_ringing_main_tap_at_zero() {
        // Main tap at index 0 — no pre-ringing possible
        let mut ir = vec![1.0, 0.5, 0.2, 0.1];
        let config = PreRingingConfig::default();
        let original = ir.clone();
        suppress_pre_ringing(&mut ir, &config, 48000.0);
        // Nothing should change since main tap is at 0
        assert_eq!(ir, original);
    }

    #[test]
    fn test_analyze_pre_ringing_main_tap_at_zero() {
        // Main tap at index 0 — pre-ringing should be -inf
        let ir = vec![1.0, 0.5, 0.1];
        let analysis = analyze_pre_ringing(&ir, 48000.0);
        assert_eq!(analysis.main_tap_index, 0);
        assert_eq!(analysis.peak_pre_ringing_db, f64::NEG_INFINITY);
        assert_eq!(analysis.pre_ringing_time_ms, 0.0);
    }

    #[test]
    fn test_suppress_pre_ringing_empty_ir() {
        let mut ir: Vec<f64> = vec![];
        let config = PreRingingConfig::default();
        suppress_pre_ringing(&mut ir, &config, 48000.0);
        assert!(ir.is_empty());
    }

    #[test]
    fn test_analyze_pre_ringing_empty_ir() {
        let ir: Vec<f64> = vec![];
        let analysis = analyze_pre_ringing(&ir, 48000.0);
        assert_eq!(analysis.main_tap_index, 0);
        assert_eq!(analysis.peak_pre_ringing_db, f64::NEG_INFINITY);
    }

    #[test]
    fn test_kirkeby_with_pre_ringing_config() {
        // Kirkeby should accept and use pre-ringing config
        let freqs = vec![20.0, 100.0, 500.0, 1000.0, 5000.0, 20000.0];
        let meas_db = vec![75.0, 82.0, 80.0, 78.0, 72.0, 65.0];
        let target_db = vec![80.0, 80.0, 80.0, 80.0, 80.0, 80.0];

        let config = FirDesignConfig {
            n_taps: 2048,
            sample_rate: 48000.0,
            phase: FirPhase::Kirkeby,
            min_freq: 20.0,
            max_freq: 1000.0,
            pre_ringing: Some(PreRingingConfig {
                threshold_db: -30.0,
                max_time_s: 0.003,
            }),
            ..Default::default()
        };

        let coeffs = generate_kirkeby_correction(&freqs, &meas_db, None, &target_db, &config);
        assert_eq!(coeffs.len(), 2048);

        // Verify pre-ringing is bounded
        let analysis = analyze_pre_ringing(&coeffs, 48000.0);
        assert!(
            analysis.peak_pre_ringing_db.is_finite()
                || analysis.peak_pre_ringing_db == f64::NEG_INFINITY
        );
    }
}
