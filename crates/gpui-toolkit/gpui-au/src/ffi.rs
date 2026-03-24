//! C-compatible FFI functions for embedding GPUI in macOS Audio Unit ViewControllers.

use crate::window::{AuWindowPtr, PendingViewInfo, PENDING_VIEW, AU_WINDOW};
use gpui::{point, px, MouseButton, PlatformInput, RequestFrameOptions};
use objc::runtime::Object;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::Once;

/// Helper to log via NSLog (always visible in Console.app, unlike Rust's log crate).
/// The message must be a null-terminated string.
fn nslog(msg: &str) {
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let ns_string: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: msg.as_ptr()];
        #[link(name = "Foundation", kind = "framework")]
        unsafe extern "C" {
            fn NSLog(format: *mut Object, ...);
        }
        let fmt: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: c"%@".as_ptr()];
        NSLog(fmt, ns_string);
    }
}

static INIT_LOGGER: Once = Once::new();

fn init_logger() {
    INIT_LOGGER.call_once(|| {
        let _ = env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .try_init();
    });
}

/// Opaque context handle passed to/from Swift.
pub struct AuContext {
    _plugin_type: String,
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

/// Create a GPUI context embedded in an NSView.
///
/// # Safety
/// `ns_view` must be a valid NSView pointer. `plugin_type` must be a valid C string.
#[unsafe(no_mangle)]
pub extern "C" fn gpui_au_create(
    ns_view: *mut Object,
    width: f32,
    height: f32,
    scale: f32,
    plugin_type: *const c_char,
) -> *mut AuContext {
    init_logger();
    nslog("SOTF gpui_au_create: entry\0");

    if ns_view.is_null() || plugin_type.is_null() {
        nslog("SOTF gpui_au_create: null pointer argument!\0");
        return std::ptr::null_mut();
    }

    let plugin_type_str = unsafe {
        match CStr::from_ptr(plugin_type).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                nslog("SOTF gpui_au_create: invalid UTF-8\0");
                return std::ptr::null_mut();
            }
        }
    };

    let msg = format!(
        "SOTF gpui_au_create: plugin={}, size={}x{} @{:.1}x, view={:p}\0",
        plugin_type_str, width, height, scale, ns_view
    );
    nslog(&msg);

    // Store the NSView info in a thread-local so AuWindow::new() can read it
    PENDING_VIEW.with(|pv| {
        *pv.borrow_mut() = Some(PendingViewInfo {
            ns_view,
            width,
            height,
            scale,
        });
    });

    // TODO: Initialize GPUI Application with Metal rendering here.
    // For now, just verify the FFI pipeline works end-to-end.
    nslog("SOTF gpui_au_create: FFI working, GPUI init deferred\0");

    let context = Box::new(AuContext {
        _plugin_type: plugin_type_str,
    });
    Box::into_raw(context)
}

/// Destroy a GPUI AU context.
#[unsafe(no_mangle)]
pub extern "C" fn gpui_au_destroy(context: *mut AuContext) {
    if !context.is_null() {
        log::info!("gpui_au_destroy");
        unsafe {
            drop(Box::from_raw(context));
        }
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Request one frame of GPUI rendering.
/// Call from a timer/CVDisplayLink callback on the main thread.
#[unsafe(no_mangle)]
pub extern "C" fn gpui_au_request_frame(context: *mut AuContext) {
    if context.is_null() {
        return;
    }
    if let Some(AuWindowPtr(window_ptr)) = AU_WINDOW.get().filter(|p| !p.0.is_null()) {
        let window = unsafe { &**window_ptr };
        let cb = window.request_frame_callback.borrow_mut().take();
        if let Some(mut cb) = cb {
            cb(RequestFrameOptions::default());
            window.request_frame_callback.borrow_mut().replace(cb);
        }
    }
}

/// Handle view resize from the host.
#[unsafe(no_mangle)]
pub extern "C" fn gpui_au_resize(context: *mut AuContext, width: f32, height: f32, scale: f32) {
    if context.is_null() {
        return;
    }
    if let Some(AuWindowPtr(window_ptr)) = AU_WINDOW.get().filter(|p| !p.0.is_null()) {
        let window = unsafe { &**window_ptr };
        window.handle_resize(width, height, scale);
    }
}

// ── Mouse Events ──────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn gpui_au_mouse_down(
    context: *mut AuContext,
    x: f32,
    y: f32,
    button: i32,
    click_count: i32,
) {
    if context.is_null() {
        return;
    }
    let mouse_button = match button {
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        _ => MouseButton::Left,
    };
    dispatch_to_window(PlatformInput::MouseDown(gpui::MouseDownEvent {
        button: mouse_button,
        position: point(px(x), px(y)),
        modifiers: gpui::Modifiers::default(),
        click_count: click_count as usize,
        first_mouse: false,
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_au_mouse_up(context: *mut AuContext, x: f32, y: f32, button: i32) {
    if context.is_null() {
        return;
    }
    let mouse_button = match button {
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        _ => MouseButton::Left,
    };
    dispatch_to_window(PlatformInput::MouseUp(gpui::MouseUpEvent {
        button: mouse_button,
        position: point(px(x), px(y)),
        modifiers: gpui::Modifiers::default(),
        click_count: 1,
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_au_mouse_moved(context: *mut AuContext, x: f32, y: f32) {
    if context.is_null() {
        return;
    }
    dispatch_to_window(PlatformInput::MouseMove(gpui::MouseMoveEvent {
        position: point(px(x), px(y)),
        pressed_button: None,
        modifiers: gpui::Modifiers::default(),
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_au_mouse_dragged(context: *mut AuContext, x: f32, y: f32, button: i32) {
    if context.is_null() {
        return;
    }
    let mouse_button = match button {
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        _ => MouseButton::Left,
    };
    dispatch_to_window(PlatformInput::MouseMove(gpui::MouseMoveEvent {
        position: point(px(x), px(y)),
        pressed_button: Some(mouse_button),
        modifiers: gpui::Modifiers::default(),
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_au_scroll_wheel(
    context: *mut AuContext,
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
) {
    if context.is_null() {
        return;
    }
    dispatch_to_window(PlatformInput::ScrollWheel(gpui::ScrollWheelEvent {
        position: point(px(x), px(y)),
        delta: gpui::ScrollDelta::Pixels(point(px(dx), px(dy))),
        modifiers: gpui::Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    }));
}

fn dispatch_to_window(event: PlatformInput) {
    if let Some(AuWindowPtr(window_ptr)) = AU_WINDOW.get().filter(|p| !p.0.is_null()) {
        let window = unsafe { &**window_ptr };
        window.dispatch_input(event);
    }
}

