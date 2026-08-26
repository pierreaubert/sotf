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

/// Explicit semantics supplied by a `dev_track` call site. GPUI does not yet
/// expose its complete platform accessibility tree to the dev API, so these
/// fields make the application's rendered control contract inspectable without
/// confusing model-only state for a painted element.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevElementState {
    pub enabled: Option<bool>,
    pub selected: Option<bool>,
    pub expanded: Option<bool>,
}

impl DevElementState {
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = Some(value);
        self
    }

    pub fn selected(mut self, value: bool) -> Self {
        self.selected = Some(value);
        self
    }

    pub fn expanded(mut self, value: bool) -> Self {
        self.expanded = Some(value);
        self
    }
}

#[derive(Debug, Clone)]
pub struct TrackedElement {
    pub bounds: Bounds<Pixels>,
    pub state: DevElementState,
}

static REGISTRY: OnceLock<Mutex<HashMap<String, TrackedElement>>> = OnceLock::new();

fn store() -> &'static Mutex<HashMap<String, TrackedElement>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn record(selector: &str, bounds: Bounds<Pixels>) {
    record_with_state(selector, bounds, DevElementState::default());
}

pub fn record_with_state(selector: &str, bounds: Bounds<Pixels>, state: DevElementState) {
    // Painting runs on GPUI's main thread. A QA request may concurrently take
    // a snapshot from the listener thread, so registry publication must never
    // stall rendering while it waits for that short-lived read lock. The next
    // frame refreshes any selector skipped here.
    if let Ok(mut map) = store().try_lock() {
        map.insert(selector.to_string(), TrackedElement { bounds, state });
    }
}

/// Start a fresh rendered-selector frame.
pub fn clear() {
    if let Ok(mut map) = store().try_lock() {
        map.clear();
    }
}

pub fn lookup(selector: &str) -> Option<Bounds<Pixels>> {
    store()
        .lock()
        .ok()
        .and_then(|m| m.get(selector).map(|element| element.bounds))
}

/// Snapshot of all known selectors and their current bounds. Useful
/// for debugging missing selectors via the dev API.
pub fn snapshot() -> Vec<(String, TrackedElement)> {
    store()
        .lock()
        .map(|m| {
            let mut entries: Vec<_> = m.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
            entries
        })
        .unwrap_or_default()
}
