use ndarray::{Array1, Array2};
use rand::Rng;

use crate::distinct_indices::distinct_indices;

pub(crate) fn mutant_rand_to_best1<R: Rng + ?Sized>(
    i: usize,
    pop: &Array2<f64>,
    best_idx: usize,
    f: f64,
    rng: &mut R,
) -> Array1<f64> {
    // x_r0 + F * (x_best - x_r0 + x_r1 - x_r2)
    let idxs = distinct_indices(i, 3, pop.nrows(), rng);
    let r0 = idxs[0];
    let r1 = idxs[1];
    let r2 = idxs[2];
    let x_r0 = pop.row(r0).to_owned();
    &x_r0
        + &((pop.row(best_idx).to_owned() - x_r0.clone() + pop.row(r1).to_owned()
            - pop.row(r2).to_owned())
            * f)
}
