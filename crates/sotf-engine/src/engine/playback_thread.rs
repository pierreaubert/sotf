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
use std::sync::mpsc::{Receiver, Sender, SyncSender};

// HAL writer removed - audio flows: HAL input → decoder thread → processing → cpal output
// No loopback to HAL needed

const SPIN_MS_RINGBUFFER: u64 = 5;
const SPIN_MS_SIGNAL: u64 = 1;
/// Max input channels for the stack-allocated downmix coefficient arrays.
const MAX_DOWNMIX_CH: usize = 16;

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
        recycle_tx: SyncSender<Vec<f32>>,
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
                    recycle_tx,
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
            underrun_count: Arc::new(AtomicU64::new(0)),
            last_buffer_level: Arc::new(AtomicU64::new(100)),
            total_callback_samples: Arc::new(AtomicU64::new(0)),
            callback_count: Arc::new(AtomicU64::new(0)),
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
    recycle_tx: SyncSender<Vec<f32>>,
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
                !name.contains("SotF")
                    && !name.contains("BlackHole")
                    && !name.contains("ZoomAudio")
                    && !name.contains("Loopback")
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
        // force a fallback to avoid feedback loop
        if device_identifier.contains("SotF") {
            log::warn!(
                "[Playback Thread] 'SotF' virtual device requested as output - forcing fallback to prevent feedback loop"
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
                        "[Playback Thread] Device '{}' not found (error: {}), using default",
                        device_identifier,
                        e
                    );
                    host.default_output_device()
                        .ok_or("No default output device available")?
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

        if name.contains("SotF") || name.contains("BlackHole") {
            log::warn!(
                "[Playback Thread] Default device is '{}' (virtual) - finding fallback physical device",
                name
            );
            let devices = host
                .output_devices()
                .map_err(|e| format!("Failed to list devices: {}", e))?;
            let physical = devices.into_iter().find(|d| {
                let n = get_name(d);
                !n.contains("SotF") && !n.contains("BlackHole") && !n.contains("Loopback")
            });
            physical.unwrap_or(default_dev)
        } else {
            default_dev
        }
    };

    // Track current channel count (can change dynamically)
    let mut channels = initial_channels;

    // Create stream config
    let mut config = StreamConfig {
        channels: channels as u16,
        sample_rate: sample_rate,
        buffer_size: cpal::BufferSize::Default,
    };

    // Create shared state (ring buffer with ~500ms capacity)
    let mut buffer_capacity = (sample_rate as usize * 500) / 1000 * channels; // 500ms * channels
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
    )?;

    // Start stream
    stream
        .play()
        .map_err(|e| format!("Failed to start stream: {}", e))?;

    // Get device name for logging
    let device_name = device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    log::info!(
        "[Playback Thread] Started - {}Hz, {} channels, device: '{}'",
        sample_rate,
        channels,
        device_name
    );

    // Verify actual device config matches what we requested
    if let Ok(actual_config) = device.default_output_config() {
        let actual_sr = actual_config.sample_rate();
        let actual_ch = actual_config.channels();
        log::warn!(
            "[Playback Thread] Requested: {}Hz {}ch, Device default config reports: {}Hz {}ch (format: {:?})",
            sample_rate,
            channels,
            actual_sr,
            actual_ch,
            actual_config.sample_format(),
        );
    }

    // Record stream start time for rate measurement
    let stream_start_time = std::time::Instant::now();

    // Warn if the device name looks like a virtual device
    if device_name.contains("SotF") || device_name.contains("BlackHole") {
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

    // End-of-stream drain tracking
    let mut end_of_stream = false;
    let mut drain_start: Option<std::time::Instant> = None;
    let drain_timeout = std::time::Duration::from_secs(2);

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
                        let new_config = StreamConfig {
                            channels: config.channels,
                            sample_rate: new_sample_rate,
                            buffer_size: config.buffer_size.clone(),
                        };

                        // Create new ring buffer
                        let new_buffer_capacity =
                            (new_sample_rate as usize * 500) / 1000 * channels;
                        let (new_producer, new_consumer) =
                            RingBuffer::<f32>::new(new_buffer_capacity);

                        let new_state = Arc::new(PlaybackState::new(new_buffer_capacity));

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

                        log::info!(
                            "[Playback Thread] Building new stream with sample rate: {}Hz (drained {} frames)",
                            new_sample_rate,
                            drained_count
                        );

                        match build_output_stream(
                            &device,
                            &new_config,
                            Arc::clone(&new_state),
                            event_tx.clone(),
                            new_consumer,
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
                                    producer = new_producer;
                                    buffer_capacity = new_buffer_capacity;

                                    // Final drain
                                    while message_rx.try_recv().is_ok() {}

                                    log::warn!(
                                        "[Playback Thread] STREAM REBUILT successfully with sample rate {}Hz",
                                        new_sample_rate
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
                            (sample_rate as usize * 500) / 1000 * new_channels;
                        let (new_producer, new_consumer) =
                            RingBuffer::<f32>::new(new_buffer_capacity);

                        let new_state = Arc::new(PlaybackState::new(new_buffer_capacity));

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
                            new_consumer,
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
                                    producer = new_producer;
                                    buffer_capacity = new_buffer_capacity;

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
                                // Resume the old stream so audio doesn't stop
                                if let Err(resume_err) = stream.play() {
                                    log::error!(
                                        "[Playback Thread] Failed to resume old stream: {}",
                                        resume_err
                                    );
                                }
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
            frames_blocked += 1;
            continue;
        }

        // Read from message queue (prioritize draining the queue if we have space)
        let message = message_rx.try_recv();

        match message {
            Ok(ProcessingMessage::Frame(frame)) => {
                frames_received += 1;

                // Track consecutive channel mismatches to detect stuck state vs transient hot-reload
                static CHANNEL_MISMATCH_COUNT: std::sync::atomic::AtomicU32 =
                    std::sync::atomic::AtomicU32::new(0);

                // Handle channel count mismatch with robust conversion
                if frame.num_channels != channels {
                    CHANNEL_MISMATCH_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

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

                        let mut coeff_sum: f32 = 1.0 + C_COEFF + SURROUND_COEFF;
                        if n > sl_idx + 2 {
                            coeff_sum += BACK_COEFF;
                        }
                        if n > tfl_idx {
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
                    recycle_tx.send(frame.data).ok();
                    continue;
                }

                CHANNEL_MISMATCH_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);

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
                        recycle_tx.send(frame.data).ok();
                        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_RINGBUFFER));
                        continue;
                    }
                };
                chunk.fill_from_iter(frame.data.iter().copied());
                recycle_tx.send(frame.data).ok();
                frames_written += 1;
                total_samples_written += frame_samples as u64;
            }
            Ok(ProcessingMessage::EndOfStream) => {
                log::debug!("[Playback Thread] End of stream - starting drain");
                end_of_stream = true;
                drain_start = Some(std::time::Instant::now());
            }
            Ok(ProcessingMessage::Flush) => {
                // Cannot easily clear rtrb producer side without consumer access
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
                    // force completion to avoid hanging forever.
                    if let Some(start) = drain_start {
                        if start.elapsed() > drain_timeout {
                            log::warn!(
                                "[Playback Thread] Drain timeout, forcing PlaybackDrained (slots={}, capacity={})",
                                producer.slots(),
                                buffer_capacity
                            );
                            event_tx.send(ThreadEvent::PlaybackDrained).ok();
                            break;
                        }
                    }
                    // Still draining, sleep briefly
                    std::thread::sleep(std::time::Duration::from_millis(5));
                } else {
                    // No message available, sleep briefly to avoid 100% CPU
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
                            log::warn!(
                                "[Playback Thread] Drain timeout after disconnect, forcing PlaybackDrained"
                            );
                            event_tx.send(ThreadEvent::PlaybackDrained).ok();
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

/// Build the cpal output stream
fn build_output_stream(
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
                let requested = data.len();

                // Track callback metrics
                state_clone
                    .total_callback_samples
                    .fetch_add(requested as u64, Ordering::Relaxed);
                state_clone.callback_count.fetch_add(1, Ordering::Relaxed);

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
                    // Cap available to requested to avoid buffer overflow
                    let available = consumer.slots().min(requested);

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
                    // Fused volume + clamp in one pass (half the memory bandwidth)
                    for sample in data.iter_mut() {
                        *sample = (*sample * volume).clamp(-1.0, 1.0);
                    }
                } else {
                    // Branchless clamp — compiles to vmaxps/vminps (auto-vectorised)
                    for sample in data.iter_mut() {
                        *sample = sample.clamp(-1.0, 1.0);
                    }
                }

                // Audio flows directly to hardware via cpal - no HAL loopback needed
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
