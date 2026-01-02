//! Embedded view for Audio Unit integration
//!
//! This module provides the bridge between GPUI and Audio Unit plugins.
//! Since GPUI's Application::run() blocks, we use a hybrid approach:
//! 1. Create a Metal-backed NSView for embedding in the AU host
//! 2. Use GPUI's text system for high-quality text rendering
//! 3. Manage our own render loop synchronized with the host

use crate::metal_view::MetalView;
use gpui::{Application, TextSystem};
use parking_lot::Mutex;
use std::ffi::c_void;
use std::sync::Arc;

/// State shared between the AU view and callbacks
pub struct EmbeddedViewState {
    /// Sample rate for audio processing context
    pub sample_rate: f32,
    /// Current parameter values (e.g., EQ filters)
    pub parameters: Vec<f32>,
    /// Dirty flag for UI updates
    pub needs_redraw: bool,
}

impl Default for EmbeddedViewState {
    fn default() -> Self {
        Self {
            sample_rate: 44100.0,
            parameters: Vec::new(),
            needs_redraw: false,
        }
    }
}

/// Embedded view for Audio Unit integration
///
/// This view uses a Metal-backed NSView that can be embedded in an AU host's
/// view hierarchy. It provides high-quality rendering using Metal and
/// optionally integrates GPUI's text system for text rendering.
pub struct EmbeddedView {
    /// The underlying Metal-backed NSView
    metal_view: MetalView,
    /// Shared state for parameter sync
    state: Arc<Mutex<EmbeddedViewState>>,
    /// GPUI text system for font rendering (optional, may fail on some systems)
    text_system: Option<Arc<TextSystem>>,
}

impl EmbeddedView {
    /// Create a new embedded view for AU integration
    ///
    /// # Arguments
    /// * `width` - Initial width in pixels
    /// * `height` - Initial height in pixels
    ///
    /// Returns None if Metal is not available.
    pub fn new(width: u32, height: u32) -> Option<Self> {
        log::info!("Creating EmbeddedView: {}x{}", width, height);

        // Create the Metal-backed NSView
        let metal_view = MetalView::new(width, height)?;
        log::info!("Metal view created successfully");

        // Try to initialize GPUI's text system for high-quality text rendering
        // This may fail if the application context isn't set up properly
        let text_system = Self::try_init_text_system();

        Some(Self {
            metal_view,
            state: Arc::new(Mutex::new(EmbeddedViewState::default())),
            text_system,
        })
    }

    /// Try to initialize GPUI's text system
    ///
    /// Returns None if initialization fails (e.g., in headless environments
    /// or when called from a non-main thread)
    fn try_init_text_system() -> Option<Arc<TextSystem>> {
        // GPUI requires main thread access
        // Use catch_unwind to handle the panic gracefully
        std::panic::catch_unwind(|| {
            // Create a headless GPUI application to access the text system
            // Note: This doesn't call run(), so it won't block
            let app = Application::headless();
            let text_system = app.text_system();

            // The application will be dropped here, but the text system
            // is reference-counted and will stay alive
            log::info!("GPUI text system initialized");
            text_system
        })
        .ok()
    }

    /// Get the native NSView pointer for embedding in AU host
    ///
    /// The returned pointer is valid as long as this EmbeddedView exists.
    pub fn native_view(&self) -> *mut c_void {
        self.metal_view.native_view()
    }

    /// Request a redraw of the view
    pub fn set_needs_display(&self) {
        self.metal_view.set_needs_display();
    }

    /// Set the background color
    pub fn set_clear_color(&mut self, r: f64, g: f64, b: f64, a: f64) {
        self.metal_view.set_clear_color(r, g, b, a);
    }

    /// Get current size
    pub fn size(&self) -> (u32, u32) {
        self.metal_view.size()
    }

    /// Get access to the shared state
    pub fn state(&self) -> Arc<Mutex<EmbeddedViewState>> {
        self.state.clone()
    }

    /// Update parameters from AU host
    pub fn update_parameters(&self, params: Vec<f32>) {
        let mut state = self.state.lock();
        state.parameters = params;
        state.needs_redraw = true;
    }

    /// Check if text system is available
    pub fn has_text_system(&self) -> bool {
        self.text_system.is_some()
    }
}

// =============================================================================
// FFI Exports for Swift
// =============================================================================

/// Create a new embedded view (FFI)
#[unsafe(no_mangle)]
pub extern "C" fn embedded_view_create(width: u32, height: u32) -> *mut EmbeddedView {
    // Initialize logging if not already done
    let _ = env_logger::try_init();

    match EmbeddedView::new(width, height) {
        Some(view) => {
            log::info!("EmbeddedView created successfully");
            Box::into_raw(Box::new(view))
        }
        None => {
            log::error!("Failed to create EmbeddedView - Metal not available");
            std::ptr::null_mut()
        }
    }
}

/// Destroy an embedded view (FFI)
#[unsafe(no_mangle)]
pub extern "C" fn embedded_view_destroy(view: *mut EmbeddedView) {
    if !view.is_null() {
        unsafe {
            log::info!("Destroying EmbeddedView");
            drop(Box::from_raw(view));
        }
    }
}

/// Get the native NSView pointer (FFI)
#[unsafe(no_mangle)]
pub extern "C" fn embedded_view_get_native(view: *const EmbeddedView) -> *mut c_void {
    if view.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { (*view).native_view() }
}

/// Check if the view is available (FFI)
#[unsafe(no_mangle)]
pub extern "C" fn embedded_view_is_available(view: *const EmbeddedView) -> bool {
    if view.is_null() {
        return false;
    }
    unsafe { !(*view).native_view().is_null() }
}

/// Request a redraw (FFI)
#[unsafe(no_mangle)]
pub extern "C" fn embedded_view_set_needs_display(view: *const EmbeddedView) {
    if !view.is_null() {
        unsafe {
            (*view).set_needs_display();
        }
    }
}

/// Set clear color (FFI)
#[unsafe(no_mangle)]
pub extern "C" fn embedded_view_set_clear_color(
    view: *mut EmbeddedView,
    r: f64,
    g: f64,
    b: f64,
    a: f64,
) {
    if !view.is_null() {
        unsafe {
            (*view).set_clear_color(r, g, b, a);
        }
    }
}

/// Update parameters (FFI)
///
/// # Safety
/// `params` must point to a valid array of `count` f32 values
#[unsafe(no_mangle)]
pub extern "C" fn embedded_view_update_parameters(
    view: *mut EmbeddedView,
    params: *const f32,
    count: usize,
) {
    if view.is_null() || params.is_null() {
        return;
    }

    unsafe {
        let param_slice = std::slice::from_raw_parts(params, count);
        (*view).update_parameters(param_slice.to_vec());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_view_creation() {
        // Note: This test may fail in CI without Metal support
        // or when run on a non-main thread
        if let Some(view) = EmbeddedView::new(800, 600) {
            assert!(!view.native_view().is_null());
            assert_eq!(view.size(), (800, 600));
            // Text system may not be available if not on main thread
            // That's OK - the view still works without it
            if view.has_text_system() {
                println!("GPUI text system available");
            } else {
                println!("GPUI text system not available (expected on non-main thread)");
            }
        } else {
            println!("Metal not available - skipping test");
        }
    }
}
