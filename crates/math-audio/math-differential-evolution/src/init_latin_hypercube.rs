use ndarray::{Array1, Array2};
use rand::seq::SliceRandom;
use rand::Rng;

pub(crate) fn init_latin_hypercube<R: Rng + ?Sized>(
    n: usize,
    npop: usize,
    lower: &Array1<f64>,
    upper: &Array1<f64>,
    is_free: &[bool],
    rng: &mut R,
) -> Array2<f64> {
    let mut samples = Array2::<f64>::zeros((npop, n));
    // For each dimension, create stratified samples and permute
    for j in 0..n {
        if !is_free[j] {
            // fixed variable
            for i in 0..npop {
                samples[(i, j)] = 0.0;
            }
            continue;
        }
        let mut vals = Vec::with_capacity(npop);
        for k in 0..npop {
            let u: f64 = rng.random::<f64>();
            vals.push(((k as f64) + u) / (npop as f64));
        }
        vals.shuffle(rng);
        for i in 0..npop {
            samples[(i, j)] = vals[i];
        }
    }
    // Scale to [lower, upper]
    for i in 0..npop {
        for j in 0..n {
            if is_free[j] {
                samples[(i, j)] = lower[j] + samples[(i, j)] * (upper[j] - lower[j]);
            } else {
                samples[(i, j)] = lower[j];
            }
        }
    }
    samples
}
