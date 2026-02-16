//! Current-to-pbest/1 mutation strategy
//!
//! This is the key mutation strategy used by SHADE, L-SHADE and their variants.
//! It blends the current individual with one of the top-p% best individuals,
//! plus a difference vector that can include archive solutions.

use ndarray::{Array1, Array2, Zip};
use rand::Rng;

use crate::distinct_indices::distinct_indices;
use crate::external_archive::ExternalArchive;

/// Current-to-pbest/1 mutation with optional archive
///
/// v_i = x_i + F * (x_pbest - x_i) + F * (x_r1 - x_r2)
///
/// Where:
/// - x_pbest is randomly selected from top p% of population
/// - x_r1 is a random individual from population
/// - x_r2 can be from population OR from external archive
pub(crate) fn mutant_current_to_pbest1<R: Rng + ?Sized>(
    i: usize,
    pop: &Array2<f64>,
    sorted_indices: &[usize],
    p_best_size: usize,
    archive: Option<&ExternalArchive>,
    f: f64,
    rng: &mut R,
) -> Array1<f64> {
    let npop = pop.nrows();

    let pbest_idx = if p_best_size >= npop {
        sorted_indices[0]
    } else {
        sorted_indices[rng.random_range(0..p_best_size)]
    };

    let r1 = distinct_indices(i, 1, npop, rng)[0];

    let r2 = if let Some(arch) = archive {
        if !arch.is_empty() && rng.random::<f64>() < 0.5 {
            if let Some(sol) = arch.random_select(rng) {
                sol.clone()
            } else {
                let available: Vec<usize> =
                    (0..npop).filter(|&idx| idx != i && idx != r1).collect();
                if available.is_empty() {
                    pop.row(r1).to_owned()
                } else {
                    pop.row(available[rng.random_range(0..available.len())])
                        .to_owned()
                }
            }
        } else {
            let available: Vec<usize> = (0..npop).filter(|&idx| idx != i && idx != r1).collect();
            if available.is_empty() {
                pop.row(r1).to_owned()
            } else {
                pop.row(available[rng.random_range(0..available.len())])
                    .to_owned()
            }
        }
    } else {
        let available: Vec<usize> = (0..npop).filter(|&idx| idx != i && idx != r1).collect();
        if available.is_empty() {
            pop.row(r1).to_owned()
        } else {
            pop.row(available[rng.random_range(0..available.len())])
                .to_owned()
        }
    };

    Zip::from(pop.row(i))
        .and(pop.row(pbest_idx))
        .and(pop.row(r1))
        .and(r2.view())
        .map_collect(|&curr, &pbest, &x1, &x2| curr + f * (pbest - curr) + f * (x1 - x2))
}

/// Compute p_best_size from p parameter (0 < p <= 1)
/// p=0.1 means top 10% of population
#[inline]
pub(crate) fn compute_pbest_size(p: f64, npop: usize) -> usize {
    ((p * npop as f64).ceil() as usize).max(1).min(npop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_pbest_size() {
        assert_eq!(compute_pbest_size(0.1, 100), 10);
        assert_eq!(compute_pbest_size(0.1, 50), 5);
        assert_eq!(compute_pbest_size(0.1, 10), 1);
        assert_eq!(compute_pbest_size(0.5, 10), 5);
    }

    #[test]
    fn test_mutant_basic() {
        let pop = array![[0.0, 0.0], [1.0, 1.0], [2.0, 2.0], [3.0, 3.0]];
        let sorted_indices = vec![3, 2, 1, 0];

        let mut rng = rand::rng();
        let mutant = mutant_current_to_pbest1(0, &pop, &sorted_indices, 2, None, 0.5, &mut rng);

        assert_eq!(mutant.len(), 2);
    }
}
