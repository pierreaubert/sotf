//\! Crossover and group consistency utilities
//\!
//\! Provides curve splitting, crossover filter response computation, and validation
//\! functions for multi-speaker groups.

use crate::Curve;
use log::warn;
use math_audio_dsp::analysis::compute_average_response;
use std::collections::HashMap;

#[allow(dead_code)]
pub(super) fn split_curve_at_frequency(curve: &Curve, crossover_freq: f64) -> (Curve, Curve) {
    // Find the index where frequency exceeds crossover
    let split_idx = curve
        .freq
        .iter()
        .position(|&f| f >= crossover_freq)
        .unwrap_or(curve.freq.len());

    // Include some overlap around crossover for better optimization
    let overlap_points = 3; // Include a few points on each side
    let low_end = (split_idx + overlap_points).min(curve.freq.len());
    let high_start = split_idx.saturating_sub(overlap_points);

    let low_curve = Curve {
        freq: curve.freq.slice(ndarray::s![..low_end]).to_owned(),
        spl: curve.spl.slice(ndarray::s![..low_end]).to_owned(),
        phase: curve
            .phase
            .as_ref()
            .map(|p| p.slice(ndarray::s![..low_end]).to_owned()),
        ..Default::default()
    };

    let high_curve = Curve {
        freq: curve.freq.slice(ndarray::s![high_start..]).to_owned(),
        spl: curve.spl.slice(ndarray::s![high_start..]).to_owned(),
        phase: curve
            .phase
            .as_ref()
            .map(|p| p.slice(ndarray::s![high_start..]).to_owned()),
        ..Default::default()
    };

    (low_curve, high_curve)
}

/// Compute Linkwitz-Riley 24dB/oct crossover filter responses
///
/// Returns (lowpass_response, highpass_response) as complex vectors
///
/// LR24 consists of two cascaded 2nd-order Butterworth filters.
/// This implementation computes the actual complex response including phase,
/// which is critical for accurate band summation in hybrid mode.
#[allow(dead_code)]
pub(super) fn compute_lr24_crossover_responses(
    frequencies: &ndarray::Array1<f64>,
    crossover_freq: f64,
    sample_rate: f64,
) -> (
    Vec<num_complex::Complex<f64>>,
    Vec<num_complex::Complex<f64>>,
) {
    use math_audio_iir_fir::{Biquad, BiquadFilterType};

    // LR24 = two cascaded Butterworth LP2 filters (Q = 0.7071 each)
    // For LR24 lowpass: two 2nd-order Butterworth lowpass filters in series
    // For LR24 highpass: two 2nd-order Butterworth highpass filters in series

    let q = std::f64::consts::FRAC_1_SQRT_2; // Q = 0.7071 for Butterworth

    // Create biquad filters for lowpass (2 cascaded)
    let lp1 = Biquad::new(
        BiquadFilterType::Lowpass,
        crossover_freq,
        sample_rate,
        q,
        0.0,
    );
    let lp2 = Biquad::new(
        BiquadFilterType::Lowpass,
        crossover_freq,
        sample_rate,
        q,
        0.0,
    );

    // Create biquad filters for highpass (2 cascaded)
    let hp1 = Biquad::new(
        BiquadFilterType::Highpass,
        crossover_freq,
        sample_rate,
        q,
        0.0,
    );
    let hp2 = Biquad::new(
        BiquadFilterType::Highpass,
        crossover_freq,
        sample_rate,
        q,
        0.0,
    );

    let mut lp_resp = Vec::with_capacity(frequencies.len());
    let mut hp_resp = Vec::with_capacity(frequencies.len());

    for &freq in frequencies.iter() {
        // Compute cascaded response: H_lp = H_lp1 * H_lp2
        let lp1_resp = lp1.complex_response(freq);
        let lp2_resp = lp2.complex_response(freq);
        let lp_total = lp1_resp * lp2_resp;

        // Compute cascaded response: H_hp = H_hp1 * H_hp2
        let hp1_resp = hp1.complex_response(freq);
        let hp2_resp = hp2.complex_response(freq);
        let hp_total = hp1_resp * hp2_resp;

        lp_resp.push(lp_total);
        hp_resp.push(hp_total);
    }

    (lp_resp, hp_resp)
}

/// Perform consistency checks between speakers in the same Acoustic Group
pub(super) fn check_group_consistency(
    group_name: &str,
    channels: &[String],
    channel_means: &HashMap<String, f64>,
    curves: &HashMap<String, Curve>,
) {
    if channels.len() < 2 {
        return;
    }

    // 1. Range Difference Check (3 dB threshold)
    let mut means = Vec::new();
    for ch in channels {
        if let Some(&mean) = channel_means.get(ch) {
            means.push((ch, mean));
        }
    }

    for i in 0..means.len() {
        for j in i + 1..means.len() {
            let (ch1, m1) = means[i];
            let (ch2, m2) = means[j];
            let diff = (m1 - m2).abs();
            if diff > 3.0 {
                warn!(
                    "Speaker group '{}' has significant difference: range SPL between '{}' and '{}' is {:.1} dB (> 3.0 dB threshold).",
                    group_name, ch1, ch2, diff
                );
            }
        }
    }

    // 2. Octave-Wise Difference Check (6 dB threshold)
    // Compare all pairs in the group
    for i in 0..channels.len() {
        for j in i + 1..channels.len() {
            let ch1 = &channels[i];
            let ch2 = &channels[j];
            if let (Some(curve1), Some(curve2)) = (curves.get(ch1), curves.get(ch2)) {
                check_octave_consistency(group_name, ch1, ch2, curve1, curve2);
            }
        }
    }
}

/// Check if two curves are consistent across all octaves (6 dB threshold)
pub(super) fn check_octave_consistency(
    group_name: &str,
    ch1: &str,
    ch2: &str,
    curve1: &Curve,
    curve2: &Curve,
) {
    // Standard acoustic octaves
    let octave_centers = [
        31.25, 62.5, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
    ];

    for &center in &octave_centers {
        let f_min = center / 2.0_f64.sqrt();
        let f_max = center * 2.0_f64.sqrt();

        // Find overlap range
        let start_freq = f_min.max(curve1.freq[0]).max(curve2.freq[0]);
        let end_freq = f_max
            .min(curve1.freq[curve1.freq.len() - 1])
            .min(curve2.freq[curve2.freq.len() - 1]);

        if end_freq <= start_freq * 1.1 {
            continue; // Not enough bandwidth in this octave for comparison
        }

        // Compute average SPL for this octave in both curves
        let freqs1_f32: Vec<f32> = curve1.freq.iter().map(|&f| f as f32).collect();
        let spl1_f32: Vec<f32> = curve1.spl.iter().map(|&s| s as f32).collect();
        let freqs2_f32: Vec<f32> = curve2.freq.iter().map(|&f| f as f32).collect();
        let spl2_f32: Vec<f32> = curve2.spl.iter().map(|&s| s as f32).collect();

        let range = Some((start_freq as f32, end_freq as f32));
        let avg1 = compute_average_response(&freqs1_f32, &spl1_f32, range);
        let avg2 = compute_average_response(&freqs2_f32, &spl2_f32, range);

        let diff = (avg1 - avg2).abs() as f64;
        if diff > 6.0 {
            warn!(
                "Speaker group '{}' has significant difference: octave around {:.0} Hz between '{}' and '{}' differs by {:.1} dB (> 6.0 dB threshold).",
                group_name, center, ch1, ch2, diff
            );
        }
    }
}
