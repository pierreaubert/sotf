//! C-callable plugin lifecycle and processing entry points.
//!
//! Every exported function follows the same ownership contract: pointers
//! received from C are borrowed for the duration of the call, pointers
//! returned as owned values must be freed with the matching `plugin_free_*`
//! function exactly once, and any pointer derived from a [`PluginHandle`] is
//! only valid while that handle remains alive and undestroyed.

use super::PluginError;
use super::PluginFfiCapabilities;
use super::PluginHandle;
use super::PluginMidiEvent;
use super::PluginNoteExpressionEvent;
use super::PluginPresetDocumentInfo;
use super::PluginSwiftPackageInfo;
use super::PluginVst3FfiDescriptor;
use super::consts::MAX_FFI_MIDI_EVENTS_PER_BLOCK;
use super::consts::MAX_FFI_OUTPUT_EVENTS_PER_BLOCK;
use super::consts::MAX_PRESET_JSON_IMPORT_BYTES;
use super::consts::MAX_PRESET_STATE_BYTES;
use super::consts::PRESET_FILE_EXTENSION;
use super::consts::PRESET_MIME_TYPE;
use super::consts::PRESET_UT_TYPE;
use super::consts::SOTF_PLUGIN_FFI_ABI_VERSION;
use super::consts::SWIFT_HEADER_NAME;
use super::consts::SWIFT_LIBRARY_NAME;
use super::consts::SWIFT_PACKAGE_NAME;
use super::consts::SWIFT_PRODUCT_NAME;
use super::consts::SWIFT_TARGET_NAME;
use super::consts::VST3_COMPONENT_NAME;
use super::consts::VST3_ENTRYPOINT;
use super::consts::VST3_SDK_VERSION;
use super::consts::VST3_VENDOR;
use super::copy::copy_bytes_to_ffi_buffer;
use super::copy::copy_midi_output_events;
use super::copy::copy_note_expression_output_events;
use super::host::host_kind_name;
use super::libc::libc_free;
use super::libc::libc_malloc;
use super::misc::sanitize_filename_component;
use super::misc::set_last_error;
pub use super::parameter_map::{ParameterInfo, ParameterMap};
use super::process::process_impl;
use super::process::process_with_ffi_events_impl;
use super::process::process_with_full_events_impl;
use super::types::current_host_kind;
use super::{LAST_ERROR, LAST_STATIC_ERROR};
use sotf_host::plugin::ProcessContext;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_int};
use std::panic::{self, AssertUnwindSafe};
use std::ptr;
use std::slice;

fn replace_linear_phase_eq_from_state(
    handle: &mut PluginHandle,
    state: &[u8],
) -> Result<(), String> {
    // Match the generic state loader's partial-update contract: values omitted
    // from the incoming object retain their current live values, including
    // automation applied after the last structural reconstruction.
    let mut merged_state: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(&plugins_bridge::state::save_state(&*handle.plugin))
            .map_err(|error| format!("Failed to capture current LinearPhaseEQ state: {error}"))?;
    let incoming_state: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(state)
        .map_err(|error| format!("Failed to parse LinearPhaseEQ state: {error}"))?;
    if let Some(num_filters) = incoming_state
        .get("num_filters")
        .and_then(serde_json::Value::as_u64)
    {
        // A smaller incoming structure replaces, rather than preserves, the
        // now-out-of-range live bands. Explicit incoming out-of-range band
        // fields remain in the overlay and are rejected by constructor merge.
        merged_state.retain(|key, _| {
            key.strip_prefix("band_")
                .and_then(|rest| rest.split_once('_'))
                .and_then(|(index, _)| index.parse::<u64>().ok())
                .is_none_or(|index| index < num_filters)
        });
    }
    merged_state.extend(incoming_state);
    let merged_state = serde_json::to_vec(&merged_state)
        .map_err(|error| format!("Failed to merge LinearPhaseEQ state: {error}"))?;
    let rebuilt_config = super::plugin_factory::merge_linear_phase_eq_state_into_config(
        &handle.config_json,
        &merged_state,
    )?;
    let mut replacement = super::plugin_factory::create_plugin_with_max_callback(
        &handle.plugin_type,
        &rebuilt_config,
        handle.input_channels,
        handle.output_channels,
        handle.sample_rate,
        handle.max_callback_frames,
    )?;
    replacement.initialize(handle.sample_rate)?;

    // LinearPhaseEQ's FFI parameter map is built from static specs and the
    // maximum band template, so structural changes do not alter it. Retaining
    // the original allocation also preserves the documented lifetime of
    // ParameterInfo pointers returned to foreign callers.
    //
    // Commit only after parsing, construction, and initialization succeeded.
    // Any earlier error leaves the live plugin and constructor config intact.
    handle.plugin = replacement;
    handle.config_json = rebuilt_config;
    Ok(())
}

/// Get the last error message.
///
/// # Returns
/// * Pointer to a null-terminated C string, or `NULL` if no error has been set.
/// * The pointer points to thread-local storage that remains valid until the
///   next FFI call on the same thread that may set an error. Copy the contents
///   if you need it to outlive the next call.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_get_last_error() -> *const c_char {
    let static_error = LAST_STATIC_ERROR.with(std::cell::Cell::get);
    if !static_error.is_null() {
        return static_error;
    }
    LAST_ERROR.with(|e| {
        e.borrow()
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(ptr::null())
    })
}

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
/// # Returns
/// * JSON string owned by the caller. It must be released with
///   [`plugin_free_string`] when no longer needed.
/// * `NULL` on error.
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
///
/// The returned string pointers reference static, null-terminated C strings
/// with program lifetime. They must not be freed.
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
///
/// The returned string pointers reference static, null-terminated C strings
/// with program lifetime. They must not be freed.
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
///
/// The returned string pointers reference static, null-terminated C strings
/// with program lifetime. They must not be freed.
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
/// * `plugin_type` and `config_json` must be valid, null-terminated UTF-8 C strings
///   that remain readable for the duration of this call.
/// * The returned handle is owned by the caller and must be released with
///   [`plugin_destroy`] exactly once. The pointer is invalid after that call.
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

        // Canonicalize direct-format aliases once so factory policy,
        // parameter metadata, serialized identity, and adapter selection
        // cannot disagree.
        let plugin_type_str = super::plugin_factory::canonical_direct_plugin_type(plugin_type_str);

        let max_callback_frames =
            match super::plugin_factory::max_callback_frames_from_config(config_str) {
                Ok(value) => value,
                Err(error) => {
                    set_last_error(&error);
                    return ptr::null_mut();
                }
            };

        // Create plugin
        let mut plugin = match super::plugin_factory::create_plugin_with_max_callback(
            plugin_type_str,
            config_str,
            input_channels,
            output_channels,
            sample_rate,
            max_callback_frames,
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
            config_json: config_str.to_string(),
            parameter_map,
            sample_rate,
            max_callback_frames,
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
/// * `handle` must be a valid plugin handle returned by [`plugin_create`] that
///   has not been destroyed.
/// * `input` and `output` must be valid, properly aligned, non-overlapping
///   buffers that remain valid for the duration of this call.
/// * `input` must contain at least `num_frames * input_channels` samples and
///   `output` must have space for at least `num_frames * output_channels`
///   samples, where `input_channels` and `output_channels` are the values
///   passed to [`plugin_create`].
/// * This function is designed to be real-time safe (no allocations).
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
///
/// # Safety
/// * `handle`, `input`, and `output` must satisfy the same contract as
///   [`plugin_process`].
/// * `midi_events` must point to `midi_event_count` valid [`PluginMidiEvent`]
///   structs and remain readable for the duration of this call. It may be
///   `NULL` only when `midi_event_count` is `0`.
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
///
/// # Safety
/// * `handle`, `input`, and `output` must satisfy the same contract as
///   [`plugin_process`].
/// * `midi_input` must point to `midi_input_count` valid [`PluginMidiEvent`]
///   structs and remain readable for the duration of this call. It may be
///   `NULL` only when `midi_input_count` is `0`.
/// * `note_expression_input` must point to `note_expression_input_count` valid
///   [`PluginNoteExpressionEvent`] structs and remain readable for the duration
///   of this call. It may be `NULL` only when `note_expression_input_count` is `0`.
/// * If queued output events exist, `midi_output` and `note_expression_output`
///   must be valid writable buffers with capacities matching the supplied
///   `_capacity` values and remain writable for the duration of this call.
/// * `midi_output_count` and `note_expression_output_count` must be valid
///   pointers to `usize` values that this call may write.
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

/// Get parameter info by index.
///
/// # Returns
/// * Pointer to parameter info. The pointer is valid only while the
///   `PluginHandle` used to obtain it remains alive and has not been destroyed.
/// * `NULL` if the handle is invalid or the index is out of bounds.
///
/// # Safety
/// * `handle` must be a valid plugin handle that has not been destroyed.
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

/// Set a parameter value (normalized 0.0-1.0).
///
/// # Arguments
/// * `handle` - Plugin handle
/// * `param_id` - Parameter ID string
/// * `normalized_value` - Normalized value (0.0 = min, 1.0 = max)
///
/// # Returns
/// * 0 on success
/// * Error code on failure
///
/// # Safety
/// * `handle` must be a valid plugin handle that has not been destroyed.
/// * `param_id` must be a valid, null-terminated UTF-8 C string that remains
///   readable for the duration of this call.
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

/// Get a parameter value (normalized 0.0-1.0).
///
/// # Returns
/// * Normalized value on success
/// * -1.0 on error
///
/// # Safety
/// * `handle` must be a valid plugin handle that has not been destroyed.
/// * `param_id` must be a valid, null-terminated UTF-8 C string that remains
///   readable for the duration of this call.
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

/// Get plugin information as a JSON string.
///
/// # Returns
/// * JSON string owned by the caller. It must be released with
///   [`plugin_free_string`] when no longer needed.
/// * `NULL` on error.
///
/// # Safety
/// * `handle` must be a valid plugin handle that has not been destroyed.
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

/// Free a string returned by the plugin.
///
/// # Safety
/// * `s` must be either `NULL` or a pointer previously returned by a function
///   documented as returning an owned string (for example [`plugin_get_info_json`],
///   [`plugin_ffi_platform_info_json`], or [`plugin_suggest_preset_filename`]).
/// * The pointer must not be freed more than once.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}

/// Save plugin state to a JSON byte buffer.
///
/// # Returns
/// * Pointer to an allocated buffer owned by the caller on success. It must be
///   released with [`plugin_free_state`] when no longer needed.
/// * `NULL` on error.
///
/// # Safety
/// * `handle` must be a valid plugin handle that has not been destroyed.
/// * `out_len` must be a valid pointer to `usize` that this call may write.
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
/// * `handle` must be a valid plugin handle that has not been destroyed.
/// * `data` must point to `len` bytes of valid JSON that remain readable for
///   the duration of this call.
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

        let load_result = if handle_ref.plugin_type == "LinearPhaseEQ" {
            replace_linear_phase_eq_from_state(handle_ref, slice)
        } else {
            plugins_bridge::state::load_state(&mut *handle_ref.plugin, slice)
        };
        match load_result {
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

/// Free a state buffer returned by [`plugin_save_state`] or
/// [`plugin_export_preset_json`].
///
/// # Safety
/// * `data` must be either `NULL` or a pointer previously returned by a
///   function documented as returning an owned byte buffer, with the same
///   `len` value that was written to the associated `out_len` pointer.
/// * The pointer must not be freed more than once.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_free_state(data: *mut u8, len: usize) {
    if !data.is_null() && len > 0 {
        libc_free(data, len);
    }
}

/// Export a named preset document as JSON bytes.
///
/// The returned buffer uses the preset document schema advertised by
/// [`plugin_preset_document_info`] and must be freed with [`plugin_free_state`].
///
/// # Safety
/// * `handle` must be a valid plugin handle that has not been destroyed.
/// * `preset_name` may be `NULL` or must be a valid, null-terminated UTF-8 C
///   string that remains readable for the duration of this call.
/// * `out_len` must be a valid pointer to `usize` that this call may write.
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

/// Import a JSON preset document created by [`plugin_export_preset_json`].
///
/// # Safety
/// * `handle` must be a valid plugin handle that has not been destroyed.
/// * `data` must point to `len` bytes of valid preset JSON that remain readable
///   for the duration of this call.
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
    if len > MAX_PRESET_JSON_IMPORT_BYTES {
        set_last_error(&format!(
            "Preset JSON exceeds {} byte import limit",
            MAX_PRESET_JSON_IMPORT_BYTES
        ));
        return PluginError::InvalidConfig.into();
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
        if state_values.len() > MAX_PRESET_STATE_BYTES {
            set_last_error(&format!(
                "Preset state exceeds {} byte limit",
                MAX_PRESET_STATE_BYTES
            ));
            return PluginError::InvalidConfig;
        }

        let mut state = Vec::with_capacity(state_values.len());
        for value in state_values {
            let Some(byte) = value.as_u64().and_then(|v| u8::try_from(v).ok()) else {
                set_last_error("Preset state contains a non-byte value");
                return PluginError::InvalidConfig;
            };
            state.push(byte);
        }

        let load_result = if handle_ref.plugin_type == "LinearPhaseEQ" {
            replace_linear_phase_eq_from_state(handle_ref, &state)
        } else {
            plugins_bridge::state::load_state(&mut *handle_ref.plugin, &state)
        };
        match load_result {
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
/// # Returns
/// * Owned string that the caller must release with [`plugin_free_string`].
/// * `NULL` on error.
///
/// # Safety
/// * `handle` must be a valid plugin handle that has not been destroyed.
/// * `preset_name` may be `NULL` or must be a valid, null-terminated UTF-8 C
///   string that remains readable for the duration of this call.
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
/// * JSON array string owned by the caller. It must be released with
///   [`plugin_free_string`] when no longer needed.
/// * `NULL` on error.
#[unsafe(no_mangle)]
pub extern "C" fn plugin_available_types() -> *mut c_char {
    let types = plugins_bridge::factory::available_plugin_types();
    let json = serde_json::json!(types);
    match CString::new(json.to_string()) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}
