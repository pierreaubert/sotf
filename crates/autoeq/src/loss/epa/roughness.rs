//! Psychoacoustic roughness estimation for room frequency responses.
//!
//! The runtime EPA path delegates to `math-dsp`'s pairwise sensory roughness
//! model so AutoEQ and reusable DSP helpers stay mathematically aligned.

/// Pairwise sensory roughness from prominent spectrum components.
pub fn spectral_roughness(freqs: &[f64], spl_db: &[f64]) -> f64 {
    math_audio_dsp::psychoacoustics::spectral_roughness(freqs, spl_db)
}

/// Estimate roughness from a frequency response.
pub fn roughness_from_spectrum(freqs: &[f64], spl_db: &[f64]) -> f64 {
    math_audio_dsp::psychoacoustics::roughness_from_spectrum(freqs, spl_db)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flat_spectrum_zero_roughness() {
        let n = 1000;
        let freqs: Vec<f64> = (0..n)
            .map(|i| 20.0 + (16000.0 - 20.0) * i as f64 / n as f64)
            .collect();
        let spl: Vec<f64> = vec![75.0; n];
        let r = spectral_roughness(&freqs, &spl);
        assert!(
            r < 1e-10,
            "Flat spectrum should have zero roughness, got {r}"
        );
    }

    #[test]
    fn test_smooth_tilt_low_roughness() {
        // Gentle tilt: -0.5 dB per octave from 1kHz, smooth within each band
        let n = 1000;
        let freqs: Vec<f64> = (0..n)
            .map(|i| 20.0 + (16000.0 - 20.0) * i as f64 / n as f64)
            .collect();
        let spl: Vec<f64> = freqs
            .iter()
            .map(|&f| 75.0 - 0.5 * (f / 1000.0).log2())
            .collect();
        let r = spectral_roughness(&freqs, &spl);
        assert!(r < 0.15, "Smooth tilt should have low roughness, got {r}");
    }

    #[test]
    fn test_close_peak_pair_high_roughness() {
        let n = 3000;
        let freqs: Vec<f64> = (0..n)
            .map(|i| 20.0 + (3000.0 - 20.0) * i as f64 / n as f64)
            .collect();
        let spl = peaked_response(&freqs, &[1000.0, 1070.0]);
        let r = spectral_roughness(&freqs, &spl);
        assert!(
            r > 0.01,
            "Close peak pair should produce measurable roughness, got {r}"
        );
    }

    #[test]
    fn test_many_peaks_higher_roughness() {
        let n = 2000;
        let freqs: Vec<f64> = (0..n)
            .map(|i| 20.0 + (16000.0 - 20.0) * i as f64 / n as f64)
            .collect();
        let spl = peaked_response(&freqs, &[200.0, 230.0, 1000.0, 1070.0, 2000.0, 2100.0]);
        let r_peaks = spectral_roughness(&freqs, &spl);
        let r_flat = spectral_roughness(&freqs, &vec![75.0; n]);

        assert!(
            r_peaks > r_flat + 0.01,
            "Many peaks ({r_peaks}) should be rougher than flat ({r_flat})"
        );
    }

    #[test]
    fn test_roughness_output_range() {
        // A severely irregular response should still be bounded
        let n = 1000;
        let freqs: Vec<f64> = (0..n)
            .map(|i| 20.0 + (16000.0 - 20.0) * i as f64 / n as f64)
            .collect();
        // Alternating +-15 dB every other sample
        let spl: Vec<f64> = (0..n)
            .map(|i| if i % 2 == 0 { 90.0 } else { 60.0 })
            .collect();
        let r = spectral_roughness(&freqs, &spl);
        assert!(r <= 2.0, "Roughness should be clamped to <= 2.0, got {r}");
        assert!(
            r > 0.5,
            "Severely irregular spectrum should have high roughness, got {r}"
        );
    }

    #[test]
    fn test_roughness_from_spectrum_delegates() {
        let freqs = vec![100.0, 200.0, 300.0, 400.0, 500.0];
        let spl = vec![75.0, 80.0, 75.0, 85.0, 75.0];
        assert_eq!(
            roughness_from_spectrum(&freqs, &spl),
            spectral_roughness(&freqs, &spl)
        );
    }

    fn peaked_response(freqs: &[f64], peaks: &[f64]) -> Vec<f64> {
        freqs
            .iter()
            .map(|&f| {
                let peak = peaks
                    .iter()
                    .map(|&center| {
                        let distance = (f - center) / 8.0;
                        18.0 * (-0.5 * distance * distance).exp()
                    })
                    .fold(0.0, f64::max);
                75.0 + peak
            })
            .collect()
    }
}
