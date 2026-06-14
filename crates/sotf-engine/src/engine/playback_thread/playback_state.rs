use super::build::build_output_stream;
use super::misc::prefill_silence;
use super::misc::select_playback_device;
use super::pick::choose_output_format;
use super::playback::playback_buffer_capacity;
use super::types::RebuildPlaybackParams;
use super::types::RebuiltPlaybackStream;
use cpal::StreamConfig;
use cpal::traits::{DeviceTrait, StreamTrait};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// Shared state between thread and cpal callback (all fields are lock-free atomics)
pub(super) struct PlaybackState {
    pub(super) capacity: usize,
    pub(super) volume: Arc<AtomicU32>, // Atomic f32 stored as u32 bits
    pub(super) muted: Arc<AtomicBool>,
    pub(super) flush_requested: Arc<AtomicBool>,
    pub(super) underrun_count: Arc<AtomicU64>,
    pub(super) last_buffer_level: Arc<AtomicU64>, // For tracking buffer fill percentage
    pub(super) total_callback_samples: Arc<AtomicU64>,
    pub(super) callback_count: Arc<AtomicU64>,
    pub(super) stream_error_count: Arc<AtomicU64>,
}

impl PlaybackState {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            muted: Arc::new(AtomicBool::new(false)),
            flush_requested: Arc::new(AtomicBool::new(false)),
            underrun_count: Arc::new(AtomicU64::new(0)),
            last_buffer_level: Arc::new(AtomicU64::new(100)),
            total_callback_samples: Arc::new(AtomicU64::new(0)),
            callback_count: Arc::new(AtomicU64::new(0)),
            stream_error_count: Arc::new(AtomicU64::new(0)),
        }
    }
}

pub(super) fn copy_playback_controls(from: &PlaybackState, to: &PlaybackState) {
    to.volume
        .store(from.volume.load(Ordering::Relaxed), Ordering::Relaxed);
    to.muted
        .store(from.muted.load(Ordering::Relaxed), Ordering::Relaxed);
}

pub(super) fn rebuild_playback_stream(
    host: &cpal::Host,
    params: RebuildPlaybackParams<'_>,
) -> Result<RebuiltPlaybackStream, String> {
    let device = select_playback_device(host, params.output_device, params.allow_virtual_output)?;
    let device_name = device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    let mut config = StreamConfig {
        channels: params.requested_channels as u16,
        sample_rate: params.sample_rate,
        buffer_size: params.buffer_size,
    };

    let (output_format, hw_channels) = choose_output_format(&device, &config);
    if hw_channels != config.channels {
        log::info!(
            "[Playback Thread] Recovery adjusted channels from {} to {} for '{}'",
            config.channels,
            hw_channels,
            device_name
        );
        config.channels = hw_channels;
    }

    let channels = hw_channels as usize;
    let buffer_capacity = playback_buffer_capacity(params.sample_rate, channels, params.buffer_ms);
    let (mut producer, consumer) = RingBuffer::<f32>::new(buffer_capacity);
    let state = Arc::new(PlaybackState::new(buffer_capacity));
    copy_playback_controls(params.old_state, &state);
    prefill_silence(&mut producer, buffer_capacity / 2);

    let stream = build_output_stream(
        &device,
        &config,
        Arc::clone(&state),
        params.event_tx,
        consumer,
        output_format,
    )?;
    stream
        .play()
        .map_err(|e| format!("Failed to start recovered stream: {}", e))?;

    Ok(RebuiltPlaybackStream {
        device,
        device_name,
        stream,
        producer,
        state,
        config,
        output_format,
        channels,
        buffer_capacity,
    })
}

pub(super) fn request_flush(state: &PlaybackState) {
    state.flush_requested.store(true, Ordering::Relaxed);
}

pub(super) fn flush_completed(
    state: &PlaybackState,
    producer: &Producer<f32>,
    buffer_capacity: usize,
) -> bool {
    if state.flush_requested.load(Ordering::Relaxed) && producer.slots() >= buffer_capacity {
        state.flush_requested.store(false, Ordering::Relaxed);
    }

    !state.flush_requested.load(Ordering::Relaxed)
}

/// Read f32 samples from the ring buffer into a scratch buffer.
/// Returns `true` if an underrun occurred (not enough data). Handles underrun by zero-filling.
#[inline(always)]
pub(super) fn read_ring_buffer(
    consumer: &mut Consumer<f32>,
    scratch: &mut [f32],
    requested: usize,
    state: &PlaybackState,
    capacity: usize,
) -> bool {
    if state.flush_requested.load(Ordering::Relaxed) {
        let available = consumer.slots().min(requested);
        if available > 0
            && let Ok(chunk) = consumer.read_chunk(available)
        {
            chunk.commit_all();
        }
        state
            .total_callback_samples
            .fetch_add(available as u64, Ordering::Relaxed);

        scratch[..requested].fill(0.0);

        if consumer.slots() == 0 {
            state.flush_requested.store(false, Ordering::Relaxed);
        }

        let fill_percent = (consumer.slots() * 100).checked_div(capacity).unwrap_or(0);
        state
            .last_buffer_level
            .store(fill_percent as u64, Ordering::Relaxed);

        return false;
    }

    let mut underrun = false;

    // Try to read requested amount
    if let Ok(chunk) = consumer.read_chunk(requested) {
        let (first, second) = chunk.as_slices();
        let first_len = first.len();
        let second_len = second.len();

        if first_len > 0 {
            scratch[..first_len].copy_from_slice(first);
        }
        if second_len > 0 {
            scratch[first_len..first_len + second_len].copy_from_slice(second);
        }

        chunk.commit_all();
        state
            .total_callback_samples
            .fetch_add(requested as u64, Ordering::Relaxed);
    } else {
        // Not enough data (underrun)
        let available = consumer.slots().min(requested);

        if let Ok(chunk) = consumer.read_chunk(available) {
            let (first, second) = chunk.as_slices();
            let first_len = first.len();
            let second_len = second.len();

            if first_len > 0 {
                scratch[..first_len].copy_from_slice(first);
            }
            if second_len > 0 {
                scratch[first_len..first_len + second_len].copy_from_slice(second);
            }
            chunk.commit_all();
        }
        state
            .total_callback_samples
            .fetch_add(available as u64, Ordering::Relaxed);

        // Zero pad the rest
        if available < requested {
            scratch[available..requested].fill(0.0);
        }

        underrun = true;
        state.underrun_count.fetch_add(1, Ordering::Relaxed);
    }

    // Update buffer level metric
    let slots = consumer.slots();
    let fill_percent = (slots * 100).checked_div(capacity).unwrap_or(0);
    state
        .last_buffer_level
        .store(fill_percent as u64, Ordering::Relaxed);

    underrun
}
