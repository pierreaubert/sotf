//! Qing test function

use ndarray::Array1;

/// Qing function - separable multimodal function
/// Global minimum: f(x) = 0 at x = (±√i, ±√2, ..., ±√n)
/// Bounds: x_i in [-500, 500]
pub fn qing(x: &Array1<f64>) -> f64 {
    x.iter()
        .enumerate()
        .map(|(i, &xi)| (xi.powi(2) - (i + 1) as f64).powi(2))
        .sum()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qing_known_properties() {
        // Test some properties of the Qing function
        use ndarray::Array1;

        // Test the known global optima
        let x_pos = Array1::from(vec![1.0, (2.0_f64).sqrt()]);
        let f_pos = qing(&x_pos);
        assert!(
            f_pos.abs() < 1e-10,
            "Positive global optimum value not as expected: {}",
            f_pos
        );

        let x_neg = Array1::from(vec![-1.0, -(2.0_f64).sqrt()]);
        let f_neg = qing(&x_neg);
        assert!(
            f_neg.abs() < 1e-10,
            "Negative global optimum value not as expected: {}",
            f_neg
        );

        // Test that function is always non-negative (sum of squares)
        let test_points = vec![
            vec![0.0, 0.0],
            vec![10.0, 10.0],
            vec![-50.0, 30.0],
            vec![100.0, -200.0],
        ];

        for point in test_points {
            let x = Array1::from(point.clone());
            let f = qing(&x);

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
        let x_boundary = Array1::from(vec![500.0, -500.0]);
        let f_boundary = qing(&x_boundary);
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
