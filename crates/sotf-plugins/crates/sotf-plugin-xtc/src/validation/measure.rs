use super::super::config::XtcPluginParams;
use super::super::filters::{
    SPEED_OF_SOUND, XtcFilters, compute_path_length, compute_xtc_filters_full,
    contralateral_shadow_angle, head_shadowing_woodworth, pinna_resonance, pinna_resonance_contra,
    woodworth_diffraction_path,
};
use std::f32::consts::PI;

/// Measure cancellation depth at a specific frequency using pre-computed filters.
///
/// Cancellation depth indicates how well the XTC system suppresses crosstalk.
/// Higher values = better cancellation.
///
/// This function simulates the actual signal path using the SAME transfer function
/// model that the filters were designed for, ensuring accurate measurement.
///
/// Optimization 2: Accepts pre-computed filters to avoid redundant computation.
pub(crate) fn measure_cancellation_depth_db_with_filters(
    filters: &XtcFilters,
    params: &XtcPluginParams,
    sample_rate: u32,
    freq_hz: f32,
    num_bins: usize,
) -> f32 {
    let freq_per_bin = sample_rate as f32 / (2.0 * (num_bins - 1) as f32);
    let bin_idx = (freq_hz / freq_per_bin) as usize;
    let bin_idx = bin_idx.min(num_bins - 1);

    // Use the SAME geometry model as the filter design
    let d = params.distance_m + params.head_offset_z;
    let theta_rad = params.speaker_angle_deg * PI / 180.0;
    let a = params.head_radius_m;
    let x_offset = params.head_offset_x;

    // Path lengths - same as compute_xtc_filters_symmetric
    let l_ipsi = compute_path_length(d, theta_rad, -x_offset);
    let diffraction_extra = woodworth_diffraction_path(theta_rad, a);
    let l_contra_full = l_ipsi + diffraction_extra;

    // Distance attenuation ratio
    let amplitude_ratio = l_ipsi / l_contra_full;

    // Geometric time difference
    let delta_t = (l_contra_full - l_ipsi) / SPEED_OF_SOUND;

    // Contralateral shadow angle
    let contra_angle = contralateral_shadow_angle(theta_rad);

    // Head shadowing (same as filter design)
    let g = head_shadowing_woodworth(freq_hz, contra_angle, a) * amplitude_ratio;

    // Phase for contralateral path
    let phase = -2.0 * PI * freq_hz * delta_t;

    // Build complex H_contra (same as filter design)
    let _h_contra_mag = g;
    let h_contra_real = g * phase.cos();
    let h_contra_imag = g * phase.sin();

    // Pinna effects (same as filter design)
    let pinna_ipsi = if params.pinna_model_enabled {
        pinna_resonance(freq_hz)
    } else {
        1.0
    };
    let pinna_contra = if params.pinna_model_enabled {
        pinna_resonance_contra(freq_hz, params.speaker_angle_deg)
    } else {
        1.0
    };

    // Apply pinna to get the final transfer functions
    // H_ipsi_shaped = 1.0 * pinna_ipsi
    // H_contra_shaped = h_contra * pinna_contra
    let h_ipsi_shaped_mag = pinna_ipsi;
    let h_contra_shaped_real = h_contra_real * pinna_contra;
    let h_contra_shaped_imag = h_contra_imag * pinna_contra;

    // Crosstalk WITHOUT XTC: |H_contra_shaped|
    let crosstalk_without = (h_contra_shaped_real.powi(2) + h_contra_shaped_imag.powi(2)).sqrt();

    // Get the filters
    let w_ll = &filters.filter_ll[bin_idx];
    let w_lr = &filters.filter_lr[bin_idx];

    // Crosstalk WITH XTC:
    // The filters were designed for H_ipsi_shaped and H_contra_shaped.
    // For a unit input at left speaker intended for left ear:
    // - Right output (crosstalk) should be ~0
    //
    // Using the same formulation as filter design:
    // crosstalk_residue = |W_ll * H_contra_shaped + W_lr * H_ipsi_shaped|
    let h_contra_complex =
        rustfft::num_complex::Complex::new(h_contra_shaped_real, h_contra_shaped_imag);
    let h_ipsi_complex = rustfft::num_complex::Complex::new(h_ipsi_shaped_mag, 0.0);

    let residue = w_ll * h_contra_complex + w_lr * h_ipsi_complex;
    let crosstalk_with = residue.norm();

    if crosstalk_with < 1e-10 {
        return 40.0; // Essentially perfect cancellation
    }

    // Cancellation depth = how much crosstalk was reduced
    let depth = 20.0 * (crosstalk_without / crosstalk_with).log10();
    depth.clamp(0.0, 40.0)
}

/// Measure cancellation depth at a specific frequency.
///
/// Convenience wrapper that computes filters internally.
/// For batch measurements, use `measure_cancellation_depth_db_with_filters` instead.
pub fn measure_cancellation_depth_db(
    params: &XtcPluginParams,
    sample_rate: u32,
    freq_hz: f32,
) -> f32 {
    let fft_size = 2048;
    let num_bins = fft_size / 2 + 1;
    let filters = compute_xtc_filters_full(params, sample_rate, num_bins);
    measure_cancellation_depth_db_with_filters(&filters, params, sample_rate, freq_hz, num_bins)
}

/// Measure cancellation depth across the frequency spectrum.
///
/// Returns (frequency, depth_db) pairs for analysis.
///
/// Optimization 2: Computes filters once and reuses for all frequency points.
pub fn measure_cancellation_depth_spectrum(
    params: &XtcPluginParams,
    sample_rate: u32,
    freq_points: &[f32],
) -> Vec<(f32, f32)> {
    let fft_size = 2048;
    let num_bins = fft_size / 2 + 1;
    let filters = compute_xtc_filters_full(params, sample_rate, num_bins);

    freq_points
        .iter()
        .map(|&freq| {
            (
                freq,
                measure_cancellation_depth_db_with_filters(
                    &filters,
                    params,
                    sample_rate,
                    freq,
                    num_bins,
                ),
            )
        })
        .collect()
}
