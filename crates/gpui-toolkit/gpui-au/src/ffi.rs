//! C-compatible FFI functions for embedding GPUI in macOS Audio Unit ViewControllers.

use crate::window::{AuWindowPtr, PendingViewInfo, PENDING_VIEW, AU_WINDOW};
use crate::AuPlatform;
use gpui::{
    div, point, px, rgb, size, App, AppContext, IntoElement, MouseButton, ParentElement,
    PlatformInput, Render, RequestFrameOptions, Styled, WindowOptions,
};
use objc::runtime::Object;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::rc::Rc;

/// Opaque context handle passed to/from Swift.
struct AuContext {
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
    if ns_view.is_null() || plugin_type.is_null() {
        log::error!("gpui_au_create: null pointer argument");
        return std::ptr::null_mut();
    }

    let plugin_type_str = unsafe {
        match CStr::from_ptr(plugin_type).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                log::error!("gpui_au_create: invalid UTF-8 in plugin_type");
                return std::ptr::null_mut();
            }
        }
    };

    log::info!(
        "gpui_au_create: plugin={}, size={}x{} @{:.1}x",
        plugin_type_str, width, height, scale
    );

    // Store the NSView info in a thread-local so AuWindow::new() can read it
    // during AuPlatform::open_window() → AuWindow::new()
    PENDING_VIEW.with(|pv| {
        *pv.borrow_mut() = Some(PendingViewInfo {
            ns_view,
            width,
            height,
            scale,
        });
    });

    let platform = Rc::new(AuPlatform::new());
    let app = gpui::Application::with_platform(platform);

    let pt = plugin_type_str.clone();

    app.run(move |cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds {
                    origin: Default::default(),
                    size: size(px(width), px(height)),
                })),
                ..Default::default()
            },
            move |_window, cx| cx.new(|_cx| AuPluginView { plugin_type: pt }),
        )
        .expect("Failed to open GPUI AU window");
        cx.activate(true);
    });

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
    if let Some(AuWindowPtr(window_ptr)) = AU_WINDOW.get() {
        if !window_ptr.is_null() {
            let window = unsafe { &**window_ptr };
            let cb = window.request_frame_callback.borrow_mut().take();
            if let Some(mut cb) = cb {
                cb(RequestFrameOptions::default());
                window.request_frame_callback.borrow_mut().replace(cb);
            }
        }
    }
}

/// Handle view resize from the host.
#[unsafe(no_mangle)]
pub extern "C" fn gpui_au_resize(context: *mut AuContext, width: f32, height: f32, scale: f32) {
    if context.is_null() {
        return;
    }
    if let Some(AuWindowPtr(window_ptr)) = AU_WINDOW.get() {
        if !window_ptr.is_null() {
            let window = unsafe { &**window_ptr };
            window.handle_resize(width, height, scale);
        }
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
    if let Some(AuWindowPtr(window_ptr)) = AU_WINDOW.get() {
        if !window_ptr.is_null() {
            let window = unsafe { &**window_ptr };
            window.dispatch_input(event);
        }
    }
}

// ── Placeholder GPUI View ─────────────────────────────────────────────────────

struct AuPluginView {
    plugin_type: String,
}

impl Render for AuPluginView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x1a1a1e))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_color(rgb(0x60a0ff))
                    .text_xl()
                    .child(format!("SOTF: {}", self.plugin_type)),
            )
            .child(
                div()
                    .text_color(rgb(0x666666))
                    .text_sm()
                    .mt_2()
                    .child("GPUI rendering active"),
            )
    }
}
