use super::PluginError;
use super::PluginMidiEvent;
use super::PluginNoteExpressionEvent;
use super::libc::libc_malloc;
use super::misc::set_last_error_static;
use std::ptr;

pub(super) fn copy_midi_output_events(
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
        set_last_error_static(c"NULL MIDI output buffer with queued events");
        return PluginError::NullPointer;
    }
    if capacity < queued.len() {
        set_last_error_static(c"MIDI output buffer is too small");
        return PluginError::BufferTooSmall;
    }

    unsafe {
        ptr::copy_nonoverlapping(queued.as_ptr(), out, queued.len());
    }
    queued.clear();
    PluginError::Success
}

pub(super) fn copy_note_expression_output_events(
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
        set_last_error_static(c"NULL Note Expression output buffer with queued events");
        return PluginError::NullPointer;
    }
    if capacity < queued.len() {
        set_last_error_static(c"Note Expression output buffer is too small");
        return PluginError::BufferTooSmall;
    }

    unsafe {
        ptr::copy_nonoverlapping(queued.as_ptr(), out, queued.len());
    }
    queued.clear();
    PluginError::Success
}

pub(super) fn copy_bytes_to_ffi_buffer(bytes: &[u8], out_len: *mut usize) -> *mut u8 {
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
