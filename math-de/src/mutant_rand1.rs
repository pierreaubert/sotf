use ndarray::{Array1, Array2};
use rand::Rng;

use crate::distinct_indices::distinct_indices;

pub(crate) fn mutant_rand1<R: Rng + ?Sized>(
    i: usize,
    pop: &Array2<f64>,
    f: f64,
    rng: &mut R,
) -> Array1<f64> {
    let idxs = distinct_indices(i, 3, pop.nrows(), rng);
    let r0 = idxs[0];
    let r1 = idxs[1];
    let r2 = idxs[2];
    &pop.row(r0).to_owned() + &(pop.row(r1).to_owned() - pop.row(r2).to_owned()) * f
}
