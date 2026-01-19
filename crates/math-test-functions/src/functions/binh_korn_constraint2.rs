//! Binh Korn Constraint2 test function

use ndarray::Array1;

/// Binh-Korn constraint 2: (x1-8)^2 + (x2+3)^2 >= 7.7
pub fn binh_korn_constraint2(x: &Array1<f64>) -> f64 {
    7.7 - ((x[0] - 8.0).powi(2) + (x[1] + 3.0).powi(2))
}
