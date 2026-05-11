//! Crosstalk cancellation filter computation.
//!
//! Contains all DSP math for computing XTC filters in the frequency domain,
//! including head shadowing models, regularization, and spectral normalization.

use super::config::XtcPluginParams;
use super::reflections::{
    RoomReflectionData, build_reflection_data_image_source, build_reflection_data_ir,
};
use rustfft::num_complex::Complex;
use std::f32::consts::PI;
use std::sync::Arc;

/// Pre-computed HRTF transfer functions for the XTC plant matrix.
///
/// When a SOFA/HRTF file is loaded, these frequency-domain transfer functions
/// replace the Woodworth analytical model for computing the crosstalk matrix C(f).
#[derive(Clone)]
pub(crate) struct HrtfTransferFunctions {
    /// Speaker L -> Left ear (ipsilateral)
    pub h_ll: Vec<Complex<f32>>,
    /// Speaker R -> Left ear (contralateral)
    pub h_lr: Vec<Complex<f32>>,
    /// Speaker L -> Right ear (contralateral)
    pub h_rl: Vec<Complex<f32>>,
    /// Speaker R -> Right ear (ipsilateral)
    pub h_rr: Vec<Complex<f32>>,
}

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
    let diffraction_extra = woodworth_diffraction_path(theta_rad, a);
    let l_contra_full = l_ipsi + diffraction_extra;
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
        let l_left_contra_ipsi = compute_path_length(d, theta_right, x_offset); // distance from right speaker to right ear
        let diffraction_left = woodworth_diffraction_path(theta_right.abs(), a);
        let l_left_contra_full = l_left_contra_ipsi + diffraction_left;
        let amplitude_ratio_left = l_left_ipsi / l_left_contra_full;
        let delay_left_ipsi = l_left_ipsi / SPEED_OF_SOUND;
        let delay_left_contra = l_left_contra_full / SPEED_OF_SOUND;
        let angle_left_contra = contralateral_shadow_angle(theta_right.abs());

        let l_right_ipsi = compute_path_length(d, theta_right, x_offset);
        let l_right_contra_ipsi = compute_path_length(d, theta_left, -x_offset); // distance from left speaker to left ear
        let diffraction_right = woodworth_diffraction_path(theta_left.abs(), a);
        let l_right_contra_full = l_right_contra_ipsi + diffraction_right;
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
    /// Optional RoomEQ-recommended matrix filters.
    ///
    /// Shape is `speaker_outputs x 2 input ears`; each entry is an RFFT
    /// half-spectrum. When present, processing maps stereo ear-intent input to
    /// N speaker outputs.
    pub speaker_filters: Option<Vec<[Vec<Complex<f32>>; 2]>>,
}

impl XtcFilters {
    pub(crate) fn output_channels(&self) -> usize {
        self.speaker_filters
            .as_ref()
            .map(|filters| filters.len())
            .unwrap_or(2)
    }
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
    compute_xtc_filters_full_with_cache_and_hrtf(
        params,
        _sample_rate,
        num_bins,
        cache,
        room_data,
        None,
    )
}

/// Internal filter computation with pre-computed geometry cache and optional HRTF data.
pub(crate) fn compute_xtc_filters_full_with_cache_and_hrtf(
    params: &XtcPluginParams,
    _sample_rate: u32,
    num_bins: usize,
    cache: &GeometryCache,
    room_data: Option<Arc<RoomReflectionData>>,
    hrtf_data: Option<&HrtfTransferFunctions>,
) -> XtcFilters {
    let is_symmetric = cache.asymmetric.is_none() && hrtf_data.is_none();
    let room_data_ref: Option<&RoomReflectionData> = room_data
        .as_ref()
        .map(|r: &Arc<RoomReflectionData>| r.as_ref());

    let mut filters = if let Some(hrtf) = hrtf_data {
        // HRTF mode: always use full 4-filter asymmetric path since HRTF
        // data is inherently asymmetric (different left/right ear responses)
        compute_xtc_filters_hrtf(params, num_bins, cache, room_data_ref, hrtf)
    } else if is_symmetric {
        // Use optimized symmetric computation
        let (filter_ll, filter_lr) =
            compute_xtc_filters_symmetric_with_cache(params, num_bins, cache, room_data_ref);
        XtcFilters {
            filter_ll,
            filter_lr,
            filter_rl: None,
            filter_rr: None,
            is_symmetric: true,
            speaker_filters: None,
        }
    } else {
        // Full asymmetric computation for yaw != 0
        let mut f =
            compute_xtc_filters_asymmetric_with_cache(params, num_bins, cache, room_data_ref);
        f.is_symmetric = false;
        f
    };

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

        // Left ear: ipsi speaker is left speaker, contra is right speaker.
        // Use relative transfer functions: ipsilateral path is our reference.
        let h_ll_ipsi = Complex::new(1.0, 0.0) * pinna_ipsi;
        let pinna_left_contra = pinna_left_contra_lut.as_deref().map_or(1.0, |lut| lut[bin]);
        let delta_t_left = delay_left_contra - delay_left_ipsi;
        let shadow_ll = head_shadowing_complex(freq, asym.angle_left_contra, a, params.head_model)
            * asym.amplitude_ratio_left;
        let phase_ll_contra = -2.0 * PI * freq * delta_t_left;
        let path_ll = Complex::new(phase_ll_contra.cos(), phase_ll_contra.sin());
        let h_ll_contra = shadow_ll * path_ll * pinna_left_contra;

        // Right ear: ipsi speaker is right speaker, contra is left speaker
        let h_rr_ipsi = Complex::new(1.0, 0.0) * pinna_ipsi;
        let pinna_right_contra = pinna_right_contra_lut
            .as_deref()
            .map_or(1.0, |lut| lut[bin]);
        let delta_t_right = delay_right_contra - delay_right_ipsi;
        let shadow_rr = head_shadowing_complex(freq, asym.angle_right_contra, a, params.head_model)
            * asym.amplitude_ratio_right;
        let phase_rr_contra = -2.0 * PI * freq * delta_t_right;
        let path_rr = Complex::new(phase_rr_contra.cos(), phase_rr_contra.sin());
        let h_rr_contra = shadow_rr * path_rr * pinna_right_contra;

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

        // Condition-number based regularization (Phase 2)
        let beta = compute_beta_condition_number_full(
            h_ll_ipsi_final,
            h_ll_contra_final,
            h_rr_contra_final,
            h_rr_ipsi_final,
            freq,
            params,
        );
        let beta = if let Some(room) = room_data {
            beta * room.beta_boost[bin]
        } else {
            beta
        };

        // Compute full 2x2 regularized inverse for both ears simultaneously.
        // Speaker L -> Ear L: h_ll_ipsi_final
        // Speaker R -> Ear L: h_ll_contra_final
        // Speaker L -> Ear R: h_rr_contra_final
        // Speaker R -> Ear R: h_rr_ipsi_final
        let (mut w_ll, mut w_lr, mut w_rl, mut w_rr) = compute_full_2x2_inverse(
            h_ll_ipsi_final,
            h_ll_contra_final,
            h_rr_contra_final,
            h_rr_ipsi_final,
            beta,
            max_gain_linear,
            params.bypass_neumann_refinement,
        );

        // Per-bin spectral normalization: target unity gain for the estimated ear response.
        if params.spectral_normalization && !params.bypass_spectral_normalization {
            // Left Ear response for unit Left Input (L_in = 1, R_in = 0)
            // Speakers emit: out_L = w_ll, out_R = w_rl
            // Left ear hears: h_ipsi * w_ll + h_contra * w_rl
            let ear_l = w_ll * h_ll_ipsi_final + w_rl * h_ll_contra_final;

            // Right Ear response for unit Right Input (L_in = 0, R_in = 1)
            // Speakers emit: out_L = w_lr, out_R = w_rr
            // Right ear hears: h_contra * w_lr + h_ipsi * w_rr
            let ear_r = w_lr * h_rr_contra_final + w_rr * h_rr_ipsi_final;

            // Compute both gains before applying, to avoid contamination
            let mag_l = ear_l.norm();
            let gain_l = if mag_l > 0.01 {
                1.0 + 0.9 * ((1.0 / mag_l).clamp(0.5, 4.0) - 1.0)
            } else {
                1.0
            };

            let mag_r = ear_r.norm();
            let gain_r = if mag_r > 0.01 {
                1.0 + 0.9 * ((1.0 / mag_r).clamp(0.5, 4.0) - 1.0)
            } else {
                1.0
            };

            // Scale COLUMNS, not rows:
            // Left column (w_ll, w_rl) → left ear correction
            // Right column (w_lr, w_rr) → right ear correction
            w_ll *= gain_l;
            w_rl *= gain_l;
            w_lr *= gain_r;
            w_rr *= gain_r;

            // Re-apply soft limit: spectral normalization can push gains past budget
            w_ll = soft_limit_complex_magnitude(w_ll, max_gain_linear);
            w_lr = soft_limit_complex_magnitude(w_lr, max_gain_linear);
            w_rl = soft_limit_complex_magnitude(w_rl, max_gain_linear);
            w_rr = soft_limit_complex_magnitude(w_rr, max_gain_linear);
        }

        // Crossfade to identity (stereo passthrough) at very low and very high frequencies
        // to prevent Tikhonov attenuation from muting the audio at band edges.
        let low_fade = 1.0 - sigmoid_smooth(100.0 - freq, 30.0);
        let high_fade = 1.0 - sigmoid_smooth(freq - 12000.0, 1500.0);
        let alpha = low_fade * high_fade; // 1.0 in passband, 0.0 at extreme edges

        let passthrough = Complex::new(1.0 - alpha, 0.0);
        filter_ll[bin] = w_ll * alpha + passthrough;
        filter_lr[bin] = w_lr * alpha;
        filter_rl[bin] = w_rl * alpha;
        filter_rr[bin] = w_rr * alpha + passthrough;
    }

    // Phase 3: effort-constrained gain limiting
    apply_effort_constraint(
        &mut filter_ll,
        &mut filter_lr,
        Some(&mut filter_rl),
        Some(&mut filter_rr),
        max_gain_linear,
    );

    XtcFilters {
        filter_ll,
        filter_lr,
        filter_rl: Some(filter_rl),
        filter_rr: Some(filter_rr),
        is_symmetric: false,
        speaker_filters: None,
    }
}

// ============================================================================
// HRTF-based filters
// ============================================================================

/// Compute XTC filters using measured HRTF data as the plant matrix.
///
/// The HRTF transfer functions replace the Woodworth analytical model.
/// The plant matrix C(f) is:
///   C = [[h_ll, h_lr],   (Speaker L->EarL, Speaker R->EarL)
///        [h_rl, h_rr]]   (Speaker L->EarR, Speaker R->EarR)
///
/// This always produces a full 4-filter (asymmetric) result since measured
/// HRTFs are inherently asymmetric between ears.
fn compute_xtc_filters_hrtf(
    params: &XtcPluginParams,
    num_bins: usize,
    cache: &GeometryCache,
    room_data: Option<&RoomReflectionData>,
    hrtf: &HrtfTransferFunctions,
) -> XtcFilters {
    let mut filter_ll = vec![Complex::new(0.0, 0.0); num_bins];
    let mut filter_lr = vec![Complex::new(0.0, 0.0); num_bins];
    let mut filter_rl = vec![Complex::new(0.0, 0.0); num_bins];
    let mut filter_rr = vec![Complex::new(0.0, 0.0); num_bins];

    let max_gain_linear = 10.0_f32.powf(params.max_gain_db / 20.0);

    // Build condition-number based beta LUT
    let beta_lut = build_beta_lut_condition_number(num_bins, cache.freq_per_bin, params, Some(hrtf));

    for bin in 0..num_bins {
        let freq = bin as f32 * cache.freq_per_bin;

        // Plant matrix from HRTF data
        let h_ll_val = hrtf.h_ll[bin];
        let h_lr_val = hrtf.h_lr[bin];
        let h_rl_val = hrtf.h_rl[bin];
        let h_rr_val = hrtf.h_rr[bin];

        // Integrate room reflections
        let (h_ll_final, h_lr_final, h_rl_final, h_rr_final) = if let Some(room) = room_data {
            (
                h_ll_val + room.h_ll_ipsi[bin],
                h_lr_val + room.h_lr_contra[bin],
                h_rl_val + room.h_rl_contra[bin],
                h_rr_val + room.h_rr_ipsi[bin],
            )
        } else {
            (h_ll_val, h_lr_val, h_rl_val, h_rr_val)
        };

        let beta = if let Some(room) = room_data {
            beta_lut[bin] * room.beta_boost[bin]
        } else {
            beta_lut[bin]
        };

        let (mut w_ll, mut w_lr, mut w_rl, mut w_rr) = compute_full_2x2_inverse(
            h_ll_final,
            h_lr_final,
            h_rl_final,
            h_rr_final,
            beta,
            max_gain_linear,
            params.bypass_neumann_refinement,
        );

        // Per-bin spectral normalization
        if params.spectral_normalization && !params.bypass_spectral_normalization {
            let ear_l = w_ll * h_ll_final + w_rl * h_lr_final;
            let ear_r = w_lr * h_rl_final + w_rr * h_rr_final;

            let mag_l = ear_l.norm();
            let gain_l = if mag_l > 0.01 {
                1.0 + 0.9 * ((1.0 / mag_l).clamp(0.5, 4.0) - 1.0)
            } else {
                1.0
            };

            let mag_r = ear_r.norm();
            let gain_r = if mag_r > 0.01 {
                1.0 + 0.9 * ((1.0 / mag_r).clamp(0.5, 4.0) - 1.0)
            } else {
                1.0
            };

            w_ll *= gain_l;
            w_rl *= gain_l;
            w_lr *= gain_r;
            w_rr *= gain_r;

            w_ll = soft_limit_complex_magnitude(w_ll, max_gain_linear);
            w_lr = soft_limit_complex_magnitude(w_lr, max_gain_linear);
            w_rl = soft_limit_complex_magnitude(w_rl, max_gain_linear);
            w_rr = soft_limit_complex_magnitude(w_rr, max_gain_linear);
        }

        // Crossfade to identity at band edges
        let low_fade = 1.0 - sigmoid_smooth(100.0 - freq, 30.0);
        let high_fade = 1.0 - sigmoid_smooth(freq - 12000.0, 1500.0);
        let alpha = low_fade * high_fade;

        let passthrough = Complex::new(1.0 - alpha, 0.0);
        filter_ll[bin] = w_ll * alpha + passthrough;
        filter_lr[bin] = w_lr * alpha;
        filter_rl[bin] = w_rl * alpha;
        filter_rr[bin] = w_rr * alpha + passthrough;
    }

    // Phase 3: effort-constrained gain limiting
    apply_effort_constraint(
        &mut filter_ll,
        &mut filter_lr,
        Some(&mut filter_rl),
        Some(&mut filter_rr),
        max_gain_linear,
    );

    XtcFilters {
        filter_ll,
        filter_lr,
        filter_rl: Some(filter_rl),
        filter_rr: Some(filter_rr),
        is_symmetric: false,
        speaker_filters: None,
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

    // Explicit ITD delay: ITD = delay_contra - delay_ipsi (seconds).
    // In the frequency domain, a pure time delay of Δt is e^{-j*2π*f*Δt}.
    // Below ~300 Hz the Woodworth implicit phase is numerically inaccurate
    // (wavelength >> head size), so we substitute an explicit delay.
    let itd = sym.delay_contra - sym.delay_ipsi;
    let use_explicit_delay = params.itd_modeling == "explicit_delay";

    for bin in 0..num_bins {
        let freq = bin as f32 * cache.freq_per_bin;

        // Use relative transfer functions: ipsilateral path is our reference (gain 1.0, phase 0)
        let h_ipsi = Complex::new(1.0, 0.0);

        // Contralateral path is relative to ipsilateral.
        let delta_t = sym.delay_contra - sym.delay_ipsi;
        let shadow = head_shadowing_complex(freq, sym.contra_angle, a, params.head_model)
            * sym.amplitude_ratio;
        let phase_contra = -2.0 * PI * freq * delta_t;
        let path_phase = Complex::new(phase_contra.cos(), phase_contra.sin());
        let h_contra_phase_only = shadow * path_phase;

        let h_contra = if use_explicit_delay {
            // Explicit delay mode: model the contralateral path at LF as a pure
            // fractional-sample delay with amplitude 1.0 (no head shadowing).
            //
            // Rationale: at low frequencies (wavelength >> head radius) the head is
            // acoustically transparent — there is no interaural level difference (ILD),
            // only an interaural time difference (ITD). The Woodworth head-shadowing
            // factor `g` drops to ~1.0 at LF anyway, but the explicit-delay model makes
            // this physically exact by always using unity amplitude for the delay phasor:
            //   h_contra_explicit = e^{-j*2π*f*itd}   (amplitude = 1, pure delay)
            //
            // Sigmoid crossover: blend = 1 at LF (use explicit), 0 at HF (use phase-only).
            // Crossover at 300 Hz, transition width 50 Hz.
            // sigmoid(x) = 1/(1 + exp((x - x0) / w)) where x = freq, x0 = 300, w = 50.
            let explicit_phase = -2.0 * PI * freq * itd;
            // Unit-amplitude phasor: no head-shadowing amplitude factor at LF.
            let h_contra_explicit = Complex::new(explicit_phase.cos(), explicit_phase.sin());

            let crossover_hz = 300.0_f32;
            let blend = 1.0 / (1.0 + ((freq - crossover_hz) / 50.0).exp());

            h_contra_explicit * blend + h_contra_phase_only * (1.0 - blend)
        } else {
            h_contra_phase_only
        };

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

        // Recompute condition-number beta with final (shaped+room) transfer functions
        let beta = compute_beta_condition_number(h_ipsi_final, h_contra_final, freq, params);
        let beta = if let Some(room) = room_data {
            beta * room.beta_boost[bin]
        } else {
            beta
        };

        // Use shared 2x2 inverse computation with gain clamping
        let (mut w_ll, mut w_lr) = compute_2x2_inverse(
            h_ipsi_final,
            h_contra_final,
            beta,
            max_gain_linear,
            params.bypass_neumann_refinement,
        );

        // Per-bin spectral normalization: target unity gain for the estimated ear response.
        // This compensates for attenuation introduced by regularization (beta),
        // preventing dull/mono-like sound at low frequencies.
        if params.spectral_normalization && !params.bypass_spectral_normalization {
            let ear_response = w_ll * h_ipsi_final + w_lr * h_contra_final;
            let mag = ear_response.norm();
            if mag > 0.01 {
                // Gentle correction: 90% of the way to unity.
                let correction = (1.0 / mag).clamp(0.5, 4.0);
                let gain = 1.0 + 0.9 * (correction - 1.0);
                w_ll *= gain;
                w_lr *= gain;
            }
            // Re-apply soft limit: spectral normalization can push gains past budget
            w_ll = soft_limit_complex_magnitude(w_ll, max_gain_linear);
            w_lr = soft_limit_complex_magnitude(w_lr, max_gain_linear);
        }

        // Crossfade to identity (stereo passthrough) at very low and very high frequencies
        // to prevent Tikhonov attenuation from muting the audio at band edges.
        let low_fade = 1.0 - sigmoid_smooth(100.0 - freq, 30.0);
        let high_fade = 1.0 - sigmoid_smooth(freq - 12000.0, 1500.0);
        let alpha = low_fade * high_fade; // 1.0 in passband, 0.0 at extreme edges

        let passthrough = Complex::new(1.0 - alpha, 0.0);
        filter_ll[bin] = w_ll * alpha + passthrough;
        filter_lr[bin] = w_lr * alpha;
    }

    // Phase 3: effort-constrained gain limiting
    apply_effort_constraint(&mut filter_ll, &mut filter_lr, None, None, max_gain_linear);

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
/// from regularization, similar to the crosstalk cancellation literature.
///
/// Returns (w_ipsi, w_contra) filter coefficients.
/// max_gain_linear limits the magnitude of each output coefficient.
#[inline]
pub(crate) fn compute_2x2_inverse(
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

    // Per-bin fallback: if refinement increased cancellation error, use first-order.
    // At ill-conditioned bins (spectral radius > ~1.43), Neumann series diverges.
    let w1_ipsi_limited = soft_limit_complex_magnitude(w1_ipsi, max_gain_linear);
    let w1_contra_limited = soft_limit_complex_magnitude(w1_contra, max_gain_linear);
    let w2_ipsi_limited = soft_limit_complex_magnitude(w2_ipsi, max_gain_linear);
    let w2_contra_limited = soft_limit_complex_magnitude(w2_contra, max_gain_linear);

    let identity = Complex::new(1.0, 0.0);
    let err1_diag = h_ipsi * w1_ipsi_limited + h_contra * w1_contra_limited - identity;
    let err1_off = h_ipsi * w1_contra_limited + h_contra * w1_ipsi_limited;
    let err1_sq = err1_diag.norm_sqr() + err1_off.norm_sqr();

    let err2_diag = h_ipsi * w2_ipsi_limited + h_contra * w2_contra_limited - identity;
    let err2_off = h_ipsi * w2_contra_limited + h_contra * w2_ipsi_limited;
    let err2_sq = err2_diag.norm_sqr() + err2_off.norm_sqr();

    if err2_sq <= err1_sq {
        (w2_ipsi_limited, w2_contra_limited)
    } else {
        (w1_ipsi_limited, w1_contra_limited)
    }
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
    bypass_neumann: bool,
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
        return (
            Complex::new(1.0, 0.0),
            Complex::new(0.0, 0.0),
            Complex::new(0.0, 0.0),
            Complex::new(1.0, 0.0),
        );
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

    let w1_ll = inv_a_00 * h_ll.conj() + inv_a_01 * h_lr.conj();
    let w1_lr = inv_a_00 * h_rl.conj() + inv_a_01 * h_rr.conj();
    let w1_rl = inv_a_10 * h_ll.conj() + inv_a_11 * h_lr.conj();
    let w1_rr = inv_a_10 * h_rl.conj() + inv_a_11 * h_rr.conj();

    if bypass_neumann {
        let w1_ll_limit = soft_limit_complex_magnitude(w1_ll, max_gain_linear);
        let w1_lr_limit = soft_limit_complex_magnitude(w1_lr, max_gain_linear);
        let w1_rl_limit = soft_limit_complex_magnitude(w1_rl, max_gain_linear);
        let w1_rr_limit = soft_limit_complex_magnitude(w1_rr, max_gain_linear);
        return (w1_ll_limit, w1_lr_limit, w1_rl_limit, w1_rr_limit);
    }

    // Neumann series refinement: W₂ = W₁ * (2I - C*W₁)
    // C = [[h_ll, h_lr], [h_rl, h_rr]]
    // W1 = [[w1_ll, w1_lr], [w1_rl, w1_rr]]

    // Compute C * W1
    let cw_00 = h_ll * w1_ll + h_lr * w1_rl;
    let cw_01 = h_ll * w1_lr + h_lr * w1_rr;
    let cw_10 = h_rl * w1_ll + h_rr * w1_rl;
    let cw_11 = h_rl * w1_lr + h_rr * w1_rr;

    // R = 2I - C * W1
    let r_00 = Complex::new(2.0, 0.0) - cw_00;
    let r_01 = Complex::new(0.0, 0.0) - cw_01;
    let r_10 = Complex::new(0.0, 0.0) - cw_10;
    let r_11 = Complex::new(2.0, 0.0) - cw_11;

    // W2 = W1 * R
    // W2[0,0] = w1_ll * r_00 + w1_lr * r_10
    // W2[0,1] = w1_ll * r_01 + w1_lr * r_11
    // W2[1,0] = w1_rl * r_00 + w1_rr * r_10
    // W2[1,1] = w1_rl * r_01 + w1_rr * r_11
    let w2_ll_full = w1_ll * r_00 + w1_lr * r_10;
    let w2_lr_full = w1_ll * r_01 + w1_lr * r_11;
    let w2_rl_full = w1_rl * r_00 + w1_rr * r_10;
    let w2_rr_full = w1_rl * r_01 + w1_rr * r_11;

    // Dampen refinement: blend 70% between first-order (w1) and full refinement (w2).
    let w2_ll = w1_ll + (w2_ll_full - w1_ll) * 0.7;
    let w2_lr = w1_lr + (w2_lr_full - w1_lr) * 0.7;
    let w2_rl = w1_rl + (w2_rl_full - w1_rl) * 0.7;
    let w2_rr = w1_rr + (w2_rr_full - w1_rr) * 0.7;

    let w1_ll_limit = soft_limit_complex_magnitude(w1_ll, max_gain_linear);
    let w1_lr_limit = soft_limit_complex_magnitude(w1_lr, max_gain_linear);
    let w1_rl_limit = soft_limit_complex_magnitude(w1_rl, max_gain_linear);
    let w1_rr_limit = soft_limit_complex_magnitude(w1_rr, max_gain_linear);

    let w2_ll_limit = soft_limit_complex_magnitude(w2_ll, max_gain_linear);
    let w2_lr_limit = soft_limit_complex_magnitude(w2_lr, max_gain_linear);
    let w2_rl_limit = soft_limit_complex_magnitude(w2_rl, max_gain_linear);
    let w2_rr_limit = soft_limit_complex_magnitude(w2_rr, max_gain_linear);

    let identity = Complex::new(1.0, 0.0);
    // Error for W1 = C*W1 - I
    let err1_00 = h_ll * w1_ll_limit + h_lr * w1_rl_limit - identity;
    let err1_01 = h_ll * w1_lr_limit + h_lr * w1_rr_limit;
    let err1_10 = h_rl * w1_ll_limit + h_rr * w1_rl_limit;
    let err1_11 = h_rl * w1_lr_limit + h_rr * w1_rr_limit - identity;
    let err1_sq = err1_00.norm_sqr() + err1_01.norm_sqr() + err1_10.norm_sqr() + err1_11.norm_sqr();

    // Error for W2 = C*W2 - I
    let err2_00 = h_ll * w2_ll_limit + h_lr * w2_rl_limit - identity;
    let err2_01 = h_ll * w2_lr_limit + h_lr * w2_rr_limit;
    let err2_10 = h_rl * w2_ll_limit + h_rr * w2_rl_limit;
    let err2_11 = h_rl * w2_lr_limit + h_rr * w2_rr_limit - identity;
    let err2_sq = err2_00.norm_sqr() + err2_01.norm_sqr() + err2_10.norm_sqr() + err2_11.norm_sqr();

    if err2_sq <= err1_sq {
        (w2_ll_limit, w2_lr_limit, w2_rl_limit, w2_rr_limit)
    } else {
        (w1_ll_limit, w1_lr_limit, w1_rl_limit, w1_rr_limit)
    }
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
        // Scaled exponent aligned with validation reference data
        let exponent = (ka / 4.0).min(3.0);
        shadow_factor.powf(exponent)
    }
}

/// Dispatch head shadowing based on the configured model.
/// Returns magnitude-only shadow gain (0..1) for API compatibility.
/// head_model: 0 = Woodworth, 1 = Brown-Duda
#[allow(dead_code)]
pub(crate) fn head_shadowing(
    freq: f32,
    angle_rad: f32,
    head_radius: f32,
    head_model: usize,
) -> f32 {
    match head_model {
        1 => head_shadowing_brown_duda(freq, angle_rad, head_radius).0,
        _ => head_shadowing_woodworth(freq, angle_rad, head_radius),
    }
}

/// Dispatch head shadowing as a complex gain.
/// head_model: 0 = Woodworth magnitude only, 1 = Brown-Duda magnitude + phase.
pub(crate) fn head_shadowing_complex(
    freq: f32,
    angle_rad: f32,
    head_radius: f32,
    head_model: usize,
) -> Complex<f32> {
    match head_model {
        1 => {
            let (magnitude, phase) = head_shadowing_brown_duda(freq, angle_rad, head_radius);
            Complex::new(magnitude * phase.cos(), magnitude * phase.sin())
        }
        _ => Complex::new(head_shadowing_woodworth(freq, angle_rad, head_radius), 0.0),
    }
}

/// Brown & Duda (1998) rigid-sphere head diffraction model.
///
/// Returns a Complex gain representing both ILD (magnitude) and ITD (phase)
/// for the contralateral path. This is more accurate than Woodworth above ~1.5kHz
/// because it models frequency-dependent diffraction around a rigid sphere.
///
/// Reference: Brown, C.P. & Duda, R.O. (1998). "A structural model for binaural
/// sound synthesis." IEEE Trans. Speech & Audio Processing, 6(5), 476-488.
pub(crate) fn head_shadowing_brown_duda(freq: f32, angle_rad: f32, head_radius: f32) -> (f32, f32) {
    if freq <= 0.0 {
        return (1.0, 0.0);
    }

    let theta = angle_rad.abs();
    let w = 2.0 * PI * freq;
    let a = head_radius;
    let c = SPEED_OF_SOUND;

    // Normalized frequency parameter
    let w0 = c / a; // characteristic frequency of the head

    // --- ILD: Head shadow magnitude (Brown & Duda Eq. 2) ---
    // The magnitude transfer function is approximated by:
    //   |H(w, theta)| = alpha_min + (1 - alpha_min) * cos(theta/2)
    // where alpha_min depends on frequency:
    //   alpha_min = 1.0 / (1.0 + (w / w0)^2 / 4.0)^0.5
    // This gives ~0dB at low frequencies and increasing attenuation at high frequencies.
    let mu = (w / w0).min(20.0); // normalized frequency, capped for stability
    // Brown-Duda magnitude model (rigid-sphere diffraction approximation):
    // At low freq (mu << 1): magnitude ≈ 1 (transparent)
    // At high freq (mu >> 1): magnitude ≈ cos(theta/2) (shadow)
    let alpha_min = (1.0 + mu * mu / 4.0).recip().sqrt(); // ~1 at low freq, ~0 at high freq
    let magnitude = alpha_min + (1.0 - alpha_min) * (theta / 2.0).cos();

    // --- ITD: Interaural time delay (Woodworth formula for ITD) ---
    // Brown & Duda use the Woodworth diffraction path for time delay:
    //   tau(theta) = (a/c) * (theta + sin(theta))  for theta < pi/2
    //   tau(theta) = (a/c) * (pi/2 + sin(theta))    extrapolated
    // This gives the additional path delay for the contralateral ear.
    let tau = if theta <= PI / 2.0 {
        (a / c) * (theta + theta.sin())
    } else {
        (a / c) * (PI / 2.0 + theta.sin())
    };
    let phase = -w * tau; // negative phase = delay

    (magnitude, phase)
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
    // At 0° (median plane), factor=0.5 (partial effect — sound arrives from front, not ear side).
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
    let angle_factor = 1.0 - ((90.0 + speaker_angle_deg) / 180.0).clamp(0.0, 1.0);

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
// Regularization (condition-number based)
// ============================================================================

/// Compute condition number of the 2x2 plant matrix C at a frequency bin.
///
/// For a 2x2 matrix, the condition number is σ_max / σ_min where σ are the
/// singular values. For [[a, b], [c, d]], the singular values can be computed
/// cheaply from the Frobenius norm and determinant.
#[inline]
fn condition_number_2x2(
    h00: Complex<f32>,
    h01: Complex<f32>,
    h10: Complex<f32>,
    h11: Complex<f32>,
) -> f32 {
    // Frobenius norm squared = |h00|^2 + |h01|^2 + |h10|^2 + |h11|^2
    let frob_sq = h00.norm_sqr() + h01.norm_sqr() + h10.norm_sqr() + h11.norm_sqr();
    // |det(C)|^2 = |h00*h11 - h01*h10|^2
    let det = h00 * h11 - h01 * h10;
    let det_sq = det.norm_sqr();

    if det_sq < 1e-20 {
        return 1e6; // Effectively singular
    }

    // For 2x2: σ_max^2 + σ_min^2 = frob_sq, σ_max * σ_min = |det|
    // σ_max^2 = (frob_sq + sqrt(frob_sq^2 - 4*det_sq)) / 2
    // σ_min^2 = (frob_sq - sqrt(frob_sq^2 - 4*det_sq)) / 2
    let disc = (frob_sq * frob_sq - 4.0 * det_sq).max(0.0);
    let disc_sqrt = disc.sqrt();
    let sigma_max_sq = (frob_sq + disc_sqrt) * 0.5;
    let sigma_min_sq = (frob_sq - disc_sqrt).max(1e-20) * 0.5;

    (sigma_max_sq / sigma_min_sq).sqrt()
}

/// Compute condition-number based regularization parameter β(f).
///
/// β(f) = β_base × max(1, κ(f) / κ_target)
///
/// This automatically increases regularization at frequency bins where the
/// plant matrix is ill-conditioned (high condition number), without needing
/// manual low/high frequency boost parameters.
pub(crate) fn compute_beta_condition_number(
    h_ipsi: Complex<f32>,
    h_contra: Complex<f32>,
    freq: f32,
    params: &XtcPluginParams,
) -> f32 {
    // Symmetric plant matrix: C = [[h_ipsi, h_contra], [h_contra, h_ipsi]]
    let kappa = condition_number_2x2(h_ipsi, h_contra, h_contra, h_ipsi);
    let beta = params.beta_base * (kappa / params.kappa_target).max(1.0);
    apply_beta_freq_boosts(beta, freq, params)
}

/// Compute condition-number based beta for a full (asymmetric) 2x2 plant matrix.
pub(crate) fn compute_beta_condition_number_full(
    h00: Complex<f32>,
    h01: Complex<f32>,
    h10: Complex<f32>,
    h11: Complex<f32>,
    freq: f32,
    params: &XtcPluginParams,
) -> f32 {
    let kappa = condition_number_2x2(h00, h01, h10, h11);
    let beta = params.beta_base * (kappa / params.kappa_target).max(1.0);
    apply_beta_freq_boosts(beta, freq, params)
}

/// Apply frequency-dependent beta boosts (low/high) to the base beta value.
#[inline]
fn apply_beta_freq_boosts(beta: f32, freq: f32, params: &XtcPluginParams) -> f32 {
    let low_factor = 1.0
        + params.beta_low_freq_boost
            * (1.0 / (1.0 + (-(100.0 - freq) / 30.0).exp()));
    let high_factor = 1.0
        + params.beta_high_freq_boost
            * (1.0 / (1.0 + (-(freq - 12000.0) / 1500.0).exp()));
    beta * low_factor * high_factor
}

/// Build condition-number based beta LUT using Woodworth model transfer functions.
///
/// Computes the plant matrix at each frequency bin and uses the condition number
/// to set the regularization strength.
pub(crate) fn build_beta_lut_condition_number(
    num_bins: usize,
    freq_per_bin: f32,
    params: &XtcPluginParams,
    hrtf_data: Option<&HrtfTransferFunctions>,
) -> Vec<f32> {
    if let Some(hrtf) = hrtf_data {
        // HRTF mode: use actual HRTF transfer functions for condition number
        (0..num_bins)
            .map(|bin| {
                let freq = bin as f32 * freq_per_bin;
                compute_beta_condition_number_full(
                    hrtf.h_ll[bin],
                    hrtf.h_lr[bin],
                    hrtf.h_rl[bin],
                    hrtf.h_rr[bin],
                    freq,
                    params,
                )
            })
            .collect()
    } else {
        // Woodworth mode: compute analytical plant matrix per bin
        // We need the geometry cache data, but this is called from contexts
        // that already have it. For a standalone LUT, use the params directly.
        (0..num_bins)
            .map(|bin| {
                let freq = bin as f32 * freq_per_bin;
                // Fallback: just use beta_base with frequency boosts
                apply_beta_freq_boosts(params.beta_base, freq, params)
            })
            .collect()
    }
}

/// Legacy compute_beta_smooth for backward compatibility with tests.
#[cfg_attr(not(test), allow(dead_code))]
/// Now implemented via condition-number computation internally but
/// maintains the same sigmoid-based interface for the legacy test path.
pub(crate) fn compute_beta_smooth(freq: f32, params: &XtcPluginParams) -> f32 {
    // For backward compatibility with legacy code paths and tests,
    // use a simplified sigmoid-based approach that approximates the
    // condition-number behavior at band edges.
    let base = params.beta_base;
    let kappa_target = params.kappa_target;

    // Low-frequency boost: condition number grows as wavelength >> head size
    let low_factor = 1.0 + (kappa_target / 10.0) * sigmoid_smooth(100.0 - freq, 30.0);

    // High-frequency boost: head shadowing makes inversion ill-conditioned
    let high_factor = 1.0 + (kappa_target / 10.0) * sigmoid_smooth(freq - 12000.0, 1500.0);

    base * low_factor * high_factor
}

/// Pre-compute beta regularization LUT for all frequency bins (legacy interface).
///
/// Used when Woodworth model is active. The per-bin condition number is computed
/// inline during filter computation for better accuracy.
#[allow(dead_code)]
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

// ============================================================================
// Effort-constrained gain limiting (Phase 3)
// ============================================================================

/// Apply global effort constraint to the inverse filters.
///
/// Computes total filter effort E = Σ|W(f)|² across all bins and all 4 filters.
/// If E > E_max (derived from max_gain_db), scales all bins uniformly:
///   W(f) *= sqrt(E_max / E)
///
/// The per-bin tanh soft limiter is kept as a safety net with a higher threshold.
pub(crate) fn apply_effort_constraint(
    filter_ll: &mut [Complex<f32>],
    filter_lr: &mut [Complex<f32>],
    filter_rl: Option<&mut [Complex<f32>]>,
    filter_rr: Option<&mut [Complex<f32>]>,
    max_gain_linear: f32,
) {
    let num_bins = filter_ll.len();
    // Scale budget by number of active filter arrays (2 for symmetric, 4 for asymmetric/HRTF)
    let num_filters =
        2 + if filter_rl.is_some() { 1 } else { 0 } + if filter_rr.is_some() { 1 } else { 0 };
    let e_max = max_gain_linear * max_gain_linear * num_bins as f32 * num_filters as f32;

    // Compute total effort across all filter components
    let mut total_effort: f32 = 0.0;
    for bin in 0..num_bins {
        total_effort += filter_ll[bin].norm_sqr();
        total_effort += filter_lr[bin].norm_sqr();
    }
    if let Some(ref rl) = filter_rl {
        for c in rl.iter() {
            total_effort += c.norm_sqr();
        }
    }
    if let Some(ref rr) = filter_rr {
        for c in rr.iter() {
            total_effort += c.norm_sqr();
        }
    }

    if total_effort > e_max {
        let scale = (e_max / total_effort).sqrt();
        for c in filter_ll.iter_mut() {
            *c *= scale;
        }
        for c in filter_lr.iter_mut() {
            *c *= scale;
        }
        if let Some(rl) = filter_rl {
            for c in rl.iter_mut() {
                *c *= scale;
            }
        }
        if let Some(rr) = filter_rr {
            for c in rr.iter_mut() {
                *c *= scale;
            }
        }
    }
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

/// Compute frequency-dependent regularization parameter β(f) (legacy interface)
///
/// Now uses kappa_target-derived boost factors instead of separate low/high boost params.
pub(crate) fn compute_beta(freq: f32, params: &XtcPluginParams) -> f32 {
    let beta_base = params.beta_base;
    let kappa_target = params.kappa_target;

    // Approximate the condition-number behavior at band edges:
    // Low frequencies have high condition number due to small ITD phase differences
    let low_freq_factor = if freq < 200.0 {
        1.0 + (kappa_target / 10.0) * (1.0 - freq / 200.0)
    } else {
        1.0
    };

    // High frequencies have high condition number due to head shadowing ambiguity
    let high_freq_factor = if freq > 8000.0 {
        1.0 + (kappa_target / 10.0) * ((freq - 8000.0) / 12000.0).min(1.0)
    } else {
        1.0
    };

    beta_base * low_freq_factor * high_freq_factor
}
