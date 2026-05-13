// ============================================================================
// Instantaneous Frequency Estimator (Hilbert-based)
// ============================================================================
//
// Per-sample frequency estimation via the analytic signal (Hilbert transform).
// Building block for pitch tracking and modulation analysis.
//
// The analytic signal z(n) = x(n) + j*H{x(n)} where H{} is the Hilbert
// transform. The instantaneous frequency is the derivative of the
// instantaneous phase.

use crate::stft::RealFftProcessor;
use rustfft::num_complex::Complex;

/// Result of instantaneous frequency analysis.
#[derive(Debug, Clone)]
pub struct InstantaneousFrequencyResult {
    /// Instantaneous frequency per sample (Hz)
    pub frequencies: Vec<f32>,
    /// Instantaneous amplitude (envelope) per sample
    pub amplitudes: Vec<f32>,
}

/// Compute the analytic signal using the Hilbert transform via FFT.
///
/// The analytic signal has the property that its real part is the original
/// signal and its imaginary part is the Hilbert transform.
///
/// # Arguments
/// * `signal` - Input real-valued signal
///
/// # Returns
/// Complex analytic signal of the same length
pub fn analytic_signal(signal: &[f32]) -> Vec<Complex<f32>> {
    let n = signal.len();
    if n == 0 {
        return Vec::new();
    }

    // Use exact signal length — zero-padding to a different size
    // changes the circular convolution and produces incorrect results
    let fft_size = n;
    let spectrum_size = fft_size / 2 + 1;

    let mut fft = RealFftProcessor::new_bidirectional(fft_size);

    // Copy signal into time buffer (zero-padded)
    fft.time_buffer[..n].copy_from_slice(signal);
    fft.time_buffer[n..].fill(0.0);

    // Forward FFT
    fft.forward();

    // Build full complex spectrum for complex IFFT
    // We need to use a complex FFT for the inverse since the result is complex
    let mut full_spectrum = vec![Complex::new(0.0, 0.0); fft_size];

    // Apply analytic signal filter:
    // DC: keep as-is (multiply by 1)
    // Positive frequencies: multiply by 2
    // Nyquist: keep as-is (multiply by 1)
    // Negative frequencies: zero
    full_spectrum[0] = fft.freq_buffer[0];

    for (dst, &src) in full_spectrum[1..spectrum_size - 1]
        .iter_mut()
        .zip(&fft.freq_buffer[1..spectrum_size - 1])
    {
        *dst = src * 2.0;
    }

    // Nyquist bin exists only for even FFT sizes; for odd sizes, the last bin
    // is a positive-frequency bin and should be doubled
    if spectrum_size > 1 {
        if fft_size.is_multiple_of(2) {
            full_spectrum[spectrum_size - 1] = fft.freq_buffer[spectrum_size - 1];
        } else {
            full_spectrum[spectrum_size - 1] = fft.freq_buffer[spectrum_size - 1] * 2.0;
        }
    }

    // Negative frequencies are already zero

    // Inverse FFT using rustfft (complex-to-complex)
    let mut planner = rustfft::FftPlanner::new();
    let ifft = planner.plan_fft_inverse(fft_size);
    ifft.process(&mut full_spectrum);

    // Normalize and truncate to original length
    let scale = 1.0 / fft_size as f32;
    full_spectrum.iter().take(n).map(|c| c * scale).collect()
}

/// Compute instantaneous frequency from a signal.
///
/// # Arguments
/// * `signal` - Input signal samples
/// * `sample_rate` - Sample rate in Hz
///
/// # Returns
/// `InstantaneousFrequencyResult` with per-sample frequencies and amplitudes
pub fn instantaneous_frequency(signal: &[f32], sample_rate: f32) -> InstantaneousFrequencyResult {
    let n = signal.len();
    if n < 2 {
        return InstantaneousFrequencyResult {
            frequencies: vec![0.0; n],
            amplitudes: vec![0.0; n],
        };
    }

    let analytic = analytic_signal(signal);

    // Compute instantaneous phase and unwrap
    let phases: Vec<f32> = analytic.iter().map(|z| z.im.atan2(z.re)).collect();

    // Unwrap phase
    let unwrapped = unwrap_phase(&phases);

    // Instantaneous frequency = sample_rate / (2*pi) * d(phase)/dt
    let factor = sample_rate / (2.0 * std::f32::consts::PI);
    let mut frequencies = Vec::with_capacity(n);
    // First sample uses forward difference (same as second sample)
    // to avoid the undefined backward difference at index 0
    if n >= 2 {
        let df0 = (unwrapped[1] - unwrapped[0]) * factor;
        frequencies.push(df0);
    } else {
        frequencies.push(0.0);
    }
    for i in 1..n {
        let df = (unwrapped[i] - unwrapped[i - 1]) * factor;
        frequencies.push(df);
    }

    // Instantaneous amplitude = |z(n)|
    let amplitudes: Vec<f32> = analytic
        .iter()
        .map(|z| (z.re * z.re + z.im * z.im).sqrt())
        .collect();

    InstantaneousFrequencyResult {
        frequencies,
        amplitudes,
    }
}

/// Unwrap phase to remove 2*pi discontinuities.
fn unwrap_phase(phases: &[f32]) -> Vec<f32> {
    let mut unwrapped = Vec::with_capacity(phases.len());
    if phases.is_empty() {
        return unwrapped;
    }

    unwrapped.push(phases[0]);
    let two_pi = 2.0 * std::f32::consts::PI;

    for i in 1..phases.len() {
        let diff = phases[i] - phases[i - 1];
        // Issue #2: use rem_euclid instead of `%` for robust wrap-to-pi.
        let wrapped = diff.rem_euclid(two_pi);
        let diff = if wrapped > std::f32::consts::PI {
            wrapped - two_pi
        } else {
            wrapped
        };
        unwrapped.push(unwrapped[i - 1] + diff);
    }

    unwrapped
}

/// Compute subband instantaneous frequency for multi-component signals.
///
/// Splits the signal into frequency bands using bandpass filters, computes
/// IF per band, then combines with amplitude weighting.
///
/// # Arguments
/// * `signal` - Input signal samples
/// * `sample_rate` - Sample rate in Hz
/// * `num_bands` - Number of frequency bands (default: 8)
///
/// # Returns
/// Amplitude-weighted instantaneous frequency estimate per sample
pub fn subband_instantaneous_frequency(
    signal: &[f32],
    sample_rate: f32,
    num_bands: Option<usize>,
) -> Vec<f32> {
    let n = signal.len();
    if n < 2 {
        return vec![0.0; n];
    }

    let bands = num_bands.unwrap_or(8).max(2);

    // Log-spaced band edges from 20 Hz to Nyquist
    let nyquist = sample_rate / 2.0;
    let log_min = 20.0_f32.ln();
    let log_max = nyquist.ln();

    let mut weighted_freq = vec![0.0f32; n];
    let mut total_weight = vec![0.0f32; n];

    for b in 0..bands {
        let f_low = ((log_min + (log_max - log_min) * b as f32 / bands as f32).exp()).max(20.0);
        let f_high = ((log_min + (log_max - log_min) * (b + 1) as f32 / bands as f32).exp())
            .min(nyquist * 0.95);

        if f_low >= f_high {
            continue;
        }

        let f_center = (f_low * f_high).sqrt();
        let q = f_center / (f_high - f_low);

        // Apply bandpass filter using biquad
        let filtered = apply_bandpass(signal, sample_rate, f_center, q);

        // Compute IF for this subband
        let if_result = instantaneous_frequency(&filtered, sample_rate);

        // Amplitude-weighted accumulation
        for i in 0..n {
            let w = if_result.amplitudes[i];
            weighted_freq[i] += if_result.frequencies[i] * w;
            total_weight[i] += w;
        }
    }

    // Normalize
    for i in 0..n {
        if total_weight[i] > 1e-10 {
            weighted_freq[i] /= total_weight[i];
        }
    }

    weighted_freq
}

/// Apply a 2nd-order bandpass biquad filter.
fn apply_bandpass(signal: &[f32], sample_rate: f32, center_freq: f32, q: f32) -> Vec<f32> {
    let omega = 2.0 * std::f32::consts::PI * center_freq / sample_rate;
    let sin_omega = omega.sin();
    let cos_omega = omega.cos();
    let alpha = sin_omega / (2.0 * q);

    let b0 = alpha;
    let b1 = 0.0;
    let b2 = -alpha;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos_omega;
    let a2 = 1.0 - alpha;

    // Normalize
    let b0 = b0 / a0;
    let b1 = b1 / a0;
    let b2 = b2 / a0;
    let a1 = a1 / a0;
    let a2 = a2 / a0;

    let mut output = Vec::with_capacity(signal.len());
    let mut x1 = 0.0f32;
    let mut x2 = 0.0f32;
    let mut y1 = 0.0f32;
    let mut y2 = 0.0f32;

    for &x in signal {
        let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        output.push(y);
        x2 = x1;
        x1 = x;
        y2 = y1;
        y1 = y;
    }

    output
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn gen_tone(freq: f32, sample_rate: f32, num_samples: usize) -> Vec<f32> {
        (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate;
                (2.0 * std::f32::consts::PI * freq * t).sin()
            })
            .collect()
    }

    #[test]
    fn test_analytic_signal_length() {
        let signal = gen_tone(1000.0, 48000.0, 1024);
        let analytic = analytic_signal(&signal);
        assert_eq!(analytic.len(), signal.len());
    }

    #[test]
    fn test_analytic_signal_real_part_matches() {
        let signal = gen_tone(1000.0, 48000.0, 1024);
        let analytic = analytic_signal(&signal);

        // Real part of analytic signal should match original (approximately)
        for i in 10..1014 {
            // Skip edges
            assert!(
                (analytic[i].re - signal[i]).abs() < 0.05,
                "Real part mismatch at i={}: {} vs {}",
                i,
                analytic[i].re,
                signal[i]
            );
        }
    }

    #[test]
    fn test_pure_tone_if_constant() {
        let freq = 1000.0;
        let sample_rate = 48000.0;
        let signal = gen_tone(freq, sample_rate, 4096);

        let result = instantaneous_frequency(&signal, sample_rate);

        // IF should be approximately constant at 1000 Hz
        // Skip transient edges
        let mid_freqs = &result.frequencies[100..3900];
        let avg_freq: f32 = mid_freqs.iter().sum::<f32>() / mid_freqs.len() as f32;

        assert!(
            (avg_freq - freq).abs() < 2.0,
            "Average IF should be ~{freq} Hz, got {avg_freq:.2}"
        );

        // Check stability (low variance) — some ripple is expected from FFT edge effects
        let variance: f32 = mid_freqs
            .iter()
            .map(|f| (f - avg_freq).powi(2))
            .sum::<f32>()
            / mid_freqs.len() as f32;
        assert!(
            variance < 100.0,
            "IF variance should be small for pure tone, got {variance:.2}"
        );
    }

    #[test]
    fn test_linear_chirp_tracking() {
        let sample_rate = 48000.0;
        let num_samples = 8192;
        let f_start = 500.0;
        let f_end = 2000.0;

        // Generate linear chirp
        let signal: Vec<f32> = (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate;
                let duration = num_samples as f32 / sample_rate;
                let _inst_freq = f_start + (f_end - f_start) * t / duration;
                let phase = 2.0
                    * std::f32::consts::PI
                    * (f_start * t + (f_end - f_start) * t * t / (2.0 * duration));
                phase.sin()
            })
            .collect();

        let result = instantaneous_frequency(&signal, sample_rate);

        // Check that IF tracks the chirp at midpoint
        let mid = num_samples / 2;
        let expected_mid_freq = (f_start + f_end) / 2.0;
        let measured_mid_freq = result.frequencies[mid];

        assert!(
            (measured_mid_freq - expected_mid_freq).abs() < 100.0,
            "IF at midpoint should be ~{expected_mid_freq} Hz, got {measured_mid_freq:.2}"
        );
    }

    #[test]
    fn test_am_signal_if_near_carrier() {
        let sample_rate = 48000.0;
        let num_samples = 4096;
        let carrier_freq = 1000.0;
        let mod_freq = 100.0;

        // AM signal: (1 + 0.5*sin(2*pi*fm*t)) * sin(2*pi*fc*t)
        let signal: Vec<f32> = (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate;
                let envelope = 1.0 + 0.5 * (2.0 * std::f32::consts::PI * mod_freq * t).sin();
                envelope * (2.0 * std::f32::consts::PI * carrier_freq * t).sin()
            })
            .collect();

        let result = instantaneous_frequency(&signal, sample_rate);

        // Average IF should be close to carrier frequency
        let mid_freqs = &result.frequencies[200..3800];
        let avg_freq: f32 = mid_freqs.iter().sum::<f32>() / mid_freqs.len() as f32;

        assert!(
            (avg_freq - carrier_freq).abs() < 50.0,
            "Average IF of AM signal should be near carrier ({carrier_freq}), got {avg_freq:.2}"
        );
    }

    #[test]
    fn test_subband_if_two_tones() {
        let sample_rate = 48000.0;
        let num_samples = 4096;

        // Two-tone signal
        let signal: Vec<f32> = (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate;
                (2.0 * std::f32::consts::PI * 500.0 * t).sin()
                    + (2.0 * std::f32::consts::PI * 3000.0 * t).sin()
            })
            .collect();

        let result = subband_instantaneous_frequency(&signal, sample_rate, Some(8));
        assert_eq!(result.len(), num_samples);

        // The weighted average should be somewhere between 500 and 3000
        let mid_vals = &result[200..3800];
        let avg: f32 = mid_vals.iter().sum::<f32>() / mid_vals.len() as f32;
        assert!(
            avg > 400.0 && avg < 3500.0,
            "Weighted IF should be between tones, got {avg:.2}"
        );
    }

    #[test]
    fn test_analytic_signal_non_power_of_two() {
        // Regression: analytic_signal must work correctly for non-power-of-2 lengths
        let signal = gen_tone(1000.0, 48000.0, 1000); // 1000 is not a power of 2
        let analytic = analytic_signal(&signal);
        assert_eq!(analytic.len(), 1000);

        // Real part should still match original (within tolerance)
        for i in 10..990 {
            assert!(
                (analytic[i].re - signal[i]).abs() < 0.1,
                "Non-power-of-2: real part mismatch at i={i}: {} vs {}",
                analytic[i].re,
                signal[i]
            );
        }
    }

    #[test]
    fn test_if_first_sample_equals_second() {
        // Regression: first sample should use forward difference (same as second),
        // not be hard-coded to 0.0
        let signal = gen_tone(1000.0, 48000.0, 4096);
        let result = instantaneous_frequency(&signal, 48000.0);

        // First sample should equal second sample (forward difference),
        // not be forced to 0.0 as in the old code
        assert!(
            (result.frequencies[0] - result.frequencies[1]).abs() < 1e-6,
            "First sample IF ({:.2}) should equal second ({:.2})",
            result.frequencies[0],
            result.frequencies[1]
        );
    }

    #[test]
    fn test_empty_signal() {
        let result = instantaneous_frequency(&[], 48000.0);
        assert!(result.frequencies.is_empty());
        assert!(result.amplitudes.is_empty());
    }

    #[test]
    fn test_single_sample() {
        let result = instantaneous_frequency(&[1.0], 48000.0);
        assert_eq!(result.frequencies.len(), 1);
        assert_eq!(result.amplitudes.len(), 1);
    }

    #[test]
    fn test_unwrap_phase_negative_diff() {
        // Issue #2: `%` on f32 gives wrong results for negative operands.
        // A phase jump of -3π/2 should unwrap to +π/2 (a +2π correction).
        let phases = vec![0.0f32, -4.712389f32]; // 0, -3π/2
        let unwrapped = unwrap_phase(&phases);
        // The unwrapped second value should be close to +π/2 (~1.5708).
        assert!(
            (unwrapped[1] - std::f32::consts::FRAC_PI_2).abs() < 0.01,
            "Expected ~π/2 for -3π/2 unwrapped, got {}",
            unwrapped[1]
        );
    }
}
