//! Xin She Yang N2 test function

use ndarray::Array1;

/// Xin-She Yang N.2 function - newer benchmark function
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-2π, 2π]
pub fn xin_she_yang_n2(x: &Array1<f64>) -> f64 {
    use std::f64::consts::PI;
    let sum_abs: f64 = x.iter().map(|&xi| xi.abs()).sum();
    let exp_sum_sin_sq: f64 = (-x.iter().map(|&xi| xi.powi(2).sin()).sum::<f64>()).exp();
    sum_abs * exp_sum_sin_sq
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xin_she_yang_n2_known_properties() {
        // Test some properties of the Xin-She Yang N.2 function
        use ndarray::Array1;
        use std::f64::consts::{PI, TAU};

        // Test the known global optimum
        let x_global = Array1::from(vec![0.0, 0.0]);
        let f_global = xin_she_yang_n2(&x_global);

        // Should be 0 at the global optimum
        assert!(
            f_global.abs() < 1e-10,
            "Global optimum value not as expected: {}",
            f_global
        );

        // Test that function is always non-negative (sum of abs * exp)
        let test_points = vec![
            vec![1.0, 1.0],
            vec![-3.0, 2.0],
            vec![PI, -PI],
            vec![-6.0, 6.0],
        ];

        for point in test_points {
            let x = Array1::from(point.clone());
            let f = xin_she_yang_n2(&x);

            assert!(
                f >= 0.0,
                "Function should be non-negative at {:?}: {}",
                point,
                f
            );
            assert!(
                f.is_finite(),
                "Function should be finite at {:?}: {}",
                point,
                f
            );
        }

        // Test boundary behavior
        let x_boundary = Array1::from(vec![TAU, -TAU]);
        let f_boundary = xin_she_yang_n2(&x_boundary);
        assert!(
            f_boundary >= 0.0,
            "Function at boundary should be non-negative"
        );
        assert!(
            f_boundary.is_finite(),
            "Function at boundary should be finite"
        );
    }
}
