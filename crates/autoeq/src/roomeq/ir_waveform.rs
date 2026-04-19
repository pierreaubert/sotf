//! Pre- and post-correction impulse response waveform computation.
//!
//! Converts frequency-domain measurements (log-spaced SPL + phase) into
//! time-domain impulse responses for visualization purposes.

use super::types::IrWaveform;
use num_complex::Complex64;
use rustfft::FftPlanner;
use std::f64::consts::PI;

const FFT_SIZE: usize = 65536;
const CROP_MS: f64 = 400.0;

/// Compute pre- and post-correction IR waveforms for one channel.
///
/// Returns `None` if phase data is absent in `initial_curve`.
pub fn compute_channel_ir_waveforms(
    initial_curve: &crate::Curve,
    biquads: &[crate::iir::Biquad],
    fir_coeffs: Option<&[f64]>,
    delay_ms: f64,
    sample_rate: f64,
) -> Option<(IrWaveform, IrWaveform)> {
    // Guard: phase data is required
    let phase_deg = initial_curve.phase.as_ref()?;

    let n_bins = FFT_SIZE / 2 + 1;
    let linear_freqs: Vec<f64> = (0..n_bins)
        .map(|k| k as f64 * sample_rate / FFT_SIZE as f64)
        .collect();

    // Interpolate SPL and unwrapped phase from log-spaced measurement to linear grid
    let unwrapped_phase = super::phase_utils::unwrap_phase_degrees(phase_deg);
    let freq_vec: Vec<f64> = initial_curve.freq.to_vec();
    let spl_grid =
        interpolate_to_linear_grid(&freq_vec, &initial_curve.spl.to_vec(), &linear_freqs);
    let phase_grid =
        interpolate_to_linear_grid(&freq_vec, &unwrapped_phase.to_vec(), &linear_freqs);

    // Build pre-IR complex spectrum
    let mut pre_spectrum: Vec<Complex64> = (0..n_bins)
        .map(|k| {
            let mag = 10f64.powf(spl_grid[k] / 20.0);
            let phase_rad = phase_grid[k].to_radians();
            Complex64::from_polar(mag, phase_rad)
        })
        .collect();

    // Force DC and Nyquist to be real
    pre_spectrum[0] = Complex64::new(pre_spectrum[0].re.abs(), 0.0);
    pre_spectrum[n_bins - 1] = Complex64::new(pre_spectrum[n_bins - 1].re, 0.0);

    let pre_ir_raw = spectrum_to_impulse_response(&pre_spectrum, FFT_SIZE);

    // Build post-IR spectrum: pre * biquad chain * fir * delay
    let freqs_arr = ndarray::Array1::from(linear_freqs.clone());
    let peq_response =
        crate::response::compute_peq_complex_response(biquads, &freqs_arr, sample_rate);
    let mut post_spectrum: Vec<Complex64> = pre_spectrum
        .iter()
        .zip(peq_response.iter())
        .map(|(&h_pre, &h_eq)| h_pre * h_eq)
        .collect();

    if let Some(coeffs) = fir_coeffs {
        let fir_response =
            crate::response::compute_fir_complex_response(coeffs, &freqs_arr, sample_rate);
        for (h, h_fir) in post_spectrum.iter_mut().zip(fir_response.iter()) {
            *h *= h_fir;
        }
    }

    // Apply delay: e^(-j * 2π * f * delay_s)
    let delay_s = delay_ms / 1000.0;
    for (k, h) in post_spectrum.iter_mut().enumerate() {
        let f = linear_freqs[k];
        let angle = -2.0 * PI * f * delay_s;
        *h *= Complex64::from_polar(1.0, angle);
    }

    let post_ir_raw = spectrum_to_impulse_response(&post_spectrum, FFT_SIZE);

    // Normalize both by pre-IR peak
    let pre_peak = pre_ir_raw.iter().map(|&x| x.abs()).fold(0.0_f64, f64::max);

    if pre_peak < 1e-12 {
        return None;
    }
    let scale = 1.0 / pre_peak;

    // Crop
    let crop_samples = ((CROP_MS / 1000.0) * sample_rate) as usize;
    let n_out = FFT_SIZE.min(crop_samples);

    let time_ms: Vec<f64> = (0..n_out)
        .map(|n| n as f64 * 1000.0 / sample_rate)
        .collect();

    let pre_amplitude: Vec<f64> = pre_ir_raw[..n_out].iter().map(|&x| x * scale).collect();
    let post_amplitude: Vec<f64> = post_ir_raw[..n_out].iter().map(|&x| x * scale).collect();

    Some((
        IrWaveform {
            time_ms: time_ms.clone(),
            amplitude: pre_amplitude,
        },
        IrWaveform {
            time_ms,
            amplitude: post_amplitude,
        },
    ))
}

/// Interpolate values from log-spaced measurement points to a linear frequency grid.
///
/// Uses linear interpolation in log-frequency space with boundary extrapolation.
fn interpolate_to_linear_grid(
    meas_freq: &[f64],
    meas_values: &[f64],
    linear_freqs: &[f64],
) -> Vec<f64> {
    let n = meas_freq.len();
    assert!(n >= 2, "measurement must have at least 2 points");

    let log_freq: Vec<f64> = meas_freq.iter().map(|&f| f.max(1e-6).log10()).collect();

    linear_freqs
        .iter()
        .map(|&f| {
            // Skip DC (f == 0): extrapolate from first measurement
            if f < meas_freq[0] {
                return meas_values[0];
            }
            if f >= meas_freq[n - 1] {
                return meas_values[n - 1];
            }

            let log_f = f.max(1e-6).log10();

            // Binary search for bracketing interval
            let idx = log_freq
                .partition_point(|&lf| lf <= log_f)
                .saturating_sub(1);
            let i0 = idx.min(n - 2);
            let i1 = i0 + 1;

            let t = (log_f - log_freq[i0]) / (log_freq[i1] - log_freq[i0]);
            meas_values[i0] + t * (meas_values[i1] - meas_values[i0])
        })
        .collect()
}

/// Convert a one-sided complex spectrum (DC … Nyquist) to a real impulse response
/// via inverse FFT.
fn spectrum_to_impulse_response(one_sided: &[Complex64], fft_size: usize) -> Vec<f64> {
    // Reconstruct the full two-sided spectrum with Hermitian symmetry
    let mut full: Vec<Complex64> = Vec::with_capacity(fft_size);
    full.extend_from_slice(one_sided); // bins 0 … N/2
    // Mirror: bins N/2+1 … N-1 = conjugate of bins N/2-1 … 1
    for k in (1..one_sided.len() - 1).rev() {
        full.push(one_sided[k].conj());
    }

    let mut planner = FftPlanner::new();
    let ifft = planner.plan_fft_inverse(fft_size);
    ifft.process(&mut full);

    let norm = fft_size as f64;
    full.iter().map(|c| c.re / norm).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;
    use std::f64::consts::PI;

    /// Build a synthetic Curve with linear-phase response (pure delay τ).
    fn make_delay_curve(tau_ms: f64, freqs: &[f64]) -> crate::Curve {
        let tau_s = tau_ms / 1000.0;
        let spl: Vec<f64> = vec![0.0; freqs.len()]; // flat magnitude
        let phase: Vec<f64> = freqs
            .iter()
            .map(|&f| (-2.0 * PI * f * tau_s).to_degrees())
            .collect();
        crate::Curve {
            freq: Array1::from(freqs.to_vec()),
            spl: Array1::from(spl),
            phase: Some(Array1::from(phase)),
            ..Default::default()
        }
    }

    #[test]
    fn test_pre_ir_peaks_at_delay() {
        // Build a 200-point log-spaced measurement with 0.5 ms pure delay.
        // At 48 kHz with 200 log-spaced points (20–20 kHz), the maximum phase step between
        // consecutive samples is ~123° (at 20 kHz), safely below the 180° unwrap threshold.
        let tau_ms = 0.5;
        let freqs: Vec<f64> = (0..200)
            .map(|i| 20.0 * (1000.0f64).powf(i as f64 / 199.0))
            .collect();
        let curve = make_delay_curve(tau_ms, &freqs);

        let sample_rate = 48000.0;
        let result = compute_channel_ir_waveforms(&curve, &[], None, 0.0, sample_rate);
        assert!(
            result.is_some(),
            "should return Some when phase data present"
        );

        let (pre_ir, _) = result.unwrap();

        // Find peak sample
        let (peak_idx, _) = pre_ir
            .amplitude
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap())
            .unwrap();

        let peak_ms = pre_ir.time_ms[peak_idx];
        // The peak should be within 1 ms of tau_ms
        assert!(
            (peak_ms - tau_ms).abs() < 1.0,
            "pre-IR peak expected near {tau_ms} ms, got {peak_ms:.2} ms"
        );

        // Amplitude at peak should be 1.0 (normalized)
        assert!(
            (pre_ir.amplitude[peak_idx] - 1.0).abs() < 0.05,
            "pre-IR peak amplitude should be ~1.0, got {}",
            pre_ir.amplitude[peak_idx]
        );
    }

    #[test]
    fn test_post_ir_peak_matches_pre_when_no_correction() {
        // With identity biquad list and zero delay, post-IR peak should match pre-IR peak.
        // Use 0.5 ms delay so phase steps stay below the 180° unwrap threshold.
        let tau_ms = 0.5;
        let freqs: Vec<f64> = (0..200)
            .map(|i| 20.0 * (1000.0f64).powf(i as f64 / 199.0))
            .collect();
        let curve = make_delay_curve(tau_ms, &freqs);

        let sample_rate = 48000.0;
        let (pre_ir, post_ir) =
            compute_channel_ir_waveforms(&curve, &[], None, 0.0, sample_rate).unwrap();

        let find_peak_idx = |ir: &IrWaveform| {
            ir.amplitude
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap())
                .map(|(i, _)| i)
                .unwrap()
        };

        let pre_peak_idx = find_peak_idx(&pre_ir);
        let post_peak_idx = find_peak_idx(&post_ir);

        assert_eq!(
            pre_peak_idx, post_peak_idx,
            "pre and post IR peaks should coincide with identity correction"
        );
    }

    #[test]
    fn test_returns_none_without_phase() {
        let freqs: Vec<f64> = (0..200)
            .map(|i| 20.0 * (1000.0f64).powf(i as f64 / 199.0))
            .collect();
        let curve = crate::Curve {
            freq: Array1::from(freqs),
            spl: Array1::zeros(200),
            phase: None, // no phase
            ..Default::default()
        };

        let result = compute_channel_ir_waveforms(&curve, &[], None, 0.0, 48000.0);
        assert!(
            result.is_none(),
            "should return None when phase data absent"
        );
    }
}
