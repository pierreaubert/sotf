use super::filter::ir_to_freq;
use crate::sofa::{SofaFile, SourcePosition};
use realfft::RealToComplex;
use rustfft::num_complex::Complex;
use std::sync::Arc;

/// Calculate VBAP gains using barycentric interpolation
///
/// Uses barycentric coordinates to interpolate between 3 source positions.
/// This is more numerically stable than matrix inversion and provides better
/// spatial accuracy.
///
/// Reference: Pulkki, "Virtual Sound Source Positioning Using Vector Base Amplitude Panning"
pub fn calculate_vbap_gains(
    target: &SourcePosition,
    nearest: &[(usize, f32); 3],
    sofa: &SofaFile,
) -> [f32; 3] {
    let p = target.to_cartesian_unit_vector();

    let v0 = sofa.positions[nearest[0].0].to_cartesian_unit_vector();
    let v1 = sofa.positions[nearest[1].0].to_cartesian_unit_vector();
    let v2 = sofa.positions[nearest[2].0].to_cartesian_unit_vector();

    // Calculate barycentric coordinates using cross products
    // This is more stable than matrix inversion
    //
    // The barycentric coordinates (w0, w1, w2) satisfy:
    // p = w0*v0 + w1*v1 + w2*v2
    // w0 + w1 + w2 = 1
    //
    // Using the formula:
    // w0 = area(p,v1,v2) / area(v0,v1,v2)
    // w1 = area(v0,p,v2) / area(v0,v1,v2)
    // w2 = area(v0,v1,p) / area(v0,v1,v2)
    //
    // Where area is computed using cross product magnitude

    // Helper to compute cross product
    let cross = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };

    // Helper to compute dot product
    let dot = |a: [f32; 3], b: [f32; 3]| -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] };

    // Compute edge vectors
    let v01 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
    let v02 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
    let v0p = [p[0] - v0[0], p[1] - v0[1], p[2] - v0[2]];

    // Normal of triangle (v0, v1, v2)
    let n = cross(v01, v02);
    let n_dot_n = dot(n, n);

    // Check for degenerate triangle (collinear points)
    if n_dot_n < 1e-6 {
        log::warn!("[BinauralDecoder] Degenerate triangle detected, using nearest neighbor");
        return [1.0, 0.0, 0.0];
    }

    // Calculate barycentric coordinates
    // w1 corresponds to v1, w2 corresponds to v2
    let n_cross_v02 = cross(n, v02);
    let n_cross_v01 = cross(n, v01);

    let w1 = dot(n_cross_v02, v0p) / n_dot_n;
    let w2 = dot(n_cross_v01, v0p) / n_dot_n;
    let w0 = 1.0 - w1 - w2;

    // Check if point is inside triangle (all weights non-negative)
    // If outside, clamp to valid range and warn
    let mut weights = [w0, w1, w2];

    if weights.iter().any(|&w| w < -0.01) {
        // Point is significantly outside triangle
        // This can happen with sparse HRTF measurements
        log::debug!(
            "[BinauralDecoder] Target outside triangle: weights=[{:.3}, {:.3}, {:.3}], clamping to boundary",
            w0,
            w1,
            w2
        );

        // Clamp negative weights to zero
        for w in &mut weights {
            if *w < 0.0 {
                *w = 0.0;
            }
        }

        // Renormalize
        let sum: f32 = weights.iter().sum();
        if sum > 1e-6 {
            for w in &mut weights {
                *w /= sum;
            }
        } else {
            // All weights were negative, use nearest neighbor
            weights = [1.0, 0.0, 0.0];
        }
    }

    // Energy normalization for VBAP
    // Ensures constant perceived loudness across panning positions
    let energy = weights[0] * weights[0] + weights[1] * weights[1] + weights[2] * weights[2];
    if energy > 1e-6 {
        let scale = 1.0 / energy.sqrt();
        [weights[0] * scale, weights[1] * scale, weights[2] * scale]
    } else {
        [1.0, 0.0, 0.0]
    }
}

/// Apply near-field head shadowing to HRTF frequency response
///
/// Implements frequency-dependent Interaural Level Difference (ILD) based on
/// head shadowing models. Uses Woodworth-Schlosberg formula combined with
/// frequency-dependent diffraction model.
///
/// Operates on half-spectrum (N/2+1 bins) - no need to mirror since
/// real FFT automatically handles conjugate symmetry.
pub fn apply_near_field_shadowing(
    left_fft: &mut [Complex<f32>],
    right_fft: &mut [Complex<f32>],
    azimuth: f32,
    elevation: f32,
    fft_size: usize,
    sample_rate: u32,
    near_field_strength: f32,
) {
    let freq_size = fft_size / 2 + 1;

    // Head model parameters
    const HEAD_RADIUS: f32 = 0.0875; // 8.75 cm (typical adult head radius)
    const SPEED_OF_SOUND: f32 = 343.0; // m/s at 20°C

    let az_rad = azimuth.to_radians();
    let el_rad = elevation.to_radians();

    // Use azimuth directly - elevation affects attenuation magnitude, not the angle
    let horizontal_angle = az_rad;

    // Determine which ear is shadowed
    let (shadowed_ear, shadow_angle) = if horizontal_angle > 0.0 {
        // Source on left, shadow right ear
        (right_fft, horizontal_angle.abs())
    } else {
        // Source on right, shadow left ear
        (left_fft, horizontal_angle.abs())
    };

    // Only apply if angle is significant (> 15 degrees)
    if shadow_angle < 15.0_f32.to_radians() {
        return;
    }

    // Process each frequency bin (half-spectrum only, no mirroring needed for real FFT)
    for (k, val) in shadowed_ear.iter_mut().enumerate().take(freq_size) {
        // Frequency for bin k
        let freq = k as f32 * sample_rate as f32 / fft_size as f32;

        if freq < 50.0 {
            // Very low frequencies: no shadowing
            continue;
        }

        // Wavelength
        let wavelength = SPEED_OF_SOUND / freq;

        // Normalized frequency: ka = 2π * radius / wavelength
        let ka = 2.0 * std::f32::consts::PI * HEAD_RADIUS / wavelength;

        // Shadowing attenuation model (combines multiple effects):
        //
        // 1. Geometric shadowing (high frequency): exponential with angle
        // 2. Diffraction (low frequency): based on Rayleigh parameter ka
        // 3. Transition region: smooth blend

        // Elevation reduces shadowing effect (source above/below head has less head shadowing)
        let elevation_factor = el_rad.cos().abs(); // 1.0 at horizontal plane, 0.0 at zenith/nadir

        // High-frequency geometric shadowing (ka >> 1)
        // Attenuation increases with angle and frequency
        let geometric_atten = if ka > 2.0 {
            // Exponential shadowing model for high frequencies
            let angle_factor = (shadow_angle / std::f32::consts::PI).powi(2);
            let freq_factor = (ka / 10.0).min(1.0);
            -6.0 * angle_factor * freq_factor * elevation_factor // Up to -6 dB, reduced at high elevations
        } else {
            0.0
        };

        // Low-frequency diffraction (ka << 1)
        // Uses Rayleigh scattering approximation
        let diffraction_atten = if ka < 2.0 {
            // Minimal shadowing at low frequencies due to diffraction
            let diffraction_factor = (ka / 2.0).powi(2);
            -2.0 * diffraction_factor
                * (shadow_angle / std::f32::consts::PI).powi(2)
                * elevation_factor
        } else {
            0.0
        };

        // Combine effects (smooth transition)
        let transition_weight = (ka / 2.0).min(1.0);
        let total_atten_db =
            geometric_atten * transition_weight + diffraction_atten * (1.0 - transition_weight);

        // Scale by near-field strength parameter
        let scaled_atten_db = total_atten_db * near_field_strength;

        // Convert dB to linear gain
        let gain = 10.0_f32.powf(scaled_atten_db / 20.0);

        // Apply to shadowed ear (no mirroring needed with real FFT)
        *val *= gain;
    }
}

/// Detect IR onset (ITD) using threshold-based method
///
/// Finds the first sample where the IR exceeds 10% of peak magnitude.
/// Returns the delay in seconds.
fn detect_ir_onset(ir: &[f32], sample_rate: u32) -> f32 {
    if ir.is_empty() {
        return 0.0;
    }

    // Find peak magnitude
    let peak = ir.iter().map(|x| x.abs()).fold(0.0f32, f32::max);

    if peak < 1e-6 {
        return 0.0; // Silent IR
    }

    // Find first sample exceeding 10% of peak
    let threshold = peak * 0.1;
    for (i, &sample) in ir.iter().enumerate() {
        if sample.abs() >= threshold {
            return i as f32 / sample_rate as f32;
        }
    }

    0.0
}

/// Interpolate complex HRTF in frequency domain with phase handling
///
/// Interpolates magnitude (in dB) and phase (unwrapped) separately,
/// then removes the interpolated ITD to avoid double-application.
///
/// Operates on half-spectrum (freq_size = N/2+1 bins).
fn interpolate_hrtf_complex(
    source_hrtfs: &[Vec<Complex<f32>>],
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
        if let Some(hrtf) = sofa.get_hrtf(*idx) {
            // Convert to frequency domain (returns freq_size bins)
            let left_fft = ir_to_freq(&hrtf.ir_left, fft_size, fft_r2c);
            let right_fft = ir_to_freq(&hrtf.ir_right, fft_size, fft_r2c);

            // Detect ITD (onset delay) using threshold method
            let left_itd = detect_ir_onset(&hrtf.ir_left, sample_rate);
            let right_itd = detect_ir_onset(&hrtf.ir_right, sample_rate);

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

    // Interpolate left ear HRTF (returns freq_size bins)
    let mut left_fft = interpolate_hrtf_complex(
        &left_hrtfs_freq,
        gains,
        target_left_itd,
        &left_itds,
        sample_rate,
        fft_size,
    );

    // Interpolate right ear HRTF (returns freq_size bins)
    let mut right_fft = interpolate_hrtf_complex(
        &right_hrtfs_freq,
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

/// Normalize HRTF gains to prevent clipping
///
/// Calculates the worst-case scenario (all input channels at full scale)
/// and normalizes all HRTFs to ensure the output stays within [-1, 1].
///
/// `freq_size` is the number of frequency bins (N/2+1 for half-spectrum).
/// HRTFs are stored as [left_freq_size | right_freq_size] per channel.
pub fn normalize_hrtf_gains(
    hrtf_filters_freq: &mut [Vec<Complex<f32>>],
    lfe_channels: &[usize],
    freq_size: usize,
    input_channels: usize,
) {
    let mut max_left_magnitude = 0.0f32;
    let mut max_right_magnitude = 0.0f32;

    // Find worst-case magnitude for each frequency bin
    // This is the sum of magnitudes when all channels play at full scale
    for k in 0..freq_size {
        let mut left_sum = 0.0f32;
        let mut right_sum = 0.0f32;

        for (ch, hrtf) in hrtf_filters_freq.iter().enumerate().take(input_channels) {
            // Skip LFE channels (they're mixed separately with -3dB gain)
            // Note: we don't have easy access to channel index here anymore if we fully removed it,
            // but we need 'ch' to check lfe_channels.
            // So let's revert to using enumerate
            if lfe_channels.contains(&ch) {
                continue;
            }

            let hrtf = &hrtf;
            left_sum += hrtf[k].norm(); // Magnitude
            right_sum += hrtf[k + freq_size].norm();
        }

        max_left_magnitude = max_left_magnitude.max(left_sum);
        max_right_magnitude = max_right_magnitude.max(right_sum);
    }

    // Include LFE contribution (mixed at -3dB = 0.707)
    let lfe_contribution = lfe_channels.len() as f32 * std::f32::consts::FRAC_1_SQRT_2;
    max_left_magnitude += lfe_contribution;
    max_right_magnitude += lfe_contribution;

    // Find the maximum across both channels
    let max_magnitude = max_left_magnitude.max(max_right_magnitude);

    // Calculate normalization factor with headroom
    // Target peak of 0.95 (-0.44 dBFS) to leave headroom for:
    // - Numerical errors
    // - Externalization reflections
    // - Sample rate conversion artifacts
    let target_peak = 0.95;
    let normalization_factor = if max_magnitude > target_peak {
        target_peak / max_magnitude
    } else {
        1.0 // No normalization needed
    };

    if normalization_factor < 1.0 {
        log::info!(
            "[BinauralDecoder] Normalizing HRTFs by {:.3} ({:.2} dB) to prevent clipping (worst-case magnitude: {:.2})",
            normalization_factor,
            20.0 * normalization_factor.log10(),
            max_magnitude
        );

        // Apply normalization to all HRTFs
        for (ch, hrtf_samples) in hrtf_filters_freq.iter_mut().enumerate().take(input_channels) {
            // Skip LFE channels (they don't use HRTFs)
            if lfe_channels.contains(&ch) {
                continue;
            }

            for sample in hrtf_samples {
                *sample *= normalization_factor;
            }
        }
    } else {
        log::debug!(
            "[BinauralDecoder] No HRTF normalization needed (worst-case magnitude: {:.2})",
            max_magnitude
        );
    }
}
