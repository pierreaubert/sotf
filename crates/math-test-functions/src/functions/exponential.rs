//! Exponential test function

use ndarray::Array1;

/// Exponential function - unimodal function
/// Global minimum: f(x) = -1 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-1, 1]
pub fn exponential(x: &Array1<f64>) -> f64 {
    let sum_squares: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    -(-0.5 * sum_squares).exp()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_known_properties() {
        // Test some properties of the exponential function
        use ndarray::Array1;

        // Test the known global optimum
        let x_global = Array1::from(vec![0.0, 0.0]);
        let f_global = exponential(&x_global);

        // Should be -1 at the global optimum
        assert!(
            (f_global + 1.0).abs() < 1e-15,
            "Global optimum value not as expected: {}",
            f_global
        );

        // Test that function is always negative and bounded above by -e^(-0.5*0) = -1
        let test_points = vec![
            vec![0.5, 0.5],
            vec![-0.5, 0.3],
            vec![1.0, -1.0],
            vec![-1.0, 1.0],
        ];

        for point in test_points {
            let x = Array1::from(point.clone());
            let f = exponential(&x);

            assert!(
                f <= -0.0,
                "Function should be negative at {:?}: {}",
                point,
                f
            );
            assert!(f >= -1.0, "Function should be >= -1 at {:?}: {}", point, f);
            assert!(
                f.is_finite(),
                "Function should be finite at {:?}: {}",
                point,
                f
            );
        }

        // Test boundary behavior
        let x_boundary = Array1::from(vec![1.0, 1.0]);
        let f_boundary = exponential(&x_boundary);
        assert!(f_boundary <= 0.0, "Function at boundary should be negative");
        assert!(
            f_boundary.is_finite(),
            "Function at boundary should be finite"
        );
    }
}
