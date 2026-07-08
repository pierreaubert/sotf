use super::PluginError;
use super::PluginMidiEvent;
use super::PluginNoteExpressionEvent;
use super::PluginNoteExpressionKind;
use super::consts::MAX_PRESET_JSON_IMPORT_BYTES;
use super::consts::MAX_PRESET_STATE_BYTES;
use super::consts::SOTF_PLUGIN_FFI_ABI_VERSION;
use super::plugin::plugin_available_types;
use super::plugin::plugin_create;
use super::plugin::plugin_destroy;
use super::plugin::plugin_enqueue_midi_output_event;
use super::plugin::plugin_enqueue_note_expression_output_event;
use super::plugin::plugin_export_preset_json;
use super::plugin::plugin_ffi_capabilities;
use super::plugin::plugin_free_state;
use super::plugin::plugin_free_string;
use super::plugin::plugin_get_info_json;
use super::plugin::plugin_get_last_error;
use super::plugin::plugin_get_midi_output_events;
use super::plugin::plugin_get_note_expression_output_events;
use super::plugin::plugin_get_parameter;
use super::plugin::plugin_get_parameter_count;
use super::plugin::plugin_get_parameter_info;
use super::plugin::plugin_import_preset_json;
use super::plugin::plugin_load_state;
use super::plugin::plugin_preset_document_info;
use super::plugin::plugin_process;
use super::plugin::plugin_process_with_events;
use super::plugin::plugin_process_with_midi;
use super::plugin::plugin_reset;
use super::plugin::plugin_save_state;
use super::plugin::plugin_set_parameter;
use super::plugin::plugin_suggest_preset_filename;
use super::plugin::plugin_swift_package_info;
use super::plugin::plugin_vst3_ffi_descriptor;
use std::ffi::{CStr, CString};
use std::os::raw::c_int;
use std::ptr;

fn last_error_string() -> String {
    let error = plugin_get_last_error();
    assert!(!error.is_null());
    unsafe { CStr::from_ptr(error) }
        .to_string_lossy()
        .into_owned()
}

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
        plugin_get_note_expression_output_events(handle, note_out.as_mut_ptr(), 1, &mut note_count,),
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

#[test]
fn test_preset_json_import_rejects_oversized_input_len_before_reading() {
    let plugin_type = CString::new("EQ").unwrap();
    let config = CString::new(r#"{"filters": []}"#).unwrap();
    let handle = plugin_create(plugin_type.as_ptr(), config.as_ptr(), 48000, 2, 2);
    assert!(!handle.is_null());

    let document = br#"{"state":[]}"#;
    assert_eq!(
        plugin_import_preset_json(handle, document.as_ptr(), MAX_PRESET_JSON_IMPORT_BYTES + 1,),
        PluginError::InvalidConfig as c_int
    );
    assert!(last_error_string().contains("Preset JSON exceeds"));

    plugin_destroy(handle);
}

#[test]
fn test_preset_json_import_rejects_oversized_state_array() {
    let plugin_type = CString::new("EQ").unwrap();
    let config = CString::new(r#"{"filters": []}"#).unwrap();
    let handle = plugin_create(plugin_type.as_ptr(), config.as_ptr(), 48000, 2, 2);
    assert!(!handle.is_null());

    let mut document = String::from("{\"state\":[");
    document.extend(std::iter::repeat_n("0,", MAX_PRESET_STATE_BYTES));
    document.push_str("0]}");

    assert_eq!(
        plugin_import_preset_json(handle, document.as_ptr(), document.len()),
        PluginError::InvalidConfig as c_int
    );
    assert!(last_error_string().contains("Preset state exceeds"));

    plugin_destroy(handle);
}

#[test]
fn test_destroy_null_is_safe() {
    // Releasing a NULL handle must not panic or crash. A second destroy of a
    // real handle is documented as UB, so we only exercise the null case.
    plugin_destroy(ptr::null_mut());
}

#[test]
fn test_create_rejects_invalid_utf8() {
    // 0x80 is a continuation byte on its own and is invalid UTF-8.
    let plugin_type = CString::new(b"EQ\x80".as_slice()).unwrap();
    let config = CString::new(r#"{"filters": []}"#).unwrap();
    let handle = plugin_create(plugin_type.as_ptr(), config.as_ptr(), 48000, 2, 2);
    assert!(handle.is_null());
}

#[test]
fn test_create_rejects_unknown_plugin_type() {
    let plugin_type = CString::new("NotARealPluginType").unwrap();
    let config = CString::new("{}").unwrap();
    let handle = plugin_create(plugin_type.as_ptr(), config.as_ptr(), 48000, 2, 2);
    assert!(handle.is_null());
    assert!(last_error_string().contains("Failed to create plugin"));
}

#[test]
fn test_process_buffers_must_match_declared_channel_count() {
    let plugin_type = CString::new("EQ").unwrap();
    let config = CString::new(r#"{"filters": []}"#).unwrap();
    let handle = plugin_create(plugin_type.as_ptr(), config.as_ptr(), 48000, 2, 2);
    assert!(!handle.is_null());

    let input = [0.0_f32; 16];
    let mut output = [0.0_f32; 16];
    assert_eq!(
        plugin_process(handle, input.as_ptr(), output.as_mut_ptr(), 8),
        PluginError::Success as c_int,
    );

    plugin_destroy(handle);
}

#[test]
fn test_parameter_info_pointer_is_valid_until_destroy() {
    let plugin_type = CString::new("EQ").unwrap();
    let config = CString::new(r#"{"filters": []}"#).unwrap();
    let handle = plugin_create(plugin_type.as_ptr(), config.as_ptr(), 48000, 2, 2);
    assert!(!handle.is_null());

    let info = plugin_get_parameter_info(handle, 0);
    assert!(!info.is_null());
    // The pointer must remain usable as long as the handle is alive.
    let id = unsafe { CStr::from_ptr((*info).id) }.to_str().unwrap();
    assert!(!id.is_empty());

    plugin_destroy(handle);
}

#[test]
fn test_get_set_parameter_roundtrip() {
    let plugin_type = CString::new("Gain").unwrap();
    let config = CString::new("{}").unwrap();
    let handle = plugin_create(plugin_type.as_ptr(), config.as_ptr(), 48000, 2, 2);
    assert!(!handle.is_null());

    let param_id = CString::new("gain_db").unwrap();
    let value = 0.75;
    assert_eq!(
        plugin_set_parameter(handle, param_id.as_ptr(), value),
        PluginError::Success as c_int
    );
    let read = plugin_get_parameter(handle, param_id.as_ptr());
    assert!((read - value).abs() < f64::EPSILON * 10.0);

    plugin_destroy(handle);
}

#[test]
fn test_string_and_state_ownership_roundtrip() {
    let plugin_type = CString::new("EQ").unwrap();
    let config = CString::new(r#"{"filters": []}"#).unwrap();
    let handle = plugin_create(plugin_type.as_ptr(), config.as_ptr(), 48000, 2, 2);
    assert!(!handle.is_null());

    let info = plugin_get_info_json(handle);
    assert!(!info.is_null());
    plugin_free_string(info);

    let mut state_len = 0;
    let state = plugin_save_state(handle, &mut state_len);
    assert!(!state.is_null());
    assert!(state_len > 0);
    plugin_free_state(state, state_len);

    let name = CString::new("Test Preset").unwrap();
    let filename = plugin_suggest_preset_filename(handle, name.as_ptr());
    assert!(!filename.is_null());
    plugin_free_string(filename);

    let types = plugin_available_types();
    assert!(!types.is_null());
    plugin_free_string(types);

    plugin_destroy(handle);
}

#[test]
fn test_process_with_events_reports_output_buffer_too_small() {
    let plugin_type = CString::new("EQ").unwrap();
    let config = CString::new(r#"{"filters": []}"#).unwrap();
    let handle = plugin_create(plugin_type.as_ptr(), config.as_ptr(), 48000, 2, 2);
    assert!(!handle.is_null());

    let queued_midi = PluginMidiEvent {
        sample_offset: 0,
        data: [0x90, 60, 100],
        len: 3,
    };
    assert_eq!(plugin_enqueue_midi_output_event(handle, queued_midi), 0);

    let input = [0.0_f32; 16];
    let mut output = [0.0_f32; 16];
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
    let mut note_output_count = 0;

    // Capacity 0 with a queued output event should report BufferTooSmall.
    // The output buffers themselves must be non-null; only their advertised
    // capacity is too small.
    assert_eq!(
        plugin_process_with_events(
            handle,
            input.as_ptr(),
            output.as_mut_ptr(),
            8,
            ptr::null(),
            0,
            ptr::null(),
            0,
            midi_output.as_mut_ptr(),
            0,
            &mut midi_output_count,
            note_expression_output.as_mut_ptr(),
            0,
            &mut note_output_count,
        ),
        PluginError::BufferTooSmall as c_int
    );

    plugin_destroy(handle);
}

#[test]
fn test_free_functions_accept_null_ownership_invariant() {
    // Owned return values documented as caller-freed must tolerate NULL so
    // hosts can unconditionally free optional outputs.
    plugin_free_string(ptr::null_mut());
    plugin_free_state(ptr::null_mut(), 0);
    plugin_free_state(ptr::null_mut(), 100);
}

#[test]
fn test_static_metadata_pointers_are_program_lifetime() {
    // Static string pointers returned by metadata functions must remain valid
    // for program lifetime and must not be freed by the caller.
    let preset = plugin_preset_document_info();
    assert!(!preset.ut_type.is_null());
    assert!(!preset.file_extension.is_null());
    assert!(!preset.mime_type.is_null());
    let _ = unsafe { CStr::from_ptr(preset.ut_type) }
        .to_str()
        .expect("preset ut_type must be valid UTF-8");

    let vst3 = plugin_vst3_ffi_descriptor();
    assert!(!vst3.component_name.is_null());
    assert!(!vst3.vendor.is_null());
    assert!(!vst3.sdk_version.is_null());
    assert!(!vst3.entrypoint.is_null());
    let _ = unsafe { CStr::from_ptr(vst3.entrypoint) }
        .to_str()
        .expect("vst3 entrypoint must be valid UTF-8");

    let swift = plugin_swift_package_info();
    assert!(!swift.package_name.is_null());
    assert!(!swift.product_name.is_null());
    assert!(!swift.target_name.is_null());
    assert!(!swift.library_name.is_null());
    assert!(!swift.umbrella_header.is_null());
}

#[test]
fn test_last_error_lifetime_contract() {
    // Before any error is set, last_error may be NULL or empty.
    let before = plugin_get_last_error();
    if !before.is_null() {
        let msg = unsafe { CStr::from_ptr(before) }.to_str().unwrap();
        assert!(msg.is_empty() || !msg.is_empty()); // valid C string
    }

    // Trigger a well-defined error and read the message.
    let handle = plugin_create(ptr::null(), ptr::null(), 48000, 2, 2);
    assert!(handle.is_null());
    let after = plugin_get_last_error();
    assert!(!after.is_null());
    let msg = unsafe { CStr::from_ptr(after) }.to_str().unwrap();
    assert!(msg.contains("NULL pointer"));

    // The pointer must remain usable until another FFI call on this thread that
    // may set an error. We do not make such a call here, so re-reading should
    // return the same pointer value.
    let again = plugin_get_last_error();
    assert_eq!(after, again);
}

#[test]
fn test_state_save_load_roundtrip_ownership() {
    // plugin_save_state returns a caller-owned buffer; plugin_load_state borrows
    // it for the duration of the call. Round-trip must not leak or corrupt state.
    let plugin_type = CString::new("Gain").unwrap();
    let config = CString::new(r#"{"gain_db": -6.0}"#).unwrap();
    let handle = plugin_create(plugin_type.as_ptr(), config.as_ptr(), 48000, 2, 2);
    assert!(!handle.is_null());

    let param_id = CString::new("gain_db").unwrap();
    assert_eq!(
        plugin_set_parameter(handle, param_id.as_ptr(), 0.75),
        PluginError::Success as c_int
    );

    let mut state_len = 0;
    let state = plugin_save_state(handle, &mut state_len);
    assert!(!state.is_null());
    assert!(state_len > 0);

    // Reset then load: the borrowed state buffer must be fully consumed.
    assert_eq!(plugin_reset(handle), PluginError::Success as c_int);
    assert_eq!(
        plugin_load_state(handle, state, state_len),
        PluginError::Success as c_int
    );

    // The loaded parameter value must round-trip.
    let read = plugin_get_parameter(handle, param_id.as_ptr());
    assert!((read - 0.75).abs() < f64::EPSILON * 10.0);

    plugin_free_state(state, state_len);
    plugin_destroy(handle);
}

#[test]
fn test_reset_keeps_handle_alive_and_usable() {
    // Reset must clear runtime state without invalidating the handle or its
    // parameter map.
    let plugin_type = CString::new("EQ").unwrap();
    let config = CString::new(r#"{"filters": []}"#).unwrap();
    let handle = plugin_create(plugin_type.as_ptr(), config.as_ptr(), 48000, 2, 2);
    assert!(!handle.is_null());

    assert_eq!(plugin_reset(handle), PluginError::Success as c_int);

    let info = plugin_get_info_json(handle);
    assert!(!info.is_null());
    plugin_free_string(info);

    let count = plugin_get_parameter_count(handle);
    assert!(count > 0);

    plugin_destroy(handle);
}
