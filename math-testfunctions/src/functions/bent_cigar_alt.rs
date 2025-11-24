//! Bent Cigar Alt test function

use ndarray::Array1;

/// Bent Cigar function (alternative implementation)
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-100, 100]
pub fn bent_cigar_alt(x: &Array1<f64>) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    let first = x[0].powi(2);
    let rest: f64 = x.iter().skip(1).map(|&xi| xi.powi(2)).sum();
    first + 1e6 * rest
}
