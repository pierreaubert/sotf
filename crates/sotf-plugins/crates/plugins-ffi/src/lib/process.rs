//! Audio-process FFI helpers.
//!
//! These functions bridge C buffer and event pointers into the SOTF host
//! process API. They are `unsafe` because they trust the caller to supply
//! valid pointers and correctly sized buffers; the public `#[unsafe(no_mangle)]`
//! wrappers validate nullity before calling into this module.

use super::PluginError;
use super::PluginHandle;
use super::PluginMidiEvent;
use super::PluginNoteExpressionEvent;
use super::consts::MAX_FFI_MIDI_EVENTS_PER_BLOCK;
use super::copy::copy_midi_output_events;
use super::copy::copy_note_expression_output_events;
use super::host::host_note_expression_kind;
use super::misc::set_last_error;
use sotf_host::plugin::{
    MidiEvent, MidiMessage, NoteExpressionEvent as HostNoteExpressionEvent,
    NoteExpressionKind as HostNoteExpressionKind, ProcessContext,
};
use std::ptr;
use std::slice;

#[inline]
pub(super) unsafe fn process_impl(
    handle: *mut PluginHandle,
    input: *const f32,
    output: *mut f32,
    num_frames: usize,
    context: &ProcessContext<'_>,
) -> PluginError {
    // SAFETY: The public FFI wrapper verified `handle` is non-null. The
    // caller must ensure it is a live `PluginHandle` and that no other thread
    // is concurrently mutating it through the FFI surface.
    let handle_ref = unsafe { &mut *handle };

    let input_samples = num_frames * handle_ref.input_channels;
    let output_samples = num_frames * handle_ref.output_channels;

    // SAFETY: The public FFI wrapper verified `input` and `output` are
    // non-null. The caller must ensure they point to at least
    // `input_samples`/`output_samples` valid `f32` values and remain valid
    // and non-overlapping for the duration of this call.
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
pub(super) unsafe fn process_with_ffi_events_impl(
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
#[allow(
    clippy::too_many_arguments,
    reason = "FFI entry point: one argument per external buffer/event stream"
)]
pub(super) unsafe fn process_with_ffi_and_note_expression_events_impl(
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
        // SAFETY: The public FFI wrapper verified `midi_events` is non-null
        // when `midi_event_count > 0`. The caller must ensure the pointer
        // remains readable for `midi_event_count` events.
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
        // SAFETY: The public FFI wrapper verified `note_expression_events` is
        // non-null when `note_expression_event_count > 0`. The caller must
        // ensure the pointer remains readable for the event count.
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

#[inline]
#[allow(
    clippy::too_many_arguments,
    reason = "FFI entry point: one argument per external input/output buffer and event stream"
)]
pub(super) unsafe fn process_with_full_events_impl(
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
