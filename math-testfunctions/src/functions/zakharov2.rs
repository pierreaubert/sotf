//! Zakharov2 test function

use crate::functions::zakharov::zakharov;
use ndarray::Array1;

/// Zakharov function variant (2D specific)
pub fn zakharov2(x: &Array1<f64>) -> f64 {
    zakharov(x)
}
