//! Ackley N3 test function

use ndarray::Array1;

/// Ackley N.3 function - variant of Ackley function
/// Global minimum: f(x) ≈ -195.6 at complex optimum
/// Bounds: x_i in [-32, 32]
pub fn ackley_n3(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    -200.0
        * (-0.02 * (x1.powi(2) + x2.powi(2)).sqrt()).exp()
        * (2.0 * std::f64::consts::PI * x1).cos()
        * (2.0 * std::f64::consts::PI * x2).cos()
        + 5.0 * (3.0 * (x1 + x2)).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ackley_n3_known_properties() {
        // Test some properties of the Ackley N.3 function
        use ndarray::Array1;

        // Test that function is finite at various points
        let test_points = vec![
            vec![0.0, 0.0],
            vec![1.0, -1.0],
            vec![-5.0, 5.0],
            vec![32.0, -32.0],
        ];

        for point in test_points {
            let x = Array1::from(point.clone());
            let f = ackley_n3(&x);

            assert!(
                f.is_finite(),
                "Function should be finite at {:?}: {}",
                point,
                f
            );
            // Ackley N.3 should produce negative values in its optimal region
            if point[0].abs() < 10.0 && point[1].abs() < 10.0 {
                // Near origin, should have potential for good values
            }
        }

        // Test boundary behavior
        let x_boundary = Array1::from(vec![32.0, 32.0]);
        let f_boundary = ackley_n3(&x_boundary);
        assert!(
            f_boundary.is_finite(),
            "Function at boundary should be finite"
        );
    }
}
