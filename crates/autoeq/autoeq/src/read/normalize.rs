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
        },
    )
}
