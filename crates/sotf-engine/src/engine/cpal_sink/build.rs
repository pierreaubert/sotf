use super::super::ThreadEvent;
use super::apply::apply_volume;
use super::apply::apply_volume_clamp;
use super::cpal_playback_state::CpalPlaybackState;
use super::cpal_playback_state::read_ring_buffer;
use super::misc::send_thread_event;
use cpal::traits::DeviceTrait;
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use rtrb::Consumer;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;

pub(super) fn build_output_stream(
    device: &Device,
    config: &StreamConfig,
    logical_channels: usize,
    state: Arc<CpalPlaybackState>,
    event_tx: Sender<ThreadEvent>,
    consumer: Consumer<f32>,
    sample_format: SampleFormat,
) -> Result<Stream, String> {
    match sample_format {
        SampleFormat::F32 => {
            build_output_stream_f32(device, config, logical_channels, state, event_tx, consumer)
        }
        SampleFormat::I32 => build_output_stream_int::<i32>(
            device,
            config,
            logical_channels,
            state,
            event_tx,
            consumer,
        ),
        SampleFormat::I16 => build_output_stream_int::<i16>(
            device,
            config,
            logical_channels,
            state,
            event_tx,
            consumer,
        ),
        SampleFormat::U32 => build_output_stream_int::<u32>(
            device,
            config,
            logical_channels,
            state,
            event_tx,
            consumer,
        ),
        SampleFormat::U16 => build_output_stream_int::<u16>(
            device,
            config,
            logical_channels,
            state,
            event_tx,
            consumer,
        ),
        _ => Err(format!("Unsupported sample format: {:?}", sample_format)),
    }
}

pub(super) fn build_output_stream_f32(
    device: &Device,
    config: &StreamConfig,
    logical_channels: usize,
    state: Arc<CpalPlaybackState>,
    event_tx: Sender<ThreadEvent>,
    mut consumer: Consumer<f32>,
) -> Result<Stream, String> {
    let state_clone = Arc::clone(&state);
    let capacity = state.capacity;
    let hardware_channels = config.channels as usize;

    if logical_channels == hardware_channels {
        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    state_clone.callback_count.fetch_add(1, Ordering::Relaxed);
                    read_ring_buffer(&mut consumer, data, data.len(), &state_clone, capacity);
                    apply_volume_clamp(data, &state_clone);
                },
                move |err| {
                    crate::rate_limited_log!(warn, 5, "[CpalSink] Stream error: {}", err);
                    static EVENT_GATE: AtomicU64 = AtomicU64::new(0);
                    if crate::rate_limit::allow(&EVENT_GATE, 5_000_000_000) {
                        send_thread_event(
                            &event_tx,
                            ThreadEvent::ProcessingError(format!("Stream error: {}", err)),
                            "f32 stream error",
                        );
                    }
                },
                None,
            )
            .map_err(|e| format!("Failed to build output stream: {}", e))?;

        return Ok(stream);
    }

    let mut scratch = vec![0.0f32; scratch_len(logical_channels)];

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                state_clone.callback_count.fetch_add(1, Ordering::Relaxed);
                process_mapped_f32_callback(
                    data,
                    &mut scratch,
                    &mut consumer,
                    &state_clone,
                    capacity,
                    logical_channels,
                    hardware_channels,
                );
            },
            move |err| {
                crate::rate_limited_log!(warn, 5, "[CpalSink] Stream error: {}", err);
                static EVENT_GATE: AtomicU64 = AtomicU64::new(0);
                if crate::rate_limit::allow(&EVENT_GATE, 5_000_000_000) {
                    send_thread_event(
                        &event_tx,
                        ThreadEvent::ProcessingError(format!("Stream error: {}", err)),
                        "f32 stream error",
                    );
                }
            },
            None,
        )
        .map_err(|e| format!("Failed to build output stream: {}", e))?;

    Ok(stream)
}

pub(super) fn build_output_stream_int<T>(
    device: &Device,
    config: &StreamConfig,
    logical_channels: usize,
    state: Arc<CpalPlaybackState>,
    event_tx: Sender<ThreadEvent>,
    mut consumer: Consumer<f32>,
) -> Result<Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let state_clone = Arc::clone(&state);
    let capacity = state.capacity;
    let hardware_channels = config.channels as usize;
    let mut scratch = vec![0.0f32; scratch_len(logical_channels)];

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                state_clone.callback_count.fetch_add(1, Ordering::Relaxed);
                let requested = data.len();
                let mut offset = 0;
                while offset < requested {
                    if logical_channels == hardware_channels {
                        let chunk_len = (requested - offset).min(scratch.len());
                        read_ring_buffer(
                            &mut consumer,
                            &mut scratch[..chunk_len],
                            chunk_len,
                            &state_clone,
                            capacity,
                        );
                        apply_volume_clamp(&mut scratch[..chunk_len], &state_clone);
                        for (out, &s) in data[offset..offset + chunk_len]
                            .iter_mut()
                            .zip(&scratch[..chunk_len])
                        {
                            *out = T::from_sample(s);
                        }
                        offset += chunk_len;
                    } else {
                        let frames = ((requested - offset) / hardware_channels)
                            .min(scratch.len() / logical_channels);
                        if frames == 0 {
                            data[offset..].fill(T::from_sample(0.0));
                            break;
                        }

                        let logical_len = frames * logical_channels;
                        let hardware_len = frames * hardware_channels;
                        read_ring_buffer(
                            &mut consumer,
                            &mut scratch[..logical_len],
                            logical_len,
                            &state_clone,
                            capacity,
                        );
                        apply_volume_clamp(&mut scratch[..logical_len], &state_clone);
                        write_logical_to_hardware_int(
                            &scratch[..logical_len],
                            &mut data[offset..offset + hardware_len],
                            logical_channels,
                            hardware_channels,
                        );
                        offset += hardware_len;
                    }
                }
            },
            move |err| {
                crate::rate_limited_log!(warn, 5, "[CpalSink] Stream error: {}", err);
                static EVENT_GATE: AtomicU64 = AtomicU64::new(0);
                if crate::rate_limit::allow(&EVENT_GATE, 5_000_000_000) {
                    send_thread_event(
                        &event_tx,
                        ThreadEvent::ProcessingError(format!("Stream error: {}", err)),
                        "integer stream error",
                    );
                }
            },
            None,
        )
        .map_err(|e| format!("Failed to build output stream: {}", e))?;

    Ok(stream)
}

fn scratch_len(logical_channels: usize) -> usize {
    16_384usize.max(logical_channels.max(1))
}

fn process_mapped_f32_callback(
    data: &mut [f32],
    scratch: &mut [f32],
    consumer: &mut Consumer<f32>,
    state: &CpalPlaybackState,
    capacity: usize,
    logical_channels: usize,
    hardware_channels: usize,
) {
    let mut offset = 0;
    while offset < data.len() {
        let frames =
            ((data.len() - offset) / hardware_channels).min(scratch.len() / logical_channels);
        if frames == 0 {
            data[offset..].fill(0.0);
            break;
        }

        let logical_len = frames * logical_channels;
        let hardware_len = frames * hardware_channels;
        read_ring_buffer(
            consumer,
            &mut scratch[..logical_len],
            logical_len,
            state,
            capacity,
        );
        apply_volume(&mut scratch[..logical_len], state);
        write_logical_to_hardware_f32(
            &scratch[..logical_len],
            &mut data[offset..offset + hardware_len],
            logical_channels,
            hardware_channels,
        );
        offset += hardware_len;
    }
}

pub(super) fn write_logical_to_hardware_f32(
    logical: &[f32],
    hardware: &mut [f32],
    logical_channels: usize,
    hardware_channels: usize,
) {
    hardware.fill(0.0);
    let frames = (logical.len() / logical_channels).min(hardware.len() / hardware_channels);
    for frame in 0..frames {
        let logical_base = frame * logical_channels;
        let hardware_base = frame * hardware_channels;
        let copy_channels = logical_channels.min(hardware_channels);
        hardware[hardware_base..hardware_base + copy_channels]
            .copy_from_slice(&logical[logical_base..logical_base + copy_channels]);
    }
}

fn write_logical_to_hardware_int<T>(
    logical: &[f32],
    hardware: &mut [T],
    logical_channels: usize,
    hardware_channels: usize,
) where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    hardware.fill(T::from_sample(0.0));
    let frames = (logical.len() / logical_channels).min(hardware.len() / hardware_channels);
    for frame in 0..frames {
        let logical_base = frame * logical_channels;
        let hardware_base = frame * hardware_channels;
        let copy_channels = logical_channels.min(hardware_channels);
        for channel in 0..copy_channels {
            hardware[hardware_base + channel] = T::from_sample(logical[logical_base + channel]);
        }
    }
}
