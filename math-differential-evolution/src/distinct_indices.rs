use rand::Rng;
use rand::seq::SliceRandom;

pub(crate) fn distinct_indices<R: Rng + ?Sized>(
    exclude: usize,
    count: usize,
    pool_size: usize,
    rng: &mut R,
) -> Vec<usize> {
    debug_assert!(count <= pool_size.saturating_sub(1));
    // Generate a shuffled pool and take first `count` not equal to exclude
    let mut idxs: Vec<usize> = (0..pool_size).collect();
    idxs.shuffle(rng);
    let mut out = Vec::with_capacity(count);
    for idx in idxs.into_iter() {
        if idx == exclude {
            continue;
        }
        out.push(idx);
        if out.len() == count {
            break;
        }
    }
    out
}
