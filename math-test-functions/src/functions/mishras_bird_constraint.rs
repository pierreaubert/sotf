//! Mishras Bird Constraint test function

use ndarray::Array1;

/// Mishra's Bird constraint: (x+5)^2 + (y+5)^2 < 25
pub fn mishras_bird_constraint(x: &Array1<f64>) -> f64 {
    (x[0] + 5.0).powi(2) + (x[1] + 5.0).powi(2) - 25.0
}
