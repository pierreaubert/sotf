use super::misc::core_audio_ffi as ca;
use super::playback_state::PlaybackState;
use rtrb::Consumer;
use std::sync::Arc;
use std::sync::atomic::Ordering;

pub(super) struct RenderContext {
    pub(super) consumer: Consumer<f32>,
    pub(super) state: Arc<PlaybackState>,
    pub(super) sample_rate: u32,
    pub(super) channels: usize,
}

/// CoreAudio render callback — called on the real-time audio thread.
/// Reads from ring buffer, applies volume/clamp, writes to AudioBufferList.
pub(super) unsafe extern "C" fn render_callback(
    in_ref_con: *mut std::os::raw::c_void,
    _io_action_flags: *mut u32,
    _in_time_stamp: *const ca::AudioTimeStamp,
    _in_bus_number: u32,
    _in_number_frames: u32,
    io_data: *mut ca::AudioBufferList,
) -> ca::OSStatus {
    let ctx = &mut *(in_ref_con as *mut RenderContext);
    let buf_list = &mut *io_data;
    let buf = &mut buf_list.buffers[0];

    let output_samples = buf.data_byte_size as usize / std::mem::size_of::<f32>();
    let out = std::slice::from_raw_parts_mut(buf.data as *mut f32, output_samples);

    // Handle flush: discard ring buffer contents, output silence
    if ctx.state.flush_requested.load(Ordering::Relaxed) {
        let available = ctx.consumer.slots().min(output_samples);
        if available > 0 {
            if let Ok(chunk) = ctx.consumer.read_chunk(available) {
                chunk.commit_all();
            }
        }
        out.fill(0.0);
        if ctx.consumer.slots() == 0 {
            ctx.state.flush_requested.store(false, Ordering::Relaxed);
        }
        return ca::noErr;
    }

    // Read from ring buffer
    let available = ctx.consumer.slots();
    let to_read = output_samples.min(available);

    if to_read > 0 {
        if let Ok(chunk) = ctx.consumer.read_chunk(to_read) {
            let (first, second) = chunk.as_slices();
            out[..first.len()].copy_from_slice(first);
            if !second.is_empty() {
                out[first.len()..first.len() + second.len()].copy_from_slice(second);
            }
            chunk.commit_all();
        }
    }

    // Zero-pad if underrun
    if to_read < output_samples {
        out[to_read..].fill(0.0);
    }

    // Apply a click-free volume/mute ramp and clamp.
    let volume = f32::from_bits(ctx.state.volume.load(Ordering::Relaxed));
    let muted = ctx.state.muted.load(Ordering::Relaxed);
    let target = if muted { 0.0 } else { volume };
    ctx.state
        .volume_ramp
        .apply(out, ctx.channels, ctx.sample_rate, target);
    for s in out.iter_mut() {
        *s = s.clamp(-1.0, 1.0);
    }

    ca::noErr
}
