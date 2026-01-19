//! Bent Cigar test function

use ndarray::Array1;

/// Bent Cigar function - ill-conditioned, unimodal
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-100, 100]
pub fn bent_cigar(x: &Array1<f64>) -> f64 {
    x[0].powi(2) + 1e6 * x.iter().skip(1).map(|&xi| xi.powi(2)).sum::<f64>()
}
