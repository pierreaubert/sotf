use super::PluginError;
use super::PluginMidiEvent;
use super::PluginNoteExpressionEvent;
use super::libc::libc_free;
use super::libc::libc_malloc;
use super::misc::set_last_error_static;
use std::ptr;

/// Write `value` to `out_count` when the pointer is non-null.
///
/// `out_count` is an optional out-parameter: hosts may pass NULL when they do
/// not need the count. Error paths report a count of 0 so callers never act on
/// a stale or speculative count; the pending-event count is only published
/// alongside `Success`.
fn set_out_count(out_count: *mut usize, value: usize) {
    if !out_count.is_null() {
        unsafe {
            *out_count = value;
        }
    }
}

pub(super) fn copy_midi_output_events(
    queued: &mut Vec<PluginMidiEvent>,
    out: *mut PluginMidiEvent,
    capacity: usize,
    out_count: *mut usize,
) -> PluginError {
    if queued.is_empty() {
        set_out_count(out_count, 0);
        return PluginError::Success;
    }
    if out.is_null() {
        set_last_error_static(c"NULL MIDI output buffer with queued events");
        set_out_count(out_count, 0);
        return PluginError::NullPointer;
    }
    if capacity < queued.len() {
        set_last_error_static(c"MIDI output buffer is too small");
        set_out_count(out_count, 0);
        return PluginError::BufferTooSmall;
    }

    unsafe {
        ptr::copy_nonoverlapping(queued.as_ptr(), out, queued.len());
    }
    let copied = queued.len();
    queued.clear();
    set_out_count(out_count, copied);
    PluginError::Success
}

pub(super) fn copy_note_expression_output_events(
    queued: &mut Vec<PluginNoteExpressionEvent>,
    out: *mut PluginNoteExpressionEvent,
    capacity: usize,
    out_count: *mut usize,
) -> PluginError {
    if queued.is_empty() {
        set_out_count(out_count, 0);
        return PluginError::Success;
    }
    if out.is_null() {
        set_last_error_static(c"NULL Note Expression output buffer with queued events");
        set_out_count(out_count, 0);
        return PluginError::NullPointer;
    }
    if capacity < queued.len() {
        set_last_error_static(c"Note Expression output buffer is too small");
        set_out_count(out_count, 0);
        return PluginError::BufferTooSmall;
    }

    unsafe {
        ptr::copy_nonoverlapping(queued.as_ptr(), out, queued.len());
    }
    let copied = queued.len();
    queued.clear();
    set_out_count(out_count, copied);
    PluginError::Success
}

pub(super) fn copy_bytes_to_ffi_buffer(bytes: &[u8], out_len: *mut usize) -> *mut u8 {
    let len = bytes.len();
    let buf = libc_malloc(len);
    if buf.is_null() {
        return ptr::null_mut();
    }
    if out_len.is_null() {
        libc_free(buf, len);
        return ptr::null_mut();
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), buf, len);
        *out_len = len;
    }
    buf
}
