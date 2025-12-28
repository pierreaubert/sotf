//! GPUI Audio Unit Integration
//!
//! This crate provides a bridge between GPUI UI framework and Audio Unit plugins.
//!
//! ## Architecture
//!
//! Following the JUCE pattern for embedding UI frameworks:
//! 1. Initialize GPUI Application without calling `run()`
//! 2. Open a GPUI window to create the NSView and Metal renderer
//! 3. Extract the native NSView from the window
//! 4. Embed NSView in AU view controller
//! 5. Forward events from AU host to GPUI
//!
//! ## Key Insight
//!
//! GPUI's rendering is Metal-layer based and doesn't depend on owning the event loop.
//! The NSView created by GPUI is self-contained and can be embedded in any view hierarchy.

// FFI functions take raw pointers from C but are not marked unsafe because
// they handle null checks internally. This is the standard pattern for C FFI.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

// Metal-backed NSView module for direct AU embedding
mod metal_view;
pub use metal_view::{MetalView, MetalViewState};

// Hybrid embedded view combining Metal NSView with GPUI text system
mod embedded_view;
pub use embedded_view::{EmbeddedView, EmbeddedViewState};

use std::borrow::Cow;
use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::Arc;

use gpui::*;
// Objc imports for future native view extraction
#[allow(unused_imports)]
use objc::{msg_send, sel, sel_impl};
use parking_lot::Mutex;
use rust_embed::RustEmbed;

// Re-export types for FFI
use autoeq_iir::BiquadFilterType;
use gpui_ui_kit::Theme;
pub use sotf_audio_player::EQFilter;

/// Embedded assets for the AU plugin UI
/// Currently unused - prepared for when GPUI window creation works
#[allow(dead_code)]
#[derive(RustEmbed)]
#[folder = "../sotf-audio-player/assets"]
#[include = "icons/*.svg"]
struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::get(path).map(|f| f.data))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter(|p| p.starts_with(path))
            .map(SharedString::from)
            .collect())
    }
}

// =============================================================================
// EQ View Component
// =============================================================================

/// Root view for the Audio Unit EQ UI
/// Currently unused - prepared for when GPUI window creation works
#[allow(dead_code)]
struct AUEqView {
    filters: Arc<Mutex<Vec<EQFilter>>>,
    selected_band: usize,
    is_editing: bool,
}

#[allow(dead_code)]
impl AUEqView {
    fn new(filters: Arc<Mutex<Vec<EQFilter>>>) -> Self {
        Self {
            filters,
            selected_band: 0,
            is_editing: false,
        }
    }
}

impl Render for AUEqView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::dark();
        let filters = self.filters.lock().clone();

        // Build a simple EQ visualization
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .p_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_primary)
                    .child("SOTF Parametric EQ"),
            )
            .child(
                div()
                    .mt_2()
                    .text_sm()
                    .text_color(theme.text_muted)
                    .child(format!("{} bands active", filters.len())),
            )
            .child(
                // EQ Graph placeholder
                div()
                    .mt_4()
                    .flex_1()
                    .rounded_lg()
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.muted)
                    .child(self.render_eq_graph(&filters, &theme)),
            )
            .child(
                // Band controls
                div().mt_4().flex().gap_2().children(
                    filters
                        .iter()
                        .enumerate()
                        .map(|(i, filter)| self.render_band_control(i, filter, &theme)),
                ),
            )
    }
}

impl AUEqView {
    fn render_eq_graph(&self, filters: &[EQFilter], theme: &Theme) -> impl IntoElement {
        // Simple text-based representation for now
        // TODO: Use gpui_ui_kit's EQ visualization when working
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .child(
                div()
                    .text_color(theme.text_muted)
                    .child("EQ Response Graph"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .mt_2()
                    .child(format!(
                        "Bands: {}",
                        filters
                            .iter()
                            .map(|f| format!("{:.0}Hz", f.frequency))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
            )
    }

    fn render_band_control(
        &self,
        idx: usize,
        filter: &EQFilter,
        theme: &Theme,
    ) -> impl IntoElement {
        let is_selected = idx == self.selected_band && self.is_editing;

        div()
            .flex()
            .flex_col()
            .items_center()
            .p_2()
            .rounded_md()
            .bg(if is_selected {
                theme.accent_muted
            } else {
                theme.surface
            })
            .border_1()
            .border_color(if is_selected {
                theme.accent
            } else {
                theme.border
            })
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_primary)
                    .child(format!("Band {}", idx + 1)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .child(format!("{:.0} Hz", filter.frequency)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(if filter.gain_db >= 0.0 {
                        theme.success
                    } else {
                        theme.error
                    })
                    .child(format!("{:+.1} dB", filter.gain_db)),
            )
    }
}

// =============================================================================
// GPUI Embedded View Manager
// =============================================================================

/// GPUI embedded view for Audio Unit integration
///
/// This struct manages a GPUI Application and Window, providing methods
/// to extract the native NSView for embedding in an AU view controller.
pub struct GPUIEmbeddedView {
    /// Current filter state (shared with the view)
    filters: Arc<Mutex<Vec<EQFilter>>>,
    /// The GPUI Application instance (kept alive but not run)
    /// We store it as a raw pointer because Application doesn't implement Send
    /// and we need to be careful about its lifetime
    _app_cell: Option<Rc<RefCell<AppHolder>>>,
    /// Native view pointer (NSView*)
    native_view: *mut c_void,
    /// View dimensions
    width: u32,
    height: u32,
}

/// Wrapper to hold application state
struct AppHolder {
    // The Application is stored here but we don't call run() on it
    // Instead we create windows and extract their views
}

// Safety: GPUIEmbeddedView is only accessed from the main thread (AU requirement)
unsafe impl Send for GPUIEmbeddedView {}
unsafe impl Sync for GPUIEmbeddedView {}

impl GPUIEmbeddedView {
    /// Create a new GPUI embedded view
    ///
    /// # Arguments
    /// * `width` - Initial width in pixels
    /// * `height` - Initial height in pixels
    ///
    /// # Note
    /// This must be called on the main thread.
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        log::info!("Creating GPUI embedded view: {}x{}", width, height);

        let filters = Arc::new(Mutex::new(Vec::new()));

        // Try to initialize GPUI
        // NOTE: This is experimental - GPUI may not support this use case fully
        let native_view = Self::try_create_gpui_view(width, height, filters.clone());

        Ok(Self {
            filters,
            _app_cell: None,
            native_view,
            width,
            height,
        })
    }

    /// Attempt to create a GPUI view without running the application
    ///
    /// Returns the native NSView* pointer, or null if initialization fails.
    fn try_create_gpui_view(
        _width: u32,
        _height: u32,
        _filters: Arc<Mutex<Vec<EQFilter>>>,
    ) -> *mut c_void {
        // IMPORTANT: This is the key integration point
        //
        // The challenge is that GPUI's Application::new() and Application::run()
        // are designed to own the main thread. We need to:
        //
        // 1. Create an Application
        // 2. Open a window
        // 3. Extract the NSView without calling run()
        //
        // This requires careful handling because:
        // - Application::run() takes ownership and never returns
        // - The window's NSView is tied to GPUI's rendering system
        //
        // For now, we return null and fall back to the placeholder UI.
        // A full implementation would require either:
        // a) Forking GPUI to add a headless/embedded mode
        // b) Using GPUI's test infrastructure which has more control
        // c) Investigating if the Application can be kept alive without run()

        log::warn!("GPUI embedded view creation is experimental - returning null for now");
        log::info!("The AU will use a placeholder UI until GPUI embedding is fully implemented");

        // Attempt to get the native view if we have GPUI running
        // This would work if we could somehow keep the Application alive
        std::ptr::null_mut()
    }

    /// Extract the NSView* from GPUI's window
    ///
    /// # Safety
    /// The returned pointer is valid as long as the GPUIEmbeddedView exists.
    pub fn get_native_view(&self) -> *mut c_void {
        if self.native_view.is_null() {
            log::debug!("GPUI view not initialized - returning null");
        }
        self.native_view
    }

    /// Update UI with new filter parameters
    ///
    /// Called from Swift when AU parameters change (e.g., from host automation).
    pub fn update_filters(&mut self, new_filters: Vec<EQFilter>) {
        log::debug!("Updating filters: {} bands", new_filters.len());
        *self.filters.lock() = new_filters;

        // If we had a working GPUI window, we would notify it here
        // For now, the state is just stored in the Arc<Mutex<>>
    }

    /// Handle resize events from AU host
    pub fn set_size(&mut self, width: u32, height: u32) {
        log::debug!("Resizing view: {}x{}", width, height);
        self.width = width;
        self.height = height;

        // If we had a working GPUI window, we would resize it here
    }

    /// Handle mouse events from NSView
    ///
    /// # Arguments
    /// * `x`, `y` - Mouse position in view coordinates
    /// * `event_type` - 0=down, 1=drag, 2=up
    pub fn mouse_event(&mut self, x: f32, y: f32, event_type: i32) {
        log::debug!("Mouse event: ({}, {}) type={}", x, y, event_type);
        // If we had a working GPUI window, we would forward events here
    }

    /// Get current filter state (for bidirectional sync)
    pub fn get_filters(&self) -> Vec<EQFilter> {
        self.filters.lock().clone()
    }
}

impl Drop for GPUIEmbeddedView {
    fn drop(&mut self) {
        log::info!("Dropping GPUI embedded view");
        // Cleanup would happen here if we had a running GPUI instance
    }
}

// =============================================================================
// FFI Exports (C-compatible interface for Swift/Objective-C)
// =============================================================================

/// Create a new GPUI embedded view (FFI)
#[no_mangle]
pub extern "C" fn gpui_view_create(width: u32, height: u32) -> *mut GPUIEmbeddedView {
    // Initialize logging if not already done
    let _ = env_logger::try_init();

    match GPUIEmbeddedView::new(width, height) {
        Ok(view) => {
            log::info!("Successfully created GPUI view wrapper");
            Box::into_raw(Box::new(view))
        }
        Err(e) => {
            log::error!("Failed to create GPUI view: {}", e);
            std::ptr::null_mut()
        }
    }
}

/// Destroy a GPUI embedded view (FFI)
#[no_mangle]
pub extern "C" fn gpui_view_destroy(view: *mut GPUIEmbeddedView) {
    if !view.is_null() {
        unsafe {
            log::info!("Destroying GPUI view");
            drop(Box::from_raw(view));
        }
    }
}

/// Get the native NSView* from GPUI window (FFI)
///
/// Returns the NSView pointer that can be embedded in the AU view controller.
/// Returns NULL if GPUI initialization failed (fallback to placeholder UI).
#[no_mangle]
pub extern "C" fn gpui_view_get_native_view(view: *mut GPUIEmbeddedView) -> *mut c_void {
    if view.is_null() {
        log::error!("gpui_view_get_native_view: null view pointer");
        return std::ptr::null_mut();
    }
    unsafe { (*view).get_native_view() }
}

/// Check if GPUI view is available (FFI)
///
/// Returns true if the GPUI view was successfully created and has a native view.
/// If false, the AU should use its fallback placeholder UI.
#[no_mangle]
pub extern "C" fn gpui_view_is_available(view: *mut GPUIEmbeddedView) -> bool {
    if view.is_null() {
        return false;
    }
    unsafe { !(*view).get_native_view().is_null() }
}

/// Update view size (FFI)
#[no_mangle]
pub extern "C" fn gpui_view_set_size(view: *mut GPUIEmbeddedView, width: u32, height: u32) {
    if !view.is_null() {
        unsafe {
            (*view).set_size(width, height);
        }
    }
}

/// Update filter parameters (FFI)
///
/// # Safety
/// `filters` must point to a valid array of `count` elements
#[no_mangle]
pub extern "C" fn gpui_view_set_filters(
    view: *mut GPUIEmbeddedView,
    filters: *const CEQFilter,
    count: usize,
) {
    if view.is_null() || filters.is_null() {
        log::error!("gpui_view_set_filters: null pointer");
        return;
    }

    unsafe {
        let filter_slice = std::slice::from_raw_parts(filters, count);
        let rust_filters: Vec<EQFilter> = filter_slice.iter().map(|f| f.to_rust()).collect();

        (*view).update_filters(rust_filters);
    }
}

/// Get filter parameters (FFI)
///
/// Copies current filter state to the provided buffer.
/// Returns the number of filters copied.
///
/// # Safety
/// `filters` must point to a buffer with at least `max_count` elements
#[no_mangle]
pub extern "C" fn gpui_view_get_filters(
    view: *mut GPUIEmbeddedView,
    filters: *mut CEQFilter,
    max_count: usize,
) -> usize {
    if view.is_null() || filters.is_null() {
        return 0;
    }

    unsafe {
        let current_filters = (*view).get_filters();
        let copy_count = current_filters.len().min(max_count);

        for (i, filter) in current_filters.iter().take(copy_count).enumerate() {
            *filters.add(i) = CEQFilter::from_rust(filter);
        }

        copy_count
    }
}

/// Handle mouse events (FFI)
#[no_mangle]
pub extern "C" fn gpui_view_mouse_event(
    view: *mut GPUIEmbeddedView,
    x: f32,
    y: f32,
    event_type: i32,
) {
    if !view.is_null() {
        unsafe {
            (*view).mouse_event(x, y, event_type);
        }
    }
}

// =============================================================================
// C-compatible types for FFI
// =============================================================================

/// C-compatible EQ filter representation
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CEQFilter {
    pub frequency: f64,
    pub q: f64,
    pub gain_db: f64,
    pub filter_type: i32, // 0=Peak, 1=LowShelf, 2=HighShelf, 3=Lowpass, 4=Highpass
}

impl CEQFilter {
    fn to_rust(&self) -> EQFilter {
        let filter_type = match self.filter_type {
            0 => BiquadFilterType::Peak,
            1 => BiquadFilterType::Lowshelf,
            2 => BiquadFilterType::Highshelf,
            3 => BiquadFilterType::Lowpass,
            4 => BiquadFilterType::Highpass,
            _ => BiquadFilterType::Peak, // Default to Peak if unknown
        };

        EQFilter::new(filter_type, self.frequency, self.q, self.gain_db)
    }

    fn from_rust(filter: &EQFilter) -> Self {
        let filter_type = match filter.filter_type {
            BiquadFilterType::Peak => 0,
            BiquadFilterType::Lowshelf => 1,
            BiquadFilterType::Highshelf => 2,
            BiquadFilterType::Lowpass => 3,
            BiquadFilterType::Highpass => 4,
            _ => 0, // Default to Peak
        };

        CEQFilter {
            frequency: filter.frequency,
            q: filter.q,
            gain_db: filter.gain_db,
            filter_type,
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

/* crash

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ceqfilter_roundtrip() {
        let original = CEQFilter {
            frequency: 1000.0,
            q: 1.5,
            gain_db: 3.0,
            filter_type: 0, // Peak
        };

        let rust = original.to_rust();
        let back = CEQFilter::from_rust(&rust);

        assert!((original.frequency - back.frequency).abs() < 0.001);
        assert!((original.q - back.q).abs() < 0.001);
        assert!((original.gain_db - back.gain_db).abs() < 0.001);
        assert_eq!(original.filter_type, back.filter_type);
    }

    #[test]
    fn test_filter_type_conversion() {
        for (c_type, expected) in [
            (0, BiquadFilterType::Peak),
            (1, BiquadFilterType::Lowshelf),
            (2, BiquadFilterType::Highshelf),
            (3, BiquadFilterType::Lowpass),
            (4, BiquadFilterType::Highpass),
        ] {
            let filter = CEQFilter {
                frequency: 1000.0,
                q: 1.0,
                gain_db: 0.0,
                filter_type: c_type,
            };
            assert_eq!(filter.to_rust().filter_type, expected);
        }
    }
}

*/
