// ============================================================================
// Playback Thread - cpal Output
// ============================================================================
//
// Highest priority thread that reads from queue and outputs to hardware.
// Must be real-time safe (no allocations, no locks in callback).

use super::{PlaybackCommand, ProcessingMessage, ThreadEvent};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};

#[cfg(target_os = "macos")]
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

/// Ring buffer for audio data
struct RingBuffer {
    buffer: Vec<f32>,
    write_pos: usize,
    read_pos: usize,
    capacity: usize,
}

impl RingBuffer {
    fn new(capacity_frames: usize, channels: usize) -> Self {
        let capacity = capacity_frames * channels;
        Self {
            buffer: vec![0.0; capacity],
            write_pos: 0,
            read_pos: 0,
            capacity,
        }
    }

    /// Write samples to the buffer
    fn write(&mut self, samples: &[f32]) -> usize {
        let mut written = 0;
        for &sample in samples {
            if self.available_write() == 0 {
                break;
            }
            self.buffer[self.write_pos] = sample;
            self.write_pos = (self.write_pos + 1) % self.capacity;
            written += 1;
        }
        written
    }

    /// Read samples from the buffer
    fn read(&mut self, output: &mut [f32]) -> usize {
        let mut read = 0;
        for out_sample in output.iter_mut() {
            if self.available_read() == 0 {
                *out_sample = 0.0; // Underrun - output silence
                read += 1;
                continue;
            }
            *out_sample = self.buffer[self.read_pos];
            self.read_pos = (self.read_pos + 1) % self.capacity;
            read += 1;
        }
        read
    }

    /// Available samples to write
    fn available_write(&self) -> usize {
        if self.write_pos >= self.read_pos {
            self.capacity - (self.write_pos - self.read_pos) - 1
        } else {
            self.read_pos - self.write_pos - 1
        }
    }

    /// Available samples to read
    fn available_read(&self) -> usize {
        if self.write_pos >= self.read_pos {
            self.write_pos - self.read_pos
        } else {
            self.capacity - (self.read_pos - self.write_pos)
        }
    }

    /// Clear the buffer
    fn clear(&mut self) {
        self.write_pos = 0;
        self.read_pos = 0;
        self.buffer.fill(0.0);
    }
}

/// Shared state between thread and cpal callback
struct PlaybackState {
    ring_buffer: parking_lot::Mutex<RingBuffer>,
    volume: Arc<parking_lot::RwLock<f32>>,
    muted: Arc<AtomicBool>,
    underrun_count: Arc<AtomicU64>,
    last_buffer_level: Arc<AtomicU64>, // For tracking buffer fill percentage

    #[cfg(target_os = "macos")]
    hal_writer: parking_lot::Mutex<Option<HalOutputWriter>>,
}

impl PlaybackState {
    fn new(buffer_frames: usize, channels: usize) -> Self {
        #[cfg(target_os = "macos")]
        let hal_writer = HalOutputWriter::new();

        #[cfg(target_os = "macos")]
        if hal_writer.is_none() {
            log::warn!("[Playback Thread] Failed to initialize HAL output writer");
        }

        Self {
            ring_buffer: parking_lot::Mutex::new(RingBuffer::new(buffer_frames, channels)),
            volume: Arc::new(parking_lot::RwLock::new(1.0)),
            muted: Arc::new(AtomicBool::new(false)),
            underrun_count: Arc::new(AtomicU64::new(0)),
            last_buffer_level: Arc::new(AtomicU64::new(100)), // Start at 100%
            #[cfg(target_os = "macos")]
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
    let device = if let Some(device_name) = output_device {
        // Try to find device by name
        log::debug!("[Playback Thread] Looking for device: '{}'", device_name);

        // Case-insensitive pattern matching with exact match priority
        let target_pattern = device_name.to_lowercase();
        let devices: Vec<_> = host
            .output_devices()
            .map_err(|e| format!("Failed to enumerate output devices: {}", e))?
            .collect();

        // First try exact match
        let found_device = devices
            .iter()
            .find(|d| {
                if let Ok(name) = d.name() {
                    name.to_lowercase() == target_pattern
                } else {
                    false
                }
            })
            .cloned()
            // Then try partial match
            .or_else(|| {
                devices
                    .iter()
                    .find(|d| {
                        if let Ok(name) = d.name() {
                            name.to_lowercase().contains(&target_pattern)
                        } else {
                            false
                        }
                    })
                    .cloned()
            });

        match found_device {
            Some(dev) => {
                let dev_name = dev.name().unwrap_or_else(|_| "Unknown".to_string());
                log::debug!("[Playback Thread] Using device: '{}'", dev_name);
                dev
            }
            None => {
                log::info!(
                    "[Playback Thread] Device '{}' not found, using default",
                    device_name
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
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    // Create shared state (ring buffer with ~200ms capacity)
    let buffer_frames = (sample_rate as usize * 200) / 1000; // 200ms
    let mut state = Arc::new(PlaybackState::new(buffer_frames, channels));

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
                    *state.volume.write() = vol;
                }
                PlaybackCommand::Mute(muted) => {
                    state.muted.store(muted, Ordering::Relaxed);
                }
                PlaybackCommand::UpdateChannels(new_channels) => {
                    if new_channels != channels {
                        log::info!(
                            "[Playback Thread] Updating channel count: {} -> {}",
                            channels,
                            new_channels
                        );
                        log::trace!(
                            "[Playback Thread] UpdateChannels: Draining pending frames with old channel count"
                        );

                        // CRITICAL: Drain all pending frames from the message queue
                        // These frames have the OLD channel count and would cause mismatches
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

                        let new_state = Arc::new(PlaybackState::new(buffer_frames, new_channels));

                        // Rebuild and start new stream
                        match build_output_stream(
                            &device,
                            &new_config,
                            Arc::clone(&new_state),
                            event_tx.clone(),
                        ) {
                            Ok(new_stream) => {
                                if let Err(e) = new_stream.play() {
                                    log::warn!(
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
                                    log::info!(
                                        "[Playback Thread] Stream rebuilt with {} channels",
                                        channels
                                    );
                                    log::trace!(
                                        "[Playback Thread] UpdateChannels: Channel update complete, ready for new frames"
                                    );
                                }
                            }
                            Err(e) => {
                                log::warn!("[Playback Thread] Failed to rebuild stream: {}", e);
                                event_tx
                                    .send(ThreadEvent::ProcessingError(format!(
                                        "Playback stream rebuild failed for {} channels: {}",
                                        new_channels, e
                                    )))
                                    .ok();
                            }
                        }
                    }
                }
                PlaybackCommand::Stop => {
                    state.ring_buffer.lock().clear();
                }
                PlaybackCommand::Shutdown => {
                    log::debug!("[Playback Thread] Shutting down");
                    break;
                }
            }
        }

        // Check if ring buffer has space (at least 50% free) before pulling from queue
        let available_space = {
            let ring_buffer = state.ring_buffer.lock();
            ring_buffer.available_write()
        };

        // Only pull from queue if we have space for at least a few frames
        // Previously we kept it at 50% capacity, which caused unnecessary backpressure and underrun risk.
        // Now we allow filling it almost completely.
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

                // Validate channel count matches current configuration
                // This prevents audio corruption during hot-reload when channel count changes
                if frame.num_channels != channels {
                    let count =
                        CHANNEL_MISMATCH_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                    // Log every mismatch for first 10, then every 100th to avoid spam
                    if count < 10 || count % 100 == 0 {
                        log::warn!(
                            "[Playback Thread] Channel mismatch #{}: frame has {} channels, \
                             expected {} - frame discarded (check plugin chain!)",
                            count + 1,
                            frame.num_channels,
                            channels
                        );
                    }

                    // After 1000 consecutive mismatches, this is clearly a stuck state
                    // TODO: Remove this crash after the channel mismatch bug is fully fixed
                    if count >= 1000 {
                        panic!(
                            "[Playback Thread] FATAL: 1000 consecutive channel mismatches. \
                             Plugin chain likely failed to build. Frame: {}ch, Expected: {}ch. \
                             TODO: Remove this panic once channel mismatch bugs are fixed.",
                            frame.num_channels, channels
                        );
                    }

                    continue; // Discard this frame and wait for UpdateChannels command
                }

                // Reset mismatch counter on successful frame
                CHANNEL_MISMATCH_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);

                // Write to ring buffer, handling partial writes
                let mut data_slice = &frame.data[..];

                while !data_slice.is_empty() {
                    let written = state.ring_buffer.lock().write(data_slice);

                    if written < data_slice.len() {
                        // Buffer full, sleep briefly to let audio callback consume
                        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_RINGBUFFER));
                    }

                    // Advance slice
                    data_slice = &data_slice[written..];
                }
            }
            Ok(ProcessingMessage::EndOfStream) => {
                log::debug!("[Playback Thread] End of stream");
                // Could notify manager here
            }
            Ok(ProcessingMessage::Flush) => {
                state.ring_buffer.lock().clear();
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

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // Read from ring buffer
                {
                    let mut ring_buffer = state_clone.ring_buffer.lock();
                    let available = ring_buffer.available_read();
                    let capacity = ring_buffer.capacity;
                    let requested = data.len();

                    // Calculate buffer fill percentage
                    let fill_percent = if capacity > 0 {
                        (available * 100) / capacity
                    } else {
                        0
                    };

                    // Track buffer level changes (log when it drops below certain thresholds)
                    let last_level = state_clone.last_buffer_level.load(Ordering::Relaxed);
                    state_clone
                        .last_buffer_level
                        .store(fill_percent as u64, Ordering::Relaxed);

                    // Log buffer level warnings
                    if fill_percent < 25 && last_level >= 25 {
                        log::warn!(
                            "[Playback] Buffer low: {}% ({}/{} samples, requested: {})",
                            fill_percent,
                            available,
                            capacity,
                            requested
                        );
                    } else if fill_percent < 10 && last_level >= 10 {
                        log::warn!(
                            "[Playback] Buffer critical: {}% ({}/{} samples, requested: {})",
                            fill_percent,
                            available,
                            capacity,
                            requested
                        );
                    }

                    // Detect underrun
                    if available < requested {
                        let current_underruns =
                            state_clone.underrun_count.fetch_add(1, Ordering::Relaxed);
                        event_tx_data.send(ThreadEvent::PlaybackUnderrun).ok();

                        log::warn!(
                            "[Playback] UNDERRUN #{}: buffer has {} samples but need {} ({}% full)",
                            current_underruns + 1,
                            available,
                            requested,
                            fill_percent
                        );

                        // TODO: Remove this crash after underrun bugs are fixed
                        // Hard crash after 50 underruns to aid debugging - the code never recovers anyway
                        if current_underruns + 1 >= 50 {
                            panic!(
                                "[Playback] FATAL: 50 underruns detected - crashing for debugging. \
                                 Buffer: {}/{} samples ({}% full), requested: {}. \
                                 TODO: Remove this panic once underrun bugs are fixed.",
                                available, capacity, fill_percent, requested
                            );
                        }
                    }

                    ring_buffer.read(data);
                };

                // Apply volume and mute
                let volume = *state_clone.volume.read();
                let muted = state_clone.muted.load(Ordering::Relaxed);

                if muted {
                    data.fill(0.0);
                } else if (volume - 1.0).abs() > 0.001 {
                    for sample in data.iter_mut() {
                        *sample *= volume;
                    }
                }

                // Write to HAL (loopback)
                #[cfg(target_os = "macos")]
                {
                    let mut writer_guard = state_clone.hal_writer.lock();
                    if let Some(writer) = &mut *writer_guard {
                        let written = writer.write(data);
                        if written < data.len() {
                            // Optional: log trace if needed, but avoid spamming audio callback
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

#[cfg(test)]
mod tests {
    use super::RingBuffer;

    #[test]
    fn ring_buffer_write_then_read_round_trip() {
        // 4 frames * 2 channels = 8 samples capacity (minus 1 for ring buffer semantics)
        let mut rb = RingBuffer::new(4, 2);

        // Write a small block of samples
        let input = vec![1.0_f32, 2.0, 3.0, 4.0];
        let written = rb.write(&input);
        assert_eq!(written, input.len());
        assert_eq!(rb.available_read(), written);

        // Read back and verify round-trip
        let mut output = vec![0.0_f32; input.len()];
        let read = rb.read(&mut output);
        assert_eq!(read, input.len());
        assert_eq!(output, input);
        assert_eq!(rb.available_read(), 0);
    }

    #[test]
    fn ring_buffer_clear_empties_buffer() {
        let mut rb = RingBuffer::new(2, 2); // 2 frames * 2 channels = 4 samples
        let input = vec![1.0_f32, 2.0, 3.0, 4.0];
        rb.write(&input);
        assert!(rb.available_read() > 0);

        rb.clear();
        assert_eq!(rb.available_read(), 0);
        assert!(rb.available_write() > 0);
    }
}
