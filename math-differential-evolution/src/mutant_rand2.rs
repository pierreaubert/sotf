use ndarray::{Array1, Array2};
use rand::Rng;

use crate::distinct_indices::distinct_indices;

pub(crate) fn mutant_rand2<R: Rng + ?Sized>(
    i: usize,
    pop: &Array2<f64>,
    f: f64,
    rng: &mut R,
) -> Array1<f64> {
    let idxs = distinct_indices(i, 5, pop.nrows(), rng);
    let r0 = idxs[0];
    let r1 = idxs[1];
    let r2 = idxs[2];
    let r3 = idxs[3];
    let r4 = idxs[4];
    &pop.row(r0).to_owned()
        + &((pop.row(r1).to_owned() + pop.row(r2).to_owned()
            - pop.row(r3).to_owned()
            - pop.row(r4).to_owned())
            * f)
}
