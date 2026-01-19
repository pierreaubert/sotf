use rand::Rng;
use std::collections::HashSet;

pub(crate) fn distinct_indices<R: Rng + ?Sized>(
    exclude: usize,
    count: usize,
    pool_size: usize,
    rng: &mut R,
) -> Vec<usize> {
    debug_assert!(count <= pool_size.saturating_sub(1));
    let mut selected: HashSet<usize> = HashSet::with_capacity(count);
    while selected.len() < count {
        let idx = rng.random_range(0..pool_size);
        if idx != exclude {
            selected.insert(idx);
        }
    }
    selected.into_iter().collect()
}
