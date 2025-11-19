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
                if let Err(e) = run_playback_thread(
                    message_rx,
                    command_rx,
                    event_tx,
                    sample_rate,
                    channels,
                    output_device,
                ) {
                    log::debug!("[Playback Thread] Error: {}", e);
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
}

impl PlaybackState {
    fn new(buffer_frames: usize, channels: usize) -> Self {
        Self {
            ring_buffer: parking_lot::Mutex::new(RingBuffer::new(buffer_frames, channels)),
            volume: Arc::new(parking_lot::RwLock::new(1.0)),
            muted: Arc::new(AtomicBool::new(false)),
            underrun_count: Arc::new(AtomicU64::new(0)),
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

                        // Update channel count
                        channels = new_channels;

                        // Rebuild config
                        config.channels = channels as u16;

                        // Create new ring buffer with updated channel count
                        state = Arc::new(PlaybackState::new(buffer_frames, channels));

                        // Rebuild and start new stream
                        match build_output_stream(
                            &device,
                            &config,
                            Arc::clone(&state),
                            event_tx.clone(),
                        ) {
                            Ok(new_stream) => {
                                if let Err(e) = new_stream.play() {
                                    log::info!(
                                        "[Playback Thread] Failed to start new stream: {}",
                                        e
                                    );
                                } else {
                                    // Replace old stream with new one (old one drops automatically)
                                    stream = new_stream;
                                    log::info!(
                                        "[Playback Thread] Stream rebuilt with {} channels",
                                        channels
                                    );
                                }
                            }
                            Err(e) => {
                                log::debug!("[Playback Thread] Failed to rebuild stream: {}", e);
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

        // Only pull from queue if we have significant space available
        let min_space_required = buffer_frames * channels / 2; // 50% of capacity

        if available_space < min_space_required {
            // Ring buffer is mostly full, sleep briefly and let the audio callback drain it
            std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_RINGBUFFER));
            continue;
        }

        // Read from message queue (non-blocking since we checked space)
        match message_rx.recv_timeout(std::time::Duration::from_millis(SPIN_MS_SIGNAL)) {
            Ok(ProcessingMessage::Frame(frame)) => {
                // Write to ring buffer (should have space now)
                let written = state.ring_buffer.lock().write(&frame.data);
                if written < frame.data.len() {
                    log::info!(
                        "[Playback Thread] Ring buffer full, dropped {} samples",
                        frame.data.len() - written
                    );
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
    let mut last_underrun_count = 0u64;

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // Read from ring buffer
                {
                    let mut ring_buffer = state_clone.ring_buffer.lock();
                    let available = ring_buffer.available_read();

                    // Detect underrun
                    if available < data.len() {
                        let current_underruns =
                            state_clone.underrun_count.fetch_add(1, Ordering::Relaxed);
                        if current_underruns != last_underrun_count {
                            event_tx.send(ThreadEvent::PlaybackUnderrun).ok();
                            last_underrun_count = current_underruns;
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
            },
            |err| {
                log::debug!("[Playback Thread] Stream error: {}", err);
            },
            None,
        )
        .map_err(|e| format!("Failed to build output stream: {}", e))?;

    Ok(stream)
}
