//! Process-wide map from selector strings to painted element bounds.
//!
//! Populated each frame by `dev_track(...)` wrappers in the UI tree.
//! Read by the dev API when synthesising mouse clicks.
//!
//! The map is keyed by an opaque selector string (the caller picks the
//! convention — usually `"<area>.<role>"` like `"library.play-button"`).
//! On every paint pass, the wrapper *overwrites* the entry, so stale
//! entries from previous frames simply get refreshed; entries for
//! elements no longer painted are still present but their bounds may
//! be off-screen — the `/click` handler should sanity-check.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use gpui::{Bounds, Pixels};

static REGISTRY: OnceLock<Mutex<HashMap<String, Bounds<Pixels>>>> = OnceLock::new();

fn store() -> &'static Mutex<HashMap<String, Bounds<Pixels>>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn record(selector: &str, bounds: Bounds<Pixels>) {
    if let Ok(mut map) = store().lock() {
        map.insert(selector.to_string(), bounds);
    }
}

pub fn lookup(selector: &str) -> Option<Bounds<Pixels>> {
    store().lock().ok().and_then(|m| m.get(selector).copied())
}

/// Snapshot of all known selectors and their current bounds. Useful
/// for debugging missing selectors via the dev API.
pub fn snapshot() -> Vec<(String, Bounds<Pixels>)> {
    store()
        .lock()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect())
        .unwrap_or_default()
}
