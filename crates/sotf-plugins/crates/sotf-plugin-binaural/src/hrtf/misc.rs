use rustfft::num_complex::Complex;
use sotf_host::sofa::{SofaFile, SourcePosition};

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
    let v02_cross_n = cross(v02, n);
    let n_cross_v01 = cross(n, v01);

    let w1 = dot(v02_cross_n, v0p) / n_dot_n;
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

        // Renormalize so the clamped weights sum to 1.0.
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

    // These are interpolation weights for an HRTF field, not loudspeaker VBAP
    // gains. Preserve affine/constant fields by normalizing to unit sum.
    let sum: f32 = weights.iter().sum();
    if sum > 1e-6 {
        [weights[0] / sum, weights[1] / sum, weights[2] / sum]
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

    // Brown-Duda spherical-head shadowing model parameters
    const HEAD_RADIUS: f32 = 0.0875; // 8.75 cm
    const SPEED_OF_SOUND: f32 = 343.0; // m/s at 20°C
    const ALPHA_MIN: f32 = 0.1;
    const TAU: f32 = 2.0 * HEAD_RADIUS / SPEED_OF_SOUND;

    let az_rad = azimuth.to_radians();
    let el_rad = elevation.to_radians();

    // Determine shadowed ear (contralateral to source)
    let (shadowed_ear, shadow_angle) = if az_rad > 0.0 {
        // Source on right -> shadow left ear
        (left_fft, az_rad.abs())
    } else {
        // Source on left -> shadow right ear
        (right_fft, az_rad.abs())
    };

    // Only apply if angle is significant (> 15 degrees)
    if shadow_angle < 15.0_f32.to_radians() {
        return;
    }

    // Elevation reduces shadowing effect
    let elevation_factor = el_rad.cos().abs();

    // Incidence angle for the shadowed ear (0 = facing ear, π = opposite ear)
    let theta_inc = if shadow_angle <= std::f32::consts::PI / 2.0 {
        std::f32::consts::PI / 2.0 + shadow_angle
    } else {
        3.0 * std::f32::consts::PI / 2.0 - shadow_angle
    };

    // Brown-Duda asymptotic high-frequency gain parameter
    let alpha = 1.0 + ALPHA_MIN / 2.0 + (1.0 - ALPHA_MIN / 2.0) * theta_inc.cos();

    // Process each frequency bin (half-spectrum only, no mirroring needed for real FFT)
    for (k, val) in shadowed_ear.iter_mut().enumerate().take(freq_size) {
        let freq = k as f32 * sample_rate as f32 / fft_size as f32;
        if freq < 50.0 {
            continue;
        }

        let omega = 2.0 * std::f32::consts::PI * freq;
        let tau_w = TAU * omega;
        let tau_w_sq = tau_w * tau_w;
        let alpha_tau_w_sq = (alpha * tau_w) * (alpha * tau_w);

        // Magnitude squared of Brown-Duda shadowing filter H(s)=(1+alpha*tau*s)/(1+tau*s)
        let mag_sq = (1.0 + alpha_tau_w_sq) / (1.0 + tau_w_sq);
        let gain = mag_sq.sqrt();
        let atten_db = 20.0 * gain.log10();

        // Scale by near-field strength and elevation
        let scaled_db = atten_db * near_field_strength * elevation_factor;
        let final_gain = 10.0_f32.powf(scaled_db / 20.0);

        *val *= final_gain;
    }
}

/// Detect IR onset (ITD) using threshold-based method
///
/// Finds the first sample where the IR exceeds 10% of peak magnitude.
/// Returns the delay in seconds.
pub(super) fn detect_ir_onset(ir: &[f32], sample_rate: u32) -> f32 {
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

    // Include LFE contribution (mixed full-scale to both ears)
    let lfe_contribution = lfe_channels.len() as f32;
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
        for (ch, hrtf_samples) in hrtf_filters_freq
            .iter_mut()
            .enumerate()
            .take(input_channels)
        {
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
