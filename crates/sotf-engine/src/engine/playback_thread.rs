// ============================================================================
// Playback Thread - cpal Output
// ============================================================================
//
// Highest priority thread that reads from queue and outputs to hardware.
// Must be real-time safe (no allocations, no locks in callback).

use super::{PlaybackCommand, ProcessingMessage, ThreadEvent};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender};

// HAL writer removed - audio flows: HAL input → decoder thread → processing → cpal output
// No loopback to HAL needed

const SPIN_MS_RINGBUFFER: u64 = 5;
/// Max input channels for the stack-allocated downmix coefficient arrays.
const MAX_DOWNMIX_CH: usize = 16;

fn is_virtual_output_device_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("sotf")
        || lower.contains("blackhole")
        || lower.contains("zoomaudio")
        || lower.contains("loopback")
        || crate::devices::is_null_device(name)
}

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
        buffer_ms: u32,
        channels: usize,
        output_device: Option<String>,
        recycle_tx: SyncSender<Vec<f32>>,
        allow_virtual_output: bool,
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
                    buffer_ms,
                    channels,
                    output_device,
                    recycle_tx,
                    allow_virtual_output,
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

/// Shared state between thread and cpal callback (all fields are lock-free atomics)
struct PlaybackState {
    capacity: usize,
    volume: Arc<AtomicU32>, // Atomic f32 stored as u32 bits
    muted: Arc<AtomicBool>,
    flush_requested: Arc<AtomicBool>,
    underrun_count: Arc<AtomicU64>,
    last_buffer_level: Arc<AtomicU64>, // For tracking buffer fill percentage
    total_callback_samples: Arc<AtomicU64>,
    callback_count: Arc<AtomicU64>,
}

impl PlaybackState {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            muted: Arc::new(AtomicBool::new(false)),
            flush_requested: Arc::new(AtomicBool::new(false)),
            underrun_count: Arc::new(AtomicU64::new(0)),
            last_buffer_level: Arc::new(AtomicU64::new(100)),
            total_callback_samples: Arc::new(AtomicU64::new(0)),
            callback_count: Arc::new(AtomicU64::new(0)),
        }
    }
}

fn playback_buffer_capacity(sample_rate: u32, channels: usize, buffer_ms: u32) -> usize {
    (((sample_rate as u64 * buffer_ms as u64) / 1000) as usize) * channels
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FlushMode {
    Normal,
    DroppingUntilFlush,
    WaitingForDrain,
}

fn request_flush(state: &PlaybackState) {
    state.flush_requested.store(true, Ordering::Relaxed);
}

fn flush_completed(
    state: &PlaybackState,
    producer: &Producer<f32>,
    buffer_capacity: usize,
) -> bool {
    if state.flush_requested.load(Ordering::Relaxed) && producer.slots() >= buffer_capacity {
        state.flush_requested.store(false, Ordering::Relaxed);
    }

    !state.flush_requested.load(Ordering::Relaxed)
}

/// Main playback thread function
fn run_playback_thread(
    message_rx: Receiver<ProcessingMessage>,
    command_rx: Receiver<PlaybackCommand>,
    event_tx: Sender<ThreadEvent>,
    sample_rate: u32,
    buffer_ms: u32,
    initial_channels: usize,
    output_device: Option<String>,
    recycle_tx: SyncSender<Vec<f32>>,
    allow_virtual_output: bool,
) -> Result<(), String> {
    // Elevate thread priority for lowest audio latency
    match super::rt_priority::set_realtime_priority(super::rt_priority::RtPriority::Playback) {
        Ok(true) => log::info!("[Playback Thread] RT priority set successfully"),
        Ok(false) => log::debug!("[Playback Thread] RT priority not available on this platform"),
        Err(e) => log::warn!("[Playback Thread] Failed to set RT priority: {e}"),
    }

    // Initialize cpal host — ASIO host if "ASIO:" prefix, default otherwise
    let host = crate::devices::get_host_for_device(output_device.as_deref());

    // Strip ASIO prefix from device name for actual device lookup
    let output_device = output_device.map(|d| {
        if crate::devices::is_asio_device(&d) {
            crate::devices::strip_asio_prefix(&d).to_string()
        } else {
            d
        }
    });

    // Select output device
    let device = if let Some(device_identifier) = output_device {
        // Try to find device by ID first, then name
        log::debug!(
            "[Playback Thread] Looking for device: '{}'",
            device_identifier
        );

        // HELPER: Find fallback device if requested one is invalid/virtual
        let find_fallback = || -> Result<Device, String> {
            let devices = host.output_devices().map_err(|e| e.to_string())?;
            // Filter out virtual devices to prevent loops
            let get_device_name = |d: &Device| -> String {
                d.description()
                    .map(|desc| desc.name().to_string())
                    .unwrap_or_else(|_| "Unknown".to_string())
            };
            let physical = devices.into_iter().find(|d| {
                let name = get_device_name(d);
                !is_virtual_output_device_name(&name)
            });

            if let Some(dev) = physical {
                log::info!(
                    "[Playback Thread] Using fallback physical device: {}",
                    get_device_name(&dev)
                );
                Ok(dev)
            } else {
                host.default_output_device()
                    .ok_or("No default device found".to_string())
            }
        };

        // If explicitly requested a virtual device (likely by accident due to it being default),
        // force a fallback to avoid feedback loop — unless allow_virtual_output is set
        if is_virtual_output_device_name(&device_identifier) && !allow_virtual_output {
            log::warn!(
                "[Playback Thread] Virtual output device '{}' requested as output - forcing fallback to prevent feedback loop",
                device_identifier
            );
            find_fallback().map_err(|e| format!("Failed to find fallback device: {}", e))?
        } else {
            // Try to find the device using shared logic
            match crate::devices::find_device(&host, &device_identifier, false) {
                Ok(dev) => {
                    let dev_name = dev
                        .description()
                        .map(|d| d.name().to_string())
                        .unwrap_or_else(|_| "Unknown Device".to_string());
                    log::debug!("[Playback Thread] Using device: '{}'", dev_name);
                    dev
                }
                Err(e) => {
                    log::info!(
                        "[Playback Thread] Device '{}' not found (error: {}), using fallback output device",
                        device_identifier,
                        e
                    );
                    find_fallback().map_err(|fallback_err| {
                        format!(
                            "Failed to find fallback output device after lookup error '{}': {}",
                            e, fallback_err
                        )
                    })?
                }
            }
        }
    } else {
        // Use default device
        // CHECK if default device is virtual -> if so, use fallback
        let default_dev = host
            .default_output_device()
            .ok_or("No output device available")?;
        let get_name = |d: &Device| -> String {
            d.description()
                .map(|desc| desc.name().to_string())
                .unwrap_or_else(|_| "Unknown".to_string())
        };
        let name = get_name(&default_dev);

        if is_virtual_output_device_name(&name) {
            log::warn!(
                "[Playback Thread] Default device is '{}' (virtual/null) - finding fallback physical device",
                name
            );
            let devices = host
                .output_devices()
                .map_err(|e| format!("Failed to list devices: {}", e))?;
            let physical = devices.into_iter().find(|d| {
                let n = get_name(d);
                !is_virtual_output_device_name(&n)
            });
            physical.unwrap_or(default_dev)
        } else {
            default_dev
        }
    };

    // Track current channel count (can change dynamically)
    let mut channels = initial_channels;

    // Create stream config - the sample rate has already been verified by the manager
    // via verify_working_sample_rate() before the engine was created.
    let mut config = StreamConfig {
        channels: channels as u16,
        sample_rate,
        buffer_size: cpal::BufferSize::Default,
    };

    // Detect the best output sample format for this device + config.
    // hw_channels may be less than channels if the device doesn't support
    // the requested channel count (e.g. 6ch file on a 2ch HDMI device).
    let (output_format, hw_channels) = choose_output_format(&device, &config);
    if hw_channels != channels as u16 {
        log::warn!(
            "[Playback Thread] Adjusting output channels from {} to {} (device limitation)",
            channels,
            hw_channels
        );
        channels = hw_channels as usize;
        config.channels = hw_channels;
    }

    // Create shared state (ring buffer with ~500ms capacity)
    let mut buffer_capacity = playback_buffer_capacity(sample_rate, channels, buffer_ms);
    let (mut producer, consumer) = RingBuffer::<f32>::new(buffer_capacity);
    let mut state = Arc::new(PlaybackState::new(buffer_capacity));

    // Pre-allocate buffer for channel conversions (fallback downmix/upmix)
    let mut conversion_buffer = Vec::with_capacity(4096);

    // Build cpal stream
    let mut stream = build_output_stream(
        &device,
        &config,
        Arc::clone(&state),
        event_tx.clone(),
        consumer,
        output_format,
    )?;

    // Start stream
    stream
        .play()
        .map_err(|e| format!("Failed to start stream: {}", e))?;
    event_tx
        .send(ThreadEvent::PlaybackChannelsChanged(channels))
        .ok();

    // Get device name for logging
    let device_name = device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    log::info!(
        "[Playback Thread] Started - {}Hz, {} channels, format: {:?}, device: '{}'",
        sample_rate,
        channels,
        output_format,
        device_name
    );

    // Log device config for diagnostics
    if let Ok(actual_config) = device.default_output_config() {
        log::debug!(
            "[Playback Thread] Device default config: {}Hz {}ch {:?} (using {}Hz {}ch)",
            actual_config.sample_rate(),
            actual_config.channels(),
            actual_config.sample_format(),
            sample_rate,
            channels,
        );
    }

    // Record stream start time for rate measurement
    let stream_start_time = std::time::Instant::now();

    // Warn if the device name looks like a virtual device (unless explicitly allowed)
    if is_virtual_output_device_name(&device_name) && !allow_virtual_output {
        log::error!(
            "[Playback Thread] WARNING: Output device '{}' appears to be a virtual device! This will cause a feedback loop.",
            device_name
        );
    }

    // Diagnostic counters for frame accounting
    let mut frames_received: u64 = 0;
    let mut frames_written: u64 = 0;
    let mut frames_dropped: u64 = 0;
    let mut frames_blocked: u64 = 0;
    let mut total_samples_written: u64 = 0;
    let mut last_diagnostic_log = std::time::Instant::now();
    let diagnostic_interval = std::time::Duration::from_secs(5);

    // End-of-stream drain tracking
    let mut end_of_stream = false;
    let mut drain_start: Option<std::time::Instant> = None;
    let drain_timeout = std::time::Duration::from_secs(2);
    let mut flush_mode = FlushMode::Normal;

    // Callback stall detection: if cpal callbacks stop firing for too long
    // while we have data to play, the device is broken (common with HDMI/monitor audio).
    let mut last_callback_count: u64 = 0;
    let mut last_callback_check = std::time::Instant::now();
    let callback_stall_timeout = std::time::Duration::from_secs(3);

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
                PlaybackCommand::UpdateSampleRate(new_sample_rate) => {
                    log::warn!(
                        "[Playback Thread] RECEIVED UpdateSampleRate({}) command, current sample_rate={}",
                        new_sample_rate,
                        config.sample_rate
                    );
                    if new_sample_rate != config.sample_rate {
                        log::info!(
                            "[Playback Thread] Updating sample rate: {} -> {}",
                            config.sample_rate,
                            new_sample_rate
                        );

                        // CRITICAL: Drain all pending frames from the message queue
                        let mut drained_count = 0;
                        while message_rx.try_recv().is_ok() {
                            drained_count += 1;
                        }
                        if drained_count > 0 {
                            log::debug!(
                                "[Playback Thread] Drained {} stale frames during sample rate update",
                                drained_count
                            );
                        }

                        // Build new config with new sample rate
                        let mut new_config = StreamConfig {
                            channels: config.channels,
                            sample_rate: new_sample_rate,
                            buffer_size: config.buffer_size,
                        };

                        // Drain any frames that arrived during setup
                        while message_rx.try_recv().is_ok() {
                            drained_count += 1;
                        }

                        // Stop the old stream
                        if let Err(e) = stream.pause() {
                            log::warn!("[Playback Thread] Failed to pause old stream: {}", e);
                        }
                        std::thread::sleep(std::time::Duration::from_millis(10));

                        // Final drain after stopping
                        while message_rx.try_recv().is_ok() {
                            drained_count += 1;
                        }

                        // Negotiate format/channels BEFORE creating ring buffer so
                        // the buffer is sized for the actual hardware channel count.
                        let (new_format, new_hw_ch) = choose_output_format(&device, &new_config);
                        let mut new_channels = channels;
                        if new_hw_ch != new_config.channels {
                            log::warn!(
                                "[Playback Thread] Adjusting rebuild channels from {} to {}",
                                new_config.channels,
                                new_hw_ch
                            );
                            new_config.channels = new_hw_ch;
                            new_channels = new_hw_ch as usize;
                        }

                        // Create new ring buffer with correct channel count
                        let new_buffer_capacity =
                            playback_buffer_capacity(new_sample_rate, new_channels, buffer_ms);
                        let (new_producer, new_consumer) =
                            RingBuffer::<f32>::new(new_buffer_capacity);

                        let new_state = Arc::new(PlaybackState::new(new_buffer_capacity));

                        log::info!(
                            "[Playback Thread] Building new stream with sample rate: {}Hz, {}ch, format: {:?} (drained {} frames)",
                            new_sample_rate,
                            new_channels,
                            new_format,
                            drained_count
                        );

                        match build_output_stream(
                            &device,
                            &new_config,
                            Arc::clone(&new_state),
                            event_tx.clone(),
                            new_consumer,
                            new_format,
                        ) {
                            Ok(new_stream) => {
                                if let Err(e) = new_stream.play() {
                                    log::error!(
                                        "[Playback Thread] Failed to start new stream: {}",
                                        e
                                    );
                                    event_tx
                                        .send(ThreadEvent::ProcessingError(format!(
                                            "Playback stream start failed for {} sample rate: {}",
                                            new_sample_rate, e
                                        )))
                                        .ok();
                                } else {
                                    stream = new_stream;
                                    config = new_config;
                                    state = new_state;
                                    channels = new_channels;
                                    producer = new_producer;
                                    buffer_capacity = new_buffer_capacity;
                                    event_tx
                                        .send(ThreadEvent::PlaybackChannelsChanged(channels))
                                        .ok();

                                    // Final drain
                                    while message_rx.try_recv().is_ok() {}

                                    log::warn!(
                                        "[Playback Thread] STREAM REBUILT successfully with {}Hz {}ch",
                                        new_sample_rate,
                                        channels
                                    );
                                }
                            }
                            Err(e) => {
                                log::error!(
                                    "[Playback Thread] Failed to build stream for sample rate {}: {}",
                                    new_sample_rate,
                                    e
                                );
                                // Resume the old stream so audio doesn't stop
                                if let Err(resume_err) = stream.play() {
                                    log::error!(
                                        "[Playback Thread] Failed to resume old stream: {}",
                                        resume_err
                                    );
                                }
                                event_tx
                                    .send(ThreadEvent::ProcessingError(format!(
                                        "Playback stream rebuild failed for sample rate {}: {}",
                                        new_sample_rate, e
                                    )))
                                    .ok();
                            }
                        }
                    } else {
                        log::debug!(
                            "[Playback Thread] UpdateSampleRate({}) - no change needed",
                            new_sample_rate
                        );
                    }
                }
                PlaybackCommand::UpdateChannels(mut new_channels) => {
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

                        // Check what the device actually supports BEFORE doing any
                        // disruptive work (pausing stream, draining frames).
                        let probe_config = StreamConfig {
                            channels: new_channels as u16,
                            sample_rate: config.sample_rate,
                            buffer_size: config.buffer_size,
                        };
                        let (new_format, new_hw_ch) = choose_output_format(&device, &probe_config);
                        if new_hw_ch as usize != new_channels {
                            log::warn!(
                                "[Playback Thread] Device adjusts requested {}ch to {}ch",
                                new_channels,
                                new_hw_ch
                            );
                            new_channels = new_hw_ch as usize;
                        }

                        // If the device-adjusted channel count matches current,
                        // skip the stream rebuild entirely. The playback thread
                        // already handles channel mismatches via downmix/upmix
                        // in the frame receive path (frame.num_channels != channels).
                        if new_channels == channels {
                            log::info!(
                                "[Playback Thread] Device adjusted channels back to {} (same as current), \
                                 skipping rebuild. Processing chain output will be converted in the frame receive path.",
                                channels
                            );
                        } else {
                            // Device supports a different channel count — rebuild stream
                            log::trace!(
                                "[Playback Thread] UpdateChannels: Draining pending frames with old channel count"
                            );

                            // Drain pending frames (may have OLD channel count)
                            let mut drained_count = 0;
                            while message_rx.try_recv().is_ok() {
                                drained_count += 1;
                            }

                            let mut new_config = StreamConfig {
                                channels: new_channels as u16,
                                sample_rate: config.sample_rate,
                                buffer_size: config.buffer_size,
                            };
                            new_config.channels = new_hw_ch;

                            let new_buffer_capacity =
                                playback_buffer_capacity(sample_rate, new_channels, buffer_ms);
                            let (new_producer, new_consumer) =
                                RingBuffer::<f32>::new(new_buffer_capacity);
                            let new_state = Arc::new(PlaybackState::new(new_buffer_capacity));

                            let drain_frames = || {
                                let mut count = 0;
                                while message_rx.try_recv().is_ok() {
                                    count += 1;
                                }
                                count
                            };

                            drained_count += drain_frames();

                            if let Err(e) = stream.pause() {
                                log::warn!("[Playback Thread] Failed to pause old stream: {}", e);
                            }
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            drained_count += drain_frames();

                            log::info!(
                                "[Playback Thread] Building new stream with config: {}ch, {}Hz, format: {:?} (drained {} frames)",
                                new_config.channels,
                                new_config.sample_rate,
                                new_format,
                                drained_count
                            );

                            match build_output_stream(
                                &device,
                                &new_config,
                                Arc::clone(&new_state),
                                event_tx.clone(),
                                new_consumer,
                                new_format,
                            ) {
                                Ok(new_stream) => {
                                    log::info!(
                                        "[Playback Thread] Stream built, starting playback..."
                                    );
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
                                        stream = new_stream;
                                        config = new_config;
                                        state = new_state;
                                        channels = new_channels;
                                        producer = new_producer;
                                        buffer_capacity = new_buffer_capacity;
                                        event_tx
                                            .send(ThreadEvent::PlaybackChannelsChanged(channels))
                                            .ok();

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
                                    // Attempt to resume old stream
                                    let resume_result = stream.play();
                                    if let Err(resume_err) = resume_result {
                                        log::error!(
                                            "[Playback Thread] Failed to resume old stream after rebuild failure: {}. \
                                             Playback is dead, exiting playback loop.",
                                            resume_err
                                        );
                                        event_tx
                                            .send(ThreadEvent::ProcessingError(format!(
                                                "Playback stream unrecoverable: rebuild failed ({}) \
                                                 and old stream failed to resume ({})",
                                                e, resume_err
                                            )))
                                            .ok();
                                        break;
                                    }
                                    // Resume succeeded — the callback stall detector will
                                    // catch the stream if it stops producing audio.
                                    // Use ProcessingWarning (not ProcessingError) so the
                                    // manager records the issue without setting Stopped —
                                    // the old stream is still playing.
                                    log::warn!(
                                        "[Playback Thread] Falling back to old stream ({} channels) after rebuild failure",
                                        channels
                                    );
                                    event_tx
                                        .send(ThreadEvent::ProcessingWarning(format!(
                                            "Playback stream rebuild failed for {} channels, falling back to previous configuration",
                                            new_channels
                                        )))
                                        .ok();
                                }
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
                    request_flush(&state);
                    flush_mode = FlushMode::DroppingUntilFlush;
                    end_of_stream = false;
                    drain_start = None;
                }
                PlaybackCommand::Shutdown => {
                    log::debug!("[Playback Thread] Shutting down");
                    break;
                }
            }
        }

        if matches!(flush_mode, FlushMode::WaitingForDrain) {
            if flush_completed(&state, &producer, buffer_capacity) {
                flush_mode = FlushMode::Normal;
            } else {
                std::thread::sleep(std::time::Duration::from_millis(1));
                continue;
            }
        }

        // Callback stall detection: check if cpal callbacks have stopped firing.
        // This catches HDMI/monitor audio devices that accept stream.play() but
        // stop calling the audio callback after a short time.
        // Active during both normal playback AND drain — if callbacks stall during
        // drain, the ring buffer will never empty and we'd hit a silent timeout.
        {
            let current_callbacks = state.callback_count.load(Ordering::Relaxed);
            if current_callbacks != last_callback_count {
                // Callbacks are still firing, reset the timer
                last_callback_count = current_callbacks;
                last_callback_check = std::time::Instant::now();
            } else if last_callback_check.elapsed() > callback_stall_timeout && frames_received > 0
            {
                // Callbacks have stalled for too long while we have data
                let msg = format!(
                    "Audio device '{}' stopped responding (callbacks stalled after {} frames played). \
                     This device may not support sustained playback.",
                    device_name, frames_written
                );
                log::error!("[Playback Thread] {}", msg);
                event_tx.send(ThreadEvent::ProcessingError(msg)).ok();
                break;
            }
        }

        // Check if ring buffer has space
        let available_space = producer.slots();

        // Only pull from queue if we have space for at least a few frames
        let min_space_required = 1024 * channels * 2; // Space for ~2 frames (assuming 1024 size)

        if available_space < min_space_required {
            // Ring buffer is full, sleep briefly and let the audio callback drain it
            std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_RINGBUFFER));
            frames_blocked += 1;
            continue;
        }

        // Periodic diagnostics: log callback rate and buffer stats every few seconds
        if last_diagnostic_log.elapsed() > diagnostic_interval && frames_received > 0 {
            let elapsed = stream_start_time.elapsed().as_secs_f64();
            let total_cb = state.callback_count.load(Ordering::Relaxed);
            let total_cb_samples = state.total_callback_samples.load(Ordering::Relaxed);
            let effective_hz = if elapsed > 0.0 && channels > 0 {
                (total_cb_samples as f64 / channels as f64 / elapsed) as u64
            } else {
                0
            };
            let fill = if buffer_capacity > 0 {
                let slots = producer.slots();
                ((buffer_capacity - slots) * 100) / buffer_capacity
            } else {
                0
            };
            log::debug!(
                "[Playback Thread] PERIODIC: callbacks={}, effective={}Hz (expected {}Hz), \
                 buffer_fill={}%, blocked={}, dropped={}, received={}",
                total_cb,
                effective_hz,
                sample_rate,
                fill,
                frames_blocked,
                frames_dropped,
                frames_received,
            );
            last_diagnostic_log = std::time::Instant::now();
        }

        // Read from message queue (prioritize draining the queue if we have space)
        let message = message_rx.try_recv();

        match message {
            Ok(ProcessingMessage::Frame(frame)) => {
                if matches!(flush_mode, FlushMode::DroppingUntilFlush) {
                    frames_dropped += 1;
                    recycle_tx.try_send(frame.data).ok();
                    continue;
                }

                frames_received += 1;

                // Handle channel count mismatch with robust conversion
                if frame.num_channels != channels {
                    conversion_buffer.clear();
                    let num_frames = frame.num_frames;
                    let target_len = num_frames * channels;
                    conversion_buffer.resize(target_len, 0.0);

                    if frame.num_channels > channels && channels == 2 {
                        // ITU-R BS.775 compliant N→2 downmix with normalization
                        // Channel layouts from speaker_config.rs:
                        //   5ch (5.0): L=0, R=1, C=2, SL=3, SR=4
                        //   6ch (5.1): L=0, R=1, C=2, LFE=3, SL=4, SR=5
                        //   8ch (7.1): L=0, R=1, C=2, LFE=3, SL=4, SR=5, BL=6, BR=7
                        //   10ch+ (Atmos): ..., TFL=8, TFR=9, ...
                        let n = frame.num_channels.min(MAX_DOWNMIX_CH);
                        let has_lfe = n != 5;

                        let (sl_idx, sr_idx) = if has_lfe { (4, 5) } else { (3, 4) };
                        let (bl_idx, br_idx) = if has_lfe { (6, 7) } else { (5, 6) };
                        let (tfl_idx, tfr_idx) = if has_lfe { (8, 9) } else { (7, 8) };

                        const C_COEFF: f32 = 0.707;
                        const SURROUND_COEFF: f32 = 0.707;
                        const BACK_COEFF: f32 = 0.5;
                        const HEIGHT_COEFF: f32 = 0.5;

                        // Build normalization sum from channels actually present
                        let mut coeff_sum: f32 = 1.0; // L or R
                        if n > 2 {
                            coeff_sum += C_COEFF;
                        }
                        if sl_idx < n {
                            coeff_sum += SURROUND_COEFF;
                        }
                        if bl_idx < n {
                            coeff_sum += BACK_COEFF;
                        }
                        if tfl_idx < n {
                            coeff_sum += HEIGHT_COEFF;
                        }
                        let norm = 1.0 / coeff_sum;

                        // Pre-compute per-channel L/R coefficients (stack, no alloc).
                        // Moves all branching out of the inner loop so the dot-product
                        // is branchless with direct indexing (auto-vectorisation friendly).
                        let mut lc = [0.0f32; MAX_DOWNMIX_CH];
                        let mut rc = [0.0f32; MAX_DOWNMIX_CH];
                        lc[0] = norm;
                        rc[1] = norm;
                        if n > 2 {
                            lc[2] = C_COEFF * norm;
                            rc[2] = C_COEFF * norm;
                        }
                        if sl_idx < n {
                            lc[sl_idx] = SURROUND_COEFF * norm;
                        }
                        if sr_idx < n {
                            rc[sr_idx] = SURROUND_COEFF * norm;
                        }
                        if bl_idx < n {
                            lc[bl_idx] = BACK_COEFF * norm;
                        }
                        if br_idx < n {
                            rc[br_idx] = BACK_COEFF * norm;
                        }
                        if tfl_idx < n {
                            lc[tfl_idx] = HEIGHT_COEFF * norm;
                        }
                        if tfr_idx < n {
                            rc[tfr_idx] = HEIGHT_COEFF * norm;
                        }

                        let lc = &lc[..n];
                        let rc = &rc[..n];
                        for i in 0..num_frames {
                            let src = &frame.data[i * n..i * n + n];
                            let mut l = 0.0f32;
                            let mut r = 0.0f32;
                            for ch in 0..n {
                                l += src[ch] * lc[ch];
                                r += src[ch] * rc[ch];
                            }
                            conversion_buffer[i * 2] = l;
                            conversion_buffer[i * 2 + 1] = r;
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
                            conversion_buffer[dst_base..dst_base + shared_channels]
                                .copy_from_slice(&frame.data[src_base..src_base + shared_channels]);
                        }
                    }

                    // Write converted audio
                    let chunk = match producer.write_chunk_uninit(conversion_buffer.len()) {
                        Ok(chunk) => chunk,
                        Err(_) => {
                            // Not enough space — recycle frame data and wait
                            frames_dropped += 1;
                            recycle_tx.try_send(frame.data).ok();
                            std::thread::sleep(std::time::Duration::from_millis(
                                SPIN_MS_RINGBUFFER,
                            ));
                            continue;
                        }
                    };
                    chunk.fill_from_iter(conversion_buffer.iter().copied());
                    recycle_tx.try_send(frame.data).ok();
                    frames_written += 1;
                    total_samples_written += conversion_buffer.len() as u64;
                    continue;
                }

                // Write to ring buffer
                let frame_samples = frame.data.len();
                let chunk = match producer.write_chunk_uninit(frame_samples) {
                    Ok(chunk) => chunk,
                    Err(_) => {
                        // FRAME DROPPED: popped from sync channel but can't write to ring buffer
                        frames_dropped += 1;
                        if frames_dropped % 100 == 1 {
                            log::warn!(
                                "[Playback Thread] FRAME DROPPED count: {} (buffer full)",
                                frames_dropped
                            );
                        }
                        recycle_tx.try_send(frame.data).ok();
                        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_RINGBUFFER));
                        continue;
                    }
                };
                chunk.fill_from_iter(frame.data.iter().copied());
                recycle_tx.try_send(frame.data).ok();
                frames_written += 1;
                total_samples_written += frame_samples as u64;
            }
            Ok(ProcessingMessage::EndOfStream) => {
                if matches!(flush_mode, FlushMode::DroppingUntilFlush) {
                    continue;
                }
                log::debug!("[Playback Thread] End of stream - starting drain");
                end_of_stream = true;
                drain_start = Some(std::time::Instant::now());
            }
            Ok(ProcessingMessage::Flush) => {
                request_flush(&state);
                end_of_stream = false;
                drain_start = None;
                flush_mode = if flush_completed(&state, &producer, buffer_capacity) {
                    FlushMode::Normal
                } else {
                    FlushMode::WaitingForDrain
                };
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                if end_of_stream {
                    // Check if ring buffer has been fully consumed by cpal callback
                    if producer.slots() >= buffer_capacity {
                        log::info!("[Playback Thread] Ring buffer drained, signaling completion");
                        event_tx.send(ThreadEvent::PlaybackDrained).ok();
                        break;
                    }
                    // Safety timeout: if drain takes too long (cpal callback stopped?),
                    // check whether the buffer actually drained or is still full.
                    if let Some(start) = drain_start
                        && start.elapsed() > drain_timeout
                    {
                        let current_slots = producer.slots();
                        let drain_percent = if buffer_capacity > 0 {
                            (current_slots * 100) / buffer_capacity
                        } else {
                            100
                        };
                        if drain_percent < 80 {
                            // Buffer is still mostly full — cpal callbacks are not consuming.
                            // This is not a normal end-of-stream, it's a playback failure.
                            let msg = format!(
                                "Playback stalled: ring buffer {}% full after {}s drain timeout \
                                 (cpal callbacks not consuming audio). Device: '{}'",
                                100 - drain_percent,
                                drain_timeout.as_secs(),
                                device_name,
                            );
                            log::error!("[Playback Thread] {}", msg);
                            event_tx.send(ThreadEvent::ProcessingError(msg)).ok();
                        } else {
                            log::warn!(
                                "[Playback Thread] Drain timeout, buffer mostly empty ({}% drained), signaling completion",
                                drain_percent
                            );
                            event_tx.send(ThreadEvent::PlaybackDrained).ok();
                        }
                        break;
                    }
                    // Still draining, sleep briefly
                    std::thread::sleep(std::time::Duration::from_millis(5));
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                if end_of_stream {
                    // Processing channel closed during drain — wait for ring buffer
                    // to empty so the remaining audio reaches hardware.
                    log::debug!(
                        "[Playback Thread] Queue disconnected during drain, waiting for ring buffer"
                    );
                    let drain_start = std::time::Instant::now();
                    let drain_timeout = std::time::Duration::from_secs(2);
                    loop {
                        if producer.slots() >= buffer_capacity {
                            log::info!(
                                "[Playback Thread] Ring buffer drained (post-disconnect), signaling completion"
                            );
                            event_tx.send(ThreadEvent::PlaybackDrained).ok();
                            break;
                        }
                        if drain_start.elapsed() > drain_timeout {
                            let current_slots = producer.slots();
                            let drain_percent = if buffer_capacity > 0 {
                                (current_slots * 100) / buffer_capacity
                            } else {
                                100
                            };
                            if drain_percent < 80 {
                                let msg = format!(
                                    "Playback stalled after disconnect: ring buffer {}% full after drain timeout. Device: '{}'",
                                    100 - drain_percent,
                                    device_name,
                                );
                                log::error!("[Playback Thread] {}", msg);
                                event_tx.send(ThreadEvent::ProcessingError(msg)).ok();
                            } else {
                                log::warn!(
                                    "[Playback Thread] Drain timeout after disconnect, buffer mostly empty ({}% drained)",
                                    drain_percent
                                );
                                event_tx.send(ThreadEvent::PlaybackDrained).ok();
                            }
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                } else {
                    log::debug!("[Playback Thread] Queue disconnected");
                }
                break;
            }
        }
    }

    // Log cpal callback rate measurement
    let elapsed = stream_start_time.elapsed();
    let elapsed_secs = elapsed.as_secs_f64();
    let total_samples = state.total_callback_samples.load(Ordering::Relaxed);
    let total_callbacks = state.callback_count.load(Ordering::Relaxed);
    let total_frames = if channels > 0 {
        total_samples / channels as u64
    } else {
        0
    };
    let effective_rate = if elapsed_secs > 0.0 {
        (total_frames as f64 / elapsed_secs) as u64
    } else {
        0
    };
    let audio_duration = total_frames as f64 / sample_rate as f64;
    let avg_samples_per_callback = if total_callbacks > 0 {
        total_samples / total_callbacks
    } else {
        0
    };
    log::warn!(
        "[Playback Thread] CALLBACK RATE: {} callbacks, {} total samples ({} frames) in {:.3}s = {} effective Hz (expected {}Hz), audio_duration={:.3}s, avg_samples/callback={}, channels={}",
        total_callbacks,
        total_samples,
        total_frames,
        elapsed_secs,
        effective_rate,
        sample_rate,
        audio_duration,
        avg_samples_per_callback,
        channels,
    );
    log::warn!(
        "[Playback Thread] FRAME ACCOUNTING: received={}, written={}, dropped={}, blocked={}, written_samples={}, written_audio={:.3}s",
        frames_received,
        frames_written,
        frames_dropped,
        frames_blocked,
        total_samples_written,
        total_samples_written as f64 / (channels as f64 * sample_rate as f64),
    );

    // Cleanup
    drop(stream);
    log::debug!("[Playback Thread] Stopped");
    Ok(())
}

/// Choose the best output sample format and channel count supported by the device.
/// Returns (format, hw_channels) where hw_channels may differ from config.channels
/// if the device doesn't support the requested channel count.
/// Prefers F32 > I32 > I16 (highest fidelity first).
fn choose_output_format(device: &Device, config: &StreamConfig) -> (SampleFormat, u16) {
    let supported: Vec<_> = match device.supported_output_configs() {
        Ok(configs) => configs.collect(),
        Err(e) => {
            let fallback = fallback_output_format(
                device
                    .default_output_config()
                    .ok()
                    .map(|cfg| (cfg.sample_format(), cfg.channels())),
                config.channels,
            );
            log::warn!(
                "[Playback Thread] Cannot query supported formats: {}, falling back to {:?}/{}ch",
                e,
                fallback.0,
                fallback.1
            );
            return fallback;
        }
    };
    let candidates: Vec<_> = supported
        .iter()
        .map(|c| {
            (
                c.sample_format(),
                c.channels(),
                c.min_sample_rate(),
                c.max_sample_rate(),
            )
        })
        .collect();

    let log_configs = || {
        supported
            .iter()
            .map(|c| {
                format!(
                    "{:?}/{}ch/{}-{}Hz",
                    c.sample_format(),
                    c.channels(),
                    c.min_sample_rate(),
                    c.max_sample_rate()
                )
            })
            .collect::<Vec<_>>()
    };

    // First try: exact channel count match
    if let Some(fmt) =
        pick_preferred_output_format(&candidates, config.channels, config.sample_rate)
    {
        log::info!(
            "[Playback Thread] Chosen output format: {:?} for {}ch {}Hz (device configs: {:?})",
            fmt,
            config.channels,
            config.sample_rate,
            log_configs()
        );
        return (fmt, config.channels);
    }

    // Second try: find the best alternative channel count.
    // Prefer the highest channel count <= requested (downmix), otherwise lowest available.
    let mut available_channels: Vec<u16> = supported
        .iter()
        .filter(|c| {
            c.min_sample_rate() <= config.sample_rate && c.max_sample_rate() >= config.sample_rate
        })
        .map(|c| c.channels())
        .collect();
    available_channels.sort();
    available_channels.dedup();

    // Pick highest ch <= requested, or fallback to the first available
    let alt_ch = available_channels
        .iter()
        .rev()
        .find(|&&ch| ch <= config.channels)
        .or(available_channels.first())
        .copied();

    if let Some(ch) = alt_ch
        && let Some(fmt) = pick_preferred_output_format(&candidates, ch, config.sample_rate)
    {
        log::warn!(
            "[Playback Thread] Device doesn't support {}ch; using {}ch {:?} (will downmix). Device configs: {:?}",
            config.channels,
            ch,
            fmt,
            log_configs()
        );
        return (fmt, ch);
    }

    log::warn!(
        "[Playback Thread] No compatible config for {}ch {}Hz among device formats, falling back to default format. Device configs: {:?}",
        config.channels,
        config.sample_rate,
        log_configs()
    );
    fallback_output_format(
        device
            .default_output_config()
            .ok()
            .map(|cfg| (cfg.sample_format(), cfg.channels())),
        config.channels,
    )
}

fn pick_preferred_output_format(
    candidates: &[(SampleFormat, u16, cpal::SampleRate, cpal::SampleRate)],
    channels: u16,
    sample_rate: cpal::SampleRate,
) -> Option<SampleFormat> {
    [
        SampleFormat::F32,
        SampleFormat::I32,
        SampleFormat::I16,
        SampleFormat::U32,
        SampleFormat::U16,
    ]
    .into_iter()
    .find(|fmt| {
        candidates.iter().any(|candidate| {
            candidate.0 == *fmt
                && candidate.1 == channels
                && candidate.2 <= sample_rate
                && candidate.3 >= sample_rate
        })
    })
}

fn fallback_output_format(
    default_format_and_channels: Option<(SampleFormat, u16)>,
    requested_channels: u16,
) -> (SampleFormat, u16) {
    default_format_and_channels.unwrap_or((SampleFormat::F32, requested_channels))
}

/// Read f32 samples from the ring buffer into a scratch buffer.
/// Returns `true` if an underrun occurred (not enough data). Handles underrun by zero-filling.
#[inline(always)]
fn read_ring_buffer(
    consumer: &mut Consumer<f32>,
    scratch: &mut [f32],
    requested: usize,
    state: &PlaybackState,
    event_tx: &Sender<ThreadEvent>,
    capacity: usize,
) -> bool {
    // Track sample count (callback_count is tracked by callers, once per cpal callback)
    state
        .total_callback_samples
        .fetch_add(requested as u64, Ordering::Relaxed);

    if state.flush_requested.load(Ordering::Relaxed) {
        let available = consumer.slots().min(requested);
        if available > 0
            && let Ok(chunk) = consumer.read_chunk(available)
        {
            chunk.commit_all();
        }

        scratch[..requested].fill(0.0);

        if consumer.slots() == 0 {
            state.flush_requested.store(false, Ordering::Relaxed);
        }

        let fill_percent = if capacity > 0 {
            (consumer.slots() * 100) / capacity
        } else {
            0
        };
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

        // Zero pad the rest
        if available < requested {
            scratch[available..requested].fill(0.0);
        }

        underrun = true;
        let current_underruns = state.underrun_count.fetch_add(1, Ordering::Relaxed) + 1;
        if current_underruns == 1 || current_underruns.is_multiple_of(100) {
            event_tx
                .send(ThreadEvent::PlaybackUnderrun(current_underruns))
                .ok();
        }
    }

    // Update buffer level metric
    let slots = consumer.slots();
    let fill_percent = if capacity > 0 {
        (slots * 100) / capacity
    } else {
        0
    };
    state
        .last_buffer_level
        .store(fill_percent as u64, Ordering::Relaxed);

    underrun
}

/// Apply volume and mute to f32 scratch buffer, then clamp to [-1, 1].
#[inline(always)]
fn apply_volume_clamp(scratch: &mut [f32], state: &PlaybackState) {
    let volume = f32::from_bits(state.volume.load(Ordering::Relaxed));
    let muted = state.muted.load(Ordering::Relaxed);

    if muted {
        scratch.fill(0.0);
    } else if (volume - 1.0).abs() > 0.001 {
        for sample in scratch.iter_mut() {
            *sample = (*sample * volume).clamp(-1.0, 1.0);
        }
    } else {
        for sample in scratch.iter_mut() {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }
}

/// Build the cpal output stream with the specified sample format.
/// Internal pipeline stays f32; conversion to the hardware format happens
/// only at the final output boundary.
fn build_output_stream(
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
fn build_output_stream_f32(
    device: &Device,
    config: &StreamConfig,
    state: Arc<PlaybackState>,
    event_tx: Sender<ThreadEvent>,
    mut consumer: Consumer<f32>,
) -> Result<Stream, String> {
    let state_clone = Arc::clone(&state);
    let event_tx_data = event_tx.clone();
    let capacity = state.capacity;

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                state_clone.callback_count.fetch_add(1, Ordering::Relaxed);
                read_ring_buffer(
                    &mut consumer,
                    data,
                    data.len(),
                    &state_clone,
                    &event_tx_data,
                    capacity,
                );
                apply_volume_clamp(data, &state_clone);
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

/// Build integer output stream (I16 or I32). Reads f32 from ring buffer
/// into a pre-allocated scratch buffer, applies volume/clamp, then converts
/// to the target integer type.
fn build_output_stream_int<T>(
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
    let event_tx_data = event_tx.clone();
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
                        &event_tx_data,
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
    use super::{
        PlaybackState, fallback_output_format, is_virtual_output_device_name,
        pick_preferred_output_format, playback_buffer_capacity, read_ring_buffer, request_flush,
    };
    use cpal::SampleFormat;
    use rtrb::RingBuffer;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::channel;

    #[test]
    fn read_ring_buffer_discards_samples_while_flush_requested() {
        let (mut producer, mut consumer) = RingBuffer::<f32>::new(8);
        let chunk = producer.write_chunk_uninit(4).unwrap();
        chunk.fill_from_iter([0.25, 0.5, 0.75, 1.0]);

        let state = PlaybackState::new(8);
        request_flush(&state);
        let (event_tx, _event_rx) = channel();
        let mut scratch = [1.0; 4];

        let underrun = read_ring_buffer(&mut consumer, &mut scratch, 4, &state, &event_tx, 8);

        assert!(!underrun);
        assert_eq!(scratch, [0.0; 4]);
        assert_eq!(consumer.slots(), 0);
        assert!(!state.flush_requested.load(Ordering::Relaxed));
    }

    #[test]
    fn read_ring_buffer_keeps_flush_requested_until_buffer_is_empty() {
        let (mut producer, mut consumer) = RingBuffer::<f32>::new(8);
        let chunk = producer.write_chunk_uninit(8).unwrap();
        chunk.fill_from_iter([0.0; 8]);

        let state = PlaybackState::new(8);
        request_flush(&state);
        let (event_tx, _event_rx) = channel();
        let mut scratch = [1.0; 4];

        read_ring_buffer(&mut consumer, &mut scratch, 4, &state, &event_tx, 8);
        assert_eq!(scratch, [0.0; 4]);
        assert_eq!(consumer.slots(), 4);
        assert!(state.flush_requested.load(Ordering::Relaxed));

        read_ring_buffer(&mut consumer, &mut scratch, 4, &state, &event_tx, 8);
        assert_eq!(scratch, [0.0; 4]);
        assert_eq!(consumer.slots(), 0);
        assert!(!state.flush_requested.load(Ordering::Relaxed));
    }

    #[test]
    fn playback_buffer_capacity_uses_configured_buffer_ms() {
        assert_eq!(playback_buffer_capacity(48_000, 2, 200), 19_200);
    }

    #[test]
    fn playback_buffer_capacity_scales_with_latency_budget() {
        assert_eq!(playback_buffer_capacity(48_000, 2, 100), 9_600);
        assert_eq!(playback_buffer_capacity(48_000, 2, 250), 24_000);
    }

    #[test]
    fn pick_preferred_output_format_falls_back_to_unsigned_formats() {
        let candidates = vec![
            (SampleFormat::U16, 2, 44_100, 48_000),
            (SampleFormat::U32, 2, 44_100, 48_000),
        ];

        assert_eq!(
            pick_preferred_output_format(&candidates, 2, 48_000),
            Some(SampleFormat::U32)
        );
    }

    #[test]
    fn pick_preferred_output_format_prefers_signed_formats_before_unsigned() {
        let candidates = vec![
            (SampleFormat::U32, 2, 44_100, 48_000),
            (SampleFormat::I16, 2, 44_100, 48_000),
        ];

        assert_eq!(
            pick_preferred_output_format(&candidates, 2, 48_000),
            Some(SampleFormat::I16)
        );
    }

    #[test]
    fn fallback_output_format_prefers_device_default_when_available() {
        assert_eq!(
            fallback_output_format(Some((SampleFormat::U16, 6)), 2),
            (SampleFormat::U16, 6)
        );
    }

    #[test]
    fn fallback_output_format_defaults_to_f32_requested_channels_when_missing() {
        assert_eq!(fallback_output_format(None, 2), (SampleFormat::F32, 2));
    }

    #[test]
    fn is_virtual_output_device_name_matches_known_virtual_outputs() {
        assert!(is_virtual_output_device_name("SotF Virtual Output"));
        assert!(is_virtual_output_device_name("BlackHole 2ch"));
        assert!(is_virtual_output_device_name("ZoomAudioDevice"));
        assert!(is_virtual_output_device_name("Loopback Audio"));
        assert!(is_virtual_output_device_name("blackhole 2ch"));
        assert!(is_virtual_output_device_name("zoomaudiodevice"));
        assert!(is_virtual_output_device_name("loopback audio"));
    }

    #[test]
    fn is_virtual_output_device_name_allows_regular_physical_outputs() {
        assert!(!is_virtual_output_device_name("Built-in Output"));
    }
}
