use super::{LAST_ERROR, LAST_STATIC_ERROR};
#[cfg(target_os = "macos")]
use gpui::AppContext as _;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
#[cfg(target_os = "macos")]
use std::rc::Rc;

pub(super) fn set_last_error(msg: &str) {
    LAST_STATIC_ERROR.with(|error| error.set(std::ptr::null()));
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = CString::new(msg).ok();
    });
}

/// Publish a process-path diagnostic without allocating, deallocating, or
/// replacing the owned control-thread error string.
pub(super) fn set_last_error_static(msg: &'static CStr) {
    LAST_STATIC_ERROR.with(|error| error.set(msg.as_ptr()));
}

pub(super) fn sanitize_filename_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_separator = false;
    for ch in input.chars() {
        let next = if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            last_was_separator = false;
            Some(ch)
        } else if !last_was_separator {
            last_was_separator = true;
            Some('-')
        } else {
            None
        };
        if let Some(ch) = next {
            out.push(ch);
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "Untitled".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Create a GPUI AU context with a real plugin UI.
///
/// Unlike `gpui_au_create` (which shows a placeholder), this function creates
/// an `AuHostState` that reads parameters from an `AtomicParamCache` and writes
/// them through callbacks to the AU `AUParameterTree` — fully thread-safe.
///
/// # Safety
/// - `ns_view` must be a valid NSView pointer
/// - `plugin_type` must be a valid C string
/// - `param_cache` must be a valid pointer from `au_param_cache_create()`
/// - `set_param_cb` / `reset_param_cb` must be valid function pointers
/// - `cb_userdata` must remain valid for the lifetime of the returned context
#[unsafe(no_mangle)]
#[cfg(target_os = "macos")]
pub extern "C" fn gpui_au_create_with_plugin(
    ns_view: *mut std::ffi::c_void,
    width: f32,
    height: f32,
    scale: f32,
    plugin_type: *const c_char,
    param_cache: *mut super::param_cache::AtomicParamCache,
    set_param_cb: super::au_host::SetParamCallback,
    reset_param_cb: super::au_host::ResetParamCallback,
    cb_userdata: *mut std::ffi::c_void,
) -> *mut std::ffi::c_void {
    if ns_view.is_null() || plugin_type.is_null() || param_cache.is_null() {
        return std::ptr::null_mut();
    }

    let plugin_type_str = unsafe {
        match CStr::from_ptr(plugin_type).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return std::ptr::null_mut(),
        }
    };

    // Wrap the cache pointer in Arc (we take ownership of the FFI allocation)
    let cache = unsafe {
        // SAFETY: param_cache was created by au_param_cache_create, we take ownership
        std::sync::Arc::from_raw(param_cache as *const super::param_cache::AtomicParamCache)
    };
    // Keep an extra Arc ref so the cache outlives both the GPUI view and the FFI caller
    let cache_for_ffi = cache.clone();
    // Leak the FFI copy back so au_param_cache_write/destroy still work
    // Intentionally leak: the FFI caller still needs au_param_cache_write/destroy to work
    let _ = std::sync::Arc::into_raw(cache_for_ffi);

    // Store the NSView info for AuWindow::new()
    gpui_au::PENDING_VIEW.with(|pv| {
        *pv.borrow_mut() = Some(gpui_au::PendingViewInfo {
            ns_view: ns_view.cast(),
            width,
            height,
            scale,
        });
    });

    let platform = Rc::new(gpui_au::AuPlatform::new());
    let app = gpui::Application::with_platform(platform);

    // Clone Rc<AppCell> to keep GPUI alive after run() returns.
    // SAFETY: Application is `pub struct Application(Rc<AppCell>)` — a single-field newtype.
    // AppCell is pub (doc(hidden)). We clone the Rc to keep AppCell alive after run() consumes
    // Application. Without this, AppCell is deallocated when run() returns because AuPlatform::run()
    // calls the callback immediately (unlike macOS which blocks on [NSApp run]).
    debug_assert_eq!(
        std::mem::size_of::<gpui::Application>(),
        std::mem::size_of::<Rc<gpui::AppCell>>(),
        "Application layout changed — transmute assumption broken"
    );
    let app_cell: Rc<gpui::AppCell> = unsafe {
        let rc: &Rc<gpui::AppCell> = std::mem::transmute(&app);
        rc.clone()
    };

    let pt = plugin_type_str.clone();
    app.run(move |cx: &mut gpui::App| {
        match cx.open_window(
            gpui::WindowOptions {
                window_bounds: None,
                ..Default::default()
            },
            |_window, cx| {
                let entity = cx.new(|_| {
                    super::au_host::AuHostState::new(
                        cache.clone(),
                        set_param_cb,
                        reset_param_cb,
                        cb_userdata,
                        pt,
                    )
                });
                entity.update(cx, |state: &mut super::au_host::AuHostState, _| {
                    state.set_entity(entity.clone());
                });
                entity
            },
        ) {
            Ok(_handle) => {}
            Err(_e) => {}
        }
    });

    let context = Box::new(gpui_au::ffi::AuContext::new(plugin_type_str, app_cell));
    Box::into_raw(context).cast()
}
