//! Spectral roughness estimation for room frequency responses.
//!
//! Measures spectral irregularity within auditory critical bands (Bark scale).
//! A smooth frequency response has low variance within each band; sharp peaks
//! and dips within a band produce audible coloration that this metric captures.
//!
//! The output is normalized to approximately [0, 1] where:
//!   0 = perfectly smooth within every critical band
//!   1 = severe irregularity (~10 dB std dev within most bands)
//!
//! This replaces the earlier Daniel & Weber mode-pair model, which produced
//! values in 1-35 for typical room EQ curves because it treated absolute SPL
//! levels as modulation depth. The critical-band variance approach is
//! physically correct for static frequency responses (transfer functions).

use super::bark::BARK_BAND_EDGES;

/// Spectral roughness via within-band variance.
///
/// For each of the 24 Bark critical bands, collects all SPL samples that
/// fall within the band's frequency range, computes the standard deviation
/// of SPL (in dB) within that band, then produces a weighted average across
/// bands. Bands with fewer than 2 samples are skipped (no variance
/// computable).
///
/// The weighting uses 1/ERB (more weight at low frequencies where narrower
/// bands make peaks more salient) and the result is normalized so that
/// 1.0 corresponds to ~10 dB average std dev — severe coloration.
///
/// Returns a value in approximately [0, 1].
pub fn spectral_roughness(freqs: &[f64], spl_db: &[f64]) -> f64 {
    debug_assert_eq!(freqs.len(), spl_db.len());

    let mut weighted_sum = 0.0;
    let mut weight_total = 0.0;

    for band in 0..24 {
        let lo = BARK_BAND_EDGES[band];
        let hi = BARK_BAND_EDGES[band + 1];
        let fc = (lo + hi) * 0.5;

        // Collect samples in this band
        let mut samples = Vec::new();
        for (i, &f) in freqs.iter().enumerate() {
            if f >= lo && f < hi {
                samples.push(spl_db[i]);
            }
        }

        if samples.len() < 2 {
            continue;
        }

        // Standard deviation of SPL within the band
        let n = samples.len() as f64;
        let mean = samples.iter().sum::<f64>() / n;
        let variance = samples.iter().map(|&s| (s - mean).powi(2)).sum::<f64>() / n;
        let std_dev = variance.sqrt();

        // Weight by 1/ERB: low-frequency bands are narrower, so peaks within
        // them are perceptually more salient.
        let erb = 24.7 * (1.0 + 4.37 * fc / 1000.0);
        let w = 1.0 / erb;

        weighted_sum += w * std_dev;
        weight_total += w;
    }

    if weight_total <= 0.0 {
        return 0.0;
    }

    let avg_std_dev = weighted_sum / weight_total;

    // Normalize: 10 dB average std dev maps to 1.0.
    // Typical well-corrected room: 1-3 dB -> 0.1-0.3
    // Uncorrected room with resonances: 5-8 dB -> 0.5-0.8
    // Severe coloration: 10+ dB -> 1.0+
    (avg_std_dev / 10.0).min(2.0)
}

/// Estimate roughness from a frequency response.
///
/// Uses critical-band spectral irregularity (within-Bark-band variance).
/// Returns a value in approximately [0, 1] where 0 is perfectly smooth.
pub fn roughness_from_spectrum(freqs: &[f64], spl_db: &[f64]) -> f64 {
    spectral_roughness(freqs, spl_db)
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
        assert!(
            r < 0.15,
            "Smooth tilt should have low roughness, got {r}"
        );
    }

    #[test]
    fn test_narrow_peak_high_roughness() {
        // Add a sharp 15 dB peak within a single band
        let n = 1000;
        let freqs: Vec<f64> = (0..n)
            .map(|i| 20.0 + (16000.0 - 20.0) * i as f64 / n as f64)
            .collect();
        let mut spl: Vec<f64> = vec![75.0; n];

        // Narrow peak at ~1000 Hz (within Bark band 8: 920-1080 Hz)
        for (i, &f) in freqs.iter().enumerate() {
            if (f - 1000.0).abs() < 10.0 {
                spl[i] = 90.0; // +15 dB peak
            }
        }

        let r = spectral_roughness(&freqs, &spl);
        assert!(
            r > 0.01,
            "Narrow peak should produce measurable roughness, got {r}"
        );
    }

    #[test]
    fn test_many_peaks_higher_roughness() {
        // Multiple narrow peaks across bands — should be rougher
        let n = 2000;
        let freqs: Vec<f64> = (0..n)
            .map(|i| 20.0 + (16000.0 - 20.0) * i as f64 / n as f64)
            .collect();
        let mut spl: Vec<f64> = vec![75.0; n];

        // Add peaks at 200, 500, 1000, 2000, 4000 Hz
        for (i, &f) in freqs.iter().enumerate() {
            for &peak_f in &[200.0, 500.0, 1000.0, 2000.0, 4000.0] {
                if (f - peak_f).abs() < 15.0 {
                    spl[i] = 85.0; // +10 dB peaks
                }
            }
        }

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
        let spl: Vec<f64> = (0..n).map(|i| if i % 2 == 0 { 90.0 } else { 60.0 }).collect();
        let r = spectral_roughness(&freqs, &spl);
        assert!(
            r <= 2.0,
            "Roughness should be clamped to <= 2.0, got {r}"
        );
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
}
