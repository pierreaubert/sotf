//! Xin She Yang N3 test function

use ndarray::Array1;

/// Xin-She Yang N.3 function - multimodal with parameter m
/// Global minimum: f(x) = -1 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-20, 20]
pub fn xin_she_yang_n3(x: &Array1<f64>) -> f64 {
    let m = 5.0; // Parameter
    let beta = 15.0; // Parameter
    let sum_pow: f64 = x.iter().map(|&xi| xi.abs().powf(m)).sum();
    let prod_cos_sq: f64 = x.iter().map(|&xi| (beta * xi).cos().powi(2)).product();
    -(-sum_pow).exp() * prod_cos_sq
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xin_she_yang_n3_known_properties() {
        // Test some properties of the Xin-She Yang N.3 function
        use ndarray::Array1;

        // Test the known global optimum
        let x_global = Array1::from(vec![0.0, 0.0]);
        let f_global = xin_she_yang_n3(&x_global);

        // Should be -1 at the global optimum
        assert!(
            (f_global + 1.0).abs() < 1e-10,
            "Global optimum value not as expected: {}",
            f_global
        );

        // Test that function values are bounded above by 0
        let test_points = vec![
            vec![1.0, 1.0],
            vec![-5.0, 3.0],
            vec![10.0, -10.0],
            vec![-15.0, 15.0],
        ];

        for point in test_points {
            let x = Array1::from(point.clone());
            let f = xin_she_yang_n3(&x);

            assert!(f <= 0.0, "Function should be <= 0 at {:?}: {}", point, f);
            assert!(f >= -1.0, "Function should be >= -1 at {:?}: {}", point, f);
            assert!(
                f.is_finite(),
                "Function should be finite at {:?}: {}",
                point,
                f
            );
        }
    }
}
