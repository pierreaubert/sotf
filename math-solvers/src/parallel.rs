//! Parallel utilities with feature-gated implementations
//!
//! Provides parallel abstractions that work across native (rayon) and WASM
//! (wasm-bindgen-rayon) environments, with sequential fallbacks.

/// Check if parallel processing is available
#[cfg(any(feature = "native", feature = "wasm"))]
pub fn is_parallel_available() -> bool {
    true
}

/// Check if parallel processing is available
#[cfg(not(any(feature = "native", feature = "wasm")))]
pub fn is_parallel_available() -> bool {
    false
}

/// Parallel map over a slice
#[cfg(any(feature = "native", feature = "wasm"))]
pub fn parallel_map<T, U, F>(data: &[T], f: F) -> Vec<U>
where
    T: Sync,
    U: Send,
    F: Fn(&T) -> U + Sync + Send,
{
    use rayon::prelude::*;
    data.par_iter().map(f).collect()
}

/// Sequential map (fallback when parallel is not available)
#[cfg(not(any(feature = "native", feature = "wasm")))]
pub fn parallel_map<T, U, F>(data: &[T], f: F) -> Vec<U>
where
    F: Fn(&T) -> U,
{
    data.iter().map(f).collect()
}

/// Parallel map with index
#[cfg(any(feature = "native", feature = "wasm"))]
pub fn parallel_map_indexed<U, F>(count: usize, f: F) -> Vec<U>
where
    U: Send,
    F: Fn(usize) -> U + Sync + Send,
{
    use rayon::prelude::*;
    (0..count).into_par_iter().map(f).collect()
}

/// Sequential map with index (fallback)
#[cfg(not(any(feature = "native", feature = "wasm")))]
pub fn parallel_map_indexed<U, F>(count: usize, f: F) -> Vec<U>
where
    F: Fn(usize) -> U,
{
    (0..count).map(f).collect()
}

/// Parallel for_each over a slice
#[cfg(any(feature = "native", feature = "wasm"))]
pub fn parallel_for_each<T, F>(data: &[T], f: F)
where
    T: Sync,
    F: Fn(&T) + Sync + Send,
{
    use rayon::prelude::*;
    data.par_iter().for_each(f);
}

/// Sequential for_each (fallback)
#[cfg(not(any(feature = "native", feature = "wasm")))]
pub fn parallel_for_each<T, F>(data: &[T], f: F)
where
    F: Fn(&T),
{
    data.iter().for_each(f);
}

/// Parallel enumerate and map
#[cfg(any(feature = "native", feature = "wasm"))]
pub fn parallel_enumerate_map<T, U, F>(data: &[T], f: F) -> Vec<U>
where
    T: Sync,
    U: Send,
    F: Fn(usize, &T) -> U + Sync + Send,
{
    use rayon::prelude::*;
    data.par_iter().enumerate().map(|(i, x)| f(i, x)).collect()
}

/// Sequential enumerate and map (fallback)
#[cfg(not(any(feature = "native", feature = "wasm")))]
pub fn parallel_enumerate_map<T, U, F>(data: &[T], f: F) -> Vec<U>
where
    F: Fn(usize, &T) -> U,
{
    data.iter().enumerate().map(|(i, x)| f(i, x)).collect()
}

/// Parallel filter_map
#[cfg(any(feature = "native", feature = "wasm"))]
pub fn parallel_filter_map<T, U, F>(data: &[T], f: F) -> Vec<U>
where
    T: Sync,
    U: Send,
    F: Fn(&T) -> Option<U> + Sync + Send,
{
    use rayon::prelude::*;
    data.par_iter().filter_map(f).collect()
}

/// Sequential filter_map (fallback)
#[cfg(not(any(feature = "native", feature = "wasm")))]
pub fn parallel_filter_map<T, U, F>(data: &[T], f: F) -> Vec<U>
where
    F: Fn(&T) -> Option<U>,
{
    data.iter().filter_map(f).collect()
}

/// Parallel flat_map
#[cfg(any(feature = "native", feature = "wasm"))]
pub fn parallel_flat_map<T, U, I, F>(data: &[T], f: F) -> Vec<U>
where
    T: Sync,
    U: Send,
    I: rayon::iter::IntoParallelIterator<Item = U>,
    F: Fn(&T) -> I + Sync + Send,
{
    use rayon::prelude::*;
    data.par_iter().flat_map(f).collect()
}

/// Sequential flat_map (fallback)
#[cfg(not(any(feature = "native", feature = "wasm")))]
pub fn parallel_flat_map<T, U, I, F>(data: &[T], f: F) -> Vec<U>
where
    I: IntoIterator<Item = U>,
    F: Fn(&T) -> I,
{
    data.iter().flat_map(f).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_map() {
        let data = vec![1, 2, 3, 4, 5];
        let result = parallel_map(&data, |x| x * 2);
        assert_eq!(result, vec![2, 4, 6, 8, 10]);
    }

    #[test]
    fn test_parallel_map_indexed() {
        let result = parallel_map_indexed(5, |i| i * 2);
        assert_eq!(result, vec![0, 2, 4, 6, 8]);
    }

    #[test]
    fn test_parallel_enumerate_map() {
        let data = vec![10, 20, 30];
        let result = parallel_enumerate_map(&data, |i, x| (i, *x));
        assert_eq!(result, vec![(0, 10), (1, 20), (2, 30)]);
    }
}
