use super::super::config::XtcPluginParams;
use super::apply::apply_beta_freq_boosts;
use super::compute::compute_beta_condition_number_full;
use super::resonance::resonance_peak_precomputed;
use super::types::HrtfTransferFunctions;

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
