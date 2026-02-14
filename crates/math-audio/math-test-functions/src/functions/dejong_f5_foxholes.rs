//! Dejong F5 Foxholes test function

use ndarray::Array1;

/// De Jong F5 (Shekel's foxholes) function - 2D
pub fn dejong_f5_foxholes(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];

    // Shekel's foxholes a matrix (2x25)
    let a = [
        [
            -32, -16, 0, 16, 32, -32, -16, 0, 16, 32, -32, -16, 0, 16, 32, -32, -16, 0, 16, 32,
            -32, -16, 0, 16, 32,
        ],
        [
            -32, -32, -32, -32, -32, -16, -16, -16, -16, -16, 0, 0, 0, 0, 0, 16, 16, 16, 16, 16,
            32, 32, 32, 32, 32,
        ],
    ];

    let mut sum = 0.0;
    for j in 0..25 {
        let mut inner_sum = 0.0;
        for (i, &a_val) in a.iter().enumerate() {
            let xi = if i == 0 { x1 } else { x2 };
            inner_sum += (xi - a_val[j] as f64).powi(6);
        }
        sum += 1.0 / (j as f64 + 1.0 + inner_sum);
    }
    1.0 / (0.002 + sum)
}
