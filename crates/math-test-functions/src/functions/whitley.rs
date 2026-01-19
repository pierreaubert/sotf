//! Whitley test function

use ndarray::Array1;

/// Whitley function - challenging multimodal function
/// Global minimum: f(x) = 0 at x = (1, 1, ..., 1)
/// Bounds: x_i in [-10.24, 10.24]
pub fn whitley(x: &Array1<f64>) -> f64 {
    let n = x.len();
    let mut sum = 0.0;

    for i in 0..n {
        for j in 0..n {
            let xi = x[i];
            let xj = x[j];
            let term = 100.0 * (xi.powi(2) - xj).powi(2) + (1.0 - xj).powi(2);
            sum += term.powi(2) / 4000.0 - term.cos() + 1.0;
        }
    }
    sum
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitley_known_properties() {
        // Test some properties of the Whitley function
        use ndarray::Array1;

        // Test the known global optimum
        let x_global = Array1::from(vec![1.0, 1.0]);
        let f_global = whitley(&x_global);

        // Should be 0 at the global optimum
        assert!(
            f_global.abs() < 1e-10,
            "Global optimum value not as expected: {}",
            f_global
        );

        // Test that function is always non-negative (based on its construction)
        let test_points = vec![
            vec![0.0, 0.0],
            vec![2.0, 2.0],
            vec![-5.0, 3.0],
            vec![10.0, -10.0],
        ];

        for point in test_points {
            let x = Array1::from(point.clone());
            let f = whitley(&x);

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
        let x_boundary = Array1::from(vec![10.24, -10.24]);
        let f_boundary = whitley(&x_boundary);
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
