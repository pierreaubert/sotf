//! Chung Reynolds test function

use ndarray::Array1;

/// Chung Reynolds function - unimodal quadratic function
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-100, 100]
pub fn chung_reynolds(x: &Array1<f64>) -> f64 {
    let sum_squares: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    sum_squares.powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chung_reynolds_known_properties() {
        // Test some properties of the Chung Reynolds function
        use ndarray::Array1;

        // Test the known global optimum
        let x_global = Array1::from(vec![0.0, 0.0]);
        let f_global = chung_reynolds(&x_global);

        // Should be exactly 0 at the global optimum
        assert!(
            f_global.abs() < 1e-15,
            "Global optimum value not as expected: {}",
            f_global
        );

        // Test that function is always non-negative (sum of squares squared)
        let test_points = vec![
            vec![1.0, 1.0],
            vec![-5.0, 3.0],
            vec![10.0, -10.0],
            vec![100.0, -50.0],
        ];

        for point in test_points {
            let x = Array1::from(point.clone());
            let f = chung_reynolds(&x);

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
        let x_boundary = Array1::from(vec![100.0, 100.0]);
        let f_boundary = chung_reynolds(&x_boundary);
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
