use realfft::RealToComplex;
use rustfft::num_complex::Complex;
use sotf_host::sofa::SofaFile;
use std::sync::Arc;

/// Convert impulse response to frequency domain using real FFT
///
/// Returns N/2+1 complex frequency bins (half-spectrum representation)
/// for efficiency since input IR is real-valued.
pub fn ir_to_freq(
    ir: &[f32],
    fft_size: usize,
    fft_r2c: &Arc<dyn RealToComplex<f32>>,
) -> Vec<Complex<f32>> {
    let freq_size = fft_size / 2 + 1;

    // Prepare time-domain buffer (zero-padded)
    let mut time_buffer = vec![0.0f32; fft_size];

    // Callers validate that the complete IR fits the linear overlap-add
    // partition capacity. Never silently truncate a spatial filter.
    assert!(ir.len() <= fft_size, "IR exceeds FFT size");
    let copy_len = ir.len();
    let mut max_val = 0.0f32;
    for i in 0..copy_len {
        time_buffer[i] = ir[i];
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

    // Real FFT: N real -> N/2+1 complex
    let mut freq = vec![Complex::new(0.0, 0.0); freq_size];
    fft_r2c
        .process(&mut time_buffer, &mut freq)
        .expect("FFT forward failed in ir_to_freq");

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
/// Returns N/2+1 complex bins per ear (half-spectrum representation for real signals).
///
/// Reference: Schörkhuber et al., "Linearly and Quadratically Constrained Least-Squares
/// Decoder for Signal-Dependent Binaural Rendering" (2018)
pub fn compute_diffuse_field_eq(
    sofa: &SofaFile,
    fft_size: usize,
    sample_rate: u32,
    fft_r2c: &Arc<dyn RealToComplex<f32>>,
) -> Result<[Vec<Complex<f32>>; 2], String> {
    log::info!("[BinauralDecoder] Computing diffuse-field equalization...");

    let freq_size = fft_size / 2 + 1;

    if sofa.num_measurements == 0 {
        return Err("SOFA dataset contains no HRTF measurements".to_string());
    }
    if sofa.ir_length == 0 {
        return Err("SOFA dataset contains zero-length HRTFs".to_string());
    }
    let expected_samples = sofa
        .num_measurements
        .checked_mul(2)
        .and_then(|count| count.checked_mul(sofa.ir_length))
        .ok_or_else(|| "SOFA HRTF dimensions overflow".to_string())?;
    if sofa.impulse_responses.len() != expected_samples
        || sofa.positions.len() != sofa.num_measurements
    {
        return Err(format!(
            "inconsistent SOFA dimensions: {} measurements, {} positions, {} HRTF samples (expected {})",
            sofa.num_measurements,
            sofa.positions.len(),
            sofa.impulse_responses.len(),
            expected_samples
        ));
    }

    // Accumulate magnitude-squared responses for all valid measurements
    let mut left_power = vec![0.0f32; freq_size];
    let mut right_power = vec![0.0f32; freq_size];

    let mut valid_measurements = 0usize;
    for m in 0..sofa.num_measurements {
        if let Some((_, left, right)) = sofa.get_hrtf_slices(m) {
            if left.iter().chain(right).any(|sample| !sample.is_finite()) {
                continue;
            }
            // Convert IRs to frequency domain (returns freq_size bins)
            let left_fft = ir_to_freq(left, fft_size, fft_r2c);
            let right_fft = ir_to_freq(right, fft_size, fft_r2c);

            // Accumulate power (magnitude squared)
            for k in 0..freq_size {
                left_power[k] += left_fft[k].norm_sqr();
                right_power[k] += right_fft[k].norm_sqr();
            }
            valid_measurements += 1;
        }
    }

    if valid_measurements == 0 {
        return Err("SOFA dataset contains no finite HRTF measurements".to_string());
    }

    // Average the power spectra
    let num_measurements = valid_measurements as f32;
    for k in 0..freq_size {
        left_power[k] /= num_measurements;
        right_power[k] /= num_measurements;
    }

    // Smooth on a logarithmic-frequency neighbourhood before inversion. Raw
    // per-bin inversion overfits narrow SOFA notches and creates ringing.
    left_power = smooth_power_log_frequency(&left_power);
    right_power = smooth_power_log_frequency(&right_power);

    // Compute inverse filter (1 / sqrt(power)) with level-relative regularization
    // Regularization prevents excessive boost at frequencies with very low energy
    let mean_power =
        left_power.iter().chain(&right_power).copied().sum::<f32>() / (freq_size * 2) as f32;
    let regularization = (mean_power * 1.0e-4).max(f32::MIN_POSITIVE);
    let mut left_eq = vec![Complex::new(0.0, 0.0); freq_size];
    let mut right_eq = vec![Complex::new(0.0, 0.0); freq_size];

    for k in 0..freq_size {
        // Compute magnitude of inverse filter with regularization
        let left_mag_inv = 1.0 / (left_power[k] + regularization).sqrt();
        let right_mag_inv = 1.0 / (right_power[k] + regularization).sqrt();

        // Zero phase filter (real-valued, symmetric)
        left_eq[k] = Complex::new(left_mag_inv, 0.0);
        right_eq[k] = Complex::new(right_mag_inv, 0.0);
    }

    // Normalize to unity gain at 1 kHz for perceptually neutral response
    // freq_size bins cover 0 to Nyquist, so bin index = freq * fft_size / sample_rate
    let freq_1khz = (1000.0 * fft_size as f32 / sample_rate as f32) as usize;
    let freq_1khz = freq_1khz.min(freq_size - 1);
    let left_ref = left_eq[freq_1khz].norm().max(0.001);
    let right_ref = right_eq[freq_1khz].norm().max(0.001);
    // One common reference preserves the measured interaural balance.
    let common_ref = (left_ref * right_ref).sqrt().max(0.001);

    for k in 0..freq_size {
        left_eq[k] /= common_ref;
        right_eq[k] /= common_ref;
        let max_boost = 10.0_f32.powf(12.0 / 20.0);
        if left_eq[k].norm() > max_boost {
            left_eq[k] = Complex::new(max_boost, 0.0);
        }
        if right_eq[k].norm() > max_boost {
            right_eq[k] = Complex::new(max_boost, 0.0);
        }
    }

    log::info!("[BinauralDecoder] Diffuse-field equalization computed (normalized to 1 kHz)");
    Ok([left_eq, right_eq])
}

fn smooth_power_log_frequency(power: &[f32]) -> Vec<f32> {
    let mut smoothed = vec![0.0; power.len()];
    for (bin, value) in smoothed.iter_mut().enumerate() {
        if bin == 0 {
            *value = power[0];
            continue;
        }
        // Approximately one-third-octave averaging, with a minimum one-bin
        // radius at low frequencies.
        let radius = ((bin as f32 * (2.0_f32.powf(1.0 / 6.0) - 1.0)).round() as usize).max(1);
        let start = bin.saturating_sub(radius);
        let end = (bin + radius + 1).min(power.len());
        *value = power[start..end].iter().copied().sum::<f32>() / (end - start) as f32;
    }
    smoothed
}

/// Compute LFE low-pass filter and gain
///
/// Creates a Butterworth low-pass filter for band-limiting LFE to subwoofer range
/// and calculates distance-dependent attenuation plus level adjustment.
///
/// Returns N/2+1 complex bins (half-spectrum representation for real signals).
///
/// Reference: ITU-R BS.775-3 (multichannel stereophonic sound system with surround channels)
pub fn compute_lfe_filter(
    fft_size: usize,
    sample_rate: u32,
    lfe_crossover: f32,
    lfe_distance: f32,
    lfe_level: f32,
) -> (Vec<Complex<f32>>, f32) {
    let freq_size = fft_size / 2 + 1;

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

    let mut lfe_lowpass_filter = vec![Complex::new(0.0, 0.0); freq_size];

    // Convert to frequency domain response (only positive frequencies for real FFT)
    for (bin, val) in lfe_lowpass_filter.iter_mut().enumerate().take(freq_size) {
        let freq = bin as f32 * fs / fft_size as f32;
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

        *val = Complex::new(h_re, h_im);
    }

    // Compute LFE gain: distance attenuation + level adjustment
    // Distance attenuation: 1/r law with reference distance of 1m
    let distance_atten = 1.0 / lfe_distance.max(0.1);

    // Level adjustment from dB
    let level_gain = 10.0_f32.powf(lfe_level / 20.0);

    // Combined gain: distance attenuation × user level. No additional factor —
    // LFE channels in cinema are calibrated +10 dB hotter than mains (ITU-R BS.775-3)
    // so an arbitrary -3 dB reduction would make the subwoofer path too quiet.
    // The user-adjustable lfe_level is the sole gain control beyond distance attenuation.
    let lfe_gain = distance_atten * level_gain;

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
