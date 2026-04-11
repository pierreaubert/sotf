/// Standard Bark band center frequencies (Hz) for 24 critical bands.
pub const BARK_CENTER_FREQUENCIES: [f64; 24] = [
    50.0, 150.0, 250.0, 350.0, 450.0, 570.0, 700.0, 840.0, 1000.0, 1170.0, 1370.0, 1600.0, 1850.0,
    2150.0, 2500.0, 2900.0, 3400.0, 4000.0, 4800.0, 5800.0, 7000.0, 8500.0, 10500.0, 13500.0,
];

/// Standard Bark band edge frequencies (Hz), 25 edges defining 24 bands.
pub const BARK_BAND_EDGES: [f64; 25] = [
    0.0, 100.0, 200.0, 300.0, 400.0, 510.0, 630.0, 770.0, 920.0, 1080.0, 1270.0, 1480.0, 1720.0,
    2000.0, 2320.0, 2700.0, 3150.0, 3700.0, 4400.0, 5300.0, 6400.0, 7700.0, 9500.0, 12000.0,
    15500.0,
];

/// Convert frequency in Hz to Bark scale using Zwicker's formula.
///
/// z = 13 * arctan(0.00076 * f) + 3.5 * arctan((f / 7500)^2)
pub fn hz_to_bark(f: f64) -> f64 {
    13.0 * (0.00076 * f).atan() + 3.5 * ((f / 7500.0).powi(2)).atan()
}

/// Convert Bark scale value back to Hz using Newton-Raphson iteration.
pub fn bark_to_hz(z: f64) -> f64 {
    // Initial estimate via rough linear inversion
    let mut f = 600.0 * z.sinh() / 7.0;
    f = f.clamp(1.0, 25000.0);

    for _ in 0..5 {
        let z_est = hz_to_bark(f);
        let err = z_est - z;

        // Numerical derivative: dz/df
        let delta = f * 1e-6 + 0.01;
        let dz_df = (hz_to_bark(f + delta) - z_est) / delta;

        if dz_df.abs() < 1e-15 {
            break;
        }
        f -= err / dz_df;
        f = f.clamp(1.0, 25000.0);
    }
    f
}

/// Critical bandwidth in Hz at a given center frequency.
///
/// CBW(f) = 25 + 75 * (1 + 1.4 * (f / 1000)^2)^0.69
pub fn critical_bandwidth(f: f64) -> f64 {
    25.0 + 75.0 * (1.0 + 1.4 * (f / 1000.0).powi(2)).powf(0.69)
}

/// Decompose a frequency response into 24 Bark bands by energy-averaging
/// the SPL (in dB) within each band's frequency range.
///
/// `freqs` and `spl_db` must be the same length. Frequencies in Hz, levels in dB SPL.
/// Returns the average SPL per Bark band. Bands with no data get -100 dB.
pub fn bark_spectrum(freqs: &[f64], spl_db: &[f64]) -> [f64; 24] {
    debug_assert_eq!(freqs.len(), spl_db.len());

    let mut result = [-100.0_f64; 24];

    for band in 0..24 {
        let lo = BARK_BAND_EDGES[band];
        let hi = BARK_BAND_EDGES[band + 1];

        let mut energy_sum = 0.0_f64;
        let mut count = 0u32;

        for (i, &f) in freqs.iter().enumerate() {
            if f >= lo && f < hi {
                // Convert dB to linear power, accumulate
                energy_sum += 10.0_f64.powf(spl_db[i] / 10.0);
                count += 1;
            }
        }

        if count > 0 {
            result[band] = 10.0 * (energy_sum / count as f64).log10();
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hz_to_bark_known() {
        let z100 = hz_to_bark(100.0);
        let z1000 = hz_to_bark(1000.0);
        let z10000 = hz_to_bark(10000.0);

        assert!(
            (z100 - 1.0).abs() < 0.15,
            "100 Hz -> Bark = {z100}, expected ~1.0"
        );
        assert!(
            (z1000 - 8.5).abs() < 0.3,
            "1000 Hz -> Bark = {z1000}, expected ~8.5"
        );
        assert!(
            (z10000 - 22.4).abs() < 0.5,
            "10000 Hz -> Bark = {z10000}, expected ~22.4"
        );
    }

    #[test]
    fn test_bark_to_hz_roundtrip() {
        for &f in &[100.0, 500.0, 1000.0, 4000.0, 10000.0] {
            let z = hz_to_bark(f);
            let f_back = bark_to_hz(z);
            let rel_err = (f_back - f).abs() / f;
            assert!(
                rel_err < 0.02,
                "Roundtrip failed for {f} Hz: got {f_back} Hz (rel err {rel_err})"
            );
        }
    }

    #[test]
    fn test_critical_bandwidth_increases() {
        let cbw_500 = critical_bandwidth(500.0);
        let cbw_5000 = critical_bandwidth(5000.0);
        assert!(
            cbw_5000 > cbw_500,
            "CBW at 5kHz ({cbw_5000}) should be > CBW at 500Hz ({cbw_500})"
        );
    }

    #[test]
    fn test_bark_spectrum_flat_input() {
        // Generate flat SPL at 70 dB across 20-16000 Hz
        let n = 1000;
        let freqs: Vec<f64> = (0..n)
            .map(|i| 20.0 + (16000.0 - 20.0) * i as f64 / n as f64)
            .collect();
        let spl: Vec<f64> = vec![70.0; n];

        let spectrum = bark_spectrum(&freqs, &spl);

        // All populated bands should be close to 70 dB
        for (i, &val) in spectrum.iter().enumerate() {
            if val > -50.0 {
                assert!(
                    (val - 70.0).abs() < 1.0,
                    "Band {i} = {val} dB, expected ~70 dB"
                );
            }
        }
    }
}
