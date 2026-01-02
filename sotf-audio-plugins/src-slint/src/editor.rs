//! EQ Plugin Editor
//!
//! Manages the plugin editor window lifecycle.

use crate::parameters::EqParameters;
use crate::view::EqPluginView;
use plinth_plugin::{Editor, Host};
use plugin_canvas::window::WindowAttributes;
use plugin_canvas::LogicalSize;
use plugin_canvas_slint::editor::{EditorHandle, SlintEditor};
use raw_window_handle::RawWindowHandle;
use std::rc::Rc;
use std::sync::Arc;

/// EQ Plugin Editor
pub struct EqPluginEditor {
    host: Rc<dyn Host>,
    editor_handle: Option<Rc<EditorHandle>>,
    parameters: Arc<EqParameters>,
}

impl EqPluginEditor {
    pub fn new(host: Rc<dyn Host>, parameters: Arc<EqParameters>) -> Self {
        Self {
            host,
            editor_handle: None,
            parameters,
        }
    }
}

impl Editor for EqPluginEditor {
    const DEFAULT_SIZE: (f64, f64) = (800.0, 500.0);

    fn open(&mut self, parent: RawWindowHandle) {
        // Close any existing editor
        self.close();

        let host = self.host.clone();
        let parameters = self.parameters.clone();

        let size = LogicalSize::new(Self::DEFAULT_SIZE.0, Self::DEFAULT_SIZE.1);
        let window_attributes = WindowAttributes::new(size, 1.0);

        self.editor_handle = Some(SlintEditor::open(parent, window_attributes, move |_window| {
            EqPluginView::new(host.clone(), parameters.clone())
        }));
    }

    fn close(&mut self) {
        self.editor_handle = None;
    }

    fn on_frame(&mut self) {
        if let Some(handle) = &self.editor_handle {
            handle.on_frame();
        }
    }
}
