//! Langermann test function

use ndarray::Array1;

/// Langermann function - complex multimodal with parameters
/// Global minimum: f(x) ≈ -5.1621 at complex optimum
/// Bounds: x_i in [0, 10]
pub fn langermann(x: &Array1<f64>) -> f64 {
    // Langermann function parameters (for 2D)
    let a = [[3.0, 5.0], [5.0, 2.0], [2.0, 1.0], [1.0, 4.0], [7.0, 9.0]];
    let c = [1.0, 2.0, 5.0, 2.0, 3.0];

    let mut sum = 0.0;
    for i in 0..5 {
        let mut inner_sum = 0.0;
        for j in 0..2.min(x.len()) {
            inner_sum += (x[j] - a[i][j]).powi(2);
        }
        sum += c[i]
            * (-inner_sum / std::f64::consts::PI).exp()
            * (std::f64::consts::PI * inner_sum).cos();
    }
    sum
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_langermann_known_properties() {
        // Test some properties of the Langermann function
        use ndarray::Array1;

        // Test that function is finite at various points within bounds
        let test_points = vec![
            vec![2.0, 1.0], // Near one of the parameter points
            vec![5.0, 2.0], // Near another parameter point
            vec![7.0, 9.0], // Near the third parameter point
            vec![1.0, 4.0], // Near the fourth parameter point
            vec![0.5, 0.5], // Corner region
            vec![9.5, 9.5], // Other corner
        ];

        for point in test_points {
            let x = Array1::from(point.clone());
            let f = langermann(&x);

            assert!(
                f.is_finite(),
                "Function should be finite at {:?}: {}",
                point,
                f
            );
            // Langermann can have both positive and negative values
        }

        // Test boundary behavior
        let x_boundary = Array1::from(vec![0.0, 10.0]);
        let f_boundary = langermann(&x_boundary);
        assert!(
            f_boundary.is_finite(),
            "Function at boundary should be finite"
        );

        let x_corner = Array1::from(vec![10.0, 0.0]);
        let f_corner = langermann(&x_corner);
        assert!(f_corner.is_finite(), "Function at corner should be finite");
    }
}
