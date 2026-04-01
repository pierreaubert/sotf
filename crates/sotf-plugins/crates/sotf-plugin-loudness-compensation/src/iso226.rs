// ============================================================================
// ISO 226:2003 Equal-Loudness Contour Lookup Table
// ============================================================================
//
// Full implementation of ISO 226:2003 "Acoustics — Normal equal-loudness-level
// contours" using the standard's Table 1 data. Computes equal-loudness contours
// for any phon level in the 20–90 phon range across 29 reference frequencies.
//
// Reference: ISO 226:2003, Equations 1–3 and Table 1.
// ============================================================================

/// ISO 226:2003 reference frequencies (Hz) — 29 points from 20 Hz to 12.5 kHz.
pub const ISO226_FREQS: [f64; 29] = [
    20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0,
    500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0,
    8000.0, 10000.0, 12500.0,
];

/// Number of ISO 226 reference frequencies.
pub const ISO226_NUM_FREQS: usize = 29;

/// ISO 226 exponent alpha_f (Table 1).
const ISO226_ALPHA_F: [f64; 29] = [
    0.532, 0.506, 0.480, 0.455, 0.432, 0.409, 0.387, 0.367, 0.349, 0.330, 0.315, 0.301, 0.288,
    0.276, 0.267, 0.259, 0.253, 0.250, 0.246, 0.244, 0.243, 0.243, 0.243, 0.242, 0.242, 0.245,
    0.254, 0.271, 0.301,
];

/// ISO 226 loudness unit factor L_U(f) (Table 1).
const ISO226_LU: [f64; 29] = [
    -31.6, -27.2, -23.0, -19.1, -15.9, -13.0, -10.3, -8.1, -6.2, -4.5, -3.1, -2.0, -1.1, -0.4, 0.0,
    0.3, 0.5, 0.0, -2.7, -4.1, -1.0, 1.7, 2.5, 1.2, -2.1, -7.1, -11.2, -10.7, -3.1,
];

/// ISO 226 threshold of hearing T_f (Table 1).
const ISO226_TF: [f64; 29] = [
    78.5, 68.7, 59.5, 51.1, 44.0, 37.5, 31.5, 26.5, 22.1, 17.9, 14.4, 11.4, 8.6, 6.2, 4.4, 3.0,
    2.2, 2.4, 3.5, 1.7, -1.3, -4.2, -6.0, -5.4, -1.5, 6.0, 12.6, 13.9, 12.3,
];

/// ISO 226:2003 Eq. 1–3: compute SPL (dB) at frequency index `i` for loudness
/// `phon` (valid for 20–90 phon).
///
/// Returns the sound pressure level in dB SPL that produces the given perceived
/// loudness at the given frequency.
pub fn iso226_spl_at_freq(i: usize, phon: f64) -> f64 {
    let alpha_f = ISO226_ALPHA_F[i];
    let lu = ISO226_LU[i];
    let tf = ISO226_TF[i];

    // Eq. 2: A_f — frequency-dependent amplitude factor
    let af = 4.47e-3 * (10.0_f64.powf(0.025 * phon) - 1.585)
        + 0.4 * 10.0_f64.powf(((tf + lu) / 10.0) - 9.0);

    // Eq. 1: L_f — sound pressure level at frequency f
    (10.0 / alpha_f) * af.log10() - lu + 94.0
}

/// Compute the ISO 226 compensation EQ curve for all 29 reference frequencies.
///
/// Returns an array of (frequency_hz, gain_db) pairs where gain_db is the
/// EQ adjustment to apply when listening at `playback_phon` vs `reference_phon`.
/// The curve is normalized so that 1 kHz (the phon reference) gets 0 dB.
/// Positive values = boost needed, negative = cut needed.
///
/// At lower playback levels, bass and treble get positive gain (boost) because
/// human hearing is less sensitive there at lower SPL. Mid-range near 1 kHz
/// stays at 0 dB.
///
/// When `playback_phon == reference_phon`, all gains are 0 (no compensation).
pub fn compute_iso226_delta(
    playback_phon: f64,
    reference_phon: f64,
) -> [(f64, f64); ISO226_NUM_FREQS] {
    // Index 17 = 1000 Hz — the phon reference frequency
    const IDX_1KHZ: usize = 17;

    // Compute the equal-loudness contour shape at each level.
    // "Shape" = SPL_f - SPL_1kHz at a given phon level.
    // At the reference level, the shape is the baseline.
    // At the playback level, the shape differs.
    // The EQ compensation = shape_play - shape_ref, which tells us how much
    // harder each frequency is to hear (relative to 1 kHz) at the play level
    // vs the reference level.

    let spl_ref_1khz = iso226_spl_at_freq(IDX_1KHZ, reference_phon);
    let spl_play_1khz = iso226_spl_at_freq(IDX_1KHZ, playback_phon);

    let mut deltas = [(0.0, 0.0); ISO226_NUM_FREQS];
    for i in 0..ISO226_NUM_FREQS {
        let spl_ref = iso226_spl_at_freq(i, reference_phon);
        let spl_play = iso226_spl_at_freq(i, playback_phon);

        // Shape at reference level: how much more SPL freq i needs vs 1 kHz
        let shape_ref = spl_ref - spl_ref_1khz;
        // Shape at playback level: same thing at lower volume
        let shape_play = spl_play - spl_play_1khz;

        // Compensation = difference in shape.
        // If shape_play > shape_ref, then at the playback level, this frequency
        // is relatively harder to hear, so we need to boost it.
        let gain = shape_play - shape_ref;
        deltas[i] = (ISO226_FREQS[i], gain);
    }

    deltas
}

/// Interpolate a gain value from the ISO 226 delta table at an arbitrary frequency.
///
/// Uses log-frequency linear interpolation between the two nearest table entries.
/// Frequencies below 20 Hz use the 20 Hz value; above 12.5 kHz use the 12.5 kHz value.
pub fn interpolate_delta(deltas: &[(f64, f64); ISO226_NUM_FREQS], freq: f64) -> f64 {
    // Clamp to table range
    if freq <= deltas[0].0 {
        return deltas[0].1;
    }
    if freq >= deltas[ISO226_NUM_FREQS - 1].0 {
        return deltas[ISO226_NUM_FREQS - 1].1;
    }

    // Find bracketing entries
    for i in 0..ISO226_NUM_FREQS - 1 {
        let (f_lo, d_lo) = deltas[i];
        let (f_hi, d_hi) = deltas[i + 1];
        if freq >= f_lo && freq <= f_hi {
            // Log-frequency interpolation
            let t = (freq.ln() - f_lo.ln()) / (f_hi.ln() - f_lo.ln());
            return d_lo + t * (d_hi - d_lo);
        }
    }

    // Fallback (should not reach here)
    deltas[ISO226_NUM_FREQS - 1].1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iso226_spl_at_1khz_reference() {
        // At 1 kHz, the SPL should equal the phon level (by definition of the phon scale)
        // The 1 kHz index is 17 (0-indexed)
        let spl_60 = iso226_spl_at_freq(17, 60.0);
        // Should be approximately 60 dB SPL at 1 kHz for 60 phon
        assert!(
            (spl_60 - 60.0).abs() < 2.0,
            "At 1 kHz, 60 phon should produce ~60 dB SPL, got {spl_60:.1}"
        );

        let spl_80 = iso226_spl_at_freq(17, 80.0);
        assert!(
            (spl_80 - 80.0).abs() < 2.0,
            "At 1 kHz, 80 phon should produce ~80 dB SPL, got {spl_80:.1}"
        );
    }

    #[test]
    fn test_iso226_bass_needs_more_spl() {
        // At low frequencies, more SPL is needed to achieve the same perceived loudness.
        // 20 Hz should require significantly more SPL than 1 kHz at the same phon level.
        let spl_20hz = iso226_spl_at_freq(0, 60.0); // 20 Hz
        let spl_1khz = iso226_spl_at_freq(17, 60.0); // 1 kHz
        assert!(
            spl_20hz > spl_1khz + 20.0,
            "20 Hz should need much more SPL than 1 kHz: 20Hz={spl_20hz:.1}, 1kHz={spl_1khz:.1}"
        );
    }

    #[test]
    fn test_iso226_delta_same_level() {
        // When playback level equals reference level, all deltas should be zero
        let deltas = compute_iso226_delta(80.0, 80.0);
        for (freq, delta) in &deltas {
            assert!(
                delta.abs() < 0.01,
                "Delta at {freq:.0} Hz should be ~0 when playback=reference, got {delta:.2}"
            );
        }
    }

    #[test]
    fn test_iso226_delta_low_volume_boosts_bass() {
        // Listening at lower volume should require a bass boost RELATIVE to 1 kHz.
        // The delta curve uses shape_play - shape_ref, normalized to 0 at 1 kHz.
        let deltas = compute_iso226_delta(60.0, 83.0);

        // 1 kHz (index 17) should be exactly 0 (normalization reference)
        assert!(
            deltas[17].1.abs() < 0.01,
            "1 kHz delta should be 0 dB (reference), got {:.2} dB",
            deltas[17].1
        );

        // At lower playback levels, bass (20 Hz) is relatively harder to hear
        // compared to 1 kHz. The shape_play at 20 Hz is larger than shape_ref,
        // meaning the frequency needs more boost. Delta should be positive.
        assert!(
            deltas[0].1 > 5.0,
            "20 Hz should need significant boost at low volume, got {:.2} dB",
            deltas[0].1
        );
    }

    #[test]
    fn test_interpolate_delta_at_table_freq() {
        let deltas = compute_iso226_delta(60.0, 83.0);
        // Interpolating at an exact table frequency should return that frequency's value
        let d_1khz = interpolate_delta(&deltas, 1000.0);
        assert!(
            (d_1khz - deltas[17].1).abs() < 0.01,
            "Interpolation at 1 kHz should match table: interp={d_1khz:.2}, table={:.2}",
            deltas[17].1
        );
    }

    #[test]
    fn test_interpolate_delta_between_freqs() {
        let deltas = compute_iso226_delta(60.0, 83.0);
        // Interpolating between two frequencies should give an intermediate value
        let d_lo = deltas[17].1; // 1000 Hz
        let d_hi = deltas[18].1; // 1250 Hz
        let d_mid = interpolate_delta(&deltas, 1100.0);
        let min = d_lo.min(d_hi);
        let max = d_lo.max(d_hi);
        assert!(
            d_mid >= min - 0.01 && d_mid <= max + 0.01,
            "Interpolated value {d_mid:.2} should be between {min:.2} and {max:.2}"
        );
    }

    #[test]
    fn test_interpolate_delta_clamps_edges() {
        let deltas = compute_iso226_delta(60.0, 83.0);
        // Below 20 Hz should use the 20 Hz value
        let d_sub = interpolate_delta(&deltas, 10.0);
        assert!(
            (d_sub - deltas[0].1).abs() < 0.01,
            "Below 20 Hz should clamp to 20 Hz value"
        );
        // Above 12.5 kHz should use the 12.5 kHz value
        let d_over = interpolate_delta(&deltas, 20000.0);
        assert!(
            (d_over - deltas[28].1).abs() < 0.01,
            "Above 12.5 kHz should clamp to 12.5 kHz value"
        );
    }
}
