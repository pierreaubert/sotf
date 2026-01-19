//! Binh Korn Weighted test function

use ndarray::Array1;

/// Binh-Korn weighted objective function
pub fn binh_korn_weighted(x: &Array1<f64>) -> f64 {
    4.0 * x[0].powi(2) + 4.0 * x[1].powi(2)
}
