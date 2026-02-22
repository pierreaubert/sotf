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
use std::sync::Arc;

/// Speed of sound at 20°C in m/s
pub(crate) const SPEED_OF_SOUND: f32 = 343.0;

pub(crate) type XtcFilterSet = (
    Vec<Complex<f32>>,
    Vec<Complex<f32>>,
    Vec<Complex<f32>>,
    Vec<Complex<f32>>,
);

/// Cached geometry values to avoid repeated computation in the hot loop.
///
/// Optimization 3: Pre-compute all geometry-dependent values that don't change
/// per frequency bin, avoiding redundant sqrt/trig operations.
pub(crate) struct GeometryCache {
    pub freq_per_bin: f32,
    // Symmetric geometry (yaw ~= 0)
    pub symmetric: SymmetricGeometry,
    // Asymmetric geometry (yaw != 0), computed lazily
    pub asymmetric: Option<AsymmetricGeometry>,
}

/// Geometry values for symmetric XTC (yaw ~= 0).
pub(crate) struct SymmetricGeometry {
    pub a: f32,
    pub amplitude_ratio: f32,
    pub delay_ipsi: f32,
    pub delay_contra: f32,
    pub contra_angle: f32,
}

/// Geometry values for asymmetric XTC (yaw != 0).
pub(crate) struct AsymmetricGeometry {
    pub a: f32,
    pub theta_left: f32,
    pub theta_right: f32,
    pub amplitude_ratio_left: f32,
    pub delay_left_ipsi: f32,
    pub delay_left_contra: f32,
    pub angle_left_contra: f32,
    pub amplitude_ratio_right: f32,
    pub delay_right_ipsi: f32,
    pub delay_right_contra: f32,
    pub angle_right_contra: f32,
}

/// Compute geometry cache for the given parameters.
///
/// Pre-computes all path lengths, angles, and ratios that don't change per bin.
pub(crate) fn compute_geometry_cache(
    params: &XtcPluginParams,
    sample_rate: u32,
    num_bins: usize,
) -> GeometryCache {
    let freq_per_bin = sample_rate as f32 / (2.0 * (num_bins - 1) as f32);

    // Symmetric geometry
    let d = params.distance_m + params.head_offset_z;
    let theta_rad = params.speaker_angle_deg * PI / 180.0;
    let a = params.head_radius_m;
    let x_offset = params.head_offset_x;

    let l_ipsi = compute_path_length(d, theta_rad, -x_offset);
    let l_contra_geometric = compute_path_length(d, theta_rad, x_offset);
    let diffraction_extra = woodworth_diffraction_path(theta_rad, a);
    let l_contra_full = l_contra_geometric + diffraction_extra;
    let amplitude_ratio = l_ipsi / l_contra_full;
    let delay_ipsi = l_ipsi / SPEED_OF_SOUND;
    let delay_contra = l_contra_full / SPEED_OF_SOUND;
    let contra_angle = contralateral_shadow_angle(theta_rad);

    let symmetric = SymmetricGeometry {
        a,
        amplitude_ratio,
        delay_ipsi,
        delay_contra,
        contra_angle,
    };

    // Asymmetric geometry (only compute if yaw != 0)
    let yaw_rad = params.head_yaw_deg * PI / 180.0;
    let asymmetric = if yaw_rad.abs() >= 0.001 {
        let theta_left = theta_rad + yaw_rad;
        let theta_right = theta_rad - yaw_rad;

        let l_left_ipsi = compute_path_length(d, theta_left, -x_offset);
        let l_left_contra_geometric = compute_path_length(d, theta_right, -x_offset);
        let diffraction_left = woodworth_diffraction_path(theta_right, a);
        let l_left_contra_full = l_left_contra_geometric + diffraction_left;
        let amplitude_ratio_left = l_left_ipsi / l_left_contra_full;
        let delay_left_ipsi = l_left_ipsi / SPEED_OF_SOUND;
        let delay_left_contra = l_left_contra_full / SPEED_OF_SOUND;
        let angle_left_contra = contralateral_shadow_angle(theta_right.abs());

        let l_right_ipsi = compute_path_length(d, theta_right, x_offset);
        let l_right_contra_geometric = compute_path_length(d, theta_left, x_offset);
        let diffraction_right = woodworth_diffraction_path(theta_left, a);
        let l_right_contra_full = l_right_contra_geometric + diffraction_right;
        let amplitude_ratio_right = l_right_ipsi / l_right_contra_full;
        let delay_right_ipsi = l_right_ipsi / SPEED_OF_SOUND;
        let delay_right_contra = l_right_contra_full / SPEED_OF_SOUND;
        let angle_right_contra = contralateral_shadow_angle(theta_left.abs());

        Some(AsymmetricGeometry {
            a,
            theta_left,
            theta_right,
            amplitude_ratio_left,
            delay_left_ipsi,
            delay_left_contra,
            angle_left_contra,
            amplitude_ratio_right,
            delay_right_ipsi,
            delay_right_contra,
            angle_right_contra,
        })
    } else {
        None
    };

    GeometryCache {
        freq_per_bin,
        symmetric,
        asymmetric,
    }
}

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
///
/// Optimization 3: Uses pre-computed geometry cache to avoid redundant calculations.
pub(crate) fn compute_xtc_filters_full(
    params: &XtcPluginParams,
    sample_rate: u32,
    num_bins: usize,
) -> XtcFilters {
    // Pre-compute geometry cache (Optimization 3)
    let cache = compute_geometry_cache(params, sample_rate, num_bins);

    // Compute room reflection data if enabled
    let room_data: Option<Arc<RoomReflectionData>> = if params.room_reflections_enabled {
        if let Some(ref ir_path) = params.room_ir_file {
            // No pre-planned FFT available here; build_reflection_data_ir will create one.
            build_reflection_data_ir(ir_path, sample_rate, num_bins, None)
                .ok()
                .map(Arc::new)
        } else {
            Some(Arc::new(build_reflection_data_image_source(
                params,
                sample_rate,
                num_bins,
            )))
        }
    } else {
        None
    };

    compute_xtc_filters_full_with_cache(params, sample_rate, num_bins, &cache, room_data)
}

/// Internal filter computation with pre-computed geometry cache.
///
/// Optimization 4: Accepts optional pre-computed room reflection data to avoid
/// redundant computation when parameters haven't changed.
pub(crate) fn compute_xtc_filters_full_with_cache(
    params: &XtcPluginParams,
    _sample_rate: u32,
    num_bins: usize,
    cache: &GeometryCache,
    room_data: Option<Arc<RoomReflectionData>>,
) -> XtcFilters {
    let is_symmetric = cache.asymmetric.is_none();
    let room_data_ref: Option<&RoomReflectionData> = room_data
        .as_ref()
        .map(|r: &Arc<RoomReflectionData>| r.as_ref());

    let mut filters = if is_symmetric {
        // Use optimized symmetric computation
        let (filter_ll, filter_lr) =
            compute_xtc_filters_symmetric_with_cache(params, num_bins, cache, room_data_ref);
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
            compute_xtc_filters_asymmetric_with_cache(params, num_bins, cache, room_data_ref);
        f.is_symmetric = false;
        f
    };

    // Post-processing: spectral energy normalization to prevent tonal imbalance
    if params.spectral_normalization && !params.bypass_spectral_normalization {
        apply_spectral_normalization(&mut filters, num_bins);
    }

    // Sanitize filter coefficients (NaN/Inf guard)
    sanitize_filter(&mut filters.filter_ll);
    sanitize_filter(&mut filters.filter_lr);
    if let Some(ref mut rl) = filters.filter_rl {
        sanitize_filter(rl);
    }
    if let Some(ref mut rr) = filters.filter_rr {
        sanitize_filter(rr);
    }

    filters
}

// ============================================================================
// Filter sanitization (NaN/Inf guard)
// ============================================================================

/// Replace any NaN or Inf values in filter coefficients with zero.
/// Prevents corrupted filter bins from producing distorted output.
pub(crate) fn sanitize_filter(filter: &mut [Complex<f32>]) {
    for c in filter.iter_mut() {
        if !c.re.is_finite() {
            c.re = 0.0;
        }
        if !c.im.is_finite() {
            c.im = 0.0;
        }
    }
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
            // Apply 80% of the correction (more effective for underwater artifacts)
            gains[bin] = 1.0 + 0.8 * (correction - 1.0);
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

/// Compute asymmetric filters for non-zero yaw angle using pre-computed geometry cache.
///
/// Optimization 5: Uses pre-computed beta and pinna LUTs to avoid per-bin exp()/powf().
fn compute_xtc_filters_asymmetric_with_cache(
    params: &XtcPluginParams,
    num_bins: usize,
    cache: &GeometryCache,
    room_data: Option<&RoomReflectionData>,
) -> XtcFilters {
    let mut filter_ll = vec![Complex::new(0.0, 0.0); num_bins];
    let mut filter_lr = vec![Complex::new(0.0, 0.0); num_bins];
    let mut filter_rl = vec![Complex::new(0.0, 0.0); num_bins];
    let mut filter_rr = vec![Complex::new(0.0, 0.0); num_bins];

    let asym = cache
        .asymmetric
        .as_ref()
        .expect("asymmetric geometry required");
    let a = asym.a;
    let max_gain_linear = 10.0_f32.powf(params.max_gain_db / 20.0);

    // Pre-compute beta LUT: avoids 2× exp() per bin (Optimization 5)
    let beta_lut = build_beta_lut(num_bins, cache.freq_per_bin, params);

    // Pre-compute pinna LUTs: avoids 3× powf() per bin (Optimization 5)
    // Angles are in degrees here: theta_right and theta_left are in radians, convert.
    let pinna_ipsi_lut = if params.pinna_model_enabled {
        Some(build_pinna_ipsi_lut(num_bins, cache.freq_per_bin))
    } else {
        None
    };
    let pinna_left_contra_lut = if params.pinna_model_enabled {
        Some(build_pinna_contra_lut(
            num_bins,
            cache.freq_per_bin,
            asym.theta_right.abs() * 180.0 / PI,
        ))
    } else {
        None
    };
    let pinna_right_contra_lut = if params.pinna_model_enabled {
        Some(build_pinna_contra_lut(
            num_bins,
            cache.freq_per_bin,
            asym.theta_left.abs() * 180.0 / PI,
        ))
    } else {
        None
    };

    for bin in 0..num_bins {
        let freq = bin as f32 * cache.freq_per_bin;

        // Pinna resonance: use pre-computed LUTs (Optimization 5)
        let pinna_ipsi = pinna_ipsi_lut.as_deref().map_or(1.0, |lut| lut[bin]);

        // Use pre-computed geometry values (Optimization 3)
        let delay_left_ipsi = asym.delay_left_ipsi;
        let delay_left_contra = asym.delay_left_contra;
        let delay_right_ipsi = asym.delay_right_ipsi;
        let delay_right_contra = asym.delay_right_contra;

        // Left ear: ipsi speaker is left speaker, contra is right speaker
        let pinna_left_contra = pinna_left_contra_lut.as_deref().map_or(1.0, |lut| lut[bin]);
        let phase_ll_ipsi = -2.0 * PI * freq * delay_left_ipsi;
        let h_ll_ipsi = Complex::new(phase_ll_ipsi.cos(), phase_ll_ipsi.sin()) * pinna_ipsi;
        
        let g_ll =
            head_shadowing_woodworth(freq, asym.angle_left_contra, a) * asym.amplitude_ratio_left;
        let phase_ll_contra = -2.0 * PI * freq * delay_left_contra;
        let h_ll_contra =
            Complex::new(g_ll * phase_ll_contra.cos(), g_ll * phase_ll_contra.sin()) * pinna_left_contra;

        // Right ear: ipsi speaker is right speaker, contra is left speaker
        let pinna_right_contra = pinna_right_contra_lut.as_deref().map_or(1.0, |lut| lut[bin]);
        let phase_rr_ipsi = -2.0 * PI * freq * delay_right_ipsi;
        let h_rr_ipsi = Complex::new(phase_rr_ipsi.cos(), phase_rr_ipsi.sin()) * pinna_ipsi;
        
        let g_rr =
            head_shadowing_woodworth(freq, asym.angle_right_contra, a) * asym.amplitude_ratio_right;
        let phase_rr_contra = -2.0 * PI * freq * delay_right_contra;
        let h_rr_contra =
            Complex::new(g_rr * phase_rr_contra.cos(), g_rr * phase_rr_contra.sin()) * pinna_right_contra;

        // Integrate room reflections: add reflection contributions to transfer functions
        let (h_ll_ipsi_final, h_ll_contra_final, h_rr_ipsi_final, h_rr_contra_final) =
            if let Some(room) = room_data {
                (
                    h_ll_ipsi + room.h_ll_ipsi[bin],
                    h_ll_contra + room.h_lr_contra[bin],
                    h_rr_ipsi + room.h_rr_ipsi[bin],
                    h_rr_contra + room.h_rl_contra[bin],
                )
            } else {
                (h_ll_ipsi, h_ll_contra, h_rr_ipsi, h_rr_contra)
            };

        // Use pre-computed beta LUT (Optimization 5)
        let beta = if let Some(room) = room_data {
            beta_lut[bin] * room.beta_boost[bin]
        } else {
            beta_lut[bin]
        };

        // Compute full 2x2 regularized inverse for both ears simultaneously.
        // Speaker L -> Ear L: h_ll_ipsi_final
        // Speaker R -> Ear L: h_ll_contra_final
        // Speaker L -> Ear R: h_rr_contra_final
        // Speaker R -> Ear R: h_rr_ipsi_final
        let (w_ll, w_lr, w_rl, w_rr) = compute_full_2x2_inverse(
            h_ll_ipsi_final,
            h_ll_contra_final,
            h_rr_contra_final,
            h_rr_ipsi_final,
            beta,
            max_gain_linear,
        );

        filter_ll[bin] = w_ll;
        filter_lr[bin] = w_lr;
        filter_rl[bin] = w_rl;
        filter_rr[bin] = w_rr;
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
///
/// Optimization 3: Uses pre-computed geometry cache.
/// Optimization 5: Uses pre-computed beta and pinna LUTs to avoid per-bin exp()/powf().
fn compute_xtc_filters_symmetric_with_cache(
    params: &XtcPluginParams,
    num_bins: usize,
    cache: &GeometryCache,
    room_data: Option<&RoomReflectionData>,
) -> (Vec<Complex<f32>>, Vec<Complex<f32>>) {
    let mut filter_ll = vec![Complex::new(0.0, 0.0); num_bins];
    let mut filter_lr = vec![Complex::new(0.0, 0.0); num_bins];

    let sym = &cache.symmetric;
    let a = sym.a;
    let max_gain_linear = 10.0_f32.powf(params.max_gain_db / 20.0);

    // Pre-compute beta LUT: avoids 2× exp() per bin (Optimization 5)
    let beta_lut = build_beta_lut(num_bins, cache.freq_per_bin, params);

    // Pre-compute pinna LUTs: avoids 3× powf() per bin (Optimization 5)
    let pinna_ipsi_lut = if params.pinna_model_enabled {
        Some(build_pinna_ipsi_lut(num_bins, cache.freq_per_bin))
    } else {
        None
    };
    let pinna_contra_lut = if params.pinna_model_enabled {
        Some(build_pinna_contra_lut(
            num_bins,
            cache.freq_per_bin,
            params.speaker_angle_deg,
        ))
    } else {
        None
    };

    for bin in 0..num_bins {
        let freq = bin as f32 * cache.freq_per_bin;

        // Transfer function for ipsilateral path
        let phase_ipsi = -2.0 * PI * freq * sym.delay_ipsi;
        let h_ipsi = Complex::new(phase_ipsi.cos(), phase_ipsi.sin());

        // Transfer function for contralateral path using Woodworth model
        let g = head_shadowing_woodworth(freq, sym.contra_angle, a) * sym.amplitude_ratio;
        let phase_contra = -2.0 * PI * freq * sym.delay_contra;
        let h_contra = Complex::new(g * phase_contra.cos(), g * phase_contra.sin());

        // Frequency-dependent regularization: use pre-computed LUT (Optimization 5)
        let beta = beta_lut[bin];

        // Pinna resonance shaping: use pre-computed LUTs (Optimization 5)
        let pinna_ipsi = pinna_ipsi_lut.as_deref().map_or(1.0, |lut| lut[bin]);
        let pinna_contra = pinna_contra_lut.as_deref().map_or(1.0, |lut| lut[bin]);
        let h_ipsi_shaped = h_ipsi * pinna_ipsi;
        let h_contra_shaped = h_contra * pinna_contra;

        // Integrate room reflections: add reflection contributions to transfer functions
        let (h_ipsi_final, h_contra_final) = if let Some(room) = room_data {
            (
                h_ipsi_shaped + room.h_ll_ipsi[bin],
                h_contra_shaped + room.h_lr_contra[bin],
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
        let (w_ll, w_lr) = compute_2x2_inverse(h_ipsi_final, h_contra_final, beta, max_gain_linear, params.bypass_neumann_refinement);

        filter_ll[bin] = w_ll;
        filter_lr[bin] = w_lr;
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
    bypass_neumann: bool,
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

    // Diagnostic bypass: skip Neumann refinement, return first-order inverse only
    if bypass_neumann {
        let w1_ipsi = soft_limit_complex_magnitude(w1_ipsi, max_gain_linear);
        let w1_contra = soft_limit_complex_magnitude(w1_contra, max_gain_linear);
        return (w1_ipsi, w1_contra);
    }

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
    let w2_ipsi_full = w1_ipsi * r_00 + w1_contra * r_01;
    let w2_contra_full = w1_contra * r_00 + w1_ipsi * r_01;

    // Dampen refinement: blend 70% between first-order (w1) and full refinement (w2).
    // Prevents over-amplification at ill-conditioned bins where refinement diverges.
    let w2_ipsi = w1_ipsi + (w2_ipsi_full - w1_ipsi) * 0.7;
    let w2_contra = w1_contra + (w2_contra_full - w1_contra) * 0.7;

    // Soft-limit magnitudes to prevent excessive boost
    let w2_ipsi = soft_limit_complex_magnitude(w2_ipsi, max_gain_linear);
    let w2_contra = soft_limit_complex_magnitude(w2_contra, max_gain_linear);

    (w2_ipsi, w2_contra)
}

/// Compute full 2x2 regularized inverse for asymmetric crosstalk cancellation.
///
/// Matrix C = [[h_ll, h_lr],
///             [h_rl, h_rr]]
///
/// Returns (w_ll, w_lr, w_rl, w_rr) such that W = (C^H * C + beta * I)^-1 * C^H.
#[inline]
fn compute_full_2x2_inverse(
    h_ll: Complex<f32>,
    h_lr: Complex<f32>,
    h_rl: Complex<f32>,
    h_rr: Complex<f32>,
    beta: f32,
    max_gain_linear: f32,
) -> (Complex<f32>, Complex<f32>, Complex<f32>, Complex<f32>) {
    // 1. Compute A = C^H * C + beta * I
    // C^H = [[h_ll*, h_rl*],
    //        [h_lr*, h_rr*]]
    //
    // A_00 = h_ll* * h_ll + h_rl* * h_rl + beta
    // A_01 = h_ll* * h_lr + h_rl* * h_rr
    // A_10 = h_lr* * h_ll + h_rr* * h_rl = A_01*
    // A_11 = h_lr* * h_lr + h_rr* * h_rr + beta
    
    let a_00 = h_ll.norm_sqr() + h_rl.norm_sqr() + beta;
    let a_01 = h_ll.conj() * h_lr + h_rl.conj() * h_rr;
    let a_10 = a_01.conj();
    let a_11 = h_lr.norm_sqr() + h_rr.norm_sqr() + beta;
    
    // 2. Compute inv(A) = (1/det) * [[a_11, -a_01], [-a_10, a_00]]
    let det = a_00 * a_11 - a_01.norm_sqr();
    if det.abs() < 1e-10 {
        return (Complex::new(1.0, 0.0), Complex::new(0.0, 0.0), Complex::new(0.0, 0.0), Complex::new(1.0, 0.0));
    }
    
    let inv_a_00 = a_11 / det;
    let inv_a_01 = -a_01 / det;
    let inv_a_10 = -a_10 / det;
    let inv_a_11 = a_00 / det;
    
    // 3. W = inv(A) * C^H
    // C^H = [[h_ll*, h_rl*],
    //        [h_lr*, h_rr*]]
    //
    // W_ll = inv_a_00 * h_ll* + inv_a_01 * h_lr*
    // W_lr = inv_a_00 * h_rl* + inv_a_01 * h_rr*
    // W_rl = inv_a_10 * h_ll* + inv_a_11 * h_lr*
    // W_rr = inv_a_10 * h_rl* + inv_a_11 * h_rr*
    
    let w_ll = inv_a_00 * h_ll.conj() + inv_a_01 * h_lr.conj();
    let w_lr = inv_a_00 * h_rl.conj() + inv_a_01 * h_rr.conj();
    let w_rl = inv_a_10 * h_ll.conj() + inv_a_11 * h_lr.conj();
    let w_rr = inv_a_10 * h_rl.conj() + inv_a_11 * h_rr.conj();
    
    // 4. Soft-limit magnitudes
    let w_ll = soft_limit_complex_magnitude(w_ll, max_gain_linear);
    let w_lr = soft_limit_complex_magnitude(w_lr, max_gain_linear);
    let w_rl = soft_limit_complex_magnitude(w_rl, max_gain_linear);
    let w_rr = soft_limit_complex_magnitude(w_rr, max_gain_linear);
    
    (w_ll, w_lr, w_rl, w_rr)
}

/// Soft-limit a complex number's magnitude using tanh saturation.
///
/// Below 50% of max_mag: passthrough (no change).
/// Above 50%: smooth tanh curve approaching max_mag asymptotically.
/// Phase is always preserved — only magnitude is affected.
#[inline]
pub(crate) fn soft_limit_complex_magnitude(c: Complex<f32>, max_mag: f32) -> Complex<f32> {
    let mag = c.norm();
    let knee_start = max_mag * 0.5;

    if mag <= knee_start {
        return c;
    }

    // Map the excess above knee_start through tanh for smooth saturation.
    // tanh(x) approaches 1.0 asymptotically, so:
    //   new_mag = knee_start + (max_mag - knee_start) * tanh((mag - knee_start) / (max_mag - knee_start))
    let headroom = max_mag - knee_start;
    let excess = mag - knee_start;
    let new_mag = knee_start + headroom * (excess / headroom).tanh();

    c * (new_mag / mag)
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
    let theta = angle_rad.abs().min(PI);
    // Standard Woodworth formula for spherical head diffraction:
    // extra_path = a * (theta + sin(theta))
    // Valid for all angles from 0 to PI.
    head_radius * (theta + theta.sin())
}

/// Compute the angular separation between a sound source and the contralateral ear.
///
/// For a source at azimuth `speaker_angle` from the median plane, the ipsilateral ear
/// (same side) is at 90° from center, and the contralateral ear (opposite side) is at
/// -90°. The angular separation from source to contralateral ear, measured around the
/// head surface, is approximately PI/2 + speaker_angle.
#[inline]
pub(crate) fn contralateral_shadow_angle(speaker_angle_rad: f32) -> f32 {
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
        // Scaled exponent to match physical ILD measurements (~15-20dB at high ka)
        let exponent = (ka / 1.5).min(15.0);
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
pub(crate) fn pinna_resonance(freq: f32) -> f32 {
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
pub(crate) fn pinna_resonance_contra(freq: f32, speaker_angle_deg: f32) -> f32 {
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
///
/// `peak_linear` must be `10.0_f32.powf(gain_db / 20.0)` — pass it pre-computed to
/// avoid a `powf` call in hot loops.
#[inline]
fn resonance_peak_precomputed(freq: f32, center_freq: f32, q: f32, peak_linear: f32) -> f32 {
    // Normalized frequency ratio
    let f_ratio = freq / center_freq;
    // 2nd-order magnitude response: |H(f)|^2 = 1 / ((1 - f^2/f0^2)^2 + (f/(Q*f0))^2)
    let x = f_ratio * f_ratio;
    let denom = (1.0 - x).powi(2) + (f_ratio / q).powi(2);
    // Normalized shape: 1.0 at center, falls off away from center
    let shape = (f_ratio / q).powi(2) / denom;
    // Blend: at center freq shape=1.0 → full gain; far away shape→0 → unity gain
    1.0 + (peak_linear - 1.0) * shape
}

/// Compute the magnitude response of a resonance peak/notch at a given frequency.
///
/// Models a 2nd-order bandpass/notch with given center frequency, Q, and peak gain in dB.
/// Returns linear gain at the specified frequency.
#[inline]
fn resonance_peak(freq: f32, center_freq: f32, q: f32, gain_db: f32) -> f32 {
    // Convert peak gain from dB to linear
    let peak_linear = 10.0_f32.powf(gain_db / 20.0);
    resonance_peak_precomputed(freq, center_freq, q, peak_linear)
}

/// Pre-compute pinna resonance LUT for ipsilateral paths (angle-independent).
///
/// Avoids 3× `powf` + trig per bin in the filter hot loop.
pub(crate) fn build_pinna_ipsi_lut(num_bins: usize, freq_per_bin: f32) -> Vec<f32> {
    // Pre-compute all peak_linear constants — each is a single powf at init time.
    let peak_ear = 10.0_f32.powf(10.0_f32 / 20.0); // +10 dB
    let peak_concha = 10.0_f32.powf(5.0_f32 / 20.0); // +5 dB
    let peak_pinna = 10.0_f32.powf(-6.0_f32 / 20.0); // -6 dB

    (0..num_bins)
        .map(|bin| {
            let freq = bin as f32 * freq_per_bin;
            if freq <= 0.0 {
                return 1.0;
            }
            let ear = resonance_peak_precomputed(freq, 2700.0, 1.2, peak_ear);
            let concha = resonance_peak_precomputed(freq, 4500.0, 1.5, peak_concha);
            let pinna = resonance_peak_precomputed(freq, 9000.0, 3.0, peak_pinna);
            ear * concha * pinna
        })
        .collect()
}

/// Pre-compute pinna resonance LUT for a contralateral path at a given speaker angle.
///
/// Avoids 3× `powf` + trig per bin in the filter hot loop.
pub(crate) fn build_pinna_contra_lut(
    num_bins: usize,
    freq_per_bin: f32,
    speaker_angle_deg: f32,
) -> Vec<f32> {
    let angle_factor =
        1.0 - ((90.0 + speaker_angle_deg) / 180.0).clamp(0.0, 1.0);

    // Pre-compute all peak_linear constants
    let peak_ear = 10.0_f32.powf(10.0_f32 / 20.0); // +10 dB (angle-independent)
    let peak_concha = 10.0_f32.powf(5.0_f32 * angle_factor / 20.0);
    let peak_pinna = 10.0_f32.powf(-6.0_f32 * angle_factor / 20.0);

    (0..num_bins)
        .map(|bin| {
            let freq = bin as f32 * freq_per_bin;
            if freq <= 0.0 {
                return 1.0;
            }
            let ear = resonance_peak_precomputed(freq, 2700.0, 1.2, peak_ear);
            let concha = resonance_peak_precomputed(freq, 4500.0, 1.5, peak_concha);
            let pinna = resonance_peak_precomputed(freq, 9000.0, 3.0, peak_pinna);
            ear * concha * pinna
        })
        .collect()
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

/// Pre-compute beta regularization LUT for all frequency bins.
///
/// Avoids 2× `exp()` calls per bin via `sigmoid_smooth` in the filter hot loop.
/// Beta depends only on params and bin frequency, not on per-frame data.
pub(crate) fn build_beta_lut(
    num_bins: usize,
    freq_per_bin: f32,
    params: &XtcPluginParams,
) -> Vec<f32> {
    (0..num_bins)
        .map(|bin| {
            let freq = bin as f32 * freq_per_bin;
            compute_beta_smooth(freq, params)
        })
        .collect()
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
pub(crate) fn compute_path_length(distance: f32, theta: f32, ear_offset: f32) -> f32 {
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
    let mut filter_ll = vec![Complex::new(0.0, 0.0); num_bins];
    let mut filter_lr = vec![Complex::new(0.0, 0.0); num_bins];
    let mut filter_rl = vec![Complex::new(0.0, 0.0); num_bins];
    let mut filter_rr = vec![Complex::new(0.0, 0.0); num_bins];

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
            filter_ll[bin] = Complex::new(1.0, 0.0);
            filter_lr[bin] = Complex::new(0.0, 0.0);
            filter_rl[bin] = Complex::new(0.0, 0.0);
            filter_rr[bin] = Complex::new(1.0, 0.0);
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

        filter_ll[bin] = w_ll;
        filter_lr[bin] = w_lr;
        filter_rl[bin] = w_lr;
        filter_rr[bin] = w_ll;
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
