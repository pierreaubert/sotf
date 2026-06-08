//! FFI module for iOS — C-compatible functions called from Objective-C app delegate.

use gpui::{App, AppCell, AppContext, RequestFrameOptions, WindowOptions};
use objc::{
    class, msg_send,
    runtime::{BOOL, Object},
    sel, sel_impl,
};
use std::backtrace::Backtrace;
use std::ffi::{CStr, c_char, c_void};
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::{
    Once, OnceLock,
    atomic::{AtomicBool, Ordering},
};

static IOS_APP_STATE: OnceLock<IosAppState> = OnceLock::new();
static REQUEST_FRAME_DISABLED: AtomicBool = AtomicBool::new(false);
static IOS_PANIC_HOOK: Once = Once::new();

#[link(name = "Metal", kind = "framework")]
unsafe extern "C" {
    fn MTLCreateSystemDefaultDevice() -> *mut c_void;
}

struct IosAppState {
    finish_launching: std::cell::UnsafeCell<Option<Box<dyn FnOnce()>>>,
}

unsafe impl Send for IosAppState {}
unsafe impl Sync for IosAppState {}

pub(crate) struct WindowListWrapper(
    pub(crate) std::cell::UnsafeCell<Vec<*const super::window::IosWindow>>,
);
unsafe impl Send for WindowListWrapper {}
unsafe impl Sync for WindowListWrapper {}

pub(crate) static IOS_WINDOW_LIST: OnceLock<WindowListWrapper> = OnceLock::new();

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_initialize() -> *mut c_void {
    install_ios_panic_hook();
    log::info!("GPUI iOS: Initializing");
    let state = IosAppState {
        finish_launching: std::cell::UnsafeCell::new(None),
    };
    if IOS_APP_STATE.set(state).is_err() {
        log::error!("GPUI iOS: Already initialized");
        return std::ptr::null_mut();
    }
    let _ = IOS_WINDOW_LIST.set(WindowListWrapper(std::cell::UnsafeCell::new(Vec::new())));
    1 as *mut c_void
}

pub(crate) fn register_window(window: *const super::window::IosWindow) {
    if let Some(wrapper) = IOS_WINDOW_LIST.get() {
        unsafe {
            (*wrapper.0.get()).push(window);
            log::info!("GPUI iOS: Registered window {:p}", window);
        }
    }
}

pub(crate) fn unregister_window(window: *const super::window::IosWindow) {
    if let Some(wrapper) = IOS_WINDOW_LIST.get() {
        unsafe {
            let windows = &mut *wrapper.0.get();
            windows.retain(|&w| w != window);
            log::info!("GPUI iOS: Unregistered window {:p}", window);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_get_window() -> *mut c_void {
    if let Some(wrapper) = IOS_WINDOW_LIST.get() {
        unsafe {
            let windows = &*wrapper.0.get();
            if let Some(&window) = windows.last() {
                return window as *mut c_void;
            }
        }
    }
    std::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_attach_to_view(parent: *mut c_void) -> *mut c_void {
    if parent.is_null() {
        return std::ptr::null_mut();
    }
    if let Some(wrapper) = IOS_WINDOW_LIST.get() {
        unsafe {
            let windows = &*wrapper.0.get();
            if let Some(&window_ptr) = windows.last() {
                if !window_ptr.is_null() {
                    (&*window_ptr).attach_to_parent_view(parent);
                    return window_ptr as *mut c_void;
                }
            }
        }
    }
    std::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_detach_view(window_ptr: *mut c_void) {
    if window_ptr.is_null() {
        return;
    }
    let window = unsafe { &*(window_ptr as *const super::window::IosWindow) };
    window.detach_from_parent_view();
}

pub(crate) fn set_finish_launching_callback(callback: Box<dyn FnOnce()>) {
    if let Some(state) = IOS_APP_STATE.get() {
        unsafe {
            *state.finish_launching.get() = Some(callback);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_did_finish_launching(_app_ptr: *mut c_void) {
    log::info!("GPUI iOS: Did finish launching");
    if let Some(state) = IOS_APP_STATE.get() {
        let callback = unsafe { (*state.finish_launching.get()).take() };
        if let Some(callback) = callback {
            callback();
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_will_enter_foreground(_app_ptr: *mut c_void) {
    log::info!("GPUI iOS: Will enter foreground");
    if let Some(wrapper) = IOS_WINDOW_LIST.get() {
        unsafe {
            let windows = &*wrapper.0.get();
            for &window_ptr in windows.iter() {
                if !window_ptr.is_null() {
                    let window = &*window_ptr;
                    window.notify_active_status_change(true);
                }
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_did_become_active(_app_ptr: *mut c_void) {
    log::info!("GPUI iOS: Did become active");
    if let Some(wrapper) = IOS_WINDOW_LIST.get() {
        unsafe {
            let windows = &*wrapper.0.get();
            for &window_ptr in windows.iter() {
                if !window_ptr.is_null() {
                    (&*window_ptr).notify_active_status_change(true);
                }
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_will_resign_active(_app_ptr: *mut c_void) {
    log::info!("GPUI iOS: Will resign active");
    if let Some(wrapper) = IOS_WINDOW_LIST.get() {
        unsafe {
            let windows = &*wrapper.0.get();
            for &window_ptr in windows.iter() {
                if !window_ptr.is_null() {
                    (&*window_ptr).notify_active_status_change(false);
                }
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_did_enter_background(_app_ptr: *mut c_void) {
    log::info!("GPUI iOS: Did enter background");
    if let Some(wrapper) = IOS_WINDOW_LIST.get() {
        unsafe {
            let windows = &*wrapper.0.get();
            for &window_ptr in windows.iter() {
                if !window_ptr.is_null() {
                    (&*window_ptr).notify_active_status_change(false);
                }
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_will_terminate(_app_ptr: *mut c_void) {
    log::info!("GPUI iOS: Will terminate");
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_handle_touch(
    window_ptr: *mut c_void,
    touch_ptr: *mut c_void,
    event_ptr: *mut c_void,
) {
    if window_ptr.is_null() || touch_ptr.is_null() {
        return;
    }
    let window = unsafe { &*(window_ptr as *const super::window::IosWindow) };
    window.handle_touch(
        touch_ptr as *mut objc::runtime::Object,
        event_ptr as *mut objc::runtime::Object,
    );
}

#[inline(never)]
#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_request_frame(window_ptr: *mut c_void) {
    if window_ptr.is_null() || REQUEST_FRAME_DISABLED.load(Ordering::Relaxed) {
        return;
    }
    let window = unsafe { &*(window_ptr as *const super::window::IosWindow) };

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        request_frame_for_window(window);
    }));

    if let Err(payload) = result {
        REQUEST_FRAME_DISABLED.store(true, Ordering::Relaxed);
        log::error!(
            "GPUI iOS: request frame panicked; disabling display-link frame requests: {}",
            panic_payload_message(payload.as_ref())
        );
    }
}

#[inline(never)]
fn request_frame_for_window(window: &super::window::IosWindow) {
    crate::instrumentation::emit_signpost(
        crate::instrumentation::IosSignpostCategory::Frame,
        "request_frame",
    );
    window.pump_momentum();
    let text_dirty = crate::TEXT_INPUT_DIRTY.swap(false, Ordering::AcqRel);
    let callback = window.request_frame_callback.borrow_mut().take();
    if let Some(mut cb) = callback {
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            cb(RequestFrameOptions {
                force_render: text_dirty,
                ..Default::default()
            });
        }));
        let mut slot = window.request_frame_callback.borrow_mut();
        if slot.is_none() {
            *slot = Some(cb);
        }
        drop(slot);
        if let Err(payload) = result {
            panic::resume_unwind(payload);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_register_platform_view_factory(
    view_type: *const c_char,
    kind: i32,
    create: Option<crate::platform_view::SwiftPlatformViewCreateCallback>,
    update_bounds: Option<crate::platform_view::SwiftPlatformViewUpdateBoundsCallback>,
    set_visible: Option<crate::platform_view::SwiftPlatformViewSetBoolCallback>,
    set_z_index: Option<crate::platform_view::SwiftPlatformViewSetZIndexCallback>,
    dispose: Option<crate::platform_view::SwiftPlatformViewDisposeCallback>,
) -> bool {
    // SAFETY: Swift passes a valid NUL-terminated view type string during this
    // registration call.
    let Some(view_type) = (unsafe { crate::platform_view::c_str_to_string(view_type) }) else {
        return false;
    };
    let Some(create) = create else {
        return false;
    };
    let kind = match kind {
        0 => crate::platform_view::PlatformViewKind::SwiftUi,
        1 => crate::platform_view::PlatformViewKind::UiKit,
        2 => crate::platform_view::PlatformViewKind::WebView,
        3 => crate::platform_view::PlatformViewKind::Map,
        4 => crate::platform_view::PlatformViewKind::Camera,
        _ => crate::platform_view::PlatformViewKind::Custom,
    };

    crate::platform_view::PlatformViewRegistry::global().register_swift_factory(
        view_type,
        kind,
        crate::platform_view::SwiftPlatformViewCallbacks {
            create,
            update_bounds,
            set_visible,
            set_z_index,
            dispose,
        },
    );
    crate::instrumentation::emit_signpost(
        crate::instrumentation::IosSignpostCategory::PlatformView,
        "register_factory",
    );
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_refresh_accessibility() {
    crate::instrumentation::emit_signpost(
        crate::instrumentation::IosSignpostCategory::Accessibility,
        "refresh_snapshot",
    );
    if let Some(wrapper) = IOS_WINDOW_LIST.get() {
        unsafe {
            for &window_ptr in (&*wrapper.0.get()).iter() {
                if !window_ptr.is_null() {
                    (&*window_ptr).refresh_accessibility();
                }
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_begin_metal_capture(label: *const c_char) -> bool {
    let label = if label.is_null() {
        "gpui-ios".to_string()
    } else {
        // SAFETY: Swift passes a NUL-terminated UTF-8 C string for the duration
        // of this call; invalid UTF-8 falls back to a stable label.
        unsafe { CStr::from_ptr(label) }
            .to_str()
            .unwrap_or("gpui-ios")
            .to_string()
    };
    if !crate::instrumentation::begin_metal_capture(label.clone()) {
        return false;
    }
    if !begin_platform_metal_capture() {
        crate::instrumentation::end_metal_capture();
        return false;
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_end_metal_capture() {
    end_platform_metal_capture();
    crate::instrumentation::end_metal_capture();
}

fn begin_platform_metal_capture() -> bool {
    unsafe {
        // SAFETY: Metal capture APIs are process-global developer tooling
        // hooks. We create the default device and ask MTLCaptureManager to
        // capture all queues for that device until `stopCapture`.
        let device = MTLCreateSystemDefaultDevice() as *mut Object;
        if device.is_null() {
            return false;
        }

        let manager: *mut Object = msg_send![class!(MTLCaptureManager), sharedCaptureManager];
        if manager.is_null() {
            let _: () = msg_send![device, release];
            return false;
        }

        let is_capturing: BOOL = msg_send![manager, isCapturing];
        if is_capturing {
            let _: () = msg_send![device, release];
            return false;
        }

        let descriptor: *mut Object = msg_send![class!(MTLCaptureDescriptor), alloc];
        let descriptor: *mut Object = msg_send![descriptor, init];
        if descriptor.is_null() {
            let _: () = msg_send![device, release];
            return false;
        }

        let _: () = msg_send![descriptor, setCaptureObject: device];
        let _: () = msg_send![descriptor, setDestination: 1_isize];
        let started: BOOL = msg_send![
            manager,
            startCaptureWithDescriptor: descriptor
            error: std::ptr::null_mut::<*mut Object>()
        ];
        let _: () = msg_send![descriptor, release];
        let _: () = msg_send![device, release];
        started
    }
}

fn end_platform_metal_capture() {
    unsafe {
        // SAFETY: `sharedCaptureManager` returns the process-wide manager; if a
        // capture is active, `stopCapture` is the documented balanced teardown
        // for captures started from code or Xcode.
        let manager: *mut Object = msg_send![class!(MTLCaptureManager), sharedCaptureManager];
        if manager.is_null() {
            return;
        }
        let is_capturing: BOOL = msg_send![manager, isCapturing];
        if is_capturing {
            let _: () = msg_send![manager, stopCapture];
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_handle_pencil_hover(
    x: f32,
    y: f32,
    altitude_angle: f32,
    azimuth_angle: f32,
    distance: f32,
    timestamp_seconds: f64,
) -> bool {
    crate::pencil::dispatch_hover_sample(crate::pencil::IosHoverSample {
        x,
        y,
        altitude_angle,
        azimuth_angle,
        distance,
        timestamp_seconds,
        device: crate::pencil::IosPointerDevice::Pencil,
    })
}

fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> &str {
    if let Some(message) = payload.downcast_ref::<&str>() {
        message
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.as_str()
    } else {
        "<non-string panic payload>"
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_handle_text_input(window_ptr: *mut c_void, text_ptr: *mut c_void) {
    if window_ptr.is_null() || text_ptr.is_null() {
        return;
    }
    let window = unsafe { &*(window_ptr as *const super::window::IosWindow) };
    window.handle_text_input(text_ptr as *mut objc::runtime::Object);
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_handle_key_event(
    window_ptr: *mut c_void,
    key_code: u32,
    modifiers: u32,
    is_key_down: bool,
) {
    if window_ptr.is_null() {
        return;
    }
    let window = unsafe { &*(window_ptr as *const super::window::IosWindow) };
    window.handle_key_event(key_code, modifiers, is_key_down);
}

// ── App callback storage ─────────────────────────────────────────────────────

struct AppCallbackCell(std::cell::UnsafeCell<Option<Box<dyn FnOnce(&mut App)>>>);
unsafe impl Send for AppCallbackCell {}
unsafe impl Sync for AppCallbackCell {}

static APP_CALLBACK: OnceLock<AppCallbackCell> = OnceLock::new();

pub fn set_app_callback(cb: Box<dyn FnOnce(&mut App)>) {
    let cell = APP_CALLBACK.get_or_init(|| AppCallbackCell(std::cell::UnsafeCell::new(None)));
    unsafe {
        *cell.0.get() = Some(cb);
    }
}

fn take_app_callback() -> Option<Box<dyn FnOnce(&mut App)>> {
    APP_CALLBACK
        .get()
        .and_then(|cell| unsafe { (*cell.0.get()).take() })
}

// ── Asset source storage ─────────────────────────────────────────────────────

/// Wrapper that forwards `AssetSource` through a `Box<dyn AssetSource>`,
/// needed because `with_assets()` takes `impl AssetSource` and the trait
/// isn't automatically implemented for `Box<dyn AssetSource>`.
struct BoxedAssetSource(Box<dyn gpui::AssetSource>);

impl gpui::AssetSource for BoxedAssetSource {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        self.0.load(path)
    }
    fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        self.0.list(path)
    }
}

struct AssetSourceCell(std::cell::UnsafeCell<Option<Box<dyn gpui::AssetSource>>>);
unsafe impl Send for AssetSourceCell {}
unsafe impl Sync for AssetSourceCell {}

static ASSET_SOURCE: OnceLock<AssetSourceCell> = OnceLock::new();

/// Register an asset source (SVG icons, fonts, images) before calling `run_app()`.
/// Without this, `svg()` elements will fail to load their paths.
pub fn set_asset_source(source: impl gpui::AssetSource + 'static) {
    let cell = ASSET_SOURCE.get_or_init(|| AssetSourceCell(std::cell::UnsafeCell::new(None)));
    unsafe {
        *cell.0.get() = Some(Box::new(source));
    }
}

fn take_asset_source() -> Option<BoxedAssetSource> {
    ASSET_SOURCE
        .get()
        .and_then(|cell| unsafe { (*cell.0.get()).take() })
        .map(BoxedAssetSource)
}

#[unsafe(no_mangle)]
pub extern "C" fn gpui_ios_run_demo() {
    run_app();
}

fn retain_application_for_process_lifetime(app: &gpui::Application) {
    // SAFETY: `gpui::Application` is a private single-field tuple struct
    // containing `Rc<AppCell>` in the pinned GPUI revision. iOS does not let
    // `Platform::run` block forever like desktop platforms do, so `run_app`
    // would otherwise drop the application after the launch callback returns.
    // Clone the hidden Rc and intentionally leak it so GPUI's AppCell, windows,
    // and frame callbacks live for the process lifetime.
    debug_assert_eq!(
        std::mem::size_of::<gpui::Application>(),
        std::mem::size_of::<Rc<AppCell>>(),
        "Application layout changed -- iOS AppCell clone assumption broken"
    );
    let retained: Rc<AppCell> = unsafe {
        let app_cell: &Rc<AppCell> = std::mem::transmute(app);
        Rc::clone(app_cell)
    };
    std::mem::forget(retained);
    log::info!("GPUI iOS: Retained application for process lifetime");
}

fn install_ios_panic_hook() {
    IOS_PANIC_HOOK.call_once(|| {
        let previous_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            log::error!(
                "GPUI iOS: Rust panic: {info}\n{}",
                Backtrace::force_capture()
            );
            previous_hook(info);
        }));
    });
}

pub fn run_app() {
    install_ios_panic_hook();
    log::info!("GPUI iOS: Starting application");
    if IOS_APP_STATE.get().is_none() {
        let state = IosAppState {
            finish_launching: std::cell::UnsafeCell::new(None),
        };
        let _ = IOS_APP_STATE.set(state);
        let _ = IOS_WINDOW_LIST.set(WindowListWrapper(std::cell::UnsafeCell::new(Vec::new())));
    }

    let platform = Rc::new(super::IosPlatform::new());
    let app = gpui::Application::with_platform(platform);
    let app = if let Some(assets) = take_asset_source() {
        app.with_assets(assets)
    } else {
        app
    };
    retain_application_for_process_lifetime(&app);
    app.run(|cx: &mut App| {
        if let Some(cb) = take_app_callback() {
            cb(cx);
        } else {
            log::warn!("GPUI iOS: No app callback registered — opening default empty window");
            cx.open_window(
                WindowOptions {
                    window_bounds: None,
                    ..Default::default()
                },
                |_, cx| cx.new(|_| gpui::Empty),
            )
            .expect("Failed to open default window");
            cx.activate(true);
        }
    });

    if let Some(state) = IOS_APP_STATE.get() {
        let callback = unsafe { (*state.finish_launching.get()).take() };
        if let Some(callback) = callback {
            callback();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_unregister_window() {
        let _ = IOS_WINDOW_LIST.set(WindowListWrapper(std::cell::UnsafeCell::new(Vec::new())));

        let dummy: *const crate::ios::window::IosWindow = 0x1234 as *const _;
        register_window(dummy);
        assert_eq!(unsafe { &*IOS_WINDOW_LIST.get().unwrap().0.get() }.len(), 1);

        unregister_window(dummy);
        assert!(unsafe { &*IOS_WINDOW_LIST.get().unwrap().0.get() }.is_empty());
    }
}
