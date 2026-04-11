use super::bark::{BARK_CENTER_FREQUENCIES, bark_spectrum};

/// Outer/middle ear transfer function in dB relative to free-field,
/// at Bark band center frequencies. Models the resonance of the ear canal
/// and pinna effects (peak ~2-4 kHz).
pub const OUTER_EAR_TF: [f64; 24] = [
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 2.0, 3.0, 5.0, 6.0, 7.0, 9.0, 10.0, 9.0,
    7.0, 5.0, 3.0, 2.0, 0.0,
];

/// Absolute threshold of hearing in dB SPL per Bark band (ISO 226 approximation).
pub const THRESHOLD_IN_QUIET: [f64; 24] = [
    40.0, 30.0, 22.0, 17.0, 14.0, 10.0, 7.0, 5.0, 4.0, 4.0, 4.0, 4.0, 5.0, 6.0, 7.0, 9.0, 12.0,
    15.0, 19.0, 25.0, 33.0, 44.0, 58.0, 80.0,
];

/// Convert dB SPL to linear power (intensity-like quantity).
fn db_to_power(db: f64) -> f64 {
    10.0_f64.powf(db / 10.0)
}

/// Apply the spreading function to produce excitation levels from Bark band levels.
///
/// For each target band, contributions from all source bands are summed,
/// with slopes that depend on distance and level:
/// - Lower slope (source below target): ~27 dB/Bark
/// - Upper slope (source above target): 24 + 230/fc - 0.2*level dB/Bark
pub fn excitation_pattern(bark_levels: &[f64; 24]) -> [f64; 24] {
    let mut excitation = [0.0_f64; 24];

    #[allow(clippy::needless_range_loop)]
    for target in 0..24 {
        let mut total_power = 0.0_f64;

        for source in 0..24 {
            let level = bark_levels[source];
            if level <= -90.0 {
                continue;
            }

            let distance = (target as f64) - (source as f64); // in Bark (band indices ~ Bark)
            let fc = BARK_CENTER_FREQUENCIES[source];

            let attenuation = if distance < 0.0 {
                // Source is above target: lower slope
                27.0 * distance.abs()
            } else if distance > 0.0 {
                // Source is below target: upper slope (level-dependent)
                let upper_slope = (24.0 + 230.0 / fc - 0.2 * level).max(5.0);
                upper_slope * distance
            } else {
                0.0
            };

            let contribution = db_to_power(level - attenuation);
            total_power += contribution;
        }

        excitation[target] = if total_power > 0.0 {
            10.0 * total_power.log10()
        } else {
            -100.0
        };
    }

    excitation
}

/// Compute specific loudness (sone/Bark) for each of 24 Bark bands.
///
/// Steps:
/// 1. Decompose frequency response into Bark bands
/// 2. Apply outer/middle ear transfer function
/// 3. Compute excitation pattern via spreading
/// 4. Convert excitation to specific loudness using Zwicker's power law
///
/// `freqs` and `spl_db` define the frequency response.
/// `_listening_level_phon` is reserved for future use (overall presentation level).
pub fn specific_loudness(freqs: &[f64], spl_db: &[f64], _listening_level_phon: f64) -> [f64; 24] {
    let mut band_levels = bark_spectrum(freqs, spl_db);

    // Apply outer/middle ear transfer function
    for i in 0..24 {
        band_levels[i] += OUTER_EAR_TF[i];
    }

    // Compute excitation pattern
    let excitation = excitation_pattern(&band_levels);

    // Convert to specific loudness
    let mut specific = [0.0_f64; 24];
    for i in 0..24 {
        let e = db_to_power(excitation[i]);
        let e_tq = db_to_power(THRESHOLD_IN_QUIET[i]);

        // Zwicker specific loudness formula
        // N'(z) = 0.08 * ((0.5 + 0.5 * E/E_TQ)^0.23 - 1)
        let ratio = e / e_tq;
        let val = 0.08 * ((0.5 + 0.5 * ratio).powf(0.23) - 1.0);
        specific[i] = val.max(0.0);
    }

    specific
}

/// Total loudness in sone: integral of specific loudness across all Bark bands.
/// Approximated as a sum with delta_z = 1 Bark per band.
pub fn total_loudness(specific: &[f64; 24]) -> f64 {
    specific.iter().sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_flat_response(level_db: f64) -> (Vec<f64>, Vec<f64>) {
        let n = 1000;
        let freqs: Vec<f64> = (0..n)
            .map(|i| 20.0 + (16000.0 - 20.0) * i as f64 / n as f64)
            .collect();
        let spl = vec![level_db; n];
        (freqs, spl)
    }

    #[test]
    fn test_specific_loudness_silence() {
        let (freqs, spl) = make_flat_response(-20.0);
        let spec = specific_loudness(&freqs, &spl, 70.0);
        let total = total_loudness(&spec);
        assert!(
            total < 0.1,
            "Very quiet input should produce near-zero loudness, got {total}"
        );
    }

    #[test]
    fn test_total_loudness_increases_with_level() {
        let (freqs50, spl50) = make_flat_response(50.0);
        let (freqs70, spl70) = make_flat_response(70.0);

        let spec50 = specific_loudness(&freqs50, &spl50, 50.0);
        let spec70 = specific_loudness(&freqs70, &spl70, 70.0);

        let total50 = total_loudness(&spec50);
        let total70 = total_loudness(&spec70);

        assert!(
            total70 > total50,
            "70 dB ({total70} sone) should be louder than 50 dB ({total50} sone)"
        );
    }

    #[test]
    fn test_specific_loudness_peaks_at_3khz() {
        let (freqs, spl) = make_flat_response(70.0);
        let spec = specific_loudness(&freqs, &spl, 70.0);

        // Find the band with maximum specific loudness
        let max_band = spec
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;

        let peak_freq = crate::loss::epa::bark::BARK_CENTER_FREQUENCIES[max_band];

        // Should peak somewhere in the 2-5 kHz range due to ear canal resonance
        assert!(
            (2000.0..=5000.0).contains(&peak_freq),
            "Peak specific loudness at band {max_band} ({peak_freq} Hz), expected 2-5 kHz"
        );
    }
}
