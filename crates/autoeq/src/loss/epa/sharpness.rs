use super::loudness::total_loudness;

/// Sharpness weighting function g(z) per Bark band (DIN 45692).
/// Bands 1-15 have weight 1.0; bands 16-24 increase linearly to emphasize
/// high-frequency content in the sharpness calculation.
pub const SHARPNESS_WEIGHT: [f64; 24] = [
    1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.2, 1.5, 1.8, 2.2,
    2.7, 3.3, 4.0, 5.0, 6.2,
];

/// Compute Zwicker sharpness (in acum) from specific loudness values.
///
/// S = 0.11 * sum(N'(z) * g(z) * z) / max(N_total, 0.001)
///
/// where z is the band index (1-based), g(z) is the sharpness weight,
/// N'(z) is specific loudness, and 0.11 is the calibration constant
/// (1 acum = narrowband noise at 1 kHz, 60 dB).
pub fn sharpness(specific_loudness: &[f64; 24]) -> f64 {
    let mut numerator = 0.0_f64;

    for z in 0..24 {
        let z_center = (z + 1) as f64; // 1-based band index
        numerator += specific_loudness[z] * SHARPNESS_WEIGHT[z] * z_center;
    }

    let denominator = total_loudness(specific_loudness).max(0.001);
    0.11 * numerator / denominator
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loss::epa::loudness::specific_loudness;

    fn make_bandlimited_response(lo_hz: f64, hi_hz: f64, level_db: f64) -> (Vec<f64>, Vec<f64>) {
        let n = 1000;
        let freqs: Vec<f64> = (0..n)
            .map(|i| 20.0 + (16000.0 - 20.0) * i as f64 / n as f64)
            .collect();
        let spl: Vec<f64> = freqs
            .iter()
            .map(|&f| {
                if f >= lo_hz && f <= hi_hz {
                    level_db
                } else {
                    -40.0
                }
            })
            .collect();
        (freqs, spl)
    }

    #[test]
    fn test_sharpness_low_freq_only() {
        let (freqs, spl) = make_bandlimited_response(20.0, 1000.0, 70.0);
        let spec = specific_loudness(&freqs, &spl, 70.0);
        let s = sharpness(&spec);
        assert!(
            s < 1.0,
            "Low-frequency-only signal should have low sharpness, got {s} acum"
        );
    }

    #[test]
    fn test_sharpness_high_freq_only() {
        let (freqs, spl) = make_bandlimited_response(5000.0, 15000.0, 70.0);
        let spec = specific_loudness(&freqs, &spl, 70.0);
        let s = sharpness(&spec);
        assert!(
            s > 2.0,
            "High-frequency-only signal should have high sharpness, got {s} acum"
        );
    }

    #[test]
    fn test_sharpness_broadband() {
        let (freqs, spl) = make_bandlimited_response(20.0, 15000.0, 70.0);
        let spec = specific_loudness(&freqs, &spl, 70.0);
        let s = sharpness(&spec);
        assert!(
            (0.5..=2.5).contains(&s),
            "Broadband signal should have moderate sharpness, got {s} acum"
        );
    }
}
