//! Roughness estimation from room mode interactions.
//!
//! In room acoustics, two close room modes create amplitude modulation
//! (beating) at a rate equal to their frequency difference. The human ear
//! perceives this as "roughness" when the modulation rate is 15-300 Hz,
//! with peak sensitivity around 70 Hz.
//!
//! Based on the Daniel & Weber model adapted for static spectra.

/// Daniel & Weber roughness weighting function.
/// Maximum sensitivity at ~70 Hz modulation rate.
/// Returns 0.0 outside [15, 300] Hz range.
pub fn roughness_weight(modulation_freq_hz: f64) -> f64 {
    if !(15.0..=300.0).contains(&modulation_freq_hz) {
        return 0.0;
    }
    let log_f = modulation_freq_hz.log2();
    let log_center = 70.0_f64.log2();
    let sigma = 0.8; // octaves
    (-0.5 * ((log_f - log_center) / sigma).powi(2)).exp()
}

/// Estimate roughness potential from detected room modes.
///
/// For each pair of modes (i, j) where i < j:
///   modulation_freq = |f_i - f_j|
///   if modulation_freq is in [15, 300] Hz (roughness range):
///     modulation_depth = min(level_i, level_j) in dB (both must be audible)
///     contribution = roughness_weight(mod_freq) * modulation_depth^2
///
/// Returns roughness estimate in arbitrary units (higher = rougher).
pub fn roughness_from_modes(modes: &[(f64, f64)]) -> f64 {
    let mut roughness = 0.0;

    for i in 0..modes.len() {
        for j in (i + 1)..modes.len() {
            let (fi, li) = modes[i];
            let (fj, lj) = modes[j];
            let mod_freq = (fi - fj).abs();
            let w = roughness_weight(mod_freq);
            if w > 0.0 {
                // Use minimum level: both modes must be present for beating
                let depth = li.min(lj).max(0.0);
                roughness += w * depth * depth;
            }
        }
    }

    // Normalize to a perceptually meaningful range.
    // Dividing by a reference level squared (70 dB typical listening level)
    // keeps the output roughly in [0, ~few units].
    roughness / (70.0 * 70.0)
}

/// Find local spectral peaks that are at least `min_prominence_db` above
/// both their immediate neighbors.
fn find_spectral_peaks(freqs: &[f64], spl_db: &[f64], min_prominence_db: f64) -> Vec<(f64, f64)> {
    let mut peaks = Vec::new();
    if spl_db.len() < 3 {
        return peaks;
    }

    for i in 1..(spl_db.len() - 1) {
        let left = spl_db[i - 1];
        let right = spl_db[i + 1];
        let center = spl_db[i];
        let prominence = center - left.max(right);
        if prominence >= min_prominence_db {
            peaks.push((freqs[i], center));
        }
    }
    peaks
}

/// Estimate roughness from a frequency response by finding spectral peaks
/// and computing mode-pair interactions.
///
/// Uses peak detection with a 3 dB prominence threshold, then evaluates
/// all pairs of peaks whose frequency difference falls in the roughness
/// range (15-300 Hz).
pub fn roughness_from_spectrum(freqs: &[f64], spl_db: &[f64]) -> f64 {
    let peaks = find_spectral_peaks(freqs, spl_db, 3.0);
    roughness_from_modes(&peaks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roughness_weight_peak_at_70hz() {
        let w70 = roughness_weight(70.0);
        // 70 Hz should be very close to the maximum (1.0)
        assert!(
            (w70 - 1.0).abs() < 0.01,
            "Weight at 70 Hz = {w70}, expected ~1.0"
        );
        // Check it's higher than neighbors
        assert!(w70 > roughness_weight(30.0));
        assert!(w70 > roughness_weight(200.0));
    }

    #[test]
    fn test_roughness_weight_zero_outside_range() {
        assert_eq!(roughness_weight(10.0), 0.0, "Below 15 Hz should be 0");
        assert_eq!(roughness_weight(500.0), 0.0, "Above 300 Hz should be 0");
        assert_eq!(roughness_weight(0.0), 0.0, "0 Hz should be 0");
        assert_eq!(roughness_weight(-5.0), 0.0, "Negative freq should be 0");
    }

    #[test]
    fn test_roughness_close_modes_high() {
        // Two modes 50 Hz apart at 80 dB -- right in the roughness sweet spot
        let modes = vec![(200.0, 80.0), (250.0, 80.0)];
        let r = roughness_from_modes(&modes);
        assert!(
            r > 0.1,
            "Close loud modes should produce significant roughness, got {r}"
        );
    }

    #[test]
    fn test_roughness_far_modes_low() {
        // Two modes 1000 Hz apart -- well outside roughness range
        let modes = vec![(200.0, 80.0), (1200.0, 80.0)];
        let r = roughness_from_modes(&modes);
        assert!(
            r < 1e-10,
            "Far-apart modes should produce zero roughness, got {r}"
        );
    }

    #[test]
    fn test_roughness_from_flat_spectrum() {
        // Flat response at 70 dB: no prominent peaks, so no roughness
        let n = 500;
        let freqs: Vec<f64> = (0..n)
            .map(|i| 20.0 + (16000.0 - 20.0) * i as f64 / n as f64)
            .collect();
        let spl: Vec<f64> = vec![70.0; n];
        let r = roughness_from_spectrum(&freqs, &spl);
        assert!(
            r < 1e-10,
            "Flat spectrum should have near-zero roughness, got {r}"
        );
    }

    #[test]
    fn test_roughness_from_spectrum_with_peaks() {
        // Create a spectrum with two prominent peaks 60 Hz apart
        let n = 500;
        let freqs: Vec<f64> = (0..n)
            .map(|i| 20.0 + (16000.0 - 20.0) * i as f64 / n as f64)
            .collect();
        let mut spl: Vec<f64> = vec![60.0; n];

        // Add single-bin peaks at ~200 Hz and ~260 Hz.
        // Find the nearest bin to each target frequency.
        let idx_200 = freqs
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (*a - 200.0).abs().partial_cmp(&(*b - 200.0).abs()).unwrap())
            .unwrap()
            .0;
        let idx_260 = freqs
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (*a - 260.0).abs().partial_cmp(&(*b - 260.0).abs()).unwrap())
            .unwrap()
            .0;
        spl[idx_200] = 75.0; // 15 dB prominence above 60 dB floor
        spl[idx_260] = 75.0;

        let r = roughness_from_spectrum(&freqs, &spl);
        assert!(
            r > 0.0,
            "Spectrum with close peaks should have nonzero roughness, got {r}"
        );
    }

    #[test]
    fn test_roughness_weight_monotonic_toward_peak() {
        // Weight should increase as we approach 70 Hz from below
        let w20 = roughness_weight(20.0);
        let w40 = roughness_weight(40.0);
        let w60 = roughness_weight(60.0);
        assert!(
            w60 > w40 && w40 > w20,
            "Weight should increase toward 70 Hz: w20={w20}, w40={w40}, w60={w60}"
        );
    }
}
