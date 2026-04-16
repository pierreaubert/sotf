//! Invocation-count profiler for bytecode functions.
//!
//! Each bytecode function is identified by an opaque `usize` id (typically
//! its index into the interpreter's function table). The profiler bumps a
//! counter on every call and signals when the count reaches the
//! compilation threshold.

use std::collections::HashMap;

/// Tracks per-function invocation counts and decides when a function
/// is hot enough to hand off to the JIT compiler.
pub struct Profiler {
    /// function id -> cumulative call count
    counters: HashMap<usize, u64>,
    /// Number of calls before a function is considered hot.
    threshold: u64,
}

impl Profiler {
    /// Create a new profiler.
    ///
    /// `threshold` is the number of invocations after which
    /// [`record_call`](Self::record_call) returns `true`.
    pub fn new(threshold: u64) -> Self {
        Self {
            counters: HashMap::new(),
            threshold,
        }
    }

    /// Bump the counter for `func_id`.
    ///
    /// Returns `true` exactly once -- the first time the counter reaches
    /// the threshold -- so the caller can trigger compilation.
    pub fn record_call(&mut self, func_id: usize) -> bool {
        let count = self.counters.entry(func_id).or_insert(0);
        *count += 1;
        *count == self.threshold
    }

    /// Check whether `func_id` has already reached the compilation
    /// threshold (without bumping the counter).
    pub fn should_compile(&self, func_id: usize) -> bool {
        self.counters
            .get(&func_id)
            .is_some_and(|&c| c >= self.threshold)
    }

    /// Reset the counter for `func_id`.
    ///
    /// Useful after a function is redefined at runtime: the old native
    /// code is invalidated and the function must be re-profiled from
    /// scratch before it is re-compiled.
    pub fn reset(&mut self, func_id: usize) {
        self.counters.remove(&func_id);
    }

    /// Total number of recorded calls across all functions.
    pub fn total_calls(&self) -> u64 {
        self.counters.values().sum()
    }

    /// Number of functions that have reached the compilation threshold.
    pub fn hot_function_count(&self) -> u64 {
        self.counters
            .values()
            .filter(|&&c| c >= self.threshold)
            .count() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_at_threshold() {
        let mut p = Profiler::new(3);
        assert!(!p.record_call(0));
        assert!(!p.record_call(0));
        assert!(p.record_call(0)); // 3rd call
                                   // subsequent calls do not re-trigger
        assert!(!p.record_call(0));
    }

    #[test]
    fn should_compile_reflects_state() {
        let mut p = Profiler::new(2);
        assert!(!p.should_compile(1));
        p.record_call(1);
        assert!(!p.should_compile(1));
        p.record_call(1);
        assert!(p.should_compile(1));
    }

    #[test]
    fn reset_clears_counter() {
        let mut p = Profiler::new(1);
        assert!(p.record_call(5));
        assert!(p.should_compile(5));
        p.reset(5);
        assert!(!p.should_compile(5));
        // fires again after re-profiling
        assert!(p.record_call(5));
    }
}
