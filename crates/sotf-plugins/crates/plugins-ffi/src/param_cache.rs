//! Atomic parameter cache for thread-safe AU plugin UI rendering.
//!
//! The cache stores denormalized parameter values as atomic `f64` (via `AtomicU64`
//! bit-casting). This enables the GPUI render thread to read parameter values
//! without synchronization, while the AU main thread writes values from the
//! `AUParameterTree` observer.
//!
//! # Thread model
//! - **AU main thread**: Writes via `au_param_cache_write()` from `implementorValueObserver`
//! - **GPUI render thread**: Reads via `AtomicParamCache::read()` — lock-free
//! - **Audio render thread**: Never touches the cache (uses `plugin_process()` directly)

use std::sync::atomic::{AtomicU64, Ordering};

/// Thread-safe parameter value cache using atomics.
///
/// Each parameter is stored as an `AtomicU64` holding the bit representation
/// of an `f64` value. Reads and writes use `Relaxed` ordering since we only
/// need eventual consistency for UI display (no ordering constraints).
pub struct AtomicParamCache {
    values: Box<[AtomicU64]>,
}

// SAFETY: AtomicU64 is Send+Sync, Box<[AtomicU64]> is Send+Sync.
// The cache is shared between the AU main thread (writes) and GPUI render (reads).
unsafe impl Send for AtomicParamCache {}
unsafe impl Sync for AtomicParamCache {}

impl AtomicParamCache {
    /// Create a new cache with `count` parameters, all initialized to 0.0.
    pub fn new(count: usize) -> Self {
        let values: Vec<AtomicU64> = (0..count)
            .map(|_| AtomicU64::new(0.0_f64.to_bits()))
            .collect();
        Self {
            values: values.into_boxed_slice(),
        }
    }

    /// Read a parameter value (lock-free).
    pub fn read(&self, index: usize) -> f64 {
        f64::from_bits(self.values[index].load(Ordering::Relaxed))
    }

    /// Write a parameter value (lock-free).
    pub fn write(&self, index: usize, value: f64) {
        self.values[index].store(value.to_bits(), Ordering::Relaxed);
    }

    /// Number of parameters in the cache.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Read all values into a pre-allocated slice.
    pub fn read_all(&self, out: &mut [f64]) {
        for (dst, src) in out.iter_mut().zip(self.values.iter()) {
            *dst = f64::from_bits(src.load(Ordering::Relaxed));
        }
    }
}

// ── FFI ──────────────────────────────────────────────────────────────────────

/// Create a new atomic parameter cache.
#[unsafe(no_mangle)]
pub extern "C" fn au_param_cache_create(count: usize) -> *mut AtomicParamCache {
    Box::into_raw(Box::new(AtomicParamCache::new(count)))
}

/// Write a denormalized parameter value into the cache.
///
/// Called from Swift's `implementorValueObserver` on the AU main thread
/// whenever a parameter changes (from host automation, MIDI, or UI).
#[unsafe(no_mangle)]
pub extern "C" fn au_param_cache_write(
    cache: *mut AtomicParamCache,
    index: usize,
    value: f64,
) {
    if cache.is_null() {
        return;
    }
    let cache = unsafe { &*cache };
    if index < cache.len() {
        cache.write(index, value);
    }
}

/// Read a denormalized parameter value from the cache.
#[unsafe(no_mangle)]
pub extern "C" fn au_param_cache_read(
    cache: *const AtomicParamCache,
    index: usize,
) -> f64 {
    if cache.is_null() {
        return 0.0;
    }
    let cache = unsafe { &*cache };
    if index < cache.len() {
        cache.read(index)
    } else {
        0.0
    }
}

/// Destroy a parameter cache.
#[unsafe(no_mangle)]
pub extern "C" fn au_param_cache_destroy(cache: *mut AtomicParamCache) {
    if !cache.is_null() {
        unsafe {
            drop(Box::from_raw(cache));
        }
    }
}
