//! Vincent test function

use ndarray::Array1;

/// Vincent function - high-dimensional multimodal
/// Global minimum: f(x) = -N at x = (7.70628, 7.70628, ..., 7.70628)
/// Bounds: x_i in [0.25, 10]
pub fn vincent(x: &Array1<f64>) -> f64 {
    -x.iter().map(|&xi| (10.0 * xi).sin()).sum::<f64>()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vincent_known_properties() {
        // Test some properties of the Vincent function
        use ndarray::Array1;

        // Test the approximate known optimum
        let x_approx = Array1::from(vec![7.70628, 7.70628]);
        let f_approx = vincent(&x_approx);

        // Should be approximately -2.0 for 2D
        assert!(
            f_approx < -1.9,
            "Approximate optimum value not as expected: {}",
            f_approx
        );

        // Test boundary behavior
        let x_low = Array1::from(vec![0.25, 0.25]);
        let f_low = vincent(&x_low);
        assert!(
            f_low.is_finite(),
            "Function at lower bound should be finite"
        );

        let x_high = Array1::from(vec![10.0, 10.0]);
        let f_high = vincent(&x_high);
        assert!(
            f_high.is_finite(),
            "Function at upper bound should be finite"
        );
    }
}
