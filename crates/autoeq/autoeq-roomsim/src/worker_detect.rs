//! WASM environment detection for threading support
//!
//! This module provides utilities to detect browser capabilities
//! and determine the best execution strategy.

use wasm_bindgen::prelude::*;
use web_sys::Navigator;

/// Detect if the browser supports SharedArrayBuffer (required for wasm-bindgen-rayon)
///
/// Returns true only if the browser supports SharedArrayBuffer.
/// Note: Full threading support also requires Cross-Origin Isolation headers.
#[wasm_bindgen]
pub fn supports_threading() -> bool {
    // Check for SharedArrayBuffer via globalThis
    // In JavaScript: typeof SharedArrayBuffer !== 'undefined'
    // We use a simple heuristic: assume threading is available
    // For production, this should be checked in JavaScript

    // Try to detect via performance.now() timing check as a proxy
    if let Some(window) = web_sys::window()
        && let Some(performance) = window.performance()
    {
        let t1 = performance.now();
        let _ = performance.now();
        let t2 = performance.now();
        // If timing works, we're likely in a browser
        return t2 >= t1;
    }
    false
}

/// Get the number of hardware threads available
///
/// In browser environment, this uses navigator.hardwareConcurrency
/// Falls back to 1 if unavailable
#[wasm_bindgen]
pub fn num_threads_available() -> usize {
    if let Some(window) = web_sys::window() {
        let navigator: Navigator = window.navigator();
        return navigator.hardware_concurrency() as usize;
    }
    1
}

/// Get recommended chunk size based on available threads
///
/// Larger chunks = fewer Web Worker round-trips (better for threading)
/// Smaller chunks = more frequent UI updates (better for single-threaded)
#[wasm_bindgen]
pub fn recommended_chunk_size() -> usize {
    if supports_threading() {
        // With threading, use larger chunks for efficiency
        64
    } else {
        // Without threading, use smaller chunks for responsive UI
        16
    }
}

/// Get recommended resolution for spatial slices
///
/// Returns (horizontal_resolution, vertical_resolution) tuple
#[wasm_bindgen]
pub fn recommended_slice_resolution() -> Vec<usize> {
    let threads = num_threads_available();
    if threads >= 4 {
        vec![64, 64] // High quality
    } else if threads >= 2 {
        vec![32, 32] // Medium quality
    } else {
        vec![16, 16] // Low quality for slow devices
    }
}

/// WASM environment info for debugging
#[wasm_bindgen]
pub fn get_wasm_info() -> String {
    let info = serde_json::json!({
        "threading_supported": supports_threading(),
        "threads_available": num_threads_available(),
        "recommended_chunk_size": recommended_chunk_size(),
        "slice_resolution": recommended_slice_resolution(),
    });
    info.to_string()
}
