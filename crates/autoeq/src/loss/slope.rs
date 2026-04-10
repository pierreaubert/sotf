//! Slope and dispersion helpers used by several loss functions.
//!
//! These live in their own module because they are shared between the
//! speaker `mixed_loss` and the Harman headphone preference model.

use ndarray::Array1;

/// Compute the slope (per octave) using linear regression of y against log2(f).
///
/// - `freq`: frequency array in Hz
/// - `y`: corresponding values (e.g., SPL in dB)
/// - Range is defined in Hz as [fmin, fmax]; only f > 0 are considered
/// - Returns `Some(slope_db_per_octave)` or `None` if insufficient data
pub fn regression_slope_per_octave_in_range(
    freq: &Array1<f64>,
    y: &Array1<f64>,
    fmin: f64,
    fmax: f64,
) -> Option<f64> {
    assert_eq!(freq.len(), y.len(), "freq and y must have same length");
    if fmax <= fmin {
        return None;
    }

    let mut n: usize = 0;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xy = 0.0;
    let mut sum_x2 = 0.0;

    for i in 0..freq.len() {
        let f = freq[i];
        if f > 0.0 && f >= fmin && f <= fmax {
            let xi = f.log2();
            let yi = y[i];
            n += 1;
            sum_x += xi;
            sum_y += yi;
            sum_xy += xi * yi;
            sum_x2 += xi * xi;
        }
    }

    if n < 2 {
        return None;
    }
    let n_f = n as f64;
    let cov_xy = sum_xy - (sum_x * sum_y) / n_f;
    let var_x = sum_x2 - (sum_x * sum_x) / n_f;
    if var_x.abs() < 1e-10 {
        return None;
    }
    Some(cov_xy / var_x)
}

/// Convenience wrapper for slope per octave on a `Curve`.
pub fn curve_slope_per_octave_in_range(curve: &crate::Curve, fmin: f64, fmax: f64) -> Option<f64> {
    regression_slope_per_octave_in_range(&curve.freq, &curve.spl, fmin, fmax)
}

/// Calculate the standard deviation of the deviation values within a frequency range.
///
/// Used as part of the Olive et al. headphone preference prediction model.
pub fn calculate_standard_deviation_in_range(
    freq: &Array1<f64>,
    deviation: &Array1<f64>,
    fmin: f64,
    fmax: f64,
) -> f64 {
    assert_eq!(
        freq.len(),
        deviation.len(),
        "freq and deviation must have same length"
    );

    let mut values = Vec::new();

    // Collect deviation values in the specified frequency range
    for i in 0..freq.len() {
        let f = freq[i];
        if f >= fmin && f <= fmax {
            values.push(deviation[i]);
        }
    }

    if values.is_empty() {
        return 0.0;
    }

    // Calculate mean
    let mean = values.iter().sum::<f64>() / values.len() as f64;

    // Calculate variance
    let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64;

    // Return standard deviation
    variance.sqrt()
}

/// Calculate the absolute slope (AS) of the deviation using logarithmic regression over the specified frequency range.
///
/// This function performs linear regression of deviation against log2(frequency) to determine
/// the slope, then returns the absolute value.
///
/// # Arguments
/// * `freq` - Frequency array in Hz
/// * `deviation` - Deviation values from Harman target curve in dB
/// * `fmin` - Minimum frequency in Hz (typically 50 Hz)
/// * `fmax` - Maximum frequency in Hz (typically 10000 Hz)
///
/// # Returns
/// * Absolute value of the slope in dB per octave
///
/// # Notes
/// Used as part of the Olive et al. headphone preference prediction model.
pub(super) fn calculate_absolute_slope_in_range(
    freq: &Array1<f64>,
    deviation: &Array1<f64>,
    fmin: f64,
    fmax: f64,
) -> f64 {
    match regression_slope_per_octave_in_range(freq, deviation, fmin, fmax) {
        Some(slope) => slope.abs(),
        None => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regression_slope_per_octave_linear_log_relation_full_range() {
        // y = 3 * log2(f) + 1
        let freq = Array1::from(vec![100.0, 200.0, 400.0, 800.0]);
        let y = freq.mapv(|f: f64| 3.0 * f.log2() + 1.0);
        let slope = regression_slope_per_octave_in_range(&freq, &y, 100.0, 800.0).unwrap();
        assert!((slope - 3.0).abs() < 1e-12);
    }

    #[test]
    fn regression_slope_per_octave_sub_range() {
        // Same log-linear relation, sub-range 200..=800
        let freq = Array1::from(vec![100.0, 200.0, 400.0, 800.0]);
        let y = freq.mapv(|f: f64| -2.5 * f.log2() + 4.0);
        let slope = regression_slope_per_octave_in_range(&freq, &y, 200.0, 800.0).unwrap();
        assert!((slope + 2.5).abs() < 1e-12);
    }

    #[test]
    fn test_calculate_standard_deviation_in_range() {
        // Test SD calculation with known values
        let freq = Array1::from(vec![50.0, 100.0, 1000.0, 5000.0, 10000.0]);
        let deviation = Array1::from(vec![1.0, 2.0, 3.0, 4.0, 5.0]); // All in range

        let sd = calculate_standard_deviation_in_range(&freq, &deviation, 50.0, 10000.0);

        // Manual calculation: mean = (1+2+3+4+5)/5 = 3.0
        // variance = ((1-3)² + (2-3)² + (3-3)² + (4-3)² + (5-3)²)/5 = (4+1+0+1+4)/5 = 2.0
        // sd = sqrt(2.0) ≈ 1.414
        let expected_sd = 2.0_f64.sqrt();
        assert!(
            (sd - expected_sd).abs() < 1e-12,
            "SD calculation incorrect: got {}, expected {}",
            sd,
            expected_sd
        );
    }

    #[test]
    fn test_calculate_standard_deviation_filtered_range() {
        // Test SD calculation with frequency filtering
        let freq = Array1::from(vec![20.0, 100.0, 1000.0, 5000.0, 15000.0]); // Some out of range
        let deviation = Array1::from(vec![10.0, 2.0, 4.0, 6.0, 20.0]); // First and last should be filtered

        let sd = calculate_standard_deviation_in_range(&freq, &deviation, 50.0, 10000.0);

        // Only values at 100Hz, 1kHz, 5kHz should be included: [2.0, 4.0, 6.0]
        // mean = (2+4+6)/3 = 4.0
        // variance = ((2-4)² + (4-4)² + (6-4)²)/3 = (4+0+4)/3 = 8/3
        // sd = sqrt(8/3) ≈ 1.633
        let expected_sd = (8.0_f64 / 3.0_f64).sqrt();
        assert!(
            (sd - expected_sd).abs() < 1e-12,
            "SD calculation with filtering incorrect: got {}, expected {}",
            sd,
            expected_sd
        );
    }

    #[test]
    fn test_calculate_absolute_slope_in_range() {
        // Test AS calculation with linear slope
        let freq = Array1::from(vec![
            50.0, 100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0, 6400.0, 10000.0,
        ]);
        // Create a perfect 2 dB/octave slope: y = 2 * log2(f) + constant
        let deviation = freq.mapv(|f: f64| 2.0 * f.log2());

        let as_value = calculate_absolute_slope_in_range(&freq, &deviation, 50.0, 10000.0);

        // Should return absolute value of 2.0
        assert!(
            (as_value - 2.0).abs() < 1e-12,
            "AS calculation incorrect: got {}, expected 2.0",
            as_value
        );
    }

    #[test]
    fn test_calculate_absolute_slope_negative() {
        // Test AS calculation with negative slope
        let freq = Array1::from(vec![
            50.0, 100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0, 6400.0, 10000.0,
        ]);
        // Create a perfect -3 dB/octave slope
        let deviation = freq.mapv(|f: f64| -3.0 * f.log2());

        let as_value = calculate_absolute_slope_in_range(&freq, &deviation, 50.0, 10000.0);

        // Should return absolute value of -3.0 = 3.0
        assert!(
            (as_value - 3.0).abs() < 1e-12,
            "AS calculation with negative slope incorrect: got {}, expected 3.0",
            as_value
        );
    }

    #[test]
    fn test_regression_slope_identical_freqs() {
        // All frequencies identical — var_x should be ~0, return None
        let freq = Array1::from_vec(vec![1000.0, 1000.0, 1000.0]);
        let y = Array1::from_vec(vec![80.0, 85.0, 90.0]);
        let result = regression_slope_per_octave_in_range(&freq, &y, 999.0, 1001.0);
        assert!(
            result.is_none(),
            "identical frequencies should return None, got {:?}",
            result
        );
    }
}
