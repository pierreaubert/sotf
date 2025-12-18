//! Hartman 3D test function

use ndarray::Array1;

/// Hartman 3D function - 3D multimodal with 4 local minima
/// Global minimum: f(x) = -3.86278 at x = (0.114614, 0.555649, 0.852547)
/// Bounds: x_i in [0, 1]
pub fn hartman_3d(x: &Array1<f64>) -> f64 {
    let a = [
        [3.0, 10.0, 30.0],
        [0.1, 10.0, 35.0],
        [3.0, 10.0, 30.0],
        [0.1, 10.0, 35.0],
    ];
    let c = [1.0, 1.2, 3.0, 3.2];
    let p = [
        [0.3689, 0.1170, 0.2673],
        [0.4699, 0.4387, 0.7470],
        [0.1091, 0.8732, 0.5547],
        [0.03815, 0.5743, 0.8828],
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
