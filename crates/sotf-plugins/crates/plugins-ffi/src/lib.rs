// ============================================================================
// SOTF Audio FFI - C bindings for Audio Unit integration
// ============================================================================
//
// This crate provides C-compatible FFI bindings for SOTF audio plugins,
// enabling integration with Audio Unit (AUv3) and portable native hosts.
//
// Architecture:
// - Opaque handles for plugin instances
// - C-compatible function signatures
// - JSON-based configuration
// - Parameter management system

// FFI functions necessarily dereference raw pointers from C callers
#![allow(clippy::not_unsafe_ptr_arg_deref)]

// Re-export gpui-au FFI symbols so they're included in this staticlib.
// Without this, the linker would strip gpui-au's #[no_mangle] functions
// since nothing in plugins-ffi directly references them.
#[cfg(target_os = "macos")]
pub use gpui_au::ffi as gpui_au_ffi;

use std::ffi::{CStr, CString};
#[cfg(target_os = "macos")]
use std::rc::Rc;

#[cfg(target_os = "macos")]
use gpui::AppContext as _;
use std::os::raw::{c_char, c_double, c_int};
use std::panic::{self, AssertUnwindSafe};
use std::ptr;
use std::slice;

use sotf_host::plugin::{
    MidiEvent, MidiMessage, NoteExpressionEvent as HostNoteExpressionEvent,
    NoteExpressionKind as HostNoteExpressionKind, Plugin, ProcessContext,
};

#[cfg(target_os = "macos")]
mod au_host;
pub mod param_cache;
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
    plugin_type: String,
    parameter_map: ParameterMap,
    sample_rate: u32,
    input_channels: usize,
    output_channels: usize,
    midi_output_events: Vec<PluginMidiEvent>,
    note_expression_output_events: Vec<PluginNoteExpressionEvent>,
}

const SOTF_PLUGIN_FFI_ABI_VERSION: u32 = 3;
const MAX_FFI_MIDI_EVENTS_PER_BLOCK: usize = 256;
const MAX_FFI_OUTPUT_EVENTS_PER_BLOCK: usize = 256;

const PRESET_UT_TYPE: &[u8] = b"org.spinorama.sotf.plugin-preset\0";
const PRESET_FILE_EXTENSION: &[u8] = b"sotfpreset\0";
const PRESET_MIME_TYPE: &[u8] = b"application/vnd.spinorama.sotf.plugin-preset+json\0";
const VST3_COMPONENT_NAME: &[u8] = b"SOTF Plugin FFI Host\0";
const VST3_VENDOR: &[u8] = b"Spinorama\0";
const VST3_SDK_VERSION: &[u8] = b"VST 3.7 compatible C ABI\0";
const VST3_ENTRYPOINT: &[u8] = b"plugin_create\0";
const SWIFT_PACKAGE_NAME: &[u8] = b"SOTFPluginFFI\0";
const SWIFT_PRODUCT_NAME: &[u8] = b"SOTFPluginFFI\0";
const SWIFT_TARGET_NAME: &[u8] = b"SOTFPluginFFI\0";
const SWIFT_LIBRARY_NAME: &[u8] = b"sotf_audio_plugins_ffi\0";
const SWIFT_HEADER_NAME: &[u8] = b"SOTFPluginFFI.h\0";

/// Host/packaging family that can consume this C ABI.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginFfiHostKind {
    Unknown = 0,
    AudioUnitV3 = 1,
    Vst3 = 2,
    SwiftPackage = 3,
}

/// Runtime-advertised FFI capabilities.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginFfiCapabilities {
    pub abi_version: u32,
    pub host_kind: PluginFfiHostKind,
    pub supports_audio: bool,
    pub supports_parameters: bool,
    pub supports_state: bool,
    pub supports_midi_input: bool,
    pub supports_midi_output: bool,
    pub supports_note_expression: bool,
    pub supports_apple_au_v3: bool,
    pub supports_ios_au_v3: bool,
    pub supports_windows_vst3: bool,
    pub supports_swift_package: bool,
    pub supports_preset_documents: bool,
}

/// Preset/document metadata shared by AUv3 and SwiftPM hosts.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginPresetDocumentInfo {
    pub schema_version: u32,
    pub ut_type: *const c_char,
    pub file_extension: *const c_char,
    pub mime_type: *const c_char,
    pub supports_full_state_for_document: bool,
    pub supports_security_scoped_bookmarks: bool,
}

/// Raw MIDI event scheduled within a processing block.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginMidiEvent {
    pub sample_offset: usize,
    pub data: [u8; 3],
    pub len: u8,
}

/// ABI-visible note expression kinds for future AUv3/VST3 bridging.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginNoteExpressionKind {
    PitchBend = 1,
    Pressure = 2,
    Timbre = 3,
    Brightness = 4,
    Volume = 5,
    Pan = 6,
}

/// Per-note expression event scheduled within a processing block.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PluginNoteExpressionEvent {
    pub sample_offset: usize,
    pub note_id: i32,
    pub channel: u8,
    pub note: u8,
    pub expression: PluginNoteExpressionKind,
    pub value: f64,
}

/// Windows/VST3 loader metadata for C#/Python hosts.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginVst3FfiDescriptor {
    pub abi_version: u32,
    pub class_id: [u8; 16],
    pub component_name: *const c_char,
    pub vendor: *const c_char,
    pub sdk_version: *const c_char,
    pub entrypoint: *const c_char,
    pub supports_com_factory: bool,
    pub supports_audio_effects: bool,
    pub supports_instruments: bool,
    pub supports_midi_output: bool,
    pub supports_note_expression: bool,
}

/// Swift Package distribution metadata.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginSwiftPackageInfo {
    pub package_name: *const c_char,
    pub product_name: *const c_char,
    pub target_name: *const c_char,
    pub library_name: *const c_char,
    pub umbrella_header: *const c_char,
    pub supports_staticlib: bool,
    pub supports_xcframework: bool,
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
    UnsupportedFeature = -9,
    BufferTooSmall = -10,
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
// ABI / Platform Capability Introspection
// ============================================================================

/// Get the stable ABI version for this FFI surface.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_ffi_abi_version() -> u32 {
    SOTF_PLUGIN_FFI_ABI_VERSION
}

/// Get runtime capabilities for the current target.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_ffi_capabilities() -> PluginFfiCapabilities {
    PluginFfiCapabilities {
        abi_version: SOTF_PLUGIN_FFI_ABI_VERSION,
        host_kind: current_host_kind(),
        supports_audio: true,
        supports_parameters: true,
        supports_state: true,
        supports_midi_input: true,
        supports_midi_output: true,
        supports_note_expression: true,
        supports_apple_au_v3: cfg!(any(target_os = "macos", target_os = "ios")),
        supports_ios_au_v3: cfg!(target_os = "ios"),
        supports_windows_vst3: cfg!(target_os = "windows"),
        supports_swift_package: cfg!(any(target_os = "macos", target_os = "ios")),
        supports_preset_documents: true,
    }
}

/// Get machine-readable platform and capability metadata as JSON.
///
/// Caller must free the returned string with plugin_free_string().
#[unsafe(no_mangle)]
pub extern "C" fn plugin_ffi_platform_info_json() -> *mut c_char {
    let caps = plugin_ffi_capabilities();
    let json = serde_json::json!({
        "abi_version": caps.abi_version,
        "target_os": std::env::consts::OS,
        "target_arch": std::env::consts::ARCH,
        "host_kind": host_kind_name(caps.host_kind),
        "supports_audio": caps.supports_audio,
        "supports_parameters": caps.supports_parameters,
        "supports_state": caps.supports_state,
        "supports_midi_input": caps.supports_midi_input,
        "supports_midi_output": caps.supports_midi_output,
        "supports_note_expression": caps.supports_note_expression,
        "supports_apple_au_v3": caps.supports_apple_au_v3,
        "supports_ios_au_v3": caps.supports_ios_au_v3,
        "supports_windows_vst3": caps.supports_windows_vst3,
        "supports_swift_package": caps.supports_swift_package,
        "supports_preset_documents": caps.supports_preset_documents,
    });

    match CString::new(json.to_string()) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Get preset document metadata for host file dialogs and AUv3 document state.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_preset_document_info() -> PluginPresetDocumentInfo {
    PluginPresetDocumentInfo {
        schema_version: 1,
        ut_type: PRESET_UT_TYPE.as_ptr().cast(),
        file_extension: PRESET_FILE_EXTENSION.as_ptr().cast(),
        mime_type: PRESET_MIME_TYPE.as_ptr().cast(),
        supports_full_state_for_document: cfg!(any(target_os = "macos", target_os = "ios")),
        supports_security_scoped_bookmarks: cfg!(target_os = "macos"),
    }
}

/// Get Windows/VST3 FFI metadata for native language bindings.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_vst3_ffi_descriptor() -> PluginVst3FfiDescriptor {
    PluginVst3FfiDescriptor {
        abi_version: SOTF_PLUGIN_FFI_ABI_VERSION,
        // Stable namespace UUID: "SOTF-FFI-VST3-01" bytes.
        class_id: *b"SOTF-FFI-VST3-01",
        component_name: VST3_COMPONENT_NAME.as_ptr().cast(),
        vendor: VST3_VENDOR.as_ptr().cast(),
        sdk_version: VST3_SDK_VERSION.as_ptr().cast(),
        entrypoint: VST3_ENTRYPOINT.as_ptr().cast(),
        supports_com_factory: false,
        supports_audio_effects: true,
        supports_instruments: true,
        supports_midi_output: true,
        supports_note_expression: true,
    }
}

/// Get Swift Package metadata for Xcode/SwiftPM integrations.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_swift_package_info() -> PluginSwiftPackageInfo {
    PluginSwiftPackageInfo {
        package_name: SWIFT_PACKAGE_NAME.as_ptr().cast(),
        product_name: SWIFT_PRODUCT_NAME.as_ptr().cast(),
        target_name: SWIFT_TARGET_NAME.as_ptr().cast(),
        library_name: SWIFT_LIBRARY_NAME.as_ptr().cast(),
        umbrella_header: SWIFT_HEADER_NAME.as_ptr().cast(),
        supports_staticlib: true,
        supports_xcframework: true,
    }
}

fn current_host_kind() -> PluginFfiHostKind {
    if cfg!(target_os = "windows") {
        PluginFfiHostKind::Vst3
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        PluginFfiHostKind::AudioUnitV3
    } else {
        PluginFfiHostKind::Unknown
    }
}

fn host_kind_name(kind: PluginFfiHostKind) -> &'static str {
    match kind {
        PluginFfiHostKind::Unknown => "unknown",
        PluginFfiHostKind::AudioUnitV3 => "au_v3",
        PluginFfiHostKind::Vst3 => "vst3",
        PluginFfiHostKind::SwiftPackage => "swift_package",
    }
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
            plugin_type: plugin_type_str.to_string(),
            parameter_map,
            sample_rate,
            input_channels,
            output_channels,
            midi_output_events: Vec::with_capacity(MAX_FFI_OUTPUT_EVENTS_PER_BLOCK),
            note_expression_output_events: Vec::with_capacity(MAX_FFI_OUTPUT_EVENTS_PER_BLOCK),
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
        let context = ProcessContext::new((*handle).sample_rate, num_frames);
        process_impl(handle, input, output, num_frames, &context)
    }));

    #[cfg(not(debug_assertions))]
    let result: Result<PluginError, ()> = Ok(unsafe {
        let context = ProcessContext::new((*handle).sample_rate, num_frames);
        process_impl(handle, input, output, num_frames, &context)
    });

    result
        .unwrap_or_else(|_| {
            set_last_error("Panic in plugin_process");
            PluginError::ProcessingFailed
        })
        .into()
}

/// Process audio samples with incoming MIDI events.
///
/// MIDI events are copied into a fixed stack buffer, then borrowed by
/// ProcessContext. No heap allocation occurs on the render path.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_process_with_midi(
    handle: *mut PluginHandle,
    input: *const f32,
    output: *mut f32,
    num_frames: usize,
    midi_events: *const PluginMidiEvent,
    midi_event_count: usize,
) -> c_int {
    if handle.is_null()
        || input.is_null()
        || output.is_null()
        || (midi_events.is_null() && midi_event_count > 0)
    {
        return PluginError::NullPointer.into();
    }

    #[cfg(debug_assertions)]
    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        process_with_ffi_events_impl(
            handle,
            input,
            output,
            num_frames,
            midi_events,
            midi_event_count,
        )
    }));

    #[cfg(not(debug_assertions))]
    let result: Result<PluginError, ()> = Ok(unsafe {
        process_with_ffi_events_impl(
            handle,
            input,
            output,
            num_frames,
            midi_events,
            midi_event_count,
        )
    });

    result
        .unwrap_or_else(|_| {
            set_last_error("Panic in plugin_process_with_midi");
            PluginError::ProcessingFailed
        })
        .into()
}

/// Process audio with MIDI/note-expression input and output ABI slots.
///
/// Incoming MIDI is bridged into ProcessContext. Queued MIDI output and Note
/// Expression events are copied into host-provided buffers without allocating
/// on the render path.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_process_with_events(
    handle: *mut PluginHandle,
    input: *const f32,
    output: *mut f32,
    num_frames: usize,
    midi_input: *const PluginMidiEvent,
    midi_input_count: usize,
    note_expression_input: *const PluginNoteExpressionEvent,
    note_expression_input_count: usize,
    _midi_output: *mut PluginMidiEvent,
    _midi_output_capacity: usize,
    midi_output_count: *mut usize,
    _note_expression_output: *mut PluginNoteExpressionEvent,
    _note_expression_output_capacity: usize,
    note_expression_output_count: *mut usize,
) -> c_int {
    if !midi_output_count.is_null() {
        unsafe {
            *midi_output_count = 0;
        }
    }
    if !note_expression_output_count.is_null() {
        unsafe {
            *note_expression_output_count = 0;
        }
    }

    if note_expression_input.is_null() && note_expression_input_count > 0 {
        return PluginError::NullPointer.into();
    }
    if note_expression_input_count > MAX_FFI_MIDI_EVENTS_PER_BLOCK {
        set_last_error("Too many Note Expression events for one FFI processing block");
        return PluginError::BufferTooSmall.into();
    }

    if handle.is_null()
        || input.is_null()
        || output.is_null()
        || (midi_input.is_null() && midi_input_count > 0)
    {
        return PluginError::NullPointer.into();
    }

    #[cfg(debug_assertions)]
    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        process_with_full_events_impl(
            handle,
            input,
            output,
            num_frames,
            midi_input,
            midi_input_count,
            note_expression_input,
            note_expression_input_count,
            _midi_output,
            _midi_output_capacity,
            midi_output_count,
            _note_expression_output,
            _note_expression_output_capacity,
            note_expression_output_count,
        )
    }));

    #[cfg(not(debug_assertions))]
    let result: Result<PluginError, ()> = Ok(unsafe {
        process_with_full_events_impl(
            handle,
            input,
            output,
            num_frames,
            midi_input,
            midi_input_count,
            note_expression_input,
            note_expression_input_count,
            _midi_output,
            _midi_output_capacity,
            midi_output_count,
            _note_expression_output,
            _note_expression_output_capacity,
            note_expression_output_count,
        )
    });

    result
        .unwrap_or_else(|_| {
            set_last_error("Panic in plugin_process_with_events");
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
    context: &ProcessContext<'_>,
) -> PluginError {
    // SAFETY: Caller must ensure handle is valid and non-null
    let handle_ref = unsafe { &mut *handle };

    let input_samples = num_frames * handle_ref.input_channels;
    let output_samples = num_frames * handle_ref.output_channels;

    // SAFETY: Caller must ensure input/output pointers are valid with correct sizes
    let input_slice = unsafe { slice::from_raw_parts(input, input_samples) };
    let output_slice = unsafe { slice::from_raw_parts_mut(output, output_samples) };

    match handle_ref
        .plugin
        .process(input_slice, output_slice, context)
    {
        Ok(_) => PluginError::Success,
        Err(_) => PluginError::ProcessingFailed,
    }
}

#[inline]
unsafe fn process_with_ffi_events_impl(
    handle: *mut PluginHandle,
    input: *const f32,
    output: *mut f32,
    num_frames: usize,
    midi_events: *const PluginMidiEvent,
    midi_event_count: usize,
) -> PluginError {
    unsafe {
        process_with_ffi_and_note_expression_events_impl(
            handle,
            input,
            output,
            num_frames,
            midi_events,
            midi_event_count,
            ptr::null(),
            0,
        )
    }
}

#[inline]
unsafe fn process_with_ffi_and_note_expression_events_impl(
    handle: *mut PluginHandle,
    input: *const f32,
    output: *mut f32,
    num_frames: usize,
    midi_events: *const PluginMidiEvent,
    midi_event_count: usize,
    note_expression_events: *const PluginNoteExpressionEvent,
    note_expression_event_count: usize,
) -> PluginError {
    if midi_event_count > MAX_FFI_MIDI_EVENTS_PER_BLOCK {
        set_last_error("Too many MIDI events for one FFI processing block");
        return PluginError::BufferTooSmall;
    }
    if note_expression_event_count > MAX_FFI_MIDI_EVENTS_PER_BLOCK {
        set_last_error("Too many Note Expression events for one FFI processing block");
        return PluginError::BufferTooSmall;
    }
    if midi_events.is_null() && midi_event_count > 0 {
        return PluginError::NullPointer;
    }
    if note_expression_events.is_null() && note_expression_event_count > 0 {
        return PluginError::NullPointer;
    }

    let mut midi_storage =
        [MidiEvent::new(0, MidiMessage::new([0; 3], 0)); MAX_FFI_MIDI_EVENTS_PER_BLOCK];
    let midi_slice = if midi_event_count == 0 {
        &midi_storage[..0]
    } else {
        let ffi_events = unsafe { slice::from_raw_parts(midi_events, midi_event_count) };
        for (dst, src) in midi_storage.iter_mut().zip(ffi_events.iter()) {
            if src.len > 3 {
                set_last_error("Invalid MIDI event length");
                return PluginError::InvalidConfig;
            }
            *dst = MidiEvent::new(src.sample_offset, MidiMessage::new(src.data, src.len));
        }
        &midi_storage[..midi_event_count]
    };

    let mut note_expression_storage =
        [HostNoteExpressionEvent::new(0, 0, 0, 0, HostNoteExpressionKind::PitchBend, 0.0);
            MAX_FFI_MIDI_EVENTS_PER_BLOCK];
    let note_expression_slice = if note_expression_event_count == 0 {
        &note_expression_storage[..0]
    } else {
        let ffi_events =
            unsafe { slice::from_raw_parts(note_expression_events, note_expression_event_count) };
        for (dst, src) in note_expression_storage.iter_mut().zip(ffi_events.iter()) {
            *dst = HostNoteExpressionEvent::new(
                src.sample_offset,
                src.note_id,
                src.channel,
                src.note,
                host_note_expression_kind(src.expression),
                src.value,
            );
        }
        &note_expression_storage[..note_expression_event_count]
    };

    let sample_rate = unsafe { (*handle).sample_rate };
    let context =
        ProcessContext::new(sample_rate, num_frames).with_events(midi_slice, note_expression_slice);
    unsafe { process_impl(handle, input, output, num_frames, &context) }
}

fn host_note_expression_kind(kind: PluginNoteExpressionKind) -> HostNoteExpressionKind {
    match kind {
        PluginNoteExpressionKind::PitchBend => HostNoteExpressionKind::PitchBend,
        PluginNoteExpressionKind::Pressure => HostNoteExpressionKind::Pressure,
        PluginNoteExpressionKind::Timbre => HostNoteExpressionKind::Timbre,
        PluginNoteExpressionKind::Brightness => HostNoteExpressionKind::Brightness,
        PluginNoteExpressionKind::Volume => HostNoteExpressionKind::Volume,
        PluginNoteExpressionKind::Pan => HostNoteExpressionKind::Pan,
    }
}

#[inline]
unsafe fn process_with_full_events_impl(
    handle: *mut PluginHandle,
    input: *const f32,
    output: *mut f32,
    num_frames: usize,
    midi_events: *const PluginMidiEvent,
    midi_event_count: usize,
    note_expression_events: *const PluginNoteExpressionEvent,
    note_expression_event_count: usize,
    midi_output: *mut PluginMidiEvent,
    midi_output_capacity: usize,
    midi_output_count: *mut usize,
    note_expression_output: *mut PluginNoteExpressionEvent,
    note_expression_output_capacity: usize,
    note_expression_output_count: *mut usize,
) -> PluginError {
    let process_result = unsafe {
        process_with_ffi_and_note_expression_events_impl(
            handle,
            input,
            output,
            num_frames,
            midi_events,
            midi_event_count,
            note_expression_events,
            note_expression_event_count,
        )
    };
    if process_result != PluginError::Success {
        return process_result;
    }

    let handle_ref = unsafe { &mut *handle };
    let midi_result = copy_midi_output_events(
        &mut handle_ref.midi_output_events,
        midi_output,
        midi_output_capacity,
        midi_output_count,
    );
    if midi_result != PluginError::Success {
        return midi_result;
    }

    copy_note_expression_output_events(
        &mut handle_ref.note_expression_output_events,
        note_expression_output,
        note_expression_output_capacity,
        note_expression_output_count,
    )
}

fn copy_midi_output_events(
    queued: &mut Vec<PluginMidiEvent>,
    out: *mut PluginMidiEvent,
    capacity: usize,
    out_count: *mut usize,
) -> PluginError {
    if !out_count.is_null() {
        unsafe {
            *out_count = queued.len();
        }
    }
    if queued.is_empty() {
        return PluginError::Success;
    }
    if out.is_null() {
        set_last_error("NULL MIDI output buffer with queued events");
        return PluginError::NullPointer;
    }
    if capacity < queued.len() {
        set_last_error("MIDI output buffer is too small");
        return PluginError::BufferTooSmall;
    }

    unsafe {
        ptr::copy_nonoverlapping(queued.as_ptr(), out, queued.len());
    }
    queued.clear();
    PluginError::Success
}

fn copy_note_expression_output_events(
    queued: &mut Vec<PluginNoteExpressionEvent>,
    out: *mut PluginNoteExpressionEvent,
    capacity: usize,
    out_count: *mut usize,
) -> PluginError {
    if !out_count.is_null() {
        unsafe {
            *out_count = queued.len();
        }
    }
    if queued.is_empty() {
        return PluginError::Success;
    }
    if out.is_null() {
        set_last_error("NULL Note Expression output buffer with queued events");
        return PluginError::NullPointer;
    }
    if capacity < queued.len() {
        set_last_error("Note Expression output buffer is too small");
        return PluginError::BufferTooSmall;
    }

    unsafe {
        ptr::copy_nonoverlapping(queued.as_ptr(), out, queued.len());
    }
    queued.clear();
    PluginError::Success
}

/// Clear queued MIDI and Note Expression output events.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_clear_output_events(handle: *mut PluginHandle) -> c_int {
    if handle.is_null() {
        return PluginError::InvalidHandle.into();
    }
    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        let handle_ref = &mut *handle;
        handle_ref.midi_output_events.clear();
        handle_ref.note_expression_output_events.clear();
        PluginError::Success
    }));
    result.unwrap_or(PluginError::UnknownError).into()
}

/// Queue one outgoing MIDI event for the next event-aware process call.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_enqueue_midi_output_event(
    handle: *mut PluginHandle,
    event: PluginMidiEvent,
) -> c_int {
    if handle.is_null() {
        return PluginError::InvalidHandle.into();
    }
    if event.len > 3 {
        set_last_error("Invalid outgoing MIDI event length");
        return PluginError::InvalidConfig.into();
    }
    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        let handle_ref = &mut *handle;
        if handle_ref.midi_output_events.len() >= MAX_FFI_OUTPUT_EVENTS_PER_BLOCK {
            set_last_error("MIDI output queue is full");
            return PluginError::BufferTooSmall;
        }
        handle_ref.midi_output_events.push(event);
        PluginError::Success
    }));
    result.unwrap_or(PluginError::UnknownError).into()
}

/// Queue one outgoing Note Expression event for the next event-aware process call.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_enqueue_note_expression_output_event(
    handle: *mut PluginHandle,
    event: PluginNoteExpressionEvent,
) -> c_int {
    if handle.is_null() {
        return PluginError::InvalidHandle.into();
    }
    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        let handle_ref = &mut *handle;
        if handle_ref.note_expression_output_events.len() >= MAX_FFI_OUTPUT_EVENTS_PER_BLOCK {
            set_last_error("Note Expression output queue is full");
            return PluginError::BufferTooSmall;
        }
        handle_ref.note_expression_output_events.push(event);
        PluginError::Success
    }));
    result.unwrap_or(PluginError::UnknownError).into()
}

/// Copy queued outgoing MIDI events without processing audio.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_get_midi_output_events(
    handle: *mut PluginHandle,
    out: *mut PluginMidiEvent,
    capacity: usize,
    out_count: *mut usize,
) -> c_int {
    if handle.is_null() {
        return PluginError::InvalidHandle.into();
    }
    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        let handle_ref = &mut *handle;
        copy_midi_output_events(&mut handle_ref.midi_output_events, out, capacity, out_count)
    }));
    result.unwrap_or(PluginError::UnknownError).into()
}

/// Copy queued outgoing Note Expression events without processing audio.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_get_note_expression_output_events(
    handle: *mut PluginHandle,
    out: *mut PluginNoteExpressionEvent,
    capacity: usize,
    out_count: *mut usize,
) -> c_int {
    if handle.is_null() {
        return PluginError::InvalidHandle.into();
    }
    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        let handle_ref = &mut *handle;
        copy_note_expression_output_events(
            &mut handle_ref.note_expression_output_events,
            out,
            capacity,
            out_count,
        )
    }));
    result.unwrap_or(PluginError::UnknownError).into()
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
pub extern "C" fn plugin_save_state(handle: *const PluginHandle, out_len: *mut usize) -> *mut u8 {
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

/// Export a named preset document as JSON bytes.
///
/// The returned buffer uses the preset document schema advertised by
/// plugin_preset_document_info() and must be freed with plugin_free_state().
#[unsafe(no_mangle)]
pub extern "C" fn plugin_export_preset_json(
    handle: *const PluginHandle,
    preset_name: *const c_char,
    out_len: *mut usize,
) -> *mut u8 {
    if handle.is_null() || out_len.is_null() {
        return ptr::null_mut();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        let handle_ref = &*handle;
        let name = if preset_name.is_null() {
            "Untitled"
        } else {
            CStr::from_ptr(preset_name).to_str().unwrap_or("Untitled")
        };
        let state = plugins_bridge::state::save_state(&*handle_ref.plugin);
        let info = handle_ref.plugin.info();
        let document = serde_json::json!({
            "schema_version": 1,
            "ut_type": "org.spinorama.sotf.plugin-preset",
            "file_extension": "sotfpreset",
            "preset_name": name,
            "plugin_type": handle_ref.plugin_type,
            "plugin_name": info.name,
            "plugin_version": info.version,
            "state": state,
        });
        let bytes = match serde_json::to_vec(&document) {
            Ok(bytes) => bytes,
            Err(err) => {
                set_last_error(&format!("Failed to serialize preset document: {err}"));
                return ptr::null_mut();
            }
        };
        copy_bytes_to_ffi_buffer(&bytes, out_len)
    }));

    result.unwrap_or_else(|_| {
        set_last_error("Panic in plugin_export_preset_json");
        ptr::null_mut()
    })
}

/// Import a JSON preset document created by plugin_export_preset_json().
#[unsafe(no_mangle)]
pub extern "C" fn plugin_import_preset_json(
    handle: *mut PluginHandle,
    data: *const u8,
    len: usize,
) -> c_int {
    if handle.is_null() || data.is_null() {
        set_last_error("NULL pointer in plugin_import_preset_json");
        return PluginError::NullPointer.into();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        let handle_ref = &mut *handle;
        let slice = slice::from_raw_parts(data, len);
        let document: serde_json::Value = match serde_json::from_slice(slice) {
            Ok(document) => document,
            Err(err) => {
                set_last_error(&format!("Invalid preset JSON: {err}"));
                return PluginError::InvalidConfig;
            }
        };

        let Some(state_values) = document.get("state").and_then(|state| state.as_array()) else {
            set_last_error("Preset JSON is missing a state byte array");
            return PluginError::InvalidConfig;
        };

        let mut state = Vec::with_capacity(state_values.len());
        for value in state_values {
            let Some(byte) = value.as_u64().and_then(|v| u8::try_from(v).ok()) else {
                set_last_error("Preset state contains a non-byte value");
                return PluginError::InvalidConfig;
            };
            state.push(byte);
        }

        match plugins_bridge::state::load_state(&mut *handle_ref.plugin, &state) {
            Ok(_) => PluginError::Success,
            Err(e) => {
                set_last_error(&format!("Failed to import preset state: {e}"));
                PluginError::InvalidConfig
            }
        }
    }));

    result
        .unwrap_or_else(|_| {
            set_last_error("Panic in plugin_import_preset_json");
            PluginError::UnknownError
        })
        .into()
}

/// Suggest a filesystem-safe preset filename.
///
/// Caller must free the returned string with plugin_free_string().
#[unsafe(no_mangle)]
pub extern "C" fn plugin_suggest_preset_filename(
    handle: *const PluginHandle,
    preset_name: *const c_char,
) -> *mut c_char {
    if handle.is_null() {
        return ptr::null_mut();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        let handle_ref = &*handle;
        let name = if preset_name.is_null() {
            "Untitled"
        } else {
            CStr::from_ptr(preset_name).to_str().unwrap_or("Untitled")
        };
        let plugin = sanitize_filename_component(&handle_ref.plugin_type);
        let preset = sanitize_filename_component(name);
        CString::new(format!("{plugin}-{preset}.sotfpreset"))
            .map(CString::into_raw)
            .unwrap_or(ptr::null_mut())
    }));

    result.unwrap_or(ptr::null_mut())
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

fn copy_bytes_to_ffi_buffer(bytes: &[u8], out_len: *mut usize) -> *mut u8 {
    let len = bytes.len();
    let buf = libc_malloc(len);
    if buf.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), buf, len);
        *out_len = len;
    }
    buf
}

fn libc_free(ptr: *mut u8, len: usize) {
    let layout = std::alloc::Layout::from_size_align(len, 1).unwrap();
    unsafe { std::alloc::dealloc(ptr, layout) }
}

fn sanitize_filename_component(input: &str) -> String {
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

// ============================================================================
// GPUI AU Plugin View (with real plugin UI instead of placeholder)
// ============================================================================

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
    param_cache: *mut param_cache::AtomicParamCache,
    set_param_cb: au_host::SetParamCallback,
    reset_param_cb: au_host::ResetParamCallback,
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
        std::sync::Arc::from_raw(param_cache as *const param_cache::AtomicParamCache)
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
                    au_host::AuHostState::new(
                        cache.clone(),
                        set_param_cb,
                        reset_param_cb,
                        cb_userdata,
                        pt,
                    )
                });
                entity.update(cx, |state: &mut au_host::AuHostState, _| {
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

    #[test]
    fn test_capabilities_and_preset_metadata() {
        let caps = plugin_ffi_capabilities();
        assert_eq!(caps.abi_version, SOTF_PLUGIN_FFI_ABI_VERSION);
        assert!(caps.supports_audio);
        assert!(caps.supports_midi_input);
        assert!(caps.supports_midi_output);
        assert!(caps.supports_note_expression);
        assert!(caps.supports_preset_documents);

        let preset = plugin_preset_document_info();
        assert_eq!(preset.schema_version, 1);
        assert!(!preset.ut_type.is_null());
        let ut_type = unsafe { CStr::from_ptr(preset.ut_type) }.to_str().unwrap();
        assert_eq!(ut_type, "org.spinorama.sotf.plugin-preset");

        let vst3 = plugin_vst3_ffi_descriptor();
        assert_eq!(vst3.abi_version, SOTF_PLUGIN_FFI_ABI_VERSION);
        assert!(!vst3.entrypoint.is_null());

        let swift = plugin_swift_package_info();
        assert!(swift.supports_staticlib);
        assert!(swift.supports_xcframework);
    }

    #[test]
    fn test_process_with_midi_null_safety() {
        let err = plugin_process_with_midi(
            ptr::null_mut(),
            ptr::null(),
            ptr::null_mut(),
            0,
            ptr::null(),
            0,
        );
        assert_eq!(err, PluginError::NullPointer as c_int);
    }

    #[test]
    fn test_output_event_queue_roundtrip() {
        let plugin_type = CString::new("EQ").unwrap();
        let config = CString::new(r#"{"filters": []}"#).unwrap();
        let handle = plugin_create(plugin_type.as_ptr(), config.as_ptr(), 48000, 2, 2);
        assert!(!handle.is_null());

        let midi = PluginMidiEvent {
            sample_offset: 4,
            data: [0x90, 60, 100],
            len: 3,
        };
        assert_eq!(plugin_enqueue_midi_output_event(handle, midi), 0);

        let note_expression = PluginNoteExpressionEvent {
            sample_offset: 8,
            note_id: 1,
            channel: 0,
            note: 60,
            expression: PluginNoteExpressionKind::PitchBend,
            value: 0.25,
        };
        assert_eq!(
            plugin_enqueue_note_expression_output_event(handle, note_expression),
            0
        );

        let mut midi_out = [PluginMidiEvent {
            sample_offset: 0,
            data: [0; 3],
            len: 0,
        }];
        let mut midi_count = 0;
        assert_eq!(
            plugin_get_midi_output_events(handle, midi_out.as_mut_ptr(), 1, &mut midi_count),
            0
        );
        assert_eq!(midi_count, 1);
        assert_eq!(midi_out[0], midi);

        let mut note_out = [PluginNoteExpressionEvent {
            sample_offset: 0,
            note_id: 0,
            channel: 0,
            note: 0,
            expression: PluginNoteExpressionKind::PitchBend,
            value: 0.0,
        }];
        let mut note_count = 0;
        assert_eq!(
            plugin_get_note_expression_output_events(
                handle,
                note_out.as_mut_ptr(),
                1,
                &mut note_count,
            ),
            0
        );
        assert_eq!(note_count, 1);
        assert_eq!(note_out[0], note_expression);

        plugin_destroy(handle);
    }

    #[test]
    fn test_process_with_events_accepts_inputs_and_copies_outputs() {
        let plugin_type = CString::new("EQ").unwrap();
        let config = CString::new(r#"{"filters": []}"#).unwrap();
        let handle = plugin_create(plugin_type.as_ptr(), config.as_ptr(), 48000, 2, 2);
        assert!(!handle.is_null());

        let queued_midi = PluginMidiEvent {
            sample_offset: 2,
            data: [0x90, 64, 110],
            len: 3,
        };
        let queued_note_expression = PluginNoteExpressionEvent {
            sample_offset: 3,
            note_id: 42,
            channel: 0,
            note: 64,
            expression: PluginNoteExpressionKind::Pressure,
            value: 0.75,
        };
        assert_eq!(plugin_enqueue_midi_output_event(handle, queued_midi), 0);
        assert_eq!(
            plugin_enqueue_note_expression_output_event(handle, queued_note_expression),
            0
        );

        let input = [0.0_f32; 16];
        let mut output = [0.0_f32; 16];
        let midi_input = [PluginMidiEvent {
            sample_offset: 1,
            data: [0x90, 60, 100],
            len: 3,
        }];
        let note_expression_input = [PluginNoteExpressionEvent {
            sample_offset: 1,
            note_id: 1,
            channel: 0,
            note: 60,
            expression: PluginNoteExpressionKind::PitchBend,
            value: 0.1,
        }];
        let mut midi_output = [PluginMidiEvent {
            sample_offset: 0,
            data: [0; 3],
            len: 0,
        }];
        let mut note_expression_output = [PluginNoteExpressionEvent {
            sample_offset: 0,
            note_id: 0,
            channel: 0,
            note: 0,
            expression: PluginNoteExpressionKind::PitchBend,
            value: 0.0,
        }];
        let mut midi_output_count = 0;
        let mut note_expression_output_count = 0;

        assert_eq!(
            plugin_process_with_events(
                handle,
                input.as_ptr(),
                output.as_mut_ptr(),
                8,
                midi_input.as_ptr(),
                midi_input.len(),
                note_expression_input.as_ptr(),
                note_expression_input.len(),
                midi_output.as_mut_ptr(),
                midi_output.len(),
                &mut midi_output_count,
                note_expression_output.as_mut_ptr(),
                note_expression_output.len(),
                &mut note_expression_output_count,
            ),
            0
        );
        assert_eq!(midi_output_count, 1);
        assert_eq!(midi_output[0], queued_midi);
        assert_eq!(note_expression_output_count, 1);
        assert_eq!(note_expression_output[0], queued_note_expression);

        plugin_destroy(handle);
    }

    #[test]
    fn test_preset_json_export_import_and_filename() {
        let plugin_type = CString::new("EQ").unwrap();
        let config = CString::new(r#"{"filters": []}"#).unwrap();
        let handle = plugin_create(plugin_type.as_ptr(), config.as_ptr(), 48000, 2, 2);
        assert!(!handle.is_null());

        let name = CString::new("Warm Room / A").unwrap();
        let mut len = 0;
        let preset = plugin_export_preset_json(handle, name.as_ptr(), &mut len);
        assert!(!preset.is_null());
        assert!(len > 0);
        assert_eq!(plugin_import_preset_json(handle, preset, len), 0);
        plugin_free_state(preset, len);

        let filename = plugin_suggest_preset_filename(handle, name.as_ptr());
        assert!(!filename.is_null());
        let filename_str = unsafe { CStr::from_ptr(filename) }.to_str().unwrap();
        assert_eq!(filename_str, "EQ-Warm-Room-A.sotfpreset");
        plugin_free_string(filename);

        plugin_destroy(handle);
    }
}
