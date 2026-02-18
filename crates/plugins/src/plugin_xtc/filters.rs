//! Crosstalk cancellation filter computation.
//!
//! Contains all DSP math for computing XTC filters in the frequency domain,
//! including head shadowing models, regularization, and spectral normalization.

use super::config::XtcPluginParams;
use super::reflections::{
    build_reflection_data_image_source, build_reflection_data_ir, RoomReflectionData,
};
use rustfft::num_complex::Complex;
use std::f32::consts::PI;

/// Speed of sound at 20°C in m/s
pub(crate) const SPEED_OF_SOUND: f32 = 343.0;

pub(crate) type XtcFilterSet = (
    Vec<Complex<f32>>,
    Vec<Complex<f32>>,
    Vec<Complex<f32>>,
    Vec<Complex<f32>>,
);

/// Crosstalk cancellation filters in frequency domain
pub(crate) struct XtcFilters {
    /// Diagonal filter for left output (L_out += filter_ll * L_in)
    pub filter_ll: Vec<Complex<f32>>,
    /// Cross filter for left output (L_out += filter_lr * R_in)
    pub filter_lr: Vec<Complex<f32>>,
    /// Cross filter for right output (R_out += filter_rl * L_in), None if symmetric
    pub filter_rl: Option<Vec<Complex<f32>>>,
    /// Diagonal filter for right output (R_out += filter_rr * R_in), None if symmetric
    pub filter_rr: Option<Vec<Complex<f32>>>,
    /// Whether the filter set is symmetric (yaw ~= 0)
    pub is_symmetric: bool,
}

// ============================================================================
// Top-level filter computation
// ============================================================================

/// Compute crosstalk cancellation filters in frequency domain
///
/// This is the main filter computation function that handles:
/// - Symmetric case (yaw = 0): returns None for filter_rl/filter_rr
/// - Asymmetric case (yaw != 0): returns full 4-filter matrix
///
/// Uses improved Woodworth head shadowing model for better accuracy.
pub(crate) fn compute_xtc_filters_full(
    params: &XtcPluginParams,
    sample_rate: u32,
    num_bins: usize,
) -> XtcFilters {
    let yaw_rad = params.head_yaw_deg * PI / 180.0;
    let is_symmetric = yaw_rad.abs() < 0.001; // ~0.06 degrees threshold

    // Precompute room reflection data if enabled
    let room_data: Option<RoomReflectionData> = if params.room_reflections_enabled {
        if let Some(ref ir_path) = params.room_ir_file {
            build_reflection_data_ir(ir_path, sample_rate, num_bins).ok()
        } else {
            Some(build_reflection_data_image_source(params, sample_rate, num_bins))
        }
    } else {
        None
    };

    let mut filters = if is_symmetric {
        // Use optimized symmetric computation
        let (filter_ll, filter_lr) =
            compute_xtc_filters_symmetric(params, sample_rate, num_bins, room_data.as_ref());
        XtcFilters {
            filter_ll,
            filter_lr,
            filter_rl: None,
            filter_rr: None,
            is_symmetric: true,
        }
    } else {
        // Full asymmetric computation for yaw != 0
        let mut f =
            compute_xtc_filters_asymmetric(params, sample_rate, num_bins, room_data.as_ref());
        f.is_symmetric = false;
        f
    };

    // Post-processing: spectral energy normalization to prevent tonal imbalance
    if params.spectral_normalization {
        apply_spectral_normalization(&mut filters, num_bins);
    }

    filters
}

// ============================================================================
// Spectral normalization
// ============================================================================

/// Apply spectral energy normalization to XTC filters.
///
/// XTC processing can create spectral tilt (boosting some frequencies, attenuating others).
/// This normalizes the average energy per bin to keep tonal balance close to the original signal.
/// Uses a gentle smoothed approach to avoid introducing artifacts.
fn apply_spectral_normalization(filters: &mut XtcFilters, num_bins: usize) {
    // First pass: compute per-bin correction gains
    let mut gains = vec![1.0_f32; num_bins];
    let is_asymmetric = filters.filter_rl.is_some();

    for bin in 1..num_bins - 1 {
        // Left output energy from unit-energy input
        let energy_l = filters.filter_ll[bin].norm_sqr() + filters.filter_lr[bin].norm_sqr();

        // Right output energy (use symmetric equivalents if not available)
        let energy_r = if is_asymmetric {
            let rl = filters.filter_rl.as_ref().unwrap();
            let rr = filters.filter_rr.as_ref().unwrap();
            rl[bin].norm_sqr() + rr[bin].norm_sqr()
        } else {
            // Symmetric: filter_rr = filter_ll, filter_rl = filter_lr
            energy_l
        };

        // Average energy across both channels
        let avg_energy = (energy_l + energy_r) / 2.0;

        if avg_energy > 0.01 {
            // Gentle normalization: blend between unity and full correction
            // This preserves the XTC effect while reducing tonal coloration
            let correction = (1.0 / avg_energy.sqrt()).clamp(0.5, 2.0);
            // Apply 50% of the correction (gentle approach)
            gains[bin] = 1.0 + 0.5 * (correction - 1.0);
        }
    }

    // Second pass: apply gains
    for bin in 1..num_bins - 1 {
        let gain = gains[bin];
        filters.filter_ll[bin] *= gain;
        filters.filter_lr[bin] *= gain;
        if let Some(ref mut rl) = filters.filter_rl {
            rl[bin] *= gain;
        }
        if let Some(ref mut rr) = filters.filter_rr {
            rr[bin] *= gain;
        }
    }
}

// ============================================================================
// Asymmetric filters (yaw != 0)
// ============================================================================

/// Compute asymmetric filters for non-zero yaw angle
fn compute_xtc_filters_asymmetric(
    params: &XtcPluginParams,
    sample_rate: u32,
    num_bins: usize,
    room_data: Option<&RoomReflectionData>,
) -> XtcFilters {
    let mut filter_ll = Vec::with_capacity(num_bins);
    let mut filter_lr = Vec::with_capacity(num_bins);
    let mut filter_rl = Vec::with_capacity(num_bins);
    let mut filter_rr = Vec::with_capacity(num_bins);

    // Geometry
    let d = params.distance_m + params.head_offset_z;
    let theta_rad = params.speaker_angle_deg * PI / 180.0;
    let yaw_rad = params.head_yaw_deg * PI / 180.0;
    let a = params.head_radius_m;
    let x_offset = params.head_offset_x;

    // Effective speaker angles relative to rotated head
    let theta_left = theta_rad + yaw_rad; // Left speaker angle
    let theta_right = theta_rad - yaw_rad; // Right speaker angle

    // Left ear paths: geometric + diffraction separated
    let l_left_ipsi = compute_path_length(d, theta_left, -x_offset);
    let l_left_contra_geometric = compute_path_length(d, theta_right, -x_offset);
    let diffraction_left = woodworth_diffraction_path(theta_right, a);
    let l_left_contra_full = l_left_contra_geometric + diffraction_left;

    // Right ear paths: geometric + diffraction separated
    let r_right_ipsi = compute_path_length(d, theta_right, x_offset);
    let r_right_contra_geometric = compute_path_length(d, theta_left, x_offset);
    let diffraction_right = woodworth_diffraction_path(theta_left, a);
    let r_right_contra_full = r_right_contra_geometric + diffraction_right;

    // Distance attenuation ratios (use full path for 1/r attenuation)
    let amplitude_ratio_left = l_left_ipsi / l_left_contra_full;
    let amplitude_ratio_right = r_right_ipsi / r_right_contra_full;

    // Geometric time differences (frequency-dependent diffraction added per-bin)
    let delta_t_left_geometric = (l_left_contra_geometric - l_left_ipsi) / SPEED_OF_SOUND;
    let delta_t_right_geometric = (r_right_contra_geometric - r_right_ipsi) / SPEED_OF_SOUND;

    // Contralateral shadow angles (angular separation from source to far ear)
    let angle_left_contra = contralateral_shadow_angle(theta_right.abs());
    let angle_right_contra = contralateral_shadow_angle(theta_left.abs());

    // Max gain limit (convert dB to linear)
    let max_gain_linear = 10.0_f32.powf(params.max_gain_db / 20.0);

    let freq_per_bin = sample_rate as f32 / (2.0 * (num_bins - 1) as f32);

    for bin in 0..num_bins {
        let freq = bin as f32 * freq_per_bin;

        // Pinna resonance: full for ipsi paths, angle-dependent for contra paths
        let pinna_ipsi = pinna_resonance(freq);

        // Frequency-dependent diffraction delays
        let diffraction_delay_left =
            frequency_dependent_diffraction_delay(freq, angle_left_contra, a);
        let diffraction_delay_right =
            frequency_dependent_diffraction_delay(freq, angle_right_contra, a);
        let delta_t_left = delta_t_left_geometric + diffraction_delay_left;
        let delta_t_right = delta_t_right_geometric + diffraction_delay_right;

        // Left ear: ipsi speaker is left speaker (theta_left), contra is right speaker (theta_right)
        let pinna_left_contra =
            pinna_resonance_contra(freq, theta_right.abs() * 180.0 / PI);
        let h_ll_ipsi = Complex::new(1.0, 0.0) * pinna_ipsi;
        let g_ll = head_shadowing_woodworth(freq, angle_left_contra, a) * amplitude_ratio_left;
        let phase_ll = -2.0 * PI * freq * delta_t_left;
        let h_ll_contra =
            Complex::new(g_ll * phase_ll.cos(), g_ll * phase_ll.sin()) * pinna_left_contra;

        // Right ear: ipsi speaker is right speaker (theta_right), contra is left speaker (theta_left)
        let pinna_right_contra =
            pinna_resonance_contra(freq, theta_left.abs() * 180.0 / PI);
        let h_rr_ipsi = Complex::new(1.0, 0.0) * pinna_ipsi;
        let g_rr = head_shadowing_woodworth(freq, angle_right_contra, a) * amplitude_ratio_right;
        let phase_rr = -2.0 * PI * freq * delta_t_right;
        let h_rr_contra =
            Complex::new(g_rr * phase_rr.cos(), g_rr * phase_rr.sin()) * pinna_right_contra;

        // Integrate room reflections: add reflection contributions to transfer functions
        let (h_ll_ipsi_final, h_ll_contra_final, h_rr_ipsi_final, h_rr_contra_final) =
            if let Some(room) = room_data {
                (
                    h_ll_ipsi + room.h_room_ipsi[bin],
                    h_ll_contra + room.h_room_contra[bin],
                    h_rr_ipsi + room.h_room_ipsi[bin],
                    h_rr_contra + room.h_room_contra[bin],
                )
            } else {
                (h_ll_ipsi, h_ll_contra, h_rr_ipsi, h_rr_contra)
            };

        let beta = if let Some(room) = room_data {
            compute_beta_smooth(freq, params) * room.beta_boost[bin]
        } else {
            compute_beta_smooth(freq, params)
        };

        // Compute 2x2 filter matrices for each ear independently
        // Left ear: L_out = w_ll * L_in + w_lr * R_in
        let (w_ll, w_lr) =
            compute_2x2_inverse(h_ll_ipsi_final, h_ll_contra_final, beta, max_gain_linear);
        // Right ear: R_out = w_rl * L_in + w_rr * R_in
        let (w_rr, w_rl) =
            compute_2x2_inverse(h_rr_ipsi_final, h_rr_contra_final, beta, max_gain_linear);

        filter_ll.push(w_ll);
        filter_lr.push(w_lr);
        filter_rl.push(w_rl);
        filter_rr.push(w_rr);
    }

    XtcFilters {
        filter_ll,
        filter_lr,
        filter_rl: Some(filter_rl),
        filter_rr: Some(filter_rr),
        is_symmetric: false,
    }
}

// ============================================================================
// Symmetric filters (yaw == 0)
// ============================================================================

/// Compute crosstalk cancellation filters in frequency domain (symmetric version)
///
/// Since the XTC matrix is symmetric (filter_rl == filter_lr and filter_rr == filter_ll),
/// we only need to compute and store 2 filters instead of 4.
///
/// Returns (filter_ll, filter_lr) where:
/// - filter_ll: diagonal filter (direct path processing)
/// - filter_lr: cross filter (crosstalk cancellation)
fn compute_xtc_filters_symmetric(
    params: &XtcPluginParams,
    sample_rate: u32,
    num_bins: usize,
    room_data: Option<&RoomReflectionData>,
) -> (Vec<Complex<f32>>, Vec<Complex<f32>>) {
    let mut filter_ll = Vec::with_capacity(num_bins);
    let mut filter_lr = Vec::with_capacity(num_bins);

    // Geometry with head tracking offsets
    let d = params.distance_m + params.head_offset_z;
    let theta_rad = params.speaker_angle_deg * PI / 180.0;
    let a = params.head_radius_m;
    let x_offset = params.head_offset_x;

    // Compute path lengths (considering head offset)
    let l_ipsi = compute_path_length(d, theta_rad, -x_offset);
    // Geometric contralateral path (without diffraction) for base delay
    let l_contra_geometric = compute_path_length(d, theta_rad, x_offset);
    // Full contralateral path with Woodworth diffraction (for 1/r amplitude attenuation)
    let diffraction_extra = woodworth_diffraction_path(theta_rad, a);
    let l_contra_full = l_contra_geometric + diffraction_extra;

    // Distance attenuation ratio uses full path (geometric + diffraction)
    let amplitude_ratio = l_ipsi / l_contra_full;

    // Geometric time difference (frequency-dependent diffraction added per-bin)
    let delta_t_geometric = (l_contra_geometric - l_ipsi) / SPEED_OF_SOUND;

    // Contralateral shadow angle: angular separation from source to far ear
    let contra_angle = contralateral_shadow_angle(theta_rad);

    // Max gain limit (convert dB to linear)
    let max_gain_linear = 10.0_f32.powf(params.max_gain_db / 20.0);

    // Process each frequency bin
    let freq_per_bin = sample_rate as f32 / (2.0 * (num_bins - 1) as f32);

    for bin in 0..num_bins {
        let freq = bin as f32 * freq_per_bin;

        // Transfer function for ipsilateral path (reference = 1)
        let h_ipsi = Complex::new(1.0, 0.0);

        // Frequency-dependent ITD: geometric delay + diffraction delay
        let diffraction_delay =
            frequency_dependent_diffraction_delay(freq, contra_angle, a);
        let delta_t = delta_t_geometric + diffraction_delay;

        // Transfer function for contralateral path using Woodworth model
        // Uses corrected shadow angle (PI/2 + theta) and distance attenuation
        let g = head_shadowing_woodworth(freq, contra_angle, a) * amplitude_ratio;
        let phase = -2.0 * PI * freq * delta_t;
        let h_contra = Complex::new(g * phase.cos(), g * phase.sin());

        // Frequency-dependent regularization with smooth transitions
        let beta = compute_beta_smooth(freq, params);

        // Pinna resonance shaping: full effect for ipsi, angle-dependent for contra
        let pinna_ipsi = pinna_resonance(freq);
        let pinna_contra = pinna_resonance_contra(freq, params.speaker_angle_deg);
        let h_ipsi_shaped = h_ipsi * pinna_ipsi;
        let h_contra_shaped = h_contra * pinna_contra;

        // Integrate room reflections: add reflection contributions to transfer functions
        let (h_ipsi_final, h_contra_final) = if let Some(room) = room_data {
            (
                h_ipsi_shaped + room.h_room_ipsi[bin],
                h_contra_shaped + room.h_room_contra[bin],
            )
        } else {
            (h_ipsi_shaped, h_contra_shaped)
        };

        let beta = if let Some(room) = room_data {
            beta * room.beta_boost[bin]
        } else {
            beta
        };

        // Use shared 2x2 inverse computation with gain clamping
        let (w_ll, w_lr) = compute_2x2_inverse(h_ipsi_final, h_contra_final, beta, max_gain_linear);

        filter_ll.push(w_ll);
        filter_lr.push(w_lr);
    }

    (filter_ll, filter_lr)
}

// ============================================================================
// 2x2 matrix inverse with Neumann refinement
// ============================================================================

/// Compute 2x2 inverse filter for one ear with multi-stage Neumann series refinement.
///
/// First computes the regularized inverse W₁ = (C^H*C + β*I)^-1 * C^H, then
/// refines it with one iteration of Neumann series: W₂ = W₁ * (2*I - C*W₁).
/// This improves broadband cancellation depth by correcting residual errors
/// from regularization, similar to the BACCH approach.
///
/// Returns (w_ipsi, w_contra) filter coefficients.
/// max_gain_linear limits the magnitude of each output coefficient.
#[inline]
fn compute_2x2_inverse(
    h_ipsi: Complex<f32>,
    h_contra: Complex<f32>,
    beta: f32,
    max_gain_linear: f32,
) -> (Complex<f32>, Complex<f32>) {
    let h_ipsi_mag_sq = h_ipsi.norm_sqr();
    let h_contra_mag_sq = h_contra.norm_sqr();
    let cross_term = (h_ipsi * h_contra.conj()).re * 2.0;

    let diag = h_ipsi_mag_sq + h_contra_mag_sq + beta;
    let off_diag = cross_term;

    let det = diag * diag - off_diag * off_diag;

    if det.abs() < 1e-10 {
        return (Complex::new(1.0, 0.0), Complex::new(0.0, 0.0));
    }

    let inv_diag = diag / det;
    let inv_off_diag = -off_diag / det;

    let h_ipsi_conj = h_ipsi.conj();
    let h_contra_conj = h_contra.conj();

    let w1_ipsi = h_ipsi_conj * inv_diag + h_contra_conj * inv_off_diag;
    let w1_contra = h_ipsi_conj * inv_off_diag + h_contra_conj * inv_diag;

    // Neumann series refinement: W₂ = W₁ * (2I - C*W₁)
    // Compute the product C*W₁ for the 2x2 symmetric matrix C = [[h_ipsi, h_contra], [h_contra, h_ipsi]]
    //
    // (C*W₁)[0,0] = h_ipsi * w1_ipsi + h_contra * w1_contra
    // (C*W₁)[0,1] = h_ipsi * w1_contra + h_contra * w1_ipsi
    let cw_00 = h_ipsi * w1_ipsi + h_contra * w1_contra;
    let cw_01 = h_ipsi * w1_contra + h_contra * w1_ipsi;

    // (2I - C*W₁)
    let r_00 = Complex::new(2.0, 0.0) - cw_00;
    let r_01 = Complex::new(0.0, 0.0) - cw_01;

    // W₂ = W₁ * R where R = (2I - C*W₁)
    // W₂[0,0] = w1_ipsi * r_00 + w1_contra * r_01  (but r is column-matched)
    // For the one-ear case, the "matrix" is really [w_ipsi, w_contra] (row vector)
    // times the 2x2 correction matrix R = [[r_00, r_01], [r_01, r_00]] (also symmetric)
    let w2_ipsi = w1_ipsi * r_00 + w1_contra * r_01;
    let w2_contra = w1_contra * r_00 + w1_ipsi * r_01;

    // Clamp magnitudes to prevent excessive boost
    let w2_ipsi = clamp_complex_magnitude(w2_ipsi, max_gain_linear);
    let w2_contra = clamp_complex_magnitude(w2_contra, max_gain_linear);

    (w2_ipsi, w2_contra)
}

/// Clamp a complex number's magnitude while preserving its phase
#[inline]
fn clamp_complex_magnitude(c: Complex<f32>, max_mag: f32) -> Complex<f32> {
    let mag = c.norm();
    if mag > max_mag {
        c * (max_mag / mag)
    } else {
        c
    }
}

// ============================================================================
// Head shadowing models
// ============================================================================

/// Compute the Woodworth diffraction path around the head for a given incidence angle.
///
/// The sound reaching the far ear must diffract around the spherical head.
/// The extra path length depends on the angle of incidence (azimuth from median plane).
///
/// For angle <= PI/2: extra_path = a * (angle + sin(angle))
/// For angle > PI/2:  extra_path = a * (PI - angle + sin(angle))
#[inline]
pub(crate) fn woodworth_diffraction_path(angle_rad: f32, head_radius: f32) -> f32 {
    let theta = angle_rad.abs();
    if theta <= PI / 2.0 {
        head_radius * (theta + theta.sin())
    } else {
        head_radius * (PI - theta + theta.sin())
    }
}

/// Compute frequency-dependent diffraction delay around the head.
///
/// At low frequencies (ka < 0.5), sound diffracts fully around the spherical head,
/// following the Woodworth model: delay = a*(θ+sin(θ))/c.
/// At high frequencies (ka ≥ 2.0), sound takes the geometric shadow path: delay = a*sin(θ)/c.
/// The transition region (0.5 ≤ ka < 2.0) linearly blends between the two models.
///
/// These ka boundaries match `head_shadowing_woodworth()` for physical consistency.
#[inline]
pub(crate) fn frequency_dependent_diffraction_delay(
    freq: f32,
    angle_rad: f32,
    head_radius: f32,
) -> f32 {
    let theta = angle_rad.abs();
    let low_freq_delay = woodworth_diffraction_path(theta, head_radius) / SPEED_OF_SOUND;

    if freq <= 0.0 {
        return low_freq_delay;
    }

    let ka = 2.0 * PI * freq * head_radius / SPEED_OF_SOUND;
    let high_freq_delay = head_radius * theta.sin() / SPEED_OF_SOUND;

    if ka < 0.5 {
        low_freq_delay
    } else if ka < 2.0 {
        let t = (ka - 0.5) / 1.5;
        low_freq_delay * (1.0 - t) + high_freq_delay * t
    } else {
        high_freq_delay
    }
}

/// Compute the angular separation between a sound source and the contralateral ear.
///
/// For a source at azimuth `speaker_angle` from the median plane, the ipsilateral ear
/// (same side) is at 90° from center, and the contralateral ear (opposite side) is at
/// -90°. The angular separation from source to contralateral ear, measured around the
/// head surface, is approximately PI/2 + speaker_angle.
#[inline]
fn contralateral_shadow_angle(speaker_angle_rad: f32) -> f32 {
    (PI / 2.0 + speaker_angle_rad).min(PI)
}

/// Woodworth-Schlosberg head shadowing model
///
/// Provides frequency and angle dependent interaural level difference (ILD)
/// based on spherical head acoustics.
pub(crate) fn head_shadowing_woodworth(freq: f32, angle_rad: f32, head_radius: f32) -> f32 {
    if freq <= 0.0 {
        return 1.0;
    }

    // Wave number times head radius (ka)
    // This determines the diffraction regime
    let ka = 2.0 * PI * freq * head_radius / SPEED_OF_SOUND;
    let theta = angle_rad.abs();

    if ka < 0.5 {
        // Low frequency: sound diffracts fully around head
        // Minimal ILD, slight angle dependence
        1.0 - 0.05 * ka * theta.sin()
    } else if ka < 2.0 {
        // Transition region: gradual shadowing
        let t = (ka - 0.5) / 1.5; // 0 to 1 over transition
        let shadow_factor = (1.0 + theta.cos()) / 2.0;
        let low_freq = 1.0 - 0.05 * ka * theta.sin();
        let high_freq = shadow_factor.powf(0.5 + t);
        low_freq * (1.0 - t) + high_freq * t
    } else {
        // High frequency: significant head shadow
        // Shadow increases with angle from direct path
        let shadow_factor = (1.0 + theta.cos()) / 2.0; // 1 at 0°, 0 at 180°
        let exponent = (ka / 4.0).min(3.0); // Cap exponent for stability
        shadow_factor.powf(exponent)
    }
}

// ============================================================================
// Pinna resonance model
// ============================================================================

/// Simplified pinna resonance model for externalization cues.
///
/// Models three key ear resonances that provide crucial spatial perception cues:
/// 1. Ear canal resonance: broad +10 dB peak at ~2.7 kHz
/// 2. Concha resonance: broad +5 dB peak at ~4.5 kHz
/// 3. Pinna anti-resonance: narrow -6 dB notch at ~9 kHz (elevation cue)
///
/// Without these cues, XTC output tends to sound "inside the head" even with
/// perfect crosstalk cancellation. Returns a real-valued gain (applied to both
/// ipsilateral and contralateral transfer functions).
#[inline]
fn pinna_resonance(freq: f32) -> f32 {
    if freq <= 0.0 {
        return 1.0;
    }

    // Ear canal resonance: 2nd-order bandpass centered at 2700 Hz, Q=1.2
    // Peak gain ~+10 dB
    let f_ear = 2700.0_f32;
    let q_ear = 1.2_f32;
    let gain_ear_db = 10.0_f32;
    let ear_response = resonance_peak(freq, f_ear, q_ear, gain_ear_db);

    // Concha resonance: broad peak at 4500 Hz, Q=1.5
    // Peak gain ~+5 dB
    let f_concha = 4500.0_f32;
    let q_concha = 1.5_f32;
    let gain_concha_db = 5.0_f32;
    let concha_response = resonance_peak(freq, f_concha, q_concha, gain_concha_db);

    // Pinna anti-resonance: narrow notch at 9000 Hz, Q=3.0
    // Depth ~-6 dB (this is a key elevation cue)
    let f_pinna = 9000.0_f32;
    let q_pinna = 3.0_f32;
    let gain_pinna_db = -6.0_f32;
    let pinna_response = resonance_peak(freq, f_pinna, q_pinna, gain_pinna_db);

    // Combine all resonances (multiplicative in linear domain = additive in dB)
    ear_response * concha_response * pinna_response
}

/// Angle-dependent pinna resonance model for the contralateral (far) ear.
///
/// The ear canal resonance (2.7 kHz) is a tube resonance and is angle-independent.
/// The concha resonance (4.5 kHz) and pinna notch (9 kHz) are angle-dependent:
/// they are strongest when the source is on the ipsilateral side and weaken as
/// the source moves toward the contralateral side.
///
/// `speaker_angle_deg` is the speaker angle from the median plane (e.g., 30°).
#[inline]
fn pinna_resonance_contra(freq: f32, speaker_angle_deg: f32) -> f32 {
    if freq <= 0.0 {
        return 1.0;
    }

    // Angle factor: how much of the angle-dependent pinna effects remain.
    // At 0° (median plane), factor=1.0 (full effect, same as ipsi).
    // At 90° (directly to the side), factor→0 (minimal concha/pinna effect).
    // For typical 30° speakers: factor ≈ 0.33
    let angle_factor = 1.0 - ((90.0 + speaker_angle_deg) / 180.0).clamp(0.0, 1.0);

    // Ear canal resonance: angle-independent (tube resonance)
    let ear_response = resonance_peak(freq, 2700.0, 1.2, 10.0);

    // Concha resonance: scaled by angle factor
    let concha_gain_db = 5.0 * angle_factor;
    let concha_response = resonance_peak(freq, 4500.0, 1.5, concha_gain_db);

    // Pinna notch: depth scaled by angle factor
    let pinna_gain_db = -6.0 * angle_factor;
    let pinna_response = resonance_peak(freq, 9000.0, 3.0, pinna_gain_db);

    ear_response * concha_response * pinna_response
}

/// Compute the magnitude response of a resonance peak/notch at a given frequency.
///
/// Models a 2nd-order bandpass/notch with given center frequency, Q, and peak gain in dB.
/// Returns linear gain at the specified frequency.
#[inline]
fn resonance_peak(freq: f32, center_freq: f32, q: f32, gain_db: f32) -> f32 {
    // Normalized frequency ratio
    let f_ratio = freq / center_freq;
    // 2nd-order magnitude response: |H(f)|^2 = 1 / ((1 - f^2/f0^2)^2 + (f/(Q*f0))^2)
    let x = f_ratio * f_ratio;
    let denom = (1.0 - x).powi(2) + (f_ratio / q).powi(2);
    // Normalized shape: 1.0 at center, falls off away from center
    let shape = (f_ratio / q).powi(2) / denom;
    // Convert peak gain from dB to linear and scale by shape
    let peak_linear = 10.0_f32.powf(gain_db / 20.0);
    // Blend: at center freq shape=1.0 → full gain; far away shape→0 → unity gain
    1.0 + (peak_linear - 1.0) * shape
}

// ============================================================================
// Regularization
// ============================================================================

/// Compute frequency-dependent regularization with smooth sigmoid transitions
pub(crate) fn compute_beta_smooth(freq: f32, params: &XtcPluginParams) -> f32 {
    let base = params.beta_base;
    let low_boost = params.beta_low_freq_boost;
    let high_boost = params.beta_high_freq_boost;

    // Smooth low-frequency boost (sigmoid transition around 100Hz)
    // Below ~100Hz, wavelength >> speaker spacing, cancellation is ineffective
    let low_freq_factor = 1.0 + (low_boost - 1.0) * sigmoid_smooth(100.0 - freq, 30.0);

    // Smooth high-frequency boost (sigmoid transition around 12kHz)
    // Head shadowing naturally limits HF cancellation, so we can allow more bandwidth
    let high_freq_factor = 1.0 + (high_boost - 1.0) * sigmoid_smooth(freq - 12000.0, 1500.0);

    base * low_freq_factor * high_freq_factor
}

/// Smooth sigmoid function for gradual transitions
#[inline]
fn sigmoid_smooth(x: f32, width: f32) -> f32 {
    1.0 / (1.0 + (-x / width).exp())
}

// ============================================================================
// Geometry helpers
// ============================================================================

/// Compute path length from speaker at angle theta to ear with offset
#[inline]
fn compute_path_length(distance: f32, theta: f32, ear_offset: f32) -> f32 {
    ((distance * theta.sin() + ear_offset).powi(2) + (distance * theta.cos()).powi(2)).sqrt()
}

// ============================================================================
// Legacy filter computation (used in tests)
// ============================================================================

/// Compute crosstalk cancellation filters in frequency domain (4-filter version for tests)
#[allow(dead_code)]
pub(crate) fn compute_xtc_filters(
    params: &XtcPluginParams,
    sample_rate: u32,
    num_bins: usize,
) -> XtcFilterSet {
    let mut filter_ll = Vec::with_capacity(num_bins);
    let mut filter_lr = Vec::with_capacity(num_bins);
    let mut filter_rl = Vec::with_capacity(num_bins);
    let mut filter_rr = Vec::with_capacity(num_bins);

    // Constants
    let speed_of_sound = 343.0; // m/s at 20°C

    // Geometry with head tracking offsets
    let d = params.distance_m + params.head_offset_z;
    let theta_rad = params.speaker_angle_deg * PI / 180.0;
    let a = params.head_radius_m;
    let x_offset = params.head_offset_x;

    // Compute path lengths (considering head offset)
    // l_ipsi: direct path (same side)
    // l_contra: crosstalk path (opposite side)
    let l_ipsi = ((d * theta_rad.sin() - x_offset).powi(2) + (d * theta_rad.cos()).powi(2)).sqrt();

    let l_contra =
        ((d * theta_rad.sin() + x_offset).powi(2) + (d * theta_rad.cos()).powi(2)).sqrt() + PI * a; // Add head shadow path

    // Time difference
    let delta_t = (l_contra - l_ipsi) / speed_of_sound;

    // Process each frequency bin
    let freq_per_bin = sample_rate as f32 / (2.0 * (num_bins - 1) as f32);

    for bin in 0..num_bins {
        let freq = bin as f32 * freq_per_bin;

        // Transfer function for ipsilateral path (reference = 1)
        let h_ipsi = Complex::new(1.0, 0.0);

        // Transfer function for contralateral path
        // H_contra(f) = g(f) * e^(-j*2*pi*f*delta_t)
        let g = head_shadowing_filter(freq, params);
        let phase = -2.0 * PI * freq * delta_t;
        let h_contra = Complex::new(g * phase.cos(), g * phase.sin());

        // Crosstalk matrix C:
        // C = [[h_ipsi, h_contra],
        //      [h_contra, h_ipsi]]
        //
        // We want to invert C to get the cancellation filters W:
        // W = (C^H * C + β(f) * I)^(-1) * C^H
        //
        // For 2x2 matrix, we can compute this directly:

        // Frequency-dependent regularization
        let beta = compute_beta(freq, params);

        // C^H * C (Hermitian transpose times C)
        // For our symmetric case:
        // C^H * C = [[|h_ipsi|^2 + |h_contra|^2, 2*Re(h_ipsi*h_contra^*)],
        //            [2*Re(h_ipsi*h_contra^*), |h_ipsi|^2 + |h_contra|^2]]

        let h_ipsi_mag_sq = h_ipsi.norm_sqr();
        let h_contra_mag_sq = h_contra.norm_sqr();
        let cross_term = (h_ipsi * h_contra.conj()).re * 2.0;

        let diag = h_ipsi_mag_sq + h_contra_mag_sq + beta;
        let off_diag = cross_term;

        // Determinant of (C^H * C + β*I)
        let det = diag * diag - off_diag * off_diag;

        if det.abs() < 1e-10 {
            // Singular matrix - use identity (bypass)
            filter_ll.push(Complex::new(1.0, 0.0));
            filter_lr.push(Complex::new(0.0, 0.0));
            filter_rl.push(Complex::new(0.0, 0.0));
            filter_rr.push(Complex::new(1.0, 0.0));
            continue;
        }

        // Inverse of (C^H * C + β*I)
        let inv_diag = diag / det;
        let inv_off_diag = -off_diag / det;

        // W = inv(C^H * C + β*I) * C^H
        // For our case:
        // C^H = [[h_ipsi^*, h_contra^*],
        //        [h_contra^*, h_ipsi^*]]

        // W[0,0] = inv_diag * h_ipsi^* + inv_off_diag * h_contra^*
        // W[0,1] = inv_off_diag * h_ipsi^* + inv_diag * h_contra^*
        // W[1,0] = inv_off_diag * h_ipsi^* + inv_diag * h_contra^*
        // W[1,1] = inv_diag * h_ipsi^* + inv_off_diag * h_contra^*

        let h_ipsi_conj = h_ipsi.conj();
        let h_contra_conj = h_contra.conj();

        let w_ll = h_ipsi_conj * inv_diag + h_contra_conj * inv_off_diag;
        let w_lr = h_ipsi_conj * inv_off_diag + h_contra_conj * inv_diag;
        let w_rl = w_lr; // Symmetric
        let w_rr = w_ll; // Symmetric

        filter_ll.push(w_ll);
        filter_lr.push(w_lr);
        filter_rl.push(w_rl);
        filter_rr.push(w_rr);
    }

    (filter_ll, filter_lr, filter_rl, filter_rr)
}

/// Head shadowing filter: low-pass filter modeling high-frequency attenuation
/// as sound diffracts around the head
pub(crate) fn head_shadowing_filter(freq: f32, params: &XtcPluginParams) -> f32 {
    if freq <= 0.0 {
        return 1.0;
    }

    // Simple low-pass model: g(f) = 1 / (1 + (f / f_c)^n)
    // where n is determined by slope
    let f_c = params.head_shadow_cutoff_hz;
    let slope = params.head_shadow_slope_db_per_octave;

    // Convert slope to filter order (approximately)
    let n = slope / 6.0; // 6 dB/octave ≈ 1st order

    let ratio = freq / f_c;

    1.0 / (1.0 + ratio.powf(n))
}

/// Compute frequency-dependent regularization parameter β(f)
pub(crate) fn compute_beta(freq: f32, params: &XtcPluginParams) -> f32 {
    let beta_base = params.beta_base;
    let low_boost = params.beta_low_freq_boost;
    let high_boost = params.beta_high_freq_boost;

    // Bell-shaped boost: stronger regularization at <200Hz and >8kHz
    let low_freq_factor = if freq < 200.0 {
        1.0 + low_boost * (1.0 - freq / 200.0)
    } else {
        1.0
    };

    let high_freq_factor = if freq > 8000.0 {
        1.0 + high_boost * ((freq - 8000.0) / 12000.0).min(1.0)
    } else {
        1.0
    };

    beta_base * low_freq_factor * high_freq_factor
}
