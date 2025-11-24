use crate::Curve;
use ndarray::Array1;

use super::interpolate::*;

pub const NORMALIZE_LOW_FREQ: f64 = 1000.0;
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

// normalize spl and resample a curve
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
        },
    )
}
