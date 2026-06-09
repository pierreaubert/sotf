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

/// Static metadata for a parameter (set once, read-only after creation).
pub struct ParamMeta {
    pub name: String,
    pub unit: String,
    pub min_value: f64,
    pub max_value: f64,
    pub default_value: f64,
}

/// Thread-safe parameter value cache using atomics.
///
/// Each parameter is stored as an `AtomicU64` holding the bit representation
/// of an `f64` value. Reads and writes use `Relaxed` ordering since we only
/// need eventual consistency for UI display (no ordering constraints).
///
/// Metadata (name, unit, min, max, default) is set once at creation and never
/// changes — no atomics needed for those fields.
pub struct AtomicParamCache {
    values: Box<[AtomicU64]>,
    meta: Vec<ParamMeta>,
}

// AtomicU64 and Vec<ParamMeta> (containing String) are both Send+Sync,
// so AtomicParamCache auto-derives Send+Sync. Compile-time assertion:
const _: () = {
    #[expect(dead_code, reason = "compile-time Send+Sync assertion")]
    fn assert_send_sync<T: Send + Sync>() {}
    fn _check() {
        assert_send_sync::<AtomicParamCache>();
    }
};

impl AtomicParamCache {
    /// Create a new cache with `count` parameters, all initialized to 0.0.
    pub fn new(count: usize) -> Self {
        let values: Vec<AtomicU64> = (0..count)
            .map(|_| AtomicU64::new(0.0_f64.to_bits()))
            .collect();
        let meta: Vec<ParamMeta> = (0..count)
            .map(|_| ParamMeta {
                name: String::new(),
                unit: String::new(),
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.0,
            })
            .collect();
        Self {
            values: values.into_boxed_slice(),
            meta,
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

    /// Get metadata for a parameter by index.
    pub fn meta(&self, index: usize) -> Option<&ParamMeta> {
        self.meta.get(index)
    }

    /// Set metadata for a parameter by index (called once during initialization).
    pub fn set_meta(
        &mut self,
        index: usize,
        name: String,
        unit: String,
        min: f64,
        max: f64,
        default: f64,
    ) {
        if let Some(m) = self.meta.get_mut(index) {
            m.name = name;
            m.unit = unit;
            m.min_value = min;
            m.max_value = max;
            m.default_value = default;
        }
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
///
/// The returned pointer is `Arc`-allocated. Consumers that reconstruct an `Arc`
/// (e.g. `gpui_au_create_with_plugin`) MUST use `Arc::from_raw`.
/// FFI read/write/destroy functions work with raw pointer dereference and are
/// compatible with both `Box` and `Arc` layout since they never free the header.
#[unsafe(no_mangle)]
pub extern "C" fn au_param_cache_create(count: usize) -> *mut AtomicParamCache {
    let arc = std::sync::Arc::new(AtomicParamCache::new(count));
    std::sync::Arc::into_raw(arc) as *mut AtomicParamCache
}

/// Write a denormalized parameter value into the cache.
///
/// Called from Swift's `implementorValueObserver` on the AU main thread
/// whenever a parameter changes (from host automation, MIDI, or UI).
#[unsafe(no_mangle)]
pub extern "C" fn au_param_cache_write(cache: *mut AtomicParamCache, index: usize, value: f64) {
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
pub extern "C" fn au_param_cache_read(cache: *const AtomicParamCache, index: usize) -> f64 {
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

/// Set metadata for a parameter in the cache.
///
/// Called from Swift during initialization to populate parameter names,
/// units, and ranges from the AUParameterTree.
#[unsafe(no_mangle)]
pub extern "C" fn au_param_cache_set_meta(
    cache: *mut AtomicParamCache,
    index: usize,
    name: *const std::os::raw::c_char,
    unit: *const std::os::raw::c_char,
    min_value: f64,
    max_value: f64,
    default_value: f64,
) {
    if cache.is_null() {
        return;
    }
    let cache = unsafe { &mut *cache };
    let name_str = if name.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(name) }
            .to_str()
            .unwrap_or("")
            .to_string()
    };
    let unit_str = if unit.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(unit) }
            .to_str()
            .unwrap_or("")
            .to_string()
    };
    cache.set_meta(
        index,
        name_str,
        unit_str,
        min_value,
        max_value,
        default_value,
    );
}

/// Destroy a parameter cache.
#[unsafe(no_mangle)]
pub extern "C" fn au_param_cache_destroy(cache: *mut AtomicParamCache) {
    if !cache.is_null() {
        unsafe {
            // SAFETY: au_param_cache_create returns Arc::into_raw, so destroy must
            // reconstruct the same Arc allocation shape before dropping it.
            drop(std::sync::Arc::from_raw(cache));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_au_param_cache_destroy_matches_arc_allocation() {
        let cache = au_param_cache_create(2);
        assert!(!cache.is_null());

        au_param_cache_write(cache, 1, 0.75);
        assert_eq!(au_param_cache_read(cache, 1), 0.75);

        au_param_cache_destroy(cache);
    }
}
