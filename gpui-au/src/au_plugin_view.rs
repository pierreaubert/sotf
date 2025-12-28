//! Audio Unit Plugin View
//!
//! Complete plugin view combining Metal rendering with EQ visualization.
//! This is the main entry point for Swift AU integration.

use crate::eq_view::{EQBand, EQView, FilterType};
use crate::renderer::Renderer2D;
use cocoa::base::{id, nil, YES};
use cocoa::foundation::{NSPoint, NSRect, NSSize};
use core_graphics::geometry::CGSize;
use metal::foreign_types::ForeignType;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel, BOOL};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::c_void;
use std::ptr;
use std::sync::Once;

const VIEW_STATE_IVAR: &str = "auPluginViewState";

static REGISTER_CLASS: Once = Once::new();
static mut AU_PLUGIN_VIEW_CLASS: *const Class = ptr::null();

/// State for the AU plugin view
pub struct AUPluginViewState {
    /// Metal device
    pub device: metal::Device,
    /// Command queue
    pub command_queue: metal::CommandQueue,
    /// 2D renderer
    pub renderer: Option<Renderer2D>,
    /// EQ visualization
    pub eq_view: EQView,
    /// Current size
    pub width: u32,
    pub height: u32,
    /// Clear color
    pub clear_color: (f64, f64, f64, f64),
    /// Dirty flag
    pub needs_redraw: bool,
    /// Parameter change callback
    pub on_parameter_change: Option<Box<dyn Fn(&[EQBand]) + Send + Sync>>,
}

impl AUPluginViewState {
    pub fn new(width: u32, height: u32) -> Option<Self> {
        let device = metal::Device::system_default()?;
        let command_queue = device.new_command_queue();

        // Create renderer
        let renderer = Renderer2D::new(device.clone(), command_queue.clone());

        Some(Self {
            device,
            command_queue,
            renderer,
            eq_view: EQView::new(),
            width,
            height,
            clear_color: (0.1, 0.1, 0.12, 1.0),
            needs_redraw: true,
            on_parameter_change: None,
        })
    }
}

/// Register the AU plugin view class
fn register_au_plugin_view_class() {
    REGISTER_CLASS.call_once(|| {
        let superclass = class!(NSView);
        let mut decl = ClassDecl::new("SOTFAUPluginView", superclass)
            .expect("Failed to create SOTFAUPluginView class");

        decl.add_ivar::<*mut c_void>(VIEW_STATE_IVAR);

        // wantsLayer
        extern "C" fn wants_layer(_this: &Object, _sel: Sel) -> BOOL {
            YES
        }
        unsafe {
            decl.add_method(
                sel!(wantsLayer),
                wants_layer as extern "C" fn(&Object, Sel) -> BOOL,
            );
        }

        // makeBackingLayer
        extern "C" fn make_backing_layer(this: &Object, _sel: Sel) -> id {
            unsafe {
                let layer: id = msg_send![class!(CAMetalLayer), new];

                let state_ptr: *mut c_void = *this.get_ivar(VIEW_STATE_IVAR);
                if !state_ptr.is_null() {
                    let state = &*(state_ptr as *const AUPluginViewState);
                    let device_ptr = state.device.as_ptr();
                    let _: () = msg_send![layer, setDevice: device_ptr];
                    let _: () = msg_send![layer, setPixelFormat: 80_u64]; // BGRA8Unorm
                    let _: () = msg_send![layer, setFramebufferOnly: YES];
                }

                layer
            }
        }
        unsafe {
            decl.add_method(
                sel!(makeBackingLayer),
                make_backing_layer as extern "C" fn(&Object, Sel) -> id,
            );
        }

        // drawRect:
        extern "C" fn draw_rect(this: &Object, _sel: Sel, _rect: NSRect) {
            unsafe {
                let state_ptr: *mut c_void = *this.get_ivar(VIEW_STATE_IVAR);
                if state_ptr.is_null() {
                    return;
                }
                let state = &mut *(state_ptr as *mut AUPluginViewState);

                let layer: id = msg_send![this, layer];
                if layer.is_null() {
                    return;
                }

                let drawable: id = msg_send![layer, nextDrawable];
                if drawable.is_null() {
                    return;
                }

                // Clear background
                let texture: id = msg_send![drawable, texture];
                let render_pass_desc: id =
                    msg_send![class!(MTLRenderPassDescriptor), renderPassDescriptor];
                let color_attachments: id = msg_send![render_pass_desc, colorAttachments];
                let color_attachment: id =
                    msg_send![color_attachments, objectAtIndexedSubscript: 0_u64];

                let _: () = msg_send![color_attachment, setTexture: texture];
                let _: () = msg_send![color_attachment, setLoadAction: 2_u64]; // Clear
                let _: () = msg_send![color_attachment, setStoreAction: 1_u64]; // Store

                let clear_color = state.clear_color;
                let _: () = msg_send![color_attachment, setClearColor: (clear_color.0, clear_color.1, clear_color.2, clear_color.3)];

                let command_buffer: id =
                    msg_send![state.command_queue.as_ptr(), commandBuffer];
                let encoder: id =
                    msg_send![command_buffer, renderCommandEncoderWithDescriptor: render_pass_desc];
                let _: () = msg_send![encoder, endEncoding];

                // Render EQ view
                if let Some(ref mut renderer) = state.renderer {
                    renderer.begin_frame();
                    state
                        .eq_view
                        .render(renderer, state.width as f32, state.height as f32);
                    renderer.render(drawable, [state.width as f32, state.height as f32]);
                }

                let _: () = msg_send![command_buffer, presentDrawable: drawable];
                let _: () = msg_send![command_buffer, commit];

                state.needs_redraw = false;
            }
        }
        unsafe {
            decl.add_method(
                sel!(drawRect:),
                draw_rect as extern "C" fn(&Object, Sel, NSRect),
            );
        }

        // Mouse events
        extern "C" fn mouse_down(this: &Object, _sel: Sel, event: id) {
            unsafe {
                let state_ptr: *mut c_void = *this.get_ivar(VIEW_STATE_IVAR);
                if state_ptr.is_null() {
                    return;
                }
                let state = &mut *(state_ptr as *mut AUPluginViewState);

                let location: NSPoint = msg_send![event, locationInWindow];
                let local: NSPoint = msg_send![this, convertPoint:location fromView:nil];

                state.eq_view.handle_mouse_down(
                    local.x as f32,
                    (state.height as f64 - local.y) as f32, // Flip Y
                    state.width as f32,
                    state.height as f32,
                );

                let _: () = msg_send![this, setNeedsDisplay: YES];
            }
        }
        unsafe {
            decl.add_method(
                sel!(mouseDown:),
                mouse_down as extern "C" fn(&Object, Sel, id),
            );
        }

        extern "C" fn mouse_dragged(this: &Object, _sel: Sel, event: id) {
            unsafe {
                let state_ptr: *mut c_void = *this.get_ivar(VIEW_STATE_IVAR);
                if state_ptr.is_null() {
                    return;
                }
                let state = &mut *(state_ptr as *mut AUPluginViewState);

                let location: NSPoint = msg_send![event, locationInWindow];
                let local: NSPoint = msg_send![this, convertPoint:location fromView:nil];

                state.eq_view.handle_mouse_drag(
                    local.x as f32,
                    (state.height as f64 - local.y) as f32,
                    state.width as f32,
                    state.height as f32,
                );

                // Notify parameter change
                if let Some(ref callback) = state.on_parameter_change {
                    callback(&state.eq_view.bands);
                }

                let _: () = msg_send![this, setNeedsDisplay: YES];
            }
        }
        unsafe {
            decl.add_method(
                sel!(mouseDragged:),
                mouse_dragged as extern "C" fn(&Object, Sel, id),
            );
        }

        extern "C" fn mouse_up(this: &Object, _sel: Sel, _event: id) {
            unsafe {
                let state_ptr: *mut c_void = *this.get_ivar(VIEW_STATE_IVAR);
                if state_ptr.is_null() {
                    return;
                }
                let state = &mut *(state_ptr as *mut AUPluginViewState);
                state.eq_view.handle_mouse_up();
            }
        }
        unsafe {
            decl.add_method(sel!(mouseUp:), mouse_up as extern "C" fn(&Object, Sel, id));
        }

        // Resize
        extern "C" fn set_frame_size(this: &mut Object, _sel: Sel, size: NSSize) {
            unsafe {
                let superclass = class!(NSView);
                let _: () = msg_send![super(this, superclass), setFrameSize: size];

                let layer: id = msg_send![this, layer];
                if !layer.is_null() {
                    let cg_size = CGSize::new(size.width, size.height);
                    let _: () = msg_send![layer, setDrawableSize: cg_size];
                }

                let state_ptr: *mut c_void = *this.get_ivar(VIEW_STATE_IVAR);
                if !state_ptr.is_null() {
                    let state = &mut *(state_ptr as *mut AUPluginViewState);
                    state.width = size.width as u32;
                    state.height = size.height as u32;
                    state.eq_view.invalidate_cache();
                }

                let _: () = msg_send![this, setNeedsDisplay: YES];
            }
        }
        unsafe {
            decl.add_method(
                sel!(setFrameSize:),
                set_frame_size as extern "C" fn(&mut Object, Sel, NSSize),
            );
        }

        unsafe {
            AU_PLUGIN_VIEW_CLASS = decl.register();
        }
    });
}

/// AU Plugin View handle
pub struct AUPluginView {
    native_view: id,
    state: *mut AUPluginViewState,
}

unsafe impl Send for AUPluginView {}
unsafe impl Sync for AUPluginView {}

impl AUPluginView {
    /// Create a new AU plugin view
    pub fn new(width: u32, height: u32) -> Option<Self> {
        register_au_plugin_view_class();

        let state = Box::into_raw(Box::new(AUPluginViewState::new(width, height)?));

        unsafe {
            let view: id = msg_send![AU_PLUGIN_VIEW_CLASS, alloc];
            let frame = NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(width as f64, height as f64),
            );
            let view: id = msg_send![view, initWithFrame: frame];

            if view.is_null() {
                let _ = Box::from_raw(state);
                return None;
            }

            (*view).set_ivar(VIEW_STATE_IVAR, state as *mut c_void);
            let _: () = msg_send![view, setWantsLayer: YES];

            Some(Self {
                native_view: view,
                state,
            })
        }
    }

    /// Get native NSView pointer
    pub fn native_view(&self) -> *mut c_void {
        self.native_view as *mut c_void
    }

    /// Request redraw
    pub fn set_needs_display(&self) {
        unsafe {
            let _: () = msg_send![self.native_view, setNeedsDisplay: YES];
        }
    }

    /// Set EQ bands from AU parameters
    pub fn set_bands(&mut self, bands: Vec<EQBand>) {
        unsafe {
            let state = &mut *self.state;
            state.eq_view.set_bands(bands);
            state.needs_redraw = true;
        }
        self.set_needs_display();
    }

    /// Get current EQ bands
    pub fn get_bands(&self) -> Vec<EQBand> {
        unsafe { (*self.state).eq_view.bands.clone() }
    }

    /// Set parameter change callback
    pub fn set_on_parameter_change<F>(&mut self, callback: F)
    where
        F: Fn(&[EQBand]) + Send + Sync + 'static,
    {
        unsafe {
            (*self.state).on_parameter_change = Some(Box::new(callback));
        }
    }
}

impl Drop for AUPluginView {
    fn drop(&mut self) {
        unsafe {
            (*self.native_view).set_ivar(VIEW_STATE_IVAR, ptr::null_mut::<c_void>());
            let _: () = msg_send![self.native_view, release];
            let _ = Box::from_raw(self.state);
        }
    }
}

// =============================================================================
// FFI Exports
// =============================================================================

/// C-compatible EQ band for FFI
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CAUEQBand {
    pub filter_type: i32, // 0=Peak, 1=LowShelf, 2=HighShelf, 3=LowPass, 4=HighPass
    pub frequency: f32,
    pub gain_db: f32,
    pub q: f32,
    pub enabled: bool,
}

impl CAUEQBand {
    fn to_rust(&self) -> EQBand {
        EQBand {
            filter_type: match self.filter_type {
                0 => FilterType::Peak,
                1 => FilterType::LowShelf,
                2 => FilterType::HighShelf,
                3 => FilterType::LowPass,
                4 => FilterType::HighPass,
                _ => FilterType::Peak,
            },
            frequency: self.frequency,
            gain_db: self.gain_db,
            q: self.q,
            enabled: self.enabled,
        }
    }

    fn from_rust(band: &EQBand) -> Self {
        Self {
            filter_type: match band.filter_type {
                FilterType::Peak => 0,
                FilterType::LowShelf => 1,
                FilterType::HighShelf => 2,
                FilterType::LowPass => 3,
                FilterType::HighPass => 4,
            },
            frequency: band.frequency,
            gain_db: band.gain_db,
            q: band.q,
            enabled: band.enabled,
        }
    }
}

/// Create AU plugin view (FFI)
#[no_mangle]
pub extern "C" fn au_plugin_view_create(width: u32, height: u32) -> *mut AUPluginView {
    let _ = env_logger::try_init();

    match AUPluginView::new(width, height) {
        Some(view) => {
            log::info!("Created AUPluginView: {}x{}", width, height);
            Box::into_raw(Box::new(view))
        }
        None => {
            log::error!("Failed to create AUPluginView");
            ptr::null_mut()
        }
    }
}

/// Destroy AU plugin view (FFI)
#[no_mangle]
pub extern "C" fn au_plugin_view_destroy(view: *mut AUPluginView) {
    if !view.is_null() {
        unsafe {
            log::info!("Destroying AUPluginView");
            drop(Box::from_raw(view));
        }
    }
}

/// Get native NSView (FFI)
#[no_mangle]
pub extern "C" fn au_plugin_view_get_native(view: *const AUPluginView) -> *mut c_void {
    if view.is_null() {
        return ptr::null_mut();
    }
    unsafe { (*view).native_view() }
}

/// Set EQ bands (FFI)
#[no_mangle]
pub extern "C" fn au_plugin_view_set_bands(
    view: *mut AUPluginView,
    bands: *const CAUEQBand,
    count: usize,
) {
    if view.is_null() || bands.is_null() {
        return;
    }

    unsafe {
        let band_slice = std::slice::from_raw_parts(bands, count);
        let rust_bands: Vec<EQBand> = band_slice.iter().map(|b| b.to_rust()).collect();
        (*view).set_bands(rust_bands);
    }
}

/// Get EQ bands (FFI)
#[no_mangle]
pub extern "C" fn au_plugin_view_get_bands(
    view: *const AUPluginView,
    bands: *mut CAUEQBand,
    max_count: usize,
) -> usize {
    if view.is_null() || bands.is_null() {
        return 0;
    }

    unsafe {
        let current_bands = (*view).get_bands();
        let copy_count = current_bands.len().min(max_count);

        for (i, band) in current_bands.iter().take(copy_count).enumerate() {
            *bands.add(i) = CAUEQBand::from_rust(band);
        }

        copy_count
    }
}

/// Request redraw (FFI)
#[no_mangle]
pub extern "C" fn au_plugin_view_set_needs_display(view: *const AUPluginView) {
    if !view.is_null() {
        unsafe {
            (*view).set_needs_display();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_class_registration() {
        register_au_plugin_view_class();
        unsafe {
            assert!(!AU_PLUGIN_VIEW_CLASS.is_null());
        }
    }
}
