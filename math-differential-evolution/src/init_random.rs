use ndarray::{Array1, Array2};
use rand::Rng;

pub(crate) fn init_random<R: Rng + ?Sized>(
    n: usize,
    npop: usize,
    lower: &Array1<f64>,
    upper: &Array1<f64>,
    is_free: &[bool],
    rng: &mut R,
) -> Array2<f64> {
    let mut pop = Array2::<f64>::zeros((npop, n));
    for i in 0..npop {
        for j in 0..n {
            if is_free[j] {
                let u: f64 = rng.random::<f64>();
                pop[(i, j)] = lower[j] + u * (upper[j] - lower[j]);
            } else {
                pop[(i, j)] = lower[j];
            }
        }
    }
    pop
}
