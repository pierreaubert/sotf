use super::super::config::XtcPluginParams;
use super::super::reflections::{
    RoomReflectionData, build_reflection_data_image_source, build_reflection_data_ir,
};
use super::apply::apply_beta_freq_boosts;
use super::apply::apply_effort_constraint;
use super::build::build_beta_lut_condition_number;
use super::build::build_pinna_contra_lut;
use super::build::build_pinna_ipsi_lut;
use super::head::head_shadowing_complex;
use super::misc::SPEED_OF_SOUND;
use super::misc::condition_number_2x2;
use super::misc::contralateral_shadow_angle;
use super::misc::sanitize_filter;
use super::misc::sigmoid_smooth;
use super::misc::soft_limit_complex_magnitude;
use super::misc::woodworth_diffraction_path;
use super::types::AsymmetricGeometry;
use super::types::GeometryCache;
use super::types::HrtfTransferFunctions;
use super::types::SymmetricGeometry;
use super::xtc_filters::XtcFilters;
use rustfft::num_complex::Complex;
use std::f32::consts::PI;
use std::sync::Arc;

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
    let beta_lut =
        build_beta_lut_condition_number(num_bins, cache.freq_per_bin, params, Some(hrtf));

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

    // Use a relative threshold to avoid falsely detecting singularity when the
    // transfer function magnitudes are small (e.g., in a deep notch).
    // An absolute 1e-10 triggers for |H| ~ 1e-3 where det ~ 1e-12, even though
    // the matrix is perfectly invertible.
    if det.abs() < 1e-10 * diag.abs() {
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

#[inline]
pub(crate) fn compute_path_length(distance: f32, theta: f32, ear_offset: f32) -> f32 {
    ((distance * theta.sin() + ear_offset).powi(2) + (distance * theta.cos()).powi(2)).sqrt()
}
