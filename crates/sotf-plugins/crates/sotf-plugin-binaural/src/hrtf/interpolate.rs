use super::super::filter::ir_to_freq;
use super::misc::apply_near_field_shadowing;
use super::misc::detect_ir_onset;
use realfft::RealToComplex;
use rustfft::num_complex::Complex;
use sotf_host::sofa::SofaFile;
use std::sync::Arc;

pub struct PreparedHrtfSpectra {
    left: Vec<Vec<Complex<f32>>>,
    right: Vec<Vec<Complex<f32>>>,
    left_itds: Vec<f32>,
    right_itds: Vec<f32>,
}

pub fn prepare_hrtf_spectra(
    sofa: &SofaFile,
    fft_size: usize,
    sample_rate: u32,
    fft_r2c: &Arc<dyn RealToComplex<f32>>,
) -> PreparedHrtfSpectra {
    let mut prepared = PreparedHrtfSpectra {
        left: Vec::with_capacity(sofa.num_measurements),
        right: Vec::with_capacity(sofa.num_measurements),
        left_itds: Vec::with_capacity(sofa.num_measurements),
        right_itds: Vec::with_capacity(sofa.num_measurements),
    };
    for measurement in 0..sofa.num_measurements {
        let (_, left, right) = sofa
            .get_hrtf_slices(measurement)
            .expect("validated SOFA measurement dimensions");
        prepared.left.push(ir_to_freq(left, fft_size, fft_r2c));
        prepared.right.push(ir_to_freq(right, fft_size, fft_r2c));
        prepared.left_itds.push(detect_ir_onset(left, sample_rate));
        prepared
            .right_itds
            .push(detect_ir_onset(right, sample_rate));
    }
    prepared
}

#[allow(clippy::too_many_arguments)]
pub fn interpolate_hrtf_prepared(
    nearest: &[(usize, f32); 3],
    gains: &[f32; 3],
    prepared: &PreparedHrtfSpectra,
    fft_size: usize,
    sample_rate: u32,
    near_field_strength: f32,
    target_azimuth: f32,
    target_elevation: f32,
) -> (Vec<Complex<f32>>, Vec<Complex<f32>>) {
    let left = [
        prepared.left[nearest[0].0].as_slice(),
        prepared.left[nearest[1].0].as_slice(),
        prepared.left[nearest[2].0].as_slice(),
    ];
    let right = [
        prepared.right[nearest[0].0].as_slice(),
        prepared.right[nearest[1].0].as_slice(),
        prepared.right[nearest[2].0].as_slice(),
    ];
    let left_itds = [
        prepared.left_itds[nearest[0].0],
        prepared.left_itds[nearest[1].0],
        prepared.left_itds[nearest[2].0],
    ];
    let right_itds = [
        prepared.right_itds[nearest[0].0],
        prepared.right_itds[nearest[1].0],
        prepared.right_itds[nearest[2].0],
    ];
    let target_left_itd = gains.iter().zip(&left_itds).map(|(g, itd)| g * itd).sum();
    let target_right_itd = gains.iter().zip(&right_itds).map(|(g, itd)| g * itd).sum();
    let mut left_fft = interpolate_hrtf_complex(
        &left,
        gains,
        target_left_itd,
        &left_itds,
        sample_rate,
        fft_size,
    );
    let mut right_fft = interpolate_hrtf_complex(
        &right,
        gains,
        target_right_itd,
        &right_itds,
        sample_rate,
        fft_size,
    );
    if near_field_strength > 0.001 {
        apply_near_field_shadowing(
            &mut left_fft,
            &mut right_fft,
            target_azimuth,
            target_elevation,
            fft_size,
            sample_rate,
            near_field_strength,
        );
    }
    (left_fft, right_fft)
}

/// Interpolate complex HRTF in frequency domain with phase handling
///
/// Interpolates magnitude (in dB) and phase (unwrapped) separately,
/// then removes the interpolated ITD to avoid double-application.
///
/// Operates on half-spectrum (freq_size = N/2+1 bins).
fn interpolate_hrtf_complex(
    source_hrtfs: &[&[Complex<f32>]],
    gains: &[f32; 3],
    target_itd: f32,
    source_itds: &[f32],
    sample_rate: u32,
    fft_size: usize,
) -> Vec<Complex<f32>> {
    let freq_size = fft_size / 2 + 1;
    let mut result = vec![Complex::new(0.0, 0.0); freq_size];

    for (k, val) in result.iter_mut().enumerate().take(freq_size) {
        let mut mag_db = 0.0f32;
        let mut phase_sum = Complex::new(0.0, 0.0);

        for (i, &gain) in gains.iter().enumerate() {
            if gain < 1e-6 {
                continue; // Skip negligible contributions
            }

            let h = source_hrtfs[i][k];
            let magnitude = h.norm();
            let phase = h.arg();

            // Interpolate magnitude in log scale (dB)
            let db = if magnitude > 1e-9 {
                20.0 * magnitude.log10()
            } else {
                -200.0 // Very quiet
            };
            mag_db += gain * db;

            // Remove source ITD from phase to avoid phase discontinuities
            let freq = k as f32 * sample_rate as f32 / fft_size as f32;
            let itd_phase_shift = -2.0 * std::f32::consts::PI * freq * source_itds[i];
            let corrected_phase = phase - itd_phase_shift;

            // Accumulate phase as complex phasor for smooth interpolation
            phase_sum += Complex::new(corrected_phase.cos(), corrected_phase.sin()) * gain;
        }

        // Convert magnitude back from dB
        let magnitude = 10.0_f32.powf(mag_db / 20.0);

        // Extract interpolated phase from phasor sum
        let phase = phase_sum.arg();

        // Apply target ITD as phase shift
        let freq = k as f32 * sample_rate as f32 / fft_size as f32;
        let target_phase_shift = -2.0 * std::f32::consts::PI * freq * target_itd;
        let final_phase = phase + target_phase_shift;

        // Reconstruct complex HRTF
        *val = Complex::new(magnitude * final_phase.cos(), magnitude * final_phase.sin());
    }

    result
}

/// Interpolate HRTF using frequency-domain method with ITD alignment
///
/// This method addresses three key issues with time-domain HRTF interpolation:
///
/// 1. **ITD Alignment**: Extracts and preserves Interaural Time Differences (ITDs)
///    by detecting onset delays in each HRTF before interpolation
///
/// 2. **Phase Coherence**: Interpolates in frequency domain using magnitude and
///    phase separately, preventing comb filtering from misaligned time-domain averaging
///
/// 3. **Robust to Sparse Data**: Works well even with sparse HRTF datasets by
///    gracefully handling phase unwrapping and magnitude smoothing
///
/// Returns half-spectrum (N/2+1 bins) per ear for use with real FFT.
#[allow(clippy::too_many_arguments)]
pub fn interpolate_hrtf_frequency_domain(
    nearest: &[(usize, f32); 3],
    gains: &[f32; 3],
    sofa: &SofaFile,
    fft_size: usize,
    sample_rate: u32,
    fft_r2c: &Arc<dyn RealToComplex<f32>>,
    near_field_strength: f32,
    target_azimuth: f32,
    target_elevation: f32,
) -> (Vec<Complex<f32>>, Vec<Complex<f32>>) {
    let freq_size = fft_size / 2 + 1;

    // Convert all source HRTFs to frequency domain (returns freq_size bins each)
    let mut left_hrtfs_freq = Vec::with_capacity(3);
    let mut right_hrtfs_freq = Vec::with_capacity(3);
    let mut left_itds = Vec::with_capacity(3);
    let mut right_itds = Vec::with_capacity(3);

    for (idx, _) in nearest.iter() {
        if let Some((_, left, right)) = sofa.get_hrtf_slices(*idx) {
            // Convert to frequency domain (returns freq_size bins)
            let left_fft = ir_to_freq(left, fft_size, fft_r2c);
            let right_fft = ir_to_freq(right, fft_size, fft_r2c);

            // Detect ITD (onset delay) using threshold method
            let left_itd = detect_ir_onset(left, sample_rate);
            let right_itd = detect_ir_onset(right, sample_rate);

            left_hrtfs_freq.push(left_fft);
            right_hrtfs_freq.push(right_fft);
            left_itds.push(left_itd);
            right_itds.push(right_itd);
        } else {
            // Fallback: use zeros
            left_hrtfs_freq.push(vec![Complex::new(0.0, 0.0); freq_size]);
            right_hrtfs_freq.push(vec![Complex::new(0.0, 0.0); freq_size]);
            left_itds.push(0.0);
            right_itds.push(0.0);
        }
    }

    // Interpolate ITDs
    let target_left_itd =
        gains[0] * left_itds[0] + gains[1] * left_itds[1] + gains[2] * left_itds[2];
    let target_right_itd =
        gains[0] * right_itds[0] + gains[1] * right_itds[1] + gains[2] * right_itds[2];

    let left_refs: Vec<&[Complex<f32>]> = left_hrtfs_freq.iter().map(Vec::as_slice).collect();
    let right_refs: Vec<&[Complex<f32>]> = right_hrtfs_freq.iter().map(Vec::as_slice).collect();

    // Interpolate left ear HRTF (returns freq_size bins)
    let mut left_fft = interpolate_hrtf_complex(
        &left_refs,
        gains,
        target_left_itd,
        &left_itds,
        sample_rate,
        fft_size,
    );

    // Interpolate right ear HRTF (returns freq_size bins)
    let mut right_fft = interpolate_hrtf_complex(
        &right_refs,
        gains,
        target_right_itd,
        &right_itds,
        sample_rate,
        fft_size,
    );

    // Apply near-field shadowing if enabled
    if near_field_strength > 0.001 {
        apply_near_field_shadowing(
            &mut left_fft,
            &mut right_fft,
            target_azimuth,
            target_elevation,
            fft_size,
            sample_rate,
            near_field_strength,
        );
    }

    (left_fft, right_fft)
}
