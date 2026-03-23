// ============================================================================
// SOTF Audio FFI - C bindings for Audio Unit integration
// ============================================================================
//
// This crate provides C-compatible FFI bindings for SOTF audio plugins,
// enabling integration with macOS Audio Units (AUv3).
//
// Architecture:
// - Opaque handles for plugin instances
// - C-compatible function signatures
// - JSON-based configuration
// - Parameter management system

#![cfg(target_os = "macos")]
// FFI functions necessarily dereference raw pointers from C callers
#![allow(clippy::not_unsafe_ptr_arg_deref)]

// Re-export gpui-au FFI symbols so they're included in this staticlib.
// Without this, the linker would strip gpui-au's #[no_mangle] functions
// since nothing in plugins-ffi directly references them.
pub use gpui_au::ffi as gpui_au_ffi;

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_int};
use std::panic::{self, AssertUnwindSafe};
use std::ptr;
use std::slice;

use sotf_host::plugin::{Plugin, ProcessContext};

mod parameter_map;
mod plugin_factory;

pub use parameter_map::{ParameterInfo, ParameterMap};

// ============================================================================
// Opaque Handle Types
// ============================================================================

/// Opaque handle to a plugin instance
///
/// This is passed to Swift/Objective-C code and must not be dereferenced
/// outside of Rust. Use plugin_* functions to interact with it.
#[repr(C)]
pub struct PluginHandle {
    plugin: Box<dyn Plugin>,
    parameter_map: ParameterMap,
    sample_rate: u32,
    input_channels: usize,
    output_channels: usize,
}

// ============================================================================
// Error Handling
// ============================================================================

/// Error codes returned by FFI functions
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginError {
    Success = 0,
    InvalidHandle = -1,
    InvalidParameter = -2,
    NullPointer = -3,
    InvalidUtf8 = -4,
    PluginCreationFailed = -5,
    ProcessingFailed = -6,
    InitializationFailed = -7,
    InvalidConfig = -8,
    UnknownError = -99,
}

impl From<PluginError> for c_int {
    fn from(err: PluginError) -> c_int {
        err as c_int
    }
}

thread_local! {
    static LAST_ERROR: std::cell::RefCell<Option<CString>> = const { std::cell::RefCell::new(None) };
}

fn set_last_error(msg: &str) {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = CString::new(msg).ok();
    });
}

/// Get the last error message
/// Returns NULL if no error
#[unsafe(no_mangle)]
pub extern "C" fn plugin_get_last_error() -> *const c_char {
    LAST_ERROR.with(|e| {
        e.borrow()
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(ptr::null())
    })
}

// ============================================================================
// Plugin Lifecycle
// ============================================================================

/// Create a new plugin instance
///
/// # Arguments
/// * `plugin_type` - Plugin type name (e.g., "EQ", "Compressor")
/// * `config_json` - JSON configuration string
/// * `sample_rate` - Sample rate in Hz
/// * `input_channels` - Number of input channels
/// * `output_channels` - Number of output channels
///
/// # Returns
/// * Opaque plugin handle on success
/// * NULL on failure (check plugin_get_last_error())
///
/// # Safety
/// * Caller must ensure plugin_type and config_json are valid UTF-8 C strings
/// * Caller must call plugin_destroy() when done
#[unsafe(no_mangle)]
pub extern "C" fn plugin_create(
    plugin_type: *const c_char,
    config_json: *const c_char,
    sample_rate: u32,
    input_channels: usize,
    output_channels: usize,
) -> *mut PluginHandle {
    // Catch panics and return NULL
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        // Validate pointers
        if plugin_type.is_null() || config_json.is_null() {
            set_last_error("NULL pointer passed to plugin_create");
            return ptr::null_mut();
        }

        // Convert C strings to Rust
        let plugin_type_str = unsafe {
            match CStr::from_ptr(plugin_type).to_str() {
                Ok(s) => s,
                Err(_) => {
                    set_last_error("Invalid UTF-8 in plugin_type");
                    return ptr::null_mut();
                }
            }
        };

        let config_str = unsafe {
            match CStr::from_ptr(config_json).to_str() {
                Ok(s) => s,
                Err(_) => {
                    set_last_error("Invalid UTF-8 in config_json");
                    return ptr::null_mut();
                }
            }
        };

        // Create plugin
        let mut plugin = match plugin_factory::create_plugin(
            plugin_type_str,
            config_str,
            input_channels,
            output_channels,
            sample_rate,
        ) {
            Ok(p) => p,
            Err(e) => {
                set_last_error(&format!("Failed to create plugin: {}", e));
                return ptr::null_mut();
            }
        };

        // Initialize plugin
        if let Err(e) = plugin.initialize(sample_rate) {
            set_last_error(&format!("Failed to initialize plugin: {}", e));
            return ptr::null_mut();
        }

        // Build parameter map
        let parameter_map = ParameterMap::from_plugin(&*plugin, plugin_type_str);

        // Create handle
        let handle = Box::new(PluginHandle {
            plugin,
            parameter_map,
            sample_rate,
            input_channels,
            output_channels,
        });

        Box::into_raw(handle)
    }));

    result.unwrap_or_else(|_| {
        set_last_error("Panic occurred in plugin_create");
        ptr::null_mut()
    })
}

/// Destroy a plugin instance
///
/// # Safety
/// * handle must be a valid pointer returned by plugin_create()
/// * handle must not be used after this call
#[unsafe(no_mangle)]
pub extern "C" fn plugin_destroy(handle: *mut PluginHandle) {
    if !handle.is_null() {
        let _ = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
            drop(Box::from_raw(handle));
        }));
    }
}

/// Reset plugin state (clear buffers, reset filters)
///
/// # Safety
/// * handle must be a valid pointer returned by plugin_create()
#[unsafe(no_mangle)]
pub extern "C" fn plugin_reset(handle: *mut PluginHandle) -> c_int {
    if handle.is_null() {
        set_last_error("NULL handle in plugin_reset");
        return PluginError::InvalidHandle.into();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        let handle_ref = &mut *handle;
        handle_ref.plugin.reset();
        PluginError::Success
    }));

    result
        .unwrap_or_else(|_| {
            set_last_error("Panic in plugin_reset");
            PluginError::UnknownError
        })
        .into()
}

// ============================================================================
// Audio Processing
// ============================================================================

/// Process audio samples
///
/// # Arguments
/// * `handle` - Plugin handle
/// * `input` - Interleaved input samples [C0_F0, C1_F0, ..., C0_F1, C1_F1, ...]
/// * `output` - Interleaved output buffer (will be filled)
/// * `num_frames` - Number of frames to process
///
/// # Returns
/// * 0 on success
/// * Error code on failure
///
/// # Safety
/// * handle must be a valid plugin handle
/// * input must point to num_frames * input_channels samples
/// * output must have space for num_frames * output_channels samples
/// * This function is designed to be real-time safe (no allocations)
#[unsafe(no_mangle)]
pub extern "C" fn plugin_process(
    handle: *mut PluginHandle,
    input: *const f32,
    output: *mut f32,
    num_frames: usize,
) -> c_int {
    if handle.is_null() || input.is_null() || output.is_null() {
        return PluginError::NullPointer.into();
    }

    // Real-time processing: avoid panic catching overhead in release builds
    #[cfg(debug_assertions)]
    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        process_impl(handle, input, output, num_frames)
    }));

    #[cfg(not(debug_assertions))]
    let result: Result<PluginError, ()> =
        Ok(unsafe { process_impl(handle, input, output, num_frames) });

    result
        .unwrap_or_else(|_| {
            set_last_error("Panic in plugin_process");
            PluginError::ProcessingFailed
        })
        .into()
}

#[inline]
unsafe fn process_impl(
    handle: *mut PluginHandle,
    input: *const f32,
    output: *mut f32,
    num_frames: usize,
) -> PluginError {
    // SAFETY: Caller must ensure handle is valid and non-null
    let handle_ref = unsafe { &mut *handle };

    let input_samples = num_frames * handle_ref.input_channels;
    let output_samples = num_frames * handle_ref.output_channels;

    // SAFETY: Caller must ensure input/output pointers are valid with correct sizes
    let input_slice = unsafe { slice::from_raw_parts(input, input_samples) };
    let output_slice = unsafe { slice::from_raw_parts_mut(output, output_samples) };

    let context = ProcessContext {
        sample_rate: handle_ref.sample_rate,
        num_frames,
    };

    match handle_ref
        .plugin
        .process(input_slice, output_slice, &context)
    {
        Ok(_) => PluginError::Success,
        Err(_) => PluginError::ProcessingFailed,
    }
}

// ============================================================================
// Parameter Management
// ============================================================================

/// Get the number of parameters
#[unsafe(no_mangle)]
pub extern "C" fn plugin_get_parameter_count(handle: *const PluginHandle) -> c_int {
    if handle.is_null() {
        return 0;
    }

    unsafe {
        let handle_ref = &*handle;
        handle_ref.parameter_map.count() as c_int
    }
}

/// Get parameter info by index
///
/// # Returns
/// * Pointer to parameter info (valid until plugin is destroyed)
/// * NULL if index is out of bounds
#[unsafe(no_mangle)]
pub extern "C" fn plugin_get_parameter_info(
    handle: *const PluginHandle,
    index: usize,
) -> *const ParameterInfo {
    if handle.is_null() {
        return ptr::null();
    }

    unsafe {
        let handle_ref = &*handle;
        handle_ref
            .parameter_map
            .get_info(index)
            .map(|info| info as *const ParameterInfo)
            .unwrap_or(ptr::null())
    }
}

/// Set a parameter value (normalized 0.0-1.0)
///
/// # Arguments
/// * `handle` - Plugin handle
/// * `param_id` - Parameter ID string
/// * `normalized_value` - Normalized value (0.0 = min, 1.0 = max)
///
/// # Returns
/// * 0 on success
/// * Error code on failure
#[unsafe(no_mangle)]
pub extern "C" fn plugin_set_parameter(
    handle: *mut PluginHandle,
    param_id: *const c_char,
    normalized_value: c_double,
) -> c_int {
    if handle.is_null() || param_id.is_null() {
        set_last_error("NULL pointer in plugin_set_parameter");
        return PluginError::NullPointer.into();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        let handle_ref = &mut *handle;

        let param_id_str = match CStr::from_ptr(param_id).to_str() {
            Ok(s) => s,
            Err(_) => {
                set_last_error("Invalid UTF-8 in param_id");
                return PluginError::InvalidUtf8;
            }
        };

        match handle_ref.parameter_map.set_normalized(
            &mut *handle_ref.plugin,
            param_id_str,
            normalized_value,
        ) {
            Ok(_) => PluginError::Success,
            Err(e) => {
                set_last_error(&format!("Failed to set parameter: {}", e));
                PluginError::InvalidParameter
            }
        }
    }));

    result
        .unwrap_or_else(|_| {
            set_last_error("Panic in plugin_set_parameter");
            PluginError::UnknownError
        })
        .into()
}

/// Get a parameter value (normalized 0.0-1.0)
///
/// # Returns
/// * Normalized value on success
/// * -1.0 on error
#[unsafe(no_mangle)]
pub extern "C" fn plugin_get_parameter(
    handle: *const PluginHandle,
    param_id: *const c_char,
) -> c_double {
    if handle.is_null() || param_id.is_null() {
        return -1.0;
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        let handle_ref = &*handle;

        let param_id_str = match CStr::from_ptr(param_id).to_str() {
            Ok(s) => s,
            Err(_) => return -1.0,
        };

        handle_ref
            .parameter_map
            .get_normalized(&*handle_ref.plugin, param_id_str)
            .unwrap_or(-1.0)
    }));

    result.unwrap_or(-1.0)
}

// ============================================================================
// Plugin Information
// ============================================================================

/// Get plugin information as JSON string
///
/// # Returns
/// * JSON string (caller must free with plugin_free_string())
/// * NULL on error
#[unsafe(no_mangle)]
pub extern "C" fn plugin_get_info_json(handle: *const PluginHandle) -> *mut c_char {
    if handle.is_null() {
        return ptr::null_mut();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        let handle_ref = &*handle;
        let info = handle_ref.plugin.info();

        let json = serde_json::json!({
            "name": info.name,
            "version": info.version,
            "author": info.author,
            "description": info.description,
            "input_channels": handle_ref.input_channels,
            "output_channels": handle_ref.output_channels,
            "latency_samples": handle_ref.plugin.latency_samples(),
        });

        match CString::new(json.to_string()) {
            Ok(s) => s.into_raw(),
            Err(_) => ptr::null_mut(),
        }
    }));

    result.unwrap_or(ptr::null_mut())
}

/// Free a string returned by the plugin
#[unsafe(no_mangle)]
pub extern "C" fn plugin_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}

// ============================================================================
// State Save/Load
// ============================================================================

/// Save plugin state to a JSON byte buffer.
///
/// # Returns
/// * Pointer to allocated buffer on success (caller must free with plugin_free_state())
/// * NULL on error
///
/// # Safety
/// * handle must be a valid plugin handle
/// * out_len must be a valid pointer to write the buffer length
#[unsafe(no_mangle)]
pub extern "C" fn plugin_save_state(
    handle: *const PluginHandle,
    out_len: *mut usize,
) -> *mut u8 {
    if handle.is_null() || out_len.is_null() {
        return ptr::null_mut();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        let handle_ref = &*handle;
        let state = plugins_bridge::state::save_state(&*handle_ref.plugin);
        let len = state.len();
        let ptr = state.as_ptr();

        // Allocate and copy to a buffer the caller can free
        let buf = libc_malloc(len);
        if buf.is_null() {
            return ptr::null_mut();
        }
        std::ptr::copy_nonoverlapping(ptr, buf, len);
        *out_len = len;
        buf
    }));

    result.unwrap_or(ptr::null_mut())
}

/// Load plugin state from a JSON byte buffer.
///
/// # Returns
/// * 0 on success
/// * Error code on failure
///
/// # Safety
/// * handle must be a valid plugin handle
/// * data must point to len bytes of valid JSON
#[unsafe(no_mangle)]
pub extern "C" fn plugin_load_state(
    handle: *mut PluginHandle,
    data: *const u8,
    len: usize,
) -> c_int {
    if handle.is_null() || data.is_null() {
        set_last_error("NULL pointer in plugin_load_state");
        return PluginError::NullPointer.into();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        let handle_ref = &mut *handle;
        let slice = slice::from_raw_parts(data, len);

        match plugins_bridge::state::load_state(&mut *handle_ref.plugin, slice) {
            Ok(_) => PluginError::Success,
            Err(e) => {
                set_last_error(&format!("Failed to load state: {e}"));
                PluginError::InvalidConfig
            }
        }
    }));

    result
        .unwrap_or_else(|_| {
            set_last_error("Panic in plugin_load_state");
            PluginError::UnknownError
        })
        .into()
}

/// Free a state buffer returned by plugin_save_state().
#[unsafe(no_mangle)]
pub extern "C" fn plugin_free_state(data: *mut u8, len: usize) {
    if !data.is_null() && len > 0 {
        libc_free(data, len);
    }
}

/// Get the list of available plugin types as a JSON array string.
///
/// # Returns
/// * JSON array string (caller must free with plugin_free_string())
/// * NULL on error
#[unsafe(no_mangle)]
pub extern "C" fn plugin_available_types() -> *mut c_char {
    let types = plugins_bridge::factory::available_plugin_types();
    let json = serde_json::json!(types);
    match CString::new(json.to_string()) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

// Simple allocator wrappers for FFI buffer management
fn libc_malloc(len: usize) -> *mut u8 {
    let layout = std::alloc::Layout::from_size_align(len, 1).unwrap();
    unsafe { std::alloc::alloc(layout) }
}

fn libc_free(ptr: *mut u8, len: usize) {
    let layout = std::alloc::Layout::from_size_align(len, 1).unwrap();
    unsafe { std::alloc::dealloc(ptr, layout) }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_plugin_lifecycle() {
        let plugin_type = CString::new("EQ").unwrap();
        let config = CString::new(r#"{"filters": []}"#).unwrap();

        let handle = plugin_create(plugin_type.as_ptr(), config.as_ptr(), 48000, 2, 2);

        assert!(!handle.is_null());

        plugin_destroy(handle);
    }

    #[test]
    fn test_null_safety() {
        let handle = plugin_create(ptr::null(), ptr::null(), 48000, 2, 2);
        assert!(handle.is_null());
    }
}
