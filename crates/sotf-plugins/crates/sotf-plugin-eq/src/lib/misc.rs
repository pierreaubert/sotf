use math_audio_iir_fir::Biquad;

pub(super) fn default_order() -> usize {
    2
}

/// Butterworth Q values for cascaded biquad sections.
/// For an Nth-order Butterworth filter implemented as N/2 cascaded biquads,
/// each section uses a Q derived from the analog prototype poles.
/// Q_k = 1 / (2 * cos(pi * (2k + 1) / (2N))) for k = 0..N/2-1
pub(super) fn butterworth_q_values(order: usize) -> Vec<f64> {
    let n = order.max(2);
    let num_stages = n / 2;
    (0..num_stages)
        .map(|k| {
            let angle = std::f64::consts::PI * (2 * k + 1) as f64 / (2 * n) as f64;
            1.0 / (2.0 * angle.cos())
        })
        .collect()
}

/// Return whether a high-order cascade uses the user Q as a bandwidth/phase
/// scale on top of the Butterworth pole staggering.
pub(super) fn scales_prototype_q(filter_type: math_audio_iir_fir::BiquadFilterType) -> bool {
    matches!(
        filter_type,
        math_audio_iir_fir::BiquadFilterType::Peak
            | math_audio_iir_fir::BiquadFilterType::PeakMatched
            | math_audio_iir_fir::BiquadFilterType::Bandpass
            | math_audio_iir_fir::BiquadFilterType::Notch
            | math_audio_iir_fir::BiquadFilterType::AllPass
    )
}

/// Recover the host-visible Q from a realized cascade.
pub(super) fn band_user_q(stages: &[Biquad], order: usize) -> f64 {
    let Some(primary) = stages.first() else {
        return 1.0;
    };
    if order > 2 && scales_prototype_q(primary.filter_type) {
        let prototype_q = butterworth_q_values(order).first().copied().unwrap_or(1.0);
        primary.q / prototype_q
    } else {
        primary.q
    }
}

/// Helper: create cascaded biquad stages for a given order.
/// For order=2, returns a single biquad with the original Q.
/// For order=4/6/8, returns N/2 biquads with Butterworth Q staggering,
/// each with gain_db split equally across stages.
pub(super) fn create_band_stages(
    filter_type: math_audio_iir_fir::BiquadFilterType,
    freq: f64,
    srate: f64,
    q: f64,
    db_gain: f64,
    order: usize,
) -> Vec<Biquad> {
    let order = order.max(2);
    if order == 2 {
        return vec![Biquad::new(filter_type, freq, srate, q, db_gain)];
    }
    let num_stages = order / 2;
    let bw_qs = butterworth_q_values(order);
    let gain_per_stage = db_gain / num_stages as f64;
    bw_qs
        .iter()
        .map(|&bw_q| {
            // LP/HP/shelves retain a conventional Butterworth alignment.
            // Peak/notch/bandpass/all-pass use Q as a bandwidth/phase scale
            // while retaining the prototype's stable pole staggering.
            let effective_q = if scales_prototype_q(filter_type) {
                q * bw_q
            } else {
                bw_q
            };
            Biquad::new(filter_type, freq, srate, effective_q, gain_per_stage)
        })
        .collect()
}
