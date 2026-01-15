// ============================================================================
// Playback Thread - cpal Output
// ============================================================================
//
// Highest priority thread that reads from queue and outputs to hardware.
// Must be real-time safe (no allocations, no locks in callback).

use super::{PlaybackCommand, ProcessingMessage, ThreadEvent};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use rtrb::{Consumer, RingBuffer};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};

#[cfg(all(target_os = "macos", feature = "hal"))]
use sotf_hal::HalOutputWriter;

const SPIN_MS_RINGBUFFER: u64 = 5;
const SPIN_MS_SIGNAL: u64 = 10;

/// Playback thread handle
pub struct PlaybackThread {
    command_tx: Sender<PlaybackCommand>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl PlaybackThread {
    /// Create and start the playback thread
    pub fn new(
        message_rx: Receiver<ProcessingMessage>,
        event_tx: Sender<ThreadEvent>,
        sample_rate: u32,
        channels: usize,
        output_device: Option<String>,
    ) -> Result<Self, String> {
        let (command_tx, command_rx) = std::sync::mpsc::channel();

        let thread_handle = std::thread::Builder::new()
            .name("playback".to_string())
            .spawn(move || {
                let error_tx = event_tx.clone();
                if let Err(e) = run_playback_thread(
                    message_rx,
                    command_rx,
                    event_tx,
                    sample_rate,
                    channels,
                    output_device,
                ) {
                    log::debug!("[Playback Thread] Error: {}", e);
                    error_tx
                        .send(ThreadEvent::ProcessingError(format!(
                            "Playback thread error: {}",
                            e
                        )))
                        .ok();
                }
            })
            .map_err(|e| format!("Failed to spawn playback thread: {}", e))?;

        Ok(Self {
            command_tx,
            thread_handle: Some(thread_handle),
        })
    }

    /// Send a command to the playback thread
    pub fn send_command(&self, command: PlaybackCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|e| format!("Failed to send command: {}", e))
    }

    /// Shutdown the playback thread
    pub fn shutdown(&mut self) {
        self.send_command(PlaybackCommand::Shutdown).ok();
        if let Some(handle) = self.thread_handle.take() {
            handle.join().ok();
        }
    }
}

impl Drop for PlaybackThread {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Shared state between thread and cpal callback
struct PlaybackState {
    // Consumer end of lock-free ring buffer
    // Moved into the callback closure, but kept here for ownership management
    // Note: This is an Option because we move it out when building the stream
    ring_buffer_consumer: parking_lot::Mutex<Option<Consumer<f32>>>,

    // Capacity of the ring buffer (for metrics)
    capacity: usize,

    volume: Arc<AtomicU32>, // Atomic f32 stored as u32 bits
    muted: Arc<AtomicBool>,
    underrun_count: Arc<AtomicU64>,
    last_buffer_level: Arc<AtomicU64>, // For tracking buffer fill percentage

    #[cfg(all(target_os = "macos", feature = "hal"))]
    hal_writer: parking_lot::Mutex<Option<HalOutputWriter>>,
}

impl PlaybackState {
    fn new(consumer: Consumer<f32>, capacity: usize) -> Self {
        #[cfg(all(target_os = "macos", feature = "hal"))]
        let hal_writer = HalOutputWriter::new();

        #[cfg(all(target_os = "macos", feature = "hal"))]
        if hal_writer.is_none() {
            log::warn!("[Playback Thread] Failed to initialize HAL output writer");
        }

        Self {
            ring_buffer_consumer: parking_lot::Mutex::new(Some(consumer)),
            capacity,
            volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            muted: Arc::new(AtomicBool::new(false)),
            underrun_count: Arc::new(AtomicU64::new(0)),
            last_buffer_level: Arc::new(AtomicU64::new(100)), // Start at 100%
            #[cfg(all(target_os = "macos", feature = "hal"))]
            hal_writer: parking_lot::Mutex::new(hal_writer),
        }
    }
}

/// Main playback thread function
fn run_playback_thread(
    message_rx: Receiver<ProcessingMessage>,
    command_rx: Receiver<PlaybackCommand>,
    event_tx: Sender<ThreadEvent>,
    sample_rate: u32,
    initial_channels: usize,
    output_device: Option<String>,
) -> Result<(), String> {
    // Initialize cpal
    let host = cpal::default_host();

    // Select output device
    let device = if let Some(device_identifier) = output_device {
        // Try to find device by ID first, then name
        log::debug!(
            "[Playback Thread] Looking for device: '{}'",
            device_identifier
        );

        let devices: Vec<_> = host
            .output_devices()
            .map_err(|e| format!("Failed to enumerate output devices: {}", e))?
            .collect();

        // Helper to get device display name
        let get_display_name = |d: &Device| -> String {
            d.description()
                .map(|desc| desc.name().to_string())
                .unwrap_or_else(|_| "Unknown Device".to_string())
        };

        // First try to match by device ID (preferred for persistence)
        let found_device = devices
            .iter()
            .find(|d| {
                if let Ok(id) = d.id() {
                    id.to_string() == device_identifier
                } else {
                    false
                }
            })
            .cloned()
            // Then try exact name match using description
            .or_else(|| {
                let target_pattern = device_identifier.to_lowercase();
                devices
                    .iter()
                    .find(|d| get_display_name(d).to_lowercase() == target_pattern)
                    .cloned()
            })
            // Then try partial match (starts with)
            .or_else(|| {
                let target_pattern = device_identifier.to_lowercase();
                devices
                    .iter()
                    .find(|d| {
                        get_display_name(d)
                            .to_lowercase()
                            .starts_with(&target_pattern)
                    })
                    .cloned()
            })
            // Finally try partial match (contains)
            .or_else(|| {
                let target_pattern = device_identifier.to_lowercase();
                devices
                    .iter()
                    .find(|d| get_display_name(d).to_lowercase().contains(&target_pattern))
                    .cloned()
            });

        match found_device {
            Some(dev) => {
                let dev_name = get_display_name(&dev);
                log::debug!("[Playback Thread] Using device: '{}'", dev_name);
                dev
            }
            None => {
                log::info!(
                    "[Playback Thread] Device '{}' not found, using default",
                    device_identifier
                );
                host.default_output_device()
                    .ok_or("No default output device available")?
            }
        }
    } else {
        // Use default device
        host.default_output_device()
            .ok_or("No output device available")?
    };

    // Track current channel count (can change dynamically)
    let mut channels = initial_channels;

    // Create stream config
    let mut config = StreamConfig {
        channels: channels as u16,
        sample_rate: sample_rate,
        buffer_size: cpal::BufferSize::Default,
    };

    // Create shared state (ring buffer with ~200ms capacity)
    let buffer_capacity = (sample_rate as usize * 200) / 1000 * channels; // 200ms * channels
    let (mut producer, consumer) = RingBuffer::<f32>::new(buffer_capacity);
    let mut state = Arc::new(PlaybackState::new(consumer, buffer_capacity));

    // Pre-allocate buffer for channel conversions (fallback downmix/upmix)
    let mut conversion_buffer = Vec::with_capacity(4096);

    // Build cpal stream
    let mut stream = build_output_stream(&device, &config, Arc::clone(&state), event_tx.clone())?;

    // Start stream
    stream
        .play()
        .map_err(|e| format!("Failed to start stream: {}", e))?;

    log::info!(
        "[Playback Thread] Started - {}Hz, {} channels",
        sample_rate,
        channels
    );

    // Main loop: read from queue and write to ring buffer
    loop {
        // Check for commands (non-blocking)
        if let Ok(command) = command_rx.try_recv() {
            match command {
                PlaybackCommand::SetVolume(vol) => {
                    state.volume.store(vol.to_bits(), Ordering::Relaxed);
                }
                PlaybackCommand::Mute(muted) => {
                    state.muted.store(muted, Ordering::Relaxed);
                }
                PlaybackCommand::UpdateChannels(new_channels) => {
                    log::warn!(
                        "[Playback Thread] RECEIVED UpdateChannels({}) command, current channels={}",
                        new_channels,
                        channels
                    );
                    if new_channels != channels {
                        log::info!(
                            "[Playback Thread] Updating channel count: {} -> {}",
                            channels,
                            new_channels
                        );
                        log::trace!(
                            "[Playback Thread] UpdateChannels: Draining pending frames with old channel count"
                        );

                        // Clear ring buffer logic is replaced by creating a new ring buffer
                        log::debug!("[Playback Thread] Recreating ring buffer for channel update");

                        // CRITICAL: Drain all pending frames from the message queue
                        // These frames may have the OLD channel count and would cause mismatches
                        let mut drained_count = 0;
                        while message_rx.try_recv().is_ok() {
                            drained_count += 1;
                        }
                        if drained_count > 0 {
                            log::debug!(
                                "[Playback Thread] Drained {} stale frames during channel update",
                                drained_count
                            );
                        }

                        // Build new config and state, but only commit them if stream rebuild succeeds
                        let new_config = StreamConfig {
                            channels: new_channels as u16,
                            sample_rate: config.sample_rate,
                            buffer_size: config.buffer_size,
                        };

                        // Create new ring buffer for the new channel configuration
                        let new_buffer_capacity =
                            (sample_rate as usize * 200) / 1000 * new_channels;
                        let (new_producer, new_consumer) =
                            RingBuffer::<f32>::new(new_buffer_capacity);

                        let new_state =
                            Arc::new(PlaybackState::new(new_consumer, new_buffer_capacity));

                        // Continuously drain frames during rebuild - they may have wrong channel count
                        // Use a closure to drain and count
                        let drain_frames = || {
                            let mut count = 0;
                            while message_rx.try_recv().is_ok() {
                                count += 1;
                            }
                            count
                        };

                        drained_count += drain_frames();

                        // Stop the old stream first to prevent any race conditions
                        // The stream.pause() ensures the audio callback stops
                        if let Err(e) = stream.pause() {
                            log::warn!("[Playback Thread] Failed to pause old stream: {}", e);
                        }

                        // Small delay to let audio callback finish
                        std::thread::sleep(std::time::Duration::from_millis(10));

                        // Final drain after stopping old stream
                        drained_count += drain_frames();

                        // Rebuild and start new stream
                        log::info!(
                            "[Playback Thread] Building new stream with config: {}ch, {}Hz (drained {} frames)",
                            new_config.channels,
                            new_config.sample_rate,
                            drained_count
                        );

                        match build_output_stream(
                            &device,
                            &new_config,
                            Arc::clone(&new_state),
                            event_tx.clone(),
                        ) {
                            Ok(new_stream) => {
                                log::info!("[Playback Thread] Stream built, starting playback...");
                                if let Err(e) = new_stream.play() {
                                    log::error!(
                                        "[Playback Thread] Failed to start new stream: {}",
                                        e
                                    );
                                    event_tx
                                        .send(ThreadEvent::ProcessingError(format!(
                                            "Playback stream start failed for {} channels: {}",
                                            new_channels, e
                                        )))
                                        .ok();
                                } else {
                                    // Replace old stream with new one (old one drops automatically)
                                    stream = new_stream;
                                    config = new_config;
                                    state = new_state;
                                    channels = new_channels;
                                    producer = new_producer; // Update producer

                                    // Final drain - discard any frames that arrived during rebuild
                                    // These might have wrong channel count
                                    let mut final_drained = 0;
                                    while message_rx.try_recv().is_ok() {
                                        final_drained += 1;
                                    }
                                    if final_drained > 0 {
                                        log::debug!(
                                            "[Playback Thread] Drained {} additional frames after stream rebuild",
                                            final_drained
                                        );
                                    }

                                    log::warn!(
                                        "[Playback Thread] STREAM REBUILT successfully with {} channels",
                                        channels
                                    );
                                }
                            }
                            Err(e) => {
                                log::error!(
                                    "[Playback Thread] Failed to build stream for {} channels: {}",
                                    new_channels,
                                    e
                                );
                                event_tx
                                    .send(ThreadEvent::ProcessingError(format!(
                                        "Playback stream rebuild failed for {} channels: {}",
                                        new_channels, e
                                    )))
                                    .ok();
                            }
                        }
                    } else {
                        log::debug!(
                            "[Playback Thread] UpdateChannels({}) - no change needed (already at {} channels)",
                            new_channels,
                            channels
                        );
                    }
                }
                PlaybackCommand::Stop => {
                    // To clear, we drop the old buffer and create a new one, but we can't easily do that inside the loop
                    // without rebuilding the stream because the consumer is moved into the stream callback.
                    // For now, stopping the stream or just letting it drain is safer.
                    // Or we could implement a "skipping" logic, but rtrb doesn't have a clear() method on producer easily accessible here.
                    // Actually, we can just drain the producer if we had access to consumer, but we don't.
                    // A simple workaround: we can't clear the buffer from the producer side directly.
                    // We rely on the fact that stopping decoding/processing will stop feeding data.
                }
                PlaybackCommand::Shutdown => {
                    log::debug!("[Playback Thread] Shutting down");
                    break;
                }
            }
        }

        // Check if ring buffer has space
        let available_space = producer.slots();

        // Only pull from queue if we have space for at least a few frames
        let min_space_required = 1024 * channels * 2; // Space for ~2 frames (assuming 1024 size)

        if available_space < min_space_required {
            // Ring buffer is full, sleep briefly and let the audio callback drain it
            std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_RINGBUFFER));
            continue;
        }

        // Read from message queue (non-blocking since we checked space)
        match message_rx.recv_timeout(std::time::Duration::from_millis(SPIN_MS_SIGNAL)) {
            Ok(ProcessingMessage::Frame(frame)) => {
                // Track consecutive channel mismatches to detect stuck state vs transient hot-reload
                static CHANNEL_MISMATCH_COUNT: std::sync::atomic::AtomicU32 =
                    std::sync::atomic::AtomicU32::new(0);

                // Handle channel count mismatch with robust conversion
                if frame.num_channels != channels {
                    let count =
                        CHANNEL_MISMATCH_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                    if count < 10 || count.is_multiple_of(1000) {
                        log::warn!(
                            "[Playback Thread] Channel mismatch #{}: frame has {} channels, \
                             output device expects {} - converting",
                            count + 1,
                            frame.num_channels,
                            channels
                        );
                    }

                    conversion_buffer.clear();
                    let num_frames = frame.num_frames;
                    let target_len = num_frames * channels;
                    conversion_buffer.resize(target_len, 0.0);

                    if frame.num_channels > channels && channels == 2 {
                        // High-quality N -> 2 Downmix
                        // 0:L, 1:R, 2:C, 3:LFE, 4:SL, 5:SR, 6:BL, 7:BR, 8:TFL, 9:TFR...
                        for i in 0..num_frames {
                            let base = i * frame.num_channels;
                            let src = &frame.data[base..base + frame.num_channels];

                            let l = src.get(0).copied().unwrap_or(0.0);
                            let r = src.get(1).copied().unwrap_or(0.0);
                            let c = src.get(2).copied().unwrap_or(0.0) * 0.707;
                            // Surrounds
                            let sl = src.get(4).copied().unwrap_or(0.0) * 0.707;
                            let sr = src.get(5).copied().unwrap_or(0.0) * 0.707;
                            // Back surrounds
                            let bl = src.get(6).copied().unwrap_or(0.0) * 0.5;
                            let br = src.get(7).copied().unwrap_or(0.0) * 0.5;
                            // Heights
                            let tfl = src.get(8).copied().unwrap_or(0.0) * 0.5;
                            let tfr = src.get(9).copied().unwrap_or(0.0) * 0.5;

                            conversion_buffer[i * 2] = l + c + sl + bl + tfl;
                            conversion_buffer[i * 2 + 1] = r + c + sr + br + tfr;
                        }
                    } else if frame.num_channels == 2 && channels > 2 {
                        // 2 -> N Upmix (L/R to fronts, rest silent)
                        for i in 0..num_frames {
                            let src_base = i * 2;
                            let dst_base = i * channels;
                            conversion_buffer[dst_base] = frame.data[src_base];
                            conversion_buffer[dst_base + 1] = frame.data[src_base + 1];
                        }
                    } else {
                        // General fallback: copy shared channels, zero the rest
                        let shared_channels = frame.num_channels.min(channels);
                        for i in 0..num_frames {
                            let src_base = i * frame.num_channels;
                            let dst_base = i * channels;
                            for ch in 0..shared_channels {
                                conversion_buffer[dst_base + ch] = frame.data[src_base + ch];
                            }
                        }
                    }

                    // Write converted audio
                    let chunk = match producer.write_chunk_uninit(conversion_buffer.len()) {
                        Ok(chunk) => chunk,
                        Err(_) => {
                            // Not enough space, wait a bit
                            std::thread::sleep(std::time::Duration::from_millis(
                                SPIN_MS_RINGBUFFER,
                            ));
                            continue;
                        }
                    };
                    chunk.fill_from_iter(conversion_buffer.iter().copied());
                    continue;
                }

                CHANNEL_MISMATCH_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);

                // Write to ring buffer
                let chunk = match producer.write_chunk_uninit(frame.data.len()) {
                    Ok(chunk) => chunk,
                    Err(_) => {
                        // Should not happen often due to available_space check above, but purely for safety
                        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_RINGBUFFER));
                        continue;
                    }
                };
                chunk.fill_from_iter(frame.data.into_iter());
            }
            Ok(ProcessingMessage::EndOfStream) => {
                log::debug!("[Playback Thread] End of stream");
            }
            Ok(ProcessingMessage::Flush) => {
                // Cannot easily clear rtrb producer side without consumer access
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // No message, continue
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                log::debug!("[Playback Thread] Queue disconnected");
                break;
            }
        }
    }

    // Cleanup
    drop(stream);
    log::debug!("[Playback Thread] Stopped");
    Ok(())
}

/// Build the cpal output stream
fn build_output_stream(
    device: &Device,
    config: &StreamConfig,
    state: Arc<PlaybackState>,
    event_tx: Sender<ThreadEvent>,
) -> Result<Stream, String> {
    let state_clone = Arc::clone(&state);
    let event_tx_data = event_tx.clone();

    // Take the consumer out of the mutex
    // This is safe because we only do this once when building the stream
    let mut consumer = {
        let mut guard = state.ring_buffer_consumer.lock();
        guard.take().ok_or("Ring buffer consumer already taken")?
    };

    let capacity = state.capacity;

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let requested = data.len();

                // Try to read requested amount
                if let Ok(chunk) = consumer.read_chunk(requested) {
                    // Happy path: enough data available
                    let (first, second) = chunk.as_slices();
                    let first_len = first.len();
                    let second_len = second.len();

                    if first_len > 0 {
                        data[..first_len].copy_from_slice(first);
                    }
                    if second_len > 0 {
                        data[first_len..first_len + second_len].copy_from_slice(second);
                    }

                    chunk.commit_all();
                } else {
                    // Not enough data (underrun)
                    let available = consumer.slots();

                    // Read what we have
                    if let Ok(chunk) = consumer.read_chunk(available) {
                        let (first, second) = chunk.as_slices();
                        let first_len = first.len();
                        let second_len = second.len();

                        if first_len > 0 {
                            data[..first_len].copy_from_slice(first);
                        }
                        if second_len > 0 {
                            data[first_len..first_len + second_len].copy_from_slice(second);
                        }
                        chunk.commit_all();
                    }

                    // Zero pad the rest
                    if available < requested {
                        data[available..].fill(0.0);
                    }

                    // Log underrun
                    let current_underruns =
                        state_clone.underrun_count.fetch_add(1, Ordering::Relaxed);
                    if current_underruns % 100 == 0 {
                        event_tx_data.send(ThreadEvent::PlaybackUnderrun).ok();
                    }
                }

                // Update buffer level metric
                let slots = consumer.slots();
                let fill_percent = if capacity > 0 {
                    (slots * 100) / capacity
                } else {
                    0
                };
                state_clone
                    .last_buffer_level
                    .store(fill_percent as u64, Ordering::Relaxed);

                // Apply volume and mute
                let volume = f32::from_bits(state_clone.volume.load(Ordering::Relaxed));
                let muted = state_clone.muted.load(Ordering::Relaxed);

                if muted {
                    data.fill(0.0);
                } else if (volume - 1.0).abs() > 0.001 {
                    for sample in data.iter_mut() {
                        *sample *= volume;
                    }
                }

                // Write to HAL (loopback) - only available with 'hal' feature
                #[cfg(all(target_os = "macos", feature = "hal"))]
                {
                    let mut writer_guard = state_clone.hal_writer.lock();
                    if let Some(writer) = &mut *writer_guard {
                        let written = writer.write(data);
                        if written < data.len() {
                            // Optional: log trace if needed
                        }
                    }
                }
            },
            move |err| {
                log::warn!("[Playback Thread] Stream error: {}", err);
                event_tx
                    .send(ThreadEvent::ProcessingError(format!(
                        "Stream error: {}",
                        err
                    )))
                    .ok();
            },
            None,
        )
        .map_err(|e| format!("Failed to build output stream: {}", e))?;

    Ok(stream)
}
