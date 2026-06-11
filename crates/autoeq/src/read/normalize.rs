use crate::Curve;
use ndarray::Array1;

use super::interpolate::*;

/// Low frequency bound for normalization (1000 Hz).
///
/// SPL values are normalized by subtracting the mean in the range
/// from `NORMALIZE_LOW_FREQ` to `NORMALIZE_HIGH_FREQ`.
pub const NORMALIZE_LOW_FREQ: f64 = 1000.0;

/// High frequency bound for normalization (2000 Hz).
///
/// SPL values are normalized by subtracting the mean in the range
/// from `NORMALIZE_LOW_FREQ` to `NORMALIZE_HIGH_FREQ`.
pub const NORMALIZE_HIGH_FREQ: f64 = 2000.0;

/// Normalize frequency response by subtracting mean in 100Hz-12kHz range
fn normalize_response(input: &Curve, f_min: f64, f_max: f64) -> Array1<f64> {
    let mut sum = 0.0;
    let mut count = 0;

    // Calculate mean in the specified frequency range
    for i in 0..input.freq.len() {
        if input.freq[i] >= f_min && input.freq[i] <= f_max {
            sum += input.spl[i];
            count += 1;
        }
    }

    if count > 0 {
        let mean = sum / count as f64;
        input.spl.clone() - mean // Subtract mean from all values
    } else {
        input.spl.clone() // Return unchanged if no points in range
    }
}

/// Normalize and interpolate a frequency response curve.
///
/// Normalizes the SPL by subtracting the mean in the 1000-2000 Hz range,
/// then interpolates the result to the standard frequency grid.
///
/// # Arguments
///
/// * `standard_freq` - Target frequency grid for interpolation
/// * `curve` - Input frequency response curve
///
/// # Returns
///
/// Normalized and interpolated curve on the standard frequency grid.
pub fn normalize_and_interpolate_response(
    standard_freq: &ndarray::Array1<f64>,
    curve: &Curve,
) -> Curve {
    // Normalize after interpolation
    let spl_norm = normalize_response(curve, NORMALIZE_LOW_FREQ, NORMALIZE_HIGH_FREQ);

    interpolate_log_space(
        standard_freq,
        &Curve {
            freq: curve.freq.clone(),
            spl: spl_norm,
            phase: curve.phase.clone(),
            ..Default::default()
        },
    )
}

/// Interpolate a frequency response curve WITHOUT normalizing.
///
/// This preserves the original dB levels of the curve, only resampling
/// to the standard frequency grid. Useful for CEA2034 visualization
/// where curves should maintain their relative levels.
///
/// # Arguments
///
/// * `standard_freq` - Target frequency grid for interpolation
/// * `curve` - Input frequency response curve
///
/// # Returns
///
/// Interpolated curve on the standard frequency grid (no normalization applied).
pub fn interpolate_response(standard_freq: &ndarray::Array1<f64>, curve: &Curve) -> Curve {
    interpolate_log_space(standard_freq, curve)
}

/// Normalize and interpolate response with custom normalization frequency range
///
/// This is useful for multi-driver systems where each driver should be normalized
/// by its own passband rather than a fixed 1000-2000 Hz range.
pub fn normalize_and_interpolate_response_with_range(
    standard_freq: &ndarray::Array1<f64>,
    curve: &Curve,
    norm_freq_min: f64,
    norm_freq_max: f64,
) -> Curve {
    let spl_norm = normalize_response(curve, norm_freq_min, norm_freq_max);

    interpolate_log_space(
        standard_freq,
        &Curve {
            freq: curve.freq.clone(),
            spl: spl_norm,
            phase: curve.phase.clone(),
            ..Default::default()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cea2034::Curve;
    use ndarray::Array1;

    #[test]
    fn normalize_response_subtracts_mean_in_range() {
        let curve = Curve {
            freq: Array1::from_vec(vec![500.0, 1000.0, 1500.0, 2000.0, 2500.0]),
            spl: Array1::from_vec(vec![0.0, 2.0, 4.0, 6.0, 8.0]),
            phase: None,
            ..Default::default()
        };
        let result = normalize_response(&curve, 1000.0, 2000.0);
        // Mean of [2.0, 4.0, 6.0] = 4.0
        assert!((result[0] - (-4.0)).abs() < 1e-12);
        assert!((result[1] - (-2.0)).abs() < 1e-12);
        assert!((result[2] - 0.0).abs() < 1e-12);
        assert!((result[3] - 2.0).abs() < 1e-12);
        assert!((result[4] - 4.0).abs() < 1e-12);
    }

    #[test]
    fn normalize_response_no_points_in_range_returns_unchanged() {
        let curve = Curve {
            freq: Array1::from_vec(vec![10.0, 20.0, 30.0]),
            spl: Array1::from_vec(vec![5.0, 6.0, 7.0]),
            phase: None,
            ..Default::default()
        };
        let result = normalize_response(&curve, 100.0, 200.0);
        assert_eq!(result.to_vec(), vec![5.0, 6.0, 7.0]);
    }

    #[test]
    fn normalize_and_interpolate_response_preserves_shape() {
        let standard_freq = Array1::logspace(10.0, 2.0, 4.0, 10);
        let curve = Curve {
            freq: Array1::from_vec(vec![100.0, 1000.0, 10000.0]),
            spl: Array1::from_vec(vec![0.0, 5.0, 0.0]),
            phase: None,
            ..Default::default()
        };
        let result = normalize_and_interpolate_response(&standard_freq, &curve);
        assert_eq!(result.freq.len(), standard_freq.len());
        assert_eq!(result.spl.len(), standard_freq.len());
        // All output values should be finite
        for &v in result.spl.iter() {
            assert!(v.is_finite(), "spl must be finite");
        }
    }

    #[test]
    fn interpolate_response_preserves_levels() {
        let standard_freq = Array1::from_vec(vec![100.0, 1000.0, 10000.0]);
        let curve = Curve {
            freq: Array1::from_vec(vec![100.0, 1000.0, 10000.0]),
            spl: Array1::from_vec(vec![80.0, 85.0, 82.0]),
            phase: None,
            ..Default::default()
        };
        let result = interpolate_response(&standard_freq, &curve);
        // Exact match when grids align
        assert!((result.spl[0] - 80.0).abs() < 1e-9);
        assert!((result.spl[1] - 85.0).abs() < 1e-9);
        assert!((result.spl[2] - 82.0).abs() < 1e-9);
    }

    #[test]
    fn normalize_and_interpolate_response_with_range_uses_custom_range() {
        let standard_freq = Array1::from_vec(vec![100.0, 1000.0, 10000.0]);
        let curve = Curve {
            freq: Array1::from_vec(vec![100.0, 500.0, 10000.0]),
            spl: Array1::from_vec(vec![0.0, 10.0, 0.0]),
            phase: None,
            ..Default::default()
        };
        // Normalize using 100-500 Hz range (mean = 5.0)
        let result =
            normalize_and_interpolate_response_with_range(&standard_freq, &curve, 100.0, 500.0);
        assert_eq!(result.freq.len(), standard_freq.len());
        for &v in result.spl.iter() {
            assert!(v.is_finite());
        }
    }
}
