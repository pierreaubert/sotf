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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
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
    pub accessibility: PlatformViewAccessibility,
}

/// Native view family requested by GPUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PlatformViewKind {
    SwiftUi,
    UiKit,
    WebView,
    Map,
    Camera,
    #[default]
    Custom,
}

impl PlatformViewKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SwiftUi => "swiftui",
            Self::UiKit => "uikit",
            Self::WebView => "webview",
            Self::Map => "map",
            Self::Camera => "camera",
            Self::Custom => "custom",
        }
    }
}

impl std::str::FromStr for PlatformViewKind {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "swiftui" | "swift_ui" | "swift-ui" => Ok(Self::SwiftUi),
            "uikit" | "ui_kit" | "ui-kit" => Ok(Self::UiKit),
            "webview" | "web_view" | "web-view" => Ok(Self::WebView),
            "map" => Ok(Self::Map),
            "camera" => Ok(Self::Camera),
            "custom" => Ok(Self::Custom),
            _ => Err(()),
        }
    }
}

/// Accessibility metadata that can be mirrored into UIAccessibility.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlatformViewAccessibility {
    pub label: Option<String>,
    pub hint: Option<String>,
    pub value: Option<String>,
    pub traits: Vec<String>,
}

impl PlatformViewAccessibility {
    pub fn named(label: impl Into<String>) -> Self {
        Self {
            label: Some(label.into()),
            hint: None,
            value: None,
            traits: Vec::new(),
        }
    }

    pub fn is_exposed(&self) -> bool {
        self.label
            .as_ref()
            .is_some_and(|label| !label.trim().is_empty())
            || self
                .hint
                .as_ref()
                .is_some_and(|hint| !hint.trim().is_empty())
            || self
                .value
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
            || !self.traits.is_empty()
    }
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
    fn kind(&self) -> PlatformViewKind {
        PlatformViewKind::Custom
    }
}

/// Renderer-facing state for a live native view.
#[derive(Debug, Clone, PartialEq)]
pub struct PlatformViewRecord {
    pub id: PlatformViewId,
    pub view_type: String,
    pub kind: PlatformViewKind,
    pub bounds: PlatformViewBounds,
    pub visible: bool,
    pub z_index: i32,
    pub accessibility: PlatformViewAccessibility,
}

impl PlatformViewRecord {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.visible
            && x >= self.bounds.x
            && x <= self.bounds.x + self.bounds.width
            && y >= self.bounds.y
            && y <= self.bounds.y + self.bounds.height
    }
}

pub struct PlatformViewRegistry {
    factories: Mutex<HashMap<String, Box<dyn PlatformViewFactory>>>,
    views: Mutex<HashMap<PlatformViewId, PlatformViewRecord>>,
}

impl PlatformViewRegistry {
    pub fn new() -> Self {
        Self {
            factories: Mutex::new(HashMap::new()),
            views: Mutex::new(HashMap::new()),
        }
    }

    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<PlatformViewRegistry> = OnceLock::new();
        INSTANCE.get_or_init(Self::new)
    }

    pub fn register_factory(&self, factory: Box<dyn PlatformViewFactory>) {
        self.factories
            .lock()
            .unwrap()
            .insert(factory.view_type().to_string(), factory);
    }

    pub fn create_view(
        &self,
        view_type: &str,
        params: &PlatformViewParams,
    ) -> Result<Box<dyn PlatformView>, String> {
        let factories = self.factories.lock().unwrap();
        let factory = factories
            .get(view_type)
            .ok_or_else(|| format!("no platform view factory registered for {view_type:?}"))?;
        let view = factory.create(params)?;
        let id = view.id();
        self.register_view(PlatformViewRecord {
            id,
            view_type: view_type.to_string(),
            kind: factory.kind(),
            bounds: params.bounds,
            visible: true,
            z_index: 0,
            accessibility: params.accessibility.clone(),
        });
        Ok(view)
    }

    pub fn register_view(&self, record: PlatformViewRecord) {
        self.views.lock().unwrap().insert(record.id, record);
    }

    pub fn update_view_bounds(&self, id: PlatformViewId, bounds: PlatformViewBounds) {
        if let Some(entry) = self.views.lock().unwrap().get_mut(&id) {
            entry.bounds = bounds;
        }
    }

    pub fn update_view_visibility(&self, id: PlatformViewId, visible: bool) {
        if let Some(entry) = self.views.lock().unwrap().get_mut(&id) {
            entry.visible = visible;
        }
    }

    pub fn update_view_z_index(&self, id: PlatformViewId, z_index: i32) {
        if let Some(entry) = self.views.lock().unwrap().get_mut(&id) {
            entry.z_index = z_index;
        }
    }

    pub fn remove_view(&self, id: PlatformViewId) {
        self.views.lock().unwrap().remove(&id);
    }

    pub fn view_snapshot(&self) -> Vec<PlatformViewRecord> {
        let mut views: Vec<_> = self.views.lock().unwrap().values().cloned().collect();
        views.sort_by_key(|view| (view.z_index, view.id.0));
        views
    }

    pub fn view_count(&self) -> usize {
        self.views.lock().unwrap().len()
    }

    pub fn hit_test(&self, x: f32, y: f32) -> bool {
        let views = self.views.lock().unwrap();
        for record in views.values() {
            if record.contains(x, y) {
                return true;
            }
        }
        false
    }
}

impl Default for PlatformViewRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct TestView {
        id: PlatformViewId,
        disposed: Mutex<bool>,
    }

    impl PlatformView for TestView {
        fn id(&self) -> PlatformViewId {
            self.id
        }

        fn view_type(&self) -> &str {
            "test"
        }

        fn set_bounds(&self, _bounds: PlatformViewBounds) {}

        fn set_visible(&self, _visible: bool) {}

        fn set_z_index(&self, _z_index: i32) {}

        fn dispose(&self) {
            *self.disposed.lock().unwrap() = true;
        }

        fn is_disposed(&self) -> bool {
            *self.disposed.lock().unwrap()
        }
    }

    struct TestFactory;

    impl PlatformViewFactory for TestFactory {
        fn create(&self, _params: &PlatformViewParams) -> Result<Box<dyn PlatformView>, String> {
            Ok(Box::new(TestView {
                id: PlatformViewId::next(),
                disposed: Mutex::new(false),
            }))
        }

        fn view_type(&self) -> &str {
            "test"
        }

        fn kind(&self) -> PlatformViewKind {
            PlatformViewKind::UiKit
        }
    }

    #[test]
    fn registry_creates_tracks_and_sorts_views() {
        let registry = PlatformViewRegistry::new();
        registry.register_factory(Box::new(TestFactory));

        let view = registry
            .create_view(
                "test",
                &PlatformViewParams {
                    bounds: PlatformViewBounds {
                        x: 10.0,
                        y: 20.0,
                        width: 30.0,
                        height: 40.0,
                    },
                    creation_params: HashMap::new(),
                    accessibility: PlatformViewAccessibility::named("Native picker"),
                },
            )
            .unwrap();

        assert_eq!(registry.view_count(), 1);
        assert!(registry.hit_test(25.0, 35.0));
        registry.update_view_visibility(view.id(), false);
        assert!(!registry.hit_test(25.0, 35.0));

        let snapshot = registry.view_snapshot();
        assert_eq!(snapshot[0].kind, PlatformViewKind::UiKit);
        assert!(snapshot[0].accessibility.is_exposed());
    }

    #[test]
    fn platform_view_kind_parses_common_spellings() {
        assert_eq!(
            "swift-ui".parse::<PlatformViewKind>(),
            Ok(PlatformViewKind::SwiftUi)
        );
        assert_eq!(PlatformViewKind::WebView.as_str(), "webview");
        assert!("unknown".parse::<PlatformViewKind>().is_err());
    }
}
