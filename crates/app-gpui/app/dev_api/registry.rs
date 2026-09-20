//! Process-wide map from selector strings to painted element bounds.
//!
//! Populated each frame by `dev_track(...)` wrappers in the UI tree.
//! Read by the dev API when synthesising mouse clicks.
//!
//! The map is keyed by an opaque selector string (the caller picks the
//! convention — usually `"<area>.<role>"` like `"library.play-button"`).
//! Application roots stage their bounds on the paint thread and publish only
//! after the complete root paints. HTTP readers see the last completed frame,
//! never an empty or partially populated registry during repaint. Standalone
//! tracked elements can still record directly for isolated component tests.

use std::cell::RefCell;
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

type SelectorMap = HashMap<String, TrackedElement>;

thread_local! {
    static PAINT_FRAMES: RefCell<HashMap<u64, SelectorMap>> = RefCell::new(HashMap::new());
}

/// Stage a complete paint on the UI thread without exposing partial bounds.
pub fn begin_frame(window_id: u64) {
    PAINT_FRAMES.with(|frames| {
        frames.borrow_mut().insert(window_id, HashMap::new());
    });
}

/// Publish all bounds together. On reader contention retain the last full frame.
pub fn finish_frame(window_id: u64) -> bool {
    if let Ok(mut map) = store().try_lock() {
        if let Some(frame) = PAINT_FRAMES.with(|frames| frames.borrow_mut().remove(&window_id)) {
            let _ = PRIMARY_WINDOW.set(window_id);
            map.insert(window_id, frame);
        }
        true
    } else {
        false
    }
}

static REGISTRY: OnceLock<Mutex<HashMap<u64, SelectorMap>>> = OnceLock::new();
static PRIMARY_WINDOW: OnceLock<u64> = OnceLock::new();

fn store() -> &'static Mutex<HashMap<u64, SelectorMap>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn record(window_id: u64, selector: &str, bounds: Bounds<Pixels>) {
    record_with_state(window_id, selector, bounds, DevElementState::default());
}

pub fn record_with_state(
    window_id: u64,
    selector: &str,
    bounds: Bounds<Pixels>,
    state: DevElementState,
) {
    let element = TrackedElement { bounds, state };
    let staged = PAINT_FRAMES.with(|frames| {
        let mut frames = frames.borrow_mut();
        if let Some(frame) = frames.get_mut(&window_id) {
            frame.insert(selector.to_string(), element.clone());
            true
        } else {
            false
        }
    });
    if staged {
        return;
    }
    // Standalone tracked elements (outside the app root) keep the existing API.
    let _ = PRIMARY_WINDOW.set(window_id);
    if let Ok(mut map) = store().lock() {
        map.entry(window_id)
            .or_default()
            .insert(selector.to_string(), element);
    }
}

/// Clear a retired window’s published and staged selectors.
pub fn clear(window_id: u64) {
    PAINT_FRAMES.with(|frames| {
        frames.borrow_mut().remove(&window_id);
    });
    let _ = PRIMARY_WINDOW.set(window_id);
    if let Ok(mut map) = store().lock() {
        map.entry(window_id).or_default().clear();
    }
}

pub fn lookup(window_id: u64, selector: &str) -> Option<Bounds<Pixels>> {
    store().lock().ok().and_then(|m| {
        m.get(&window_id)
            .and_then(|selectors| selectors.get(selector))
            .map(|element| element.bounds)
    })
}

/// Snapshot of all known selectors and their current bounds. Useful
/// for debugging missing selectors via the dev API.
pub fn snapshot() -> Vec<(String, TrackedElement)> {
    PRIMARY_WINDOW
        .get()
        .map_or_else(Vec::new, |window_id| snapshot_for(*window_id))
}

pub fn snapshot_for(window_id: u64) -> Vec<(String, TrackedElement)> {
    store()
        .lock()
        .map(|m| {
            let mut entries: Vec<_> = m
                .get(&window_id)
                .into_iter()
                .flatten()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
            entries
        })
        .unwrap_or_default()
}
