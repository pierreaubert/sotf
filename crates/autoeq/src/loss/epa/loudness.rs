pub use math_audio_dsp::psychoacoustics::{OUTER_EAR_TF, THRESHOLD_IN_QUIET, excitation_pattern};

/// Compute specific loudness (sone/Bark) for each of 24 Bark bands.
///
/// Steps:
/// 1. Decompose frequency response into Bark bands
/// 2. Apply outer/middle ear transfer function
/// 3. Compute excitation pattern via spreading
/// 4. Convert excitation to specific loudness using Zwicker's power law
///
/// `freqs` and `spl_db` define the frequency response. The response is
/// calibrated so its interpolated 1 kHz level equals `listening_level_phon`.
pub fn specific_loudness(freqs: &[f64], spl_db: &[f64], listening_level_phon: f64) -> [f64; 24] {
    math_audio_dsp::psychoacoustics::specific_loudness(freqs, spl_db, listening_level_phon)
}

/// Total loudness in sone: integral of specific loudness across all Bark bands.
/// Approximated as a sum with delta_z = 1 Bark per band.
pub fn total_loudness(specific: &[f64; 24]) -> f64 {
    math_audio_dsp::psychoacoustics::total_loudness(specific)
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
        let spec = specific_loudness(&freqs, &spl, -20.0);
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
