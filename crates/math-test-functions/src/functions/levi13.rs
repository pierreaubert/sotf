//! Levi13 test function

use crate::functions::levy_n13::levy_n13;
use ndarray::Array1;

/// Lévi N.13 function (alias for levy_n13 for compatibility)
pub fn levi13(x: &Array1<f64>) -> f64 {
    levy_n13(x)
}
