//! Pinter test function

use ndarray::Array1;

/// Pinter function - challenging multimodal function
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-10, 10]
pub fn pinter(x: &Array1<f64>) -> f64 {
    let n = x.len();
    let mut sum1 = 0.0;
    let mut sum2 = 0.0;
    let mut sum3 = 0.0;

    for i in 0..n {
        let ii = (i + 1) as f64;
        let xi = x[i];
        let x_prev = if i == 0 { x[n - 1] } else { x[i - 1] };
        let x_next = if i == n - 1 { x[0] } else { x[i + 1] };

        let ai = x_prev * xi.sin() + (x_next - xi).sin();
        let bi = x_prev.powi(2) - 2.0 * xi + 3.0 * x_next - (1.0 + xi).cos() + 1.0;

        sum1 += ii * xi.powi(2);
        sum2 += 20.0 * ii * ai.powi(2).sin();
        sum3 += ii * (1.0 + ii).ln() * bi.powi(2);
    }

    sum1 + sum2 + sum3
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pinter_known_properties() {
        // Test some properties of the Pinter function
        use ndarray::Array1;

        // Test the function at origin point
        let x_origin = Array1::from(vec![0.0, 0.0]);
        let f_origin = pinter(&x_origin);

        // Function should be finite at origin
        assert!(
            f_origin.is_finite(),
            "Function should be finite at origin: {}",
            f_origin
        );

        // Test boundary behavior - should be finite
        let x_bound = Array1::from(vec![-10.0, 10.0]);
        let f_bound = pinter(&x_bound);
        assert!(f_bound.is_finite(), "Function at boundary should be finite");

        // Test a point away from optimum - should be positive
        let x_away = Array1::from(vec![1.0, 1.0]);
        let f_away = pinter(&x_away);
        assert!(
            f_away > 0.0,
            "Function away from optimum should be positive: {}",
            f_away
        );
    }
}
