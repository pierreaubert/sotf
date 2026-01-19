//! Xin She Yang N4 test function

use ndarray::Array1;

/// Xin-She Yang N.4 function - challenging multimodal
/// Global minimum: f(x) = -1 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-10, 10]
pub fn xin_she_yang_n4(x: &Array1<f64>) -> f64 {
    let sum_sin_sq: f64 = x.iter().map(|&xi| xi.powi(2).sin()).sum();
    let sum_squares: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    (sum_sin_sq - (-sum_squares).exp()) * (-sum_squares.sin().powi(2)).exp()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xin_she_yang_n4_known_properties() {
        // Test some properties of the Xin-She Yang N.4 function
        use ndarray::Array1;

        // Test the known global optimum
        let x_global = Array1::from(vec![0.0, 0.0]);
        let f_global = xin_she_yang_n4(&x_global);

        // Should be -1 at the global optimum
        assert!(
            (f_global + 1.0).abs() < 1e-10,
            "Global optimum value not as expected: {}",
            f_global
        );

        // Test that function is finite at various points
        let test_points = vec![
            vec![1.0, 1.0],
            vec![-3.0, 2.0],
            vec![5.0, -5.0],
            vec![-8.0, 8.0],
        ];

        for point in test_points {
            let x = Array1::from(point.clone());
            let f = xin_she_yang_n4(&x);

            assert!(
                f.is_finite(),
                "Function should be finite at {:?}: {}",
                point,
                f
            );
            // This is a very complex function, so just check it's bounded reasonably
            assert!(
                f > -2.0,
                "Function seems too negative at {:?}: {}",
                point,
                f
            );
            assert!(
                f < 100.0,
                "Function seems too positive at {:?}: {}",
                point,
                f
            );
        }

        // Test boundary behavior
        let x_boundary = Array1::from(vec![10.0, -10.0]);
        let f_boundary = xin_she_yang_n4(&x_boundary);
        assert!(
            f_boundary.is_finite(),
            "Function at boundary should be finite"
        );
    }
}
