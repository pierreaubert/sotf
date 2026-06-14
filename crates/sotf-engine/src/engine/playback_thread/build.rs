use super::super::ThreadEvent;
use super::apply::apply_volume;
use super::apply::apply_volume_clamp;
use super::misc::send_playback_event;
use super::playback_state::PlaybackState;
use super::playback_state::read_ring_buffer;
use cpal::traits::DeviceTrait;
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use rtrb::Consumer;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;

/// Build the cpal output stream with the specified sample format.
/// Internal pipeline stays f32; conversion to the hardware format happens
/// only at the final output boundary.
pub(super) fn build_output_stream(
    device: &Device,
    config: &StreamConfig,
    state: Arc<PlaybackState>,
    event_tx: Sender<ThreadEvent>,
    consumer: Consumer<f32>,
    sample_format: SampleFormat,
) -> Result<Stream, String> {
    match sample_format {
        SampleFormat::F32 => build_output_stream_f32(device, config, state, event_tx, consumer),
        SampleFormat::I32 => {
            build_output_stream_int::<i32>(device, config, state, event_tx, consumer)
        }
        SampleFormat::I16 => {
            build_output_stream_int::<i16>(device, config, state, event_tx, consumer)
        }
        SampleFormat::U32 => {
            build_output_stream_int::<u32>(device, config, state, event_tx, consumer)
        }
        SampleFormat::U16 => {
            build_output_stream_int::<u16>(device, config, state, event_tx, consumer)
        }
        _ => Err(format!("Unsupported sample format: {:?}", sample_format)),
    }
}

/// Build f32 output stream (direct path, no format conversion).
pub(super) fn build_output_stream_f32(
    device: &Device,
    config: &StreamConfig,
    state: Arc<PlaybackState>,
    event_tx: Sender<ThreadEvent>,
    mut consumer: Consumer<f32>,
) -> Result<Stream, String> {
    let state_clone = Arc::clone(&state);
    let error_state = Arc::clone(&state);
    let capacity = state.capacity;

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                state_clone.callback_count.fetch_add(1, Ordering::Relaxed);
                read_ring_buffer(&mut consumer, data, data.len(), &state_clone, capacity);
                apply_volume(data, &state_clone);
            },
            move |err| {
                error_state
                    .stream_error_count
                    .fetch_add(1, Ordering::Relaxed);
                crate::rate_limited_log!(warn, 5, "[Playback Thread] Stream error: {}", err);
                static EVENT_GATE: AtomicU64 = AtomicU64::new(0);
                if crate::rate_limit::allow(&EVENT_GATE, 5_000_000_000) {
                    send_playback_event(
                        &event_tx,
                        ThreadEvent::ProcessingWarning(format!("Stream error: {}", err)),
                        "f32 stream error",
                    );
                }
            },
            None,
        )
        .map_err(|e| format!("Failed to build output stream: {}", e))?;

    Ok(stream)
}

/// Build integer output stream (I16 or I32). Reads f32 from ring buffer
/// into a pre-allocated scratch buffer, applies volume/clamp, then converts
/// to the target integer type.
pub(super) fn build_output_stream_int<T>(
    device: &Device,
    config: &StreamConfig,
    state: Arc<PlaybackState>,
    event_tx: Sender<ThreadEvent>,
    mut consumer: Consumer<f32>,
) -> Result<Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let state_clone = Arc::clone(&state);
    let error_state = Arc::clone(&state);
    let capacity = state.capacity;

    // Pre-allocate scratch buffer (captured by closure, no alloc in callback).
    // 16384 samples covers typical callbacks (256–4096). Process in chunks if larger.
    let mut scratch = vec![0.0f32; 16384];

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                state_clone.callback_count.fetch_add(1, Ordering::Relaxed);
                let requested = data.len();

                // Process in chunks if callback is larger than scratch buffer
                let mut offset = 0;
                while offset < requested {
                    let chunk_len = (requested - offset).min(scratch.len());
                    read_ring_buffer(
                        &mut consumer,
                        &mut scratch[..chunk_len],
                        chunk_len,
                        &state_clone,
                        capacity,
                    );
                    apply_volume_clamp(&mut scratch[..chunk_len], &state_clone);

                    // Convert f32 -> target integer type
                    for (out, &s) in data[offset..offset + chunk_len]
                        .iter_mut()
                        .zip(&scratch[..chunk_len])
                    {
                        *out = T::from_sample(s);
                    }
                    offset += chunk_len;
                }
            },
            move |err| {
                error_state
                    .stream_error_count
                    .fetch_add(1, Ordering::Relaxed);
                crate::rate_limited_log!(warn, 5, "[Playback Thread] Stream error: {}", err);
                static EVENT_GATE: AtomicU64 = AtomicU64::new(0);
                if crate::rate_limit::allow(&EVENT_GATE, 5_000_000_000) {
                    send_playback_event(
                        &event_tx,
                        ThreadEvent::ProcessingWarning(format!("Stream error: {}", err)),
                        "integer stream error",
                    );
                }
            },
            None,
        )
        .map_err(|e| format!("Failed to build output stream: {}", e))?;

    Ok(stream)
}
