//! Powell test function

use ndarray::Array1;

/// Powell function - unimodal but ill-conditioned
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-4, 5]
pub fn powell(x: &Array1<f64>) -> f64 {
    let n = x.len();
    let mut sum = 0.0;
    for i in (0..n).step_by(4) {
        if i + 3 < n {
            let x1 = x[i];
            let x2 = x[i + 1];
            let x3 = x[i + 2];
            let x4 = x[i + 3];
            sum += (x1 + 10.0 * x2).powi(2)
                + 5.0 * (x3 - x4).powi(2)
                + (x2 - 2.0 * x3).powi(4)
                + 10.0 * (x1 - x4).powi(4);
        }
    }
    sum
}
