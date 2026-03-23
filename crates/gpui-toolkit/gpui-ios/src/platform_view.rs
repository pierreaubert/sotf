//! Platform Views — embedding native views in the GPUI render tree.
//! Stripped-down version for initial iOS experiment (no video/camera/webview).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlatformViewId(pub u64);

impl PlatformViewId {
    pub fn next() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl std::fmt::Display for PlatformViewId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PlatformView({})", self.0)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PlatformViewBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Default)]
pub struct PlatformViewParams {
    pub bounds: PlatformViewBounds,
    pub creation_params: HashMap<String, String>,
}

pub trait PlatformView: Send + Sync {
    fn id(&self) -> PlatformViewId;
    fn view_type(&self) -> &str;
    fn set_bounds(&self, bounds: PlatformViewBounds);
    fn set_visible(&self, visible: bool);
    fn set_z_index(&self, z_index: i32);
    fn dispose(&self);
    fn is_disposed(&self) -> bool;
}

pub trait PlatformViewFactory: Send + Sync {
    fn create(&self, params: &PlatformViewParams) -> Result<Box<dyn PlatformView>, String>;
    fn view_type(&self) -> &str;
}

pub struct PlatformViewRegistry {
    #[allow(dead_code)]
    factories: Mutex<HashMap<String, Box<dyn PlatformViewFactory>>>,
    views: Mutex<HashMap<PlatformViewId, PlatformViewBounds>>,
}

impl PlatformViewRegistry {
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<PlatformViewRegistry> = OnceLock::new();
        INSTANCE.get_or_init(|| Self {
            factories: Mutex::new(HashMap::new()),
            views: Mutex::new(HashMap::new()),
        })
    }

    pub fn update_view_bounds(&self, id: PlatformViewId, bounds: PlatformViewBounds) {
        if let Some(entry) = self.views.lock().unwrap().get_mut(&id) {
            *entry = bounds;
        }
    }

    pub fn remove_view(&self, id: PlatformViewId) {
        self.views.lock().unwrap().remove(&id);
    }

    pub fn hit_test(&self, x: f32, y: f32) -> bool {
        let views = self.views.lock().unwrap();
        for bounds in views.values() {
            if x >= bounds.x
                && x <= bounds.x + bounds.width
                && y >= bounds.y
                && y <= bounds.y + bounds.height
            {
                return true;
            }
        }
        false
    }
}
