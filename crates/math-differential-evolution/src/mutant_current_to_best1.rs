use ndarray::{Array1, Array2, Zip};
use rand::Rng;

use crate::distinct_indices::distinct_indices;

pub(crate) fn mutant_current_to_best1<R: Rng + ?Sized>(
    i: usize,
    pop: &Array2<f64>,
    best_idx: usize,
    f: f64,
    rng: &mut R,
) -> Array1<f64> {
    let idxs = distinct_indices(i, 2, pop.nrows(), rng);
    let r0 = idxs[0];
    let r1 = idxs[1];

    Zip::from(pop.row(i))
        .and(pop.row(best_idx))
        .and(pop.row(r0))
        .and(pop.row(r1))
        .map_collect(|&curr, &best, &x0, &x1| curr + f * (best - curr + x0 - x1))
}
