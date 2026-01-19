//! Hartman 6D test function

use ndarray::Array1;

/// Hartmann 6-D function - 6D multimodal with 4 local minima
/// Global minimum: f(x) = -3.32237 at complex optimum
/// Bounds: x_i in [0, 1]
pub fn hartman_6d(x: &Array1<f64>) -> f64 {
    let a = [
        [10.0, 3.0, 17.0, 3.5, 1.7, 8.0],
        [0.05, 10.0, 17.0, 0.1, 8.0, 14.0],
        [3.0, 3.5, 1.7, 10.0, 17.0, 8.0],
        [17.0, 8.0, 0.05, 10.0, 0.1, 14.0],
    ];
    let c = [1.0, 1.2, 3.0, 3.2];
    let p = [
        [0.1312, 0.1696, 0.5569, 0.0124, 0.8283, 0.5886],
        [0.2329, 0.4135, 0.8307, 0.3736, 0.1004, 0.9991],
        [0.2348, 0.1451, 0.3522, 0.2883, 0.3047, 0.6650],
        [0.4047, 0.8828, 0.8732, 0.5743, 0.1091, 0.0381],
    ];

    -c.iter()
        .enumerate()
        .map(|(i, &ci)| {
            let inner_sum = a[i]
                .iter()
                .zip(p[i].iter())
                .enumerate()
                .map(|(j, (&aij, &pij))| aij * (x[j] - pij).powi(2))
                .sum::<f64>();
            ci * (-inner_sum).exp()
        })
        .sum::<f64>()
}
