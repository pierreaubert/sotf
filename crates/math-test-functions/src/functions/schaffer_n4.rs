//! Schaffer N4 test function

use ndarray::Array1;

/// Schaffer N.4 function - multimodal, 2D only
/// Global minimum: f(x) = 0.292579 at x = (0, ±1.25313) or (±1.25313, 0)
/// Bounds: x_i in [-100, 100]
pub fn schaffer_n4(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    0.5 + ((x1.powi(2) - x2.powi(2)).sin().powi(2) - 0.5)
        / (1.0 + 0.001 * (x1.powi(2) + x2.powi(2))).powi(2)
}
