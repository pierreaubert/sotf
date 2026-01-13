//! Metal-backed NSView for Audio Unit embedding
//!
//! This module provides a Rust-native NSView subclass with CAMetalLayer backing,
//! suitable for embedding in Audio Unit view controllers.

use cocoa::base::{YES, id, nil};
use cocoa::foundation::{NSPoint, NSRect, NSSize};
use core_graphics::geometry::CGSize;
use metal::foreign_types::ForeignType;
use objc::declare::ClassDecl;
use objc::runtime::{BOOL, Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::c_void;
use std::ptr;
use std::sync::Once;

const VIEW_STATE_IVAR: &str = "rustViewState";

// Class registration happens once
static REGISTER_CLASS: Once = Once::new();
static mut METAL_VIEW_CLASS: *const Class = ptr::null();

/// State stored in the NSView's instance variable
pub struct MetalViewState {
    /// Metal device
    pub device: metal::Device,
    /// Command queue for rendering
    pub command_queue: metal::CommandQueue,
    /// Current size
    pub width: u32,
    pub height: u32,
    /// Background color (RGBA)
    pub clear_color: (f64, f64, f64, f64),
}

impl MetalViewState {
    pub fn new(width: u32, height: u32) -> Option<Self> {
        let device = metal::Device::system_default()?;
        let command_queue = device.new_command_queue();

        Some(Self {
            device,
            command_queue,
            width,
            height,
            clear_color: (0.1, 0.1, 0.12, 1.0), // Dark theme background
        })
    }
}

/// Register the custom NSView subclass
fn register_metal_view_class() {
    REGISTER_CLASS.call_once(|| {
        let superclass = class!(NSView);
        let mut decl = ClassDecl::new("SOTFMetalView", superclass)
            .expect("Failed to create SOTFMetalView class");

        // Add instance variable to store Rust state
        decl.add_ivar::<*mut c_void>(VIEW_STATE_IVAR);

        // Override wantsLayer to return YES (layer-backed view)
        extern "C" fn wants_layer(_this: &Object, _sel: Sel) -> BOOL {
            YES
        }
        unsafe {
            decl.add_method(sel!(wantsLayer), wants_layer as extern "C" fn(&Object, Sel) -> BOOL);
        }

        // Override makeBackingLayer to return CAMetalLayer
        extern "C" fn make_backing_layer(this: &Object, _sel: Sel) -> id {
            unsafe {
                let layer: id = msg_send![class!(CAMetalLayer), new];

                // Get the Rust state to access the Metal device
                let state_ptr: *mut c_void = *this.get_ivar(VIEW_STATE_IVAR);
                if !state_ptr.is_null() {
                    let state = &*(state_ptr as *const MetalViewState);
                    let device_ptr = state.device.as_ptr();
                    let _: () = msg_send![layer, setDevice: device_ptr];
                    let _: () = msg_send![layer, setPixelFormat: 80_u64]; // MTLPixelFormatBGRA8Unorm
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

        // Override drawRect: to trigger Metal rendering
        extern "C" fn draw_rect(this: &Object, _sel: Sel, _rect: NSRect) {
            unsafe {
                let state_ptr: *mut c_void = *this.get_ivar(VIEW_STATE_IVAR);
                if state_ptr.is_null() {
                    return;
                }
                let state = &*(state_ptr as *const MetalViewState);

                // Get the CAMetalLayer
                let layer: id = msg_send![this, layer];
                if layer.is_null() {
                    return;
                }

                // Get next drawable
                let drawable: id = msg_send![layer, nextDrawable];
                if drawable.is_null() {
                    return;
                }

                // Create render pass descriptor
                let texture: id = msg_send![drawable, texture];
                let render_pass_desc: id = msg_send![class!(MTLRenderPassDescriptor), renderPassDescriptor];
                let color_attachments: id = msg_send![render_pass_desc, colorAttachments];
                let color_attachment: id = msg_send![color_attachments, objectAtIndexedSubscript: 0_u64];

                let _: () = msg_send![color_attachment, setTexture: texture];
                let _: () = msg_send![color_attachment, setLoadAction: 2_u64]; // MTLLoadActionClear
                let _: () = msg_send![color_attachment, setStoreAction: 1_u64]; // MTLStoreActionStore

                // Set clear color
                let clear_color = state.clear_color;
                let _: () = msg_send![color_attachment, setClearColor: (clear_color.0, clear_color.1, clear_color.2, clear_color.3)];

                // Create command buffer and render encoder
                let command_buffer: id = msg_send![state.command_queue.as_ptr(), commandBuffer];
                let encoder: id = msg_send![command_buffer, renderCommandEncoderWithDescriptor: render_pass_desc];

                // End encoding and present
                let _: () = msg_send![encoder, endEncoding];
                let _: () = msg_send![command_buffer, presentDrawable: drawable];
                let _: () = msg_send![command_buffer, commit];
            }
        }
        unsafe {
            decl.add_method(
                sel!(drawRect:),
                draw_rect as extern "C" fn(&Object, Sel, NSRect),
            );
        }

        // Mouse event handlers
        extern "C" fn mouse_down(this: &Object, _sel: Sel, event: id) {
            unsafe {
                let location: NSPoint = msg_send![event, locationInWindow];
                let local: NSPoint = msg_send![this, convertPoint:location fromView:nil];
                log::debug!("Mouse down at ({}, {})", local.x, local.y);
                // Forward to Rust state if needed
            }
        }
        unsafe {
            decl.add_method(
                sel!(mouseDown:),
                mouse_down as extern "C" fn(&Object, Sel, id),
            );
        }

        extern "C" fn mouse_up(this: &Object, _sel: Sel, event: id) {
            unsafe {
                let location: NSPoint = msg_send![event, locationInWindow];
                let local: NSPoint = msg_send![this, convertPoint:location fromView:nil];
                log::debug!("Mouse up at ({}, {})", local.x, local.y);
            }
        }
        unsafe {
            decl.add_method(sel!(mouseUp:), mouse_up as extern "C" fn(&Object, Sel, id));
        }

        extern "C" fn mouse_dragged(this: &Object, _sel: Sel, event: id) {
            unsafe {
                let location: NSPoint = msg_send![event, locationInWindow];
                let local: NSPoint = msg_send![this, convertPoint:location fromView:nil];
                log::debug!("Mouse dragged at ({}, {})", local.x, local.y);
            }
        }
        unsafe {
            decl.add_method(
                sel!(mouseDragged:),
                mouse_dragged as extern "C" fn(&Object, Sel, id),
            );
        }

        // Resize handling
        extern "C" fn set_frame_size(this: &mut Object, _sel: Sel, size: NSSize) {
            unsafe {
                // Call super
                let superclass = class!(NSView);
                let _: () = msg_send![super(this, superclass), setFrameSize: size];

                // Update layer size
                let layer: id = msg_send![this, layer];
                if !layer.is_null() {
                    let cg_size = CGSize::new(size.width, size.height);
                    let _: () = msg_send![layer, setDrawableSize: cg_size];
                }

                // Update Rust state
                let state_ptr: *mut c_void = *this.get_ivar(VIEW_STATE_IVAR);
                if !state_ptr.is_null() {
                    let state = &mut *(state_ptr as *mut MetalViewState);
                    state.width = size.width as u32;
                    state.height = size.height as u32;
                }

                // Request redraw
                let _: () = msg_send![this, setNeedsDisplay: YES];
            }
        }
        unsafe {
            decl.add_method(
                sel!(setFrameSize:),
                set_frame_size as extern "C" fn(&mut Object, Sel, NSSize),
            );
        }

        // Register the class
        unsafe {
            METAL_VIEW_CLASS = decl.register();
        }
    });
}

/// Opaque handle to a Metal-backed NSView
pub struct MetalView {
    /// The native NSView pointer
    native_view: id,
    /// Rust state (owned, freed on drop)
    state: *mut MetalViewState,
}

// Safety: MetalView is only accessed from main thread (Cocoa requirement)
unsafe impl Send for MetalView {}
unsafe impl Sync for MetalView {}

impl MetalView {
    /// Create a new Metal-backed NSView
    ///
    /// Returns None if Metal device is not available.
    pub fn new(width: u32, height: u32) -> Option<Self> {
        // Ensure class is registered
        register_metal_view_class();

        // Create Rust state
        let state = Box::into_raw(Box::new(MetalViewState::new(width, height)?));

        unsafe {
            // Create NSView instance
            let view: id = msg_send![METAL_VIEW_CLASS, alloc];
            let frame = NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(width as f64, height as f64),
            );
            let view: id = msg_send![view, initWithFrame: frame];

            if view.is_null() {
                // Clean up state if view creation failed
                let _ = Box::from_raw(state);
                return None;
            }

            // Store Rust state in ivar
            (*view).set_ivar(VIEW_STATE_IVAR, state as *mut c_void);

            // Trigger layer creation
            let _: () = msg_send![view, setWantsLayer: YES];

            Some(Self {
                native_view: view,
                state,
            })
        }
    }

    /// Get the native NSView pointer for embedding
    ///
    /// The returned pointer is valid as long as this MetalView exists.
    pub fn native_view(&self) -> *mut c_void {
        self.native_view as *mut c_void
    }

    /// Request a redraw
    pub fn set_needs_display(&self) {
        unsafe {
            let _: () = msg_send![self.native_view, setNeedsDisplay: YES];
        }
    }

    /// Set the background clear color
    pub fn set_clear_color(&mut self, r: f64, g: f64, b: f64, a: f64) {
        unsafe {
            let state = &mut *self.state;
            state.clear_color = (r, g, b, a);
        }
        self.set_needs_display();
    }

    /// Get current size
    pub fn size(&self) -> (u32, u32) {
        unsafe {
            let state = &*self.state;
            (state.width, state.height)
        }
    }
}

impl Drop for MetalView {
    fn drop(&mut self) {
        unsafe {
            // Clear the ivar before releasing
            (*self.native_view).set_ivar(VIEW_STATE_IVAR, ptr::null_mut::<c_void>());

            // Release the view
            let _: () = msg_send![self.native_view, release];

            // Free the Rust state
            let _ = Box::from_raw(self.state);
        }
    }
}

// =============================================================================
// FFI Exports for Swift
// =============================================================================

/// Create a new Metal-backed view (FFI)
#[unsafe(no_mangle)]
pub extern "C" fn metal_view_create(width: u32, height: u32) -> *mut MetalView {
    match MetalView::new(width, height) {
        Some(view) => Box::into_raw(Box::new(view)),
        None => {
            log::error!("Failed to create MetalView - Metal not available");
            ptr::null_mut()
        }
    }
}

/// Destroy a Metal-backed view (FFI)
#[unsafe(no_mangle)]
pub extern "C" fn metal_view_destroy(view: *mut MetalView) {
    if !view.is_null() {
        unsafe {
            let _ = Box::from_raw(view);
        }
    }
}

/// Get the native NSView pointer (FFI)
#[unsafe(no_mangle)]
pub extern "C" fn metal_view_get_native(view: *const MetalView) -> *mut c_void {
    if view.is_null() {
        return ptr::null_mut();
    }
    unsafe { (*view).native_view() }
}

/// Request a redraw (FFI)
#[unsafe(no_mangle)]
pub extern "C" fn metal_view_set_needs_display(view: *const MetalView) {
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
        register_metal_view_class();
        unsafe {
            assert!(!METAL_VIEW_CLASS.is_null());
        }
    }
}
