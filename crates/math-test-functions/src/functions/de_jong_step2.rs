//! De Jong Step2 test function

use ndarray::Array1;

/// De Jong step function (variant)
pub fn de_jong_step2(x: &Array1<f64>) -> f64 {
    x.iter().map(|&xi| (xi + 0.5).floor().powi(2)).sum()
}
