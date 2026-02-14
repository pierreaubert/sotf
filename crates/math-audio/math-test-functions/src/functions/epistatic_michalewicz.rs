//! Epistatic Michalewicz test function

use ndarray::Array1;

/// Epistatic Michalewicz function - modified version for GA testing
/// Global minimum: varies by dimension
/// Bounds: x_i in [0, π]
pub fn epistatic_michalewicz(x: &Array1<f64>) -> f64 {
    let m = 10.0; // Steepness parameter
    let n = x.len();

    // Add epistatic (interaction) terms
    let base_sum = -x
        .iter()
        .enumerate()
        .map(|(i, &xi)| {
            xi.sin()
                * ((i as f64 + 1.0) * xi.powi(2) / std::f64::consts::PI)
                    .sin()
                    .powf(2.0 * m)
        })
        .sum::<f64>();

    // Add epistatic interactions between adjacent variables
    let epistatic_sum: f64 = (0..n - 1)
        .map(|i| {
            let xi = x[i];
            let xi_plus_1 = x[i + 1];
            0.1 * (xi * xi_plus_1).sin().powi(2)
        })
        .sum();

    base_sum + epistatic_sum
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epistatic_michalewicz_known_properties() {
        // Test some properties of the Epistatic Michalewicz function
        use ndarray::Array1;

        // Test that function is finite at various points within bounds
        let test_points = vec![
            vec![std::f64::consts::PI / 4.0, std::f64::consts::PI / 4.0],
            vec![std::f64::consts::PI / 2.0, std::f64::consts::PI / 2.0],
            vec![std::f64::consts::PI * 3.0 / 4.0, std::f64::consts::PI / 4.0],
            vec![std::f64::consts::PI, std::f64::consts::PI],
            vec![0.1, 0.1],
            vec![3.0, 2.8],
        ];

        for point in test_points {
            let x = Array1::from(point.clone());
            let f = epistatic_michalewicz(&x);

            assert!(
                f.is_finite(),
                "Function should be finite at {:?}: {}",
                point,
                f
            );
            // Epistatic Michalewicz should generally be negative for good points
        }

        // Test boundary behavior
        let x_boundary = Array1::from(vec![0.0, std::f64::consts::PI]);
        let f_boundary = epistatic_michalewicz(&x_boundary);
        assert!(
            f_boundary.is_finite(),
            "Function at boundary should be finite"
        );

        let x_corner = Array1::from(vec![std::f64::consts::PI, 0.0]);
        let f_corner = epistatic_michalewicz(&x_corner);
        assert!(f_corner.is_finite(), "Function at corner should be finite");
    }
}
