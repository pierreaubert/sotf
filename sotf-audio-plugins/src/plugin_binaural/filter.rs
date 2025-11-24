use rustfft::num_complex::Complex;
use rustfft::Fft;
use std::sync::Arc;
use crate::sofa::SofaFile;

/// Convert impulse response to frequency domain
pub fn ir_to_freq(
    ir: &[f32],
    fft_size: usize,
    fft_forward: &Arc<dyn Fft<f32>>,
) -> Vec<Complex<f32>> {
    let mut buffer = vec![Complex::new(0.0, 0.0); fft_size];

    // Copy IR data (pad with zeros if IR is shorter, truncate if longer)
    // Use full fft_size to preserve spatial information (low-frequency cues are in the tail)
    let copy_len = ir.len().min(fft_size);
    let mut max_val = 0.0f32;
    for i in 0..copy_len {
        buffer[i] = Complex::new(ir[i], 0.0);
        max_val = max_val.max(ir[i].abs());
    }

    if max_val > 0.9 {
        log::warn!(
            "[BinauralDecoder] HRTF IR peak is very high: {:.4} (near 0dBFS). This might cause clipping.",
            max_val
        );
    } else {
        log::debug!("[BinauralDecoder] HRTF IR peak: {:.4}", max_val);
    }

    // FFT
    let mut freq = buffer.clone();
    fft_forward.process(&mut freq);

    freq
}

/// Compute diffuse-field equalization filter
///
/// Calculates the average frequency response over all directions (diffuse field)
/// and creates an inverse filter to compensate for HRTF coloration.
///
/// This improves timbre neutrality by removing the "average" spectral signature
/// of the HRTF set, while preserving the spatial cues (ITD/ILD variations).
///
/// Reference: Schörkhuber et al., "Linearly and Quadratically Constrained Least-Squares
/// Decoder for Signal-Dependent Binaural Rendering" (2018)
pub fn compute_diffuse_field_eq(
    sofa: &SofaFile,
    fft_size: usize,
    sample_rate: u32,
    fft_forward: &Arc<dyn Fft<f32>>,
) -> Result<[Vec<Complex<f32>>; 2], String> {
    log::info!("[BinauralDecoder] Computing diffuse-field equalization...");

    // Accumulate magnitude-squared responses for all measurements
    let mut left_power = vec![0.0f32; fft_size];
    let mut right_power = vec![0.0f32; fft_size];

    for m in 0..sofa.num_measurements {
        if let Some(hrtf) = sofa.get_hrtf(m) {
            // Convert IRs to frequency domain
            let left_fft = ir_to_freq(&hrtf.ir_left, fft_size, fft_forward);
            let right_fft = ir_to_freq(&hrtf.ir_right, fft_size, fft_forward);

            // Accumulate power (magnitude squared)
            for k in 0..fft_size {
                left_power[k] += left_fft[k].norm_sqr();
                right_power[k] += right_fft[k].norm_sqr();
            }
        }
    }

    // Average the power spectra
    let num_measurements = sofa.num_measurements as f32;
    for k in 0..fft_size {
        left_power[k] /= num_measurements;
        right_power[k] /= num_measurements;
    }

    // Compute inverse filter (1 / sqrt(power)) with regularization
    // Regularization prevents excessive boost at frequencies with very low energy
    let regularization = 0.001; // -60 dB
    let mut left_eq = vec![Complex::new(0.0, 0.0); fft_size];
    let mut right_eq = vec![Complex::new(0.0, 0.0); fft_size];

    for k in 0..fft_size {
        // Compute magnitude of inverse filter with regularization
        let left_mag_inv = 1.0 / (left_power[k] + regularization).sqrt();
        let right_mag_inv = 1.0 / (right_power[k] + regularization).sqrt();

        // Limit maximum boost to +12 dB for stability
        let max_boost = 10.0_f32.powf(12.0 / 20.0); // ~4.0
        let left_gain = left_mag_inv.min(max_boost);
        let right_gain = right_mag_inv.min(max_boost);

        // Zero phase filter (real-valued, symmetric)
        left_eq[k] = Complex::new(left_gain, 0.0);
        right_eq[k] = Complex::new(right_gain, 0.0);
    }

    // Normalize to unity gain at 1 kHz for perceptually neutral response
    let freq_1khz = (1000.0 * fft_size as f32 / sample_rate as f32) as usize;
    let left_ref = left_eq[freq_1khz].norm().max(0.001);
    let right_ref = right_eq[freq_1khz].norm().max(0.001);

    for k in 0..fft_size {
        left_eq[k] /= left_ref;
        right_eq[k] /= right_ref;
    }

    log::info!("[BinauralDecoder] Diffuse-field equalization computed (normalized to 1 kHz)");
    Ok([left_eq, right_eq])
}

/// Compute LFE low-pass filter and gain
///
/// Creates a Butterworth low-pass filter for band-limiting LFE to subwoofer range
/// and calculates distance-dependent attenuation plus level adjustment.
///
/// Reference: ITU-R BS.775-3 (multichannel stereophonic sound system with surround channels)
pub fn compute_lfe_filter(
    fft_size: usize,
    sample_rate: u32,
    lfe_crossover: f32,
    lfe_distance: f32,
    lfe_level: f32,
) -> (Vec<Complex<f32>>, f32) {
    // Compute 2nd-order Butterworth low-pass filter (12 dB/octave rolloff)
    // This is typical for LFE/subwoofer crossover
    let fc = lfe_crossover; // Cutoff frequency in Hz
    let fs = sample_rate as f32;

    // Pre-warp frequency for bilinear transform
    // Use standard bilinear transform: k = tan(π * fc / fs)
    let k = (std::f32::consts::PI * fc / fs).tan();
    let k_sq = k * k;

    // Butterworth coefficients (s-domain): H(s) = 1 / (s^2 + sqrt(2)*s + 1)
    // After bilinear transform to z-domain
    let a0 = 1.0 + std::f32::consts::SQRT_2 * k + k_sq;
    let b0 = k_sq / a0;
    let b1 = 2.0 * k_sq / a0;
    let b2 = k_sq / a0;
    let a1 = (2.0 * k_sq - 2.0) / a0;
    let a2 = (1.0 - std::f32::consts::SQRT_2 * k + k_sq) / a0;

    let mut lfe_lowpass_filter = vec![Complex::new(0.0, 0.0); fft_size];

    // Convert to frequency domain response
    for k in 0..fft_size {
        let freq = k as f32 * fs / fft_size as f32;
        let omega = 2.0 * std::f32::consts::PI * freq / fs;

        // Z-transform evaluation: H(z) at z = e^(jω)
        let cos_w = omega.cos();
        let sin_w = omega.sin();
        let cos_2w = (2.0 * omega).cos();
        let sin_2w = (2.0 * omega).sin();

        // Numerator: b0 + b1*z^-1 + b2*z^-2
        let num_re = b0 + b1 * cos_w + b2 * cos_2w;
        let num_im = -(b1 * sin_w + b2 * sin_2w);

        // Denominator: 1 + a1*z^-1 + a2*z^-2
        let den_re = 1.0 + a1 * cos_w + a2 * cos_2w;
        let den_im = -(a1 * sin_w + a2 * sin_2w);

        // Complex division: (num_re + j*num_im) / (den_re + j*den_im)
        let denom = den_re * den_re + den_im * den_im;
        let h_re = (num_re * den_re + num_im * den_im) / denom;
        let h_im = (num_im * den_re - num_re * den_im) / denom;

        lfe_lowpass_filter[k] = Complex::new(h_re, h_im);
    }

    // Compute LFE gain: distance attenuation + level adjustment
    // Distance attenuation: 1/r law with reference distance of 1m
    let distance_atten = 1.0 / lfe_distance.max(0.1);

    // Level adjustment from dB
    let level_gain = 10.0_f32.powf(lfe_level / 20.0);

    // Combined gain (also include -3dB for dual-mono mixing)
    let lfe_gain = distance_atten * level_gain * std::f32::consts::FRAC_1_SQRT_2;

    log::info!(
        "[BinauralDecoder] LFE filter: fc={}Hz, distance={}m ({:.2}dB atten), level={:.1}dB, total_gain={:.3}",
        fc,
        lfe_distance,
        -20.0 * distance_atten.log10(),
        lfe_level,
        lfe_gain
    );

    (lfe_lowpass_filter, lfe_gain)
}
