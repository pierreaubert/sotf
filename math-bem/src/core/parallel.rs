//! Portable parallel iteration abstractions
//!
//! This module provides parallel iteration that works across all build targets:
//! - `native` feature: Uses native rayon for maximum performance
//! - `wasm` feature: Uses wasm-bindgen-rayon for Web Worker parallelism
//! - Neither: Falls back to sequential iteration
//!
//! ## Usage
//!
//! ```ignore
//! use crate::core::parallel::*;
//!
//! // Parallel map over a slice
//! let results: Vec<i32> = parallel_map(&data, |item| item * 2);
//!
//! // Parallel map over indices
//! let results: Vec<i32> = parallel_map_indexed(100, |i| i * 2);
//! ```

/// Check if parallel processing is available
#[inline]
pub fn is_parallel_available() -> bool {
    cfg!(any(feature = "native", feature = "wasm"))
}

/// Parallel map over a slice
///
/// When parallel features are enabled, uses rayon's parallel iterator.
/// Otherwise, falls back to sequential iteration.
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

#[cfg(not(any(feature = "native", feature = "wasm")))]
pub fn parallel_map<T, U, F>(data: &[T], f: F) -> Vec<U>
where
    F: Fn(&T) -> U,
{
    data.iter().map(f).collect()
}

/// Parallel map over a range of indices
///
/// When parallel features are enabled, uses rayon's parallel iterator.
/// Otherwise, falls back to sequential iteration.
#[cfg(any(feature = "native", feature = "wasm"))]
pub fn parallel_map_indexed<U, F>(count: usize, f: F) -> Vec<U>
where
    U: Send,
    F: Fn(usize) -> U + Sync + Send,
{
    use rayon::prelude::*;
    (0..count).into_par_iter().map(f).collect()
}

#[cfg(not(any(feature = "native", feature = "wasm")))]
pub fn parallel_map_indexed<U, F>(count: usize, f: F) -> Vec<U>
where
    F: Fn(usize) -> U,
{
    (0..count).map(f).collect()
}

/// Parallel flat_map over a slice
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

#[cfg(not(any(feature = "native", feature = "wasm")))]
pub fn parallel_flat_map<T, U, I, F>(data: &[T], f: F) -> Vec<U>
where
    I: IntoIterator<Item = U>,
    F: Fn(&T) -> I,
{
    data.iter().flat_map(f).collect()
}

/// Parallel filter_map over a slice
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

#[cfg(not(any(feature = "native", feature = "wasm")))]
pub fn parallel_filter_map<T, U, F>(data: &[T], f: F) -> Vec<U>
where
    F: Fn(&T) -> Option<U>,
{
    data.iter().filter_map(f).collect()
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

#[cfg(not(any(feature = "native", feature = "wasm")))]
pub fn parallel_for_each<T, F>(data: &[T], f: F)
where
    F: Fn(&T),
{
    data.iter().for_each(f);
}

/// Parallel enumerate and map over a slice
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

#[cfg(not(any(feature = "native", feature = "wasm")))]
pub fn parallel_enumerate_map<T, U, F>(data: &[T], f: F) -> Vec<U>
where
    F: Fn(usize, &T) -> U,
{
    data.iter().enumerate().map(|(i, x)| f(i, x)).collect()
}

/// Parallel enumerate and filter_map over a slice
#[cfg(any(feature = "native", feature = "wasm"))]
pub fn parallel_enumerate_filter_map<T, U, F>(data: &[T], f: F) -> Vec<U>
where
    T: Sync,
    U: Send,
    F: Fn(usize, &T) -> Option<U> + Sync + Send,
{
    use rayon::prelude::*;
    data.par_iter()
        .enumerate()
        .filter_map(|(i, x)| f(i, x))
        .collect()
}

#[cfg(not(any(feature = "native", feature = "wasm")))]
pub fn parallel_enumerate_filter_map<T, U, F>(data: &[T], f: F) -> Vec<U>
where
    F: Fn(usize, &T) -> Option<U>,
{
    data.iter()
        .enumerate()
        .filter_map(|(i, x)| f(i, x))
        .collect()
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
    fn test_parallel_filter_map() {
        let data = vec![1, 2, 3, 4, 5];
        let result = parallel_filter_map(&data, |x| if *x % 2 == 0 { Some(*x) } else { None });
        assert_eq!(result, vec![2, 4]);
    }

    #[test]
    fn test_parallel_enumerate_map() {
        let data = vec![10, 20, 30];
        let result = parallel_enumerate_map(&data, |i, x| i + *x);
        assert_eq!(result, vec![10, 21, 32]);
    }
}
