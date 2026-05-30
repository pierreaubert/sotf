// ============================================================================
// Playback Thread - cpal Output
// ============================================================================
//
// Highest priority thread that reads from queue and outputs to hardware.
// Must be real-time safe (no allocations, no locks in callback).

use super::{PlaybackCommand, ProcessingMessage, ThreadEvent};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use rtrb::{Consumer, CopyToUninit, Producer, RingBuffer, chunks::WriteChunkUninit};
use sotf_types::{OutputAccessMode, OutputAccessStatus};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender};

// HAL writer removed - audio flows: HAL input → decoder thread → processing → cpal output
// No loopback to HAL needed

const SPIN_MS_RINGBUFFER: u64 = 5;
/// Max input channels for the stack-allocated downmix coefficient arrays.
const MAX_DOWNMIX_CH: usize = 32;

/// Bulk-copy a slice into a ring buffer chunk using memcpy instead of per-element iteration.
/// For 96K f32 samples this is ~2× faster than `fill_from_iter`.
fn write_chunk_bulk(mut chunk: WriteChunkUninit<'_, f32>, data: &[f32]) {
    let (first, second) = chunk.as_mut_slices();
    let first_len = first.len().min(data.len());
    data[..first_len].copy_to_uninit(&mut first[..first_len]);
    let remaining = data.len() - first_len;
    if remaining > 0 {
        let second_len = second.len().min(remaining);
        data[first_len..first_len + second_len].copy_to_uninit(&mut second[..second_len]);
    }
    // Safety: we've initialized exactly data.len() elements via copy_to_uninit (memcpy).
    unsafe { chunk.commit(data.len()) };
}

fn send_playback_event(event_tx: &Sender<ThreadEvent>, event: ThreadEvent, context: &str) {
    if let Err(e) = event_tx.send(event) {
        crate::rate_limited_log!(
            trace,
            5,
            "[Playback Thread] Dropped event in {}: {}",
            context,
            e
        );
    }
}

fn recycle_frame_data(recycle_tx: &SyncSender<Vec<f32>>, data: Vec<f32>, context: &str) {
    if let Err(e) = recycle_tx.try_send(data) {
        crate::rate_limited_log!(
            trace,
            5,
            "[Playback Thread] Dropped recycled frame buffer in {}: {}",
            context,
            e
        );
    }
}

fn is_virtual_output_device_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("sotf")
        || lower.contains("blackhole")
        || lower.contains("zoomaudio")
        || lower.contains("loopback")
        || lower.contains("virtual")
        || lower.contains("soundflower")
        || lower.contains("background music")
        || lower.contains("audio bridge")
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
        frame_size: usize,
        output_device: Option<String>,
        recycle_tx: SyncSender<Vec<f32>>,
        allow_virtual_output: bool,
        output_access: OutputAccessMode,
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
                    frame_size,
                    output_device,
                    recycle_tx,
                    allow_virtual_output,
                    output_access,
                ) {
                    log::debug!("[Playback Thread] Error: {}", e);
                    send_playback_event(
                        &error_tx,
                        ThreadEvent::ProcessingError(format!("Playback thread error: {}", e)),
                        "thread error",
                    );
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
        if let Err(e) = self.send_command(PlaybackCommand::Shutdown) {
            log::trace!("[Playback Thread] Shutdown command receiver dropped: {}", e);
        }
        if let Some(handle) = self.thread_handle.take() {
            if let Err(e) = handle.join() {
                log::warn!("[Playback Thread] Thread panicked during shutdown: {:?}", e);
            }
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
    stream_error_count: Arc<AtomicU64>,
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
            stream_error_count: Arc::new(AtomicU64::new(0)),
        }
    }
}

fn playback_buffer_capacity(sample_rate: u32, channels: usize, buffer_ms: u32) -> usize {
    (((sample_rate as u64 * buffer_ms as u64) / 1000) as usize) * channels
}

/// Push `samples` zeros into the producer to give the cpal callback a cushion
/// before real audio arrives. Silently truncates if the ring has less free
/// space (newly-created ring is fully empty so this only happens after a
/// rebuild that races with cpal startup).
fn prefill_silence(producer: &mut Producer<f32>, samples: usize) {
    let to_write = samples.min(producer.slots());
    if to_write == 0 {
        return;
    }
    let Ok(mut chunk) = producer.write_chunk_uninit(to_write) else {
        return;
    };
    let (first, second) = chunk.as_mut_slices();
    for slot in first.iter_mut() {
        slot.write(0.0);
    }
    for slot in second.iter_mut() {
        slot.write(0.0);
    }
    // Safety: we initialized exactly `to_write` elements across the two slices.
    unsafe { chunk.commit(to_write) };
}

fn copy_playback_controls(from: &PlaybackState, to: &PlaybackState) {
    to.volume
        .store(from.volume.load(Ordering::Relaxed), Ordering::Relaxed);
    to.muted
        .store(from.muted.load(Ordering::Relaxed), Ordering::Relaxed);
}

fn select_playback_device(
    host: &cpal::Host,
    output_device: Option<&str>,
    allow_virtual_output: bool,
) -> Result<Device, String> {
    let get_name = |d: &Device| -> String {
        d.description()
            .map(|desc| desc.name().to_string())
            .unwrap_or_else(|_| "Unknown".to_string())
    };

    let find_physical_output = || -> Result<Device, String> {
        let devices = host.output_devices().map_err(|e| e.to_string())?;
        let physical = devices.into_iter().find(|d| {
            let name = get_name(d);
            !is_virtual_output_device_name(&name)
        });

        if let Some(dev) = physical {
            log::info!(
                "[Playback Thread] Using fallback physical device: {}",
                get_name(&dev)
            );
            Ok(dev)
        } else {
            Err("No physical output device found".to_string())
        }
    };

    if let Some(device_identifier) = output_device {
        log::debug!(
            "[Playback Thread] Looking for device: '{}'",
            device_identifier
        );

        if is_virtual_output_device_name(device_identifier) && !allow_virtual_output {
            return Err(format!(
                "Selected output device '{}' is virtual/loopback and cannot be used as speaker output",
                device_identifier
            ));
        }

        match crate::devices::find_device(host, device_identifier, false) {
            Ok(dev) => {
                log::debug!("[Playback Thread] Using device: '{}'", get_name(&dev));
                Ok(dev)
            }
            Err(e) => {
                log::warn!(
                    "[Playback Thread] Explicit output device '{}' was not found: {}",
                    device_identifier,
                    e
                );
                Err(format!(
                    "Selected output device '{}' is not available: {}",
                    device_identifier, e
                ))
            }
        }
    } else if !allow_virtual_output {
        // In systemwide mode the macOS default output is the SotF virtual
        // device. Do not open it even transiently: CoreAudio can keep the
        // daemon registered as an active virtual-device client, which starves
        // the app-audio ingress path.
        find_physical_output()
    } else {
        let default_dev = host
            .default_output_device()
            .ok_or("No output device available")?;
        Ok(default_dev)
    }
}

#[cfg(target_os = "macos")]
fn coreaudio_output_device_id(name: &str) -> Option<u32> {
    coreaudio::audio_unit::macos_helpers::get_device_id_from_name(name, false)
}

#[cfg(not(target_os = "macos"))]
fn coreaudio_output_device_id(_name: &str) -> Option<u32> {
    None
}

#[cfg(target_os = "macos")]
#[derive(Default)]
struct CoreAudioExclusiveModeGuard {
    device_id: Option<u32>,
    device_name: String,
    acquired_by_guard: bool,
}

#[cfg(target_os = "macos")]
impl CoreAudioExclusiveModeGuard {
    fn inactive() -> Self {
        Self::default()
    }

    fn activate_for_device(
        &mut self,
        device_name: &str,
        mode: OutputAccessMode,
    ) -> Result<OutputAccessStatus, String> {
        if !mode.prefers_exclusive() {
            self.release();
            return Ok(OutputAccessStatus::Shared);
        }

        let Some(device_id) = coreaudio_output_device_id(device_name) else {
            return self.unavailable_for_mode(
                device_name,
                mode,
                "CoreAudio device id could not be resolved".to_string(),
            );
        };

        let current_pid = std::process::id() as i32;
        if self.acquired_by_guard && self.device_id == Some(device_id) {
            match coreaudio::audio_unit::macos_helpers::get_hogging_pid(device_id) {
                Ok(owner) if owner == current_pid => {
                    return Ok(OutputAccessStatus::ExclusiveActive);
                }
                Ok(owner) => {
                    log::warn!(
                        "[Playback Thread] CoreAudio exclusive ownership for '{}' moved to pid {}; reacquiring",
                        self.device_name,
                        owner
                    );
                    self.device_id = None;
                    self.device_name.clear();
                    self.acquired_by_guard = false;
                }
                Err(e) => {
                    return self.unavailable_for_mode(
                        device_name,
                        mode,
                        format!("CoreAudio hog-mode owner query failed: {}", e),
                    );
                }
            }
        }

        self.release();

        let owner = match coreaudio::audio_unit::macos_helpers::get_hogging_pid(device_id) {
            Ok(owner) => owner,
            Err(e) => {
                return self.unavailable_for_mode(
                    device_name,
                    mode,
                    format!("CoreAudio hog-mode owner query failed: {}", e),
                );
            }
        };

        if owner == current_pid {
            self.device_id = Some(device_id);
            self.device_name = device_name.to_string();
            self.acquired_by_guard = false;
            return Ok(OutputAccessStatus::ExclusiveActive);
        }

        if owner != -1 {
            return self.unavailable_for_mode(
                device_name,
                mode,
                format!("device is already hogged by pid {}", owner),
            );
        }

        let new_owner = match coreaudio::audio_unit::macos_helpers::toggle_hog_mode(device_id) {
            Ok(owner) => owner,
            Err(e) => {
                return self.unavailable_for_mode(
                    device_name,
                    mode,
                    format!("CoreAudio hog-mode acquisition failed: {}", e),
                );
            }
        };

        if new_owner == current_pid {
            self.device_id = Some(device_id);
            self.device_name = device_name.to_string();
            self.acquired_by_guard = true;
            Ok(OutputAccessStatus::ExclusiveActive)
        } else {
            self.unavailable_for_mode(
                device_name,
                mode,
                format!("CoreAudio returned hog owner pid {}", new_owner),
            )
        }
    }

    fn unavailable_for_mode(
        &mut self,
        device_name: &str,
        mode: OutputAccessMode,
        reason: String,
    ) -> Result<OutputAccessStatus, String> {
        self.release();
        if mode.requires_exclusive() {
            Err(format!(
                "Exclusive output is required, but CoreAudio exclusive mode could not be acquired for '{}': {}",
                device_name, reason
            ))
        } else {
            log::warn!(
                "[Playback Thread] CoreAudio exclusive output unavailable for '{}': {}; falling back to shared output",
                device_name,
                reason
            );
            Ok(OutputAccessStatus::FallbackShared)
        }
    }

    fn release(&mut self) {
        if self.acquired_by_guard
            && let Some(device_id) = self.device_id
        {
            let current_pid = std::process::id() as i32;
            match coreaudio::audio_unit::macos_helpers::get_hogging_pid(device_id) {
                Ok(owner) if owner == current_pid => {
                    match coreaudio::audio_unit::macos_helpers::toggle_hog_mode(device_id) {
                        Ok(-1) => {
                            log::info!(
                                "[Playback Thread] Released CoreAudio exclusive mode for '{}'",
                                self.device_name
                            );
                        }
                        Ok(owner) => {
                            log::warn!(
                                "[Playback Thread] CoreAudio exclusive release for '{}' left owner pid {}",
                                self.device_name,
                                owner
                            );
                        }
                        Err(e) => {
                            log::warn!(
                                "[Playback Thread] Failed to release CoreAudio exclusive mode for '{}': {}",
                                self.device_name,
                                e
                            );
                        }
                    }
                }
                Ok(owner) => {
                    log::debug!(
                        "[Playback Thread] CoreAudio exclusive mode for '{}' is now owned by pid {}; not releasing",
                        self.device_name,
                        owner
                    );
                }
                Err(e) => {
                    log::warn!(
                        "[Playback Thread] Failed to query CoreAudio exclusive owner during release for '{}': {}",
                        self.device_name,
                        e
                    );
                }
            }
        }

        self.device_id = None;
        self.device_name.clear();
        self.acquired_by_guard = false;
    }
}

#[cfg(target_os = "macos")]
impl Drop for CoreAudioExclusiveModeGuard {
    fn drop(&mut self) {
        self.release();
    }
}

struct RebuiltPlaybackStream {
    device: Device,
    device_name: String,
    stream: Stream,
    producer: Producer<f32>,
    state: Arc<PlaybackState>,
    config: StreamConfig,
    output_format: SampleFormat,
    channels: usize,
    buffer_capacity: usize,
}

struct RebuildPlaybackParams<'a> {
    output_device: Option<&'a str>,
    allow_virtual_output: bool,
    sample_rate: u32,
    requested_channels: usize,
    buffer_ms: u32,
    buffer_size: cpal::BufferSize,
    event_tx: Sender<ThreadEvent>,
    old_state: &'a PlaybackState,
}

fn rebuild_playback_stream(
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

fn playback_recovery_reason(
    current_stream_errors: u64,
    last_stream_error_count: &mut u64,
    current_callbacks: u64,
    last_callback_count: &mut u64,
    last_callback_check: &mut std::time::Instant,
    callback_stall_timeout: std::time::Duration,
    frames_received: u64,
    frames_written: u64,
    coreaudio_identity_reason: Option<String>,
) -> Option<String> {
    if current_stream_errors != *last_stream_error_count {
        *last_stream_error_count = current_stream_errors;
        Some(format!(
            "stream error reported by CoreAudio ({} total)",
            current_stream_errors
        ))
    } else if current_callbacks != *last_callback_count {
        *last_callback_count = current_callbacks;
        *last_callback_check = std::time::Instant::now();
        None
    } else if let Some(reason) = coreaudio_identity_reason {
        Some(reason)
    } else if last_callback_check.elapsed() > callback_stall_timeout && frames_received > 0 {
        Some(format!(
            "callbacks stalled after {} frames played",
            frames_written
        ))
    } else {
        None
    }
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

fn output_access_status_for_device(
    mode: OutputAccessMode,
    output_device: Option<&str>,
) -> OutputAccessStatus {
    match mode {
        OutputAccessMode::Shared => OutputAccessStatus::Shared,
        OutputAccessMode::ExclusivePreferred | OutputAccessMode::ExclusiveRequired => {
            if output_device.is_some_and(crate::devices::is_asio_device) {
                OutputAccessStatus::ExclusiveActive
            } else if cfg!(target_os = "macos") {
                OutputAccessStatus::ExclusivePending
            } else if matches!(mode, OutputAccessMode::ExclusivePreferred) {
                OutputAccessStatus::FallbackShared
            } else {
                OutputAccessStatus::Unsupported
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn set_output_access_status(
    event_tx: &Sender<ThreadEvent>,
    status: &mut OutputAccessStatus,
    new_status: OutputAccessStatus,
    context: &str,
) {
    if *status == new_status {
        return;
    }
    *status = new_status;
    send_playback_event(
        event_tx,
        ThreadEvent::PlaybackOutputAccessChanged(new_status),
        context,
    );
}

fn initial_buffer_size(status: OutputAccessStatus, frame_size: usize) -> cpal::BufferSize {
    if status == OutputAccessStatus::ExclusiveActive {
        cpal::BufferSize::Fixed(frame_size.clamp(1, u32::MAX as usize) as u32)
    } else {
        cpal::BufferSize::Default
    }
}

/// Main playback thread function
fn run_playback_thread(
    message_rx: Receiver<ProcessingMessage>,
    command_rx: Receiver<PlaybackCommand>,
    event_tx: Sender<ThreadEvent>,
    sample_rate: u32,
    buffer_ms: u32,
    initial_channels: usize,
    frame_size: usize,
    output_device: Option<String>,
    recycle_tx: SyncSender<Vec<f32>>,
    allow_virtual_output: bool,
    output_access: OutputAccessMode,
) -> Result<(), String> {
    // Elevate thread priority for lowest audio latency
    match super::rt_priority::set_realtime_priority(super::rt_priority::RtPriority::Playback) {
        Ok(true) => log::info!("[Playback Thread] RT priority set successfully"),
        Ok(false) => log::debug!("[Playback Thread] RT priority not available on this platform"),
        Err(e) => log::warn!("[Playback Thread] Failed to set RT priority: {e}"),
    }

    // Initialize cpal host — ASIO host if "ASIO:" prefix, default otherwise
    let host = crate::devices::get_host_for_device(output_device.as_deref());
    #[cfg(target_os = "macos")]
    let backend_exclusive_active = output_device
        .as_deref()
        .is_some_and(crate::devices::is_asio_device);
    let mut output_access_status =
        output_access_status_for_device(output_access, output_device.as_deref());
    if output_access.requires_exclusive() && output_access_status == OutputAccessStatus::Unsupported
    {
        return Err(
            "Exclusive output is required, but the selected cpal backend cannot open an exclusive stream"
                .to_string(),
        );
    }

    // Strip ASIO prefix from device name for actual device lookup
    let output_device = output_device.map(|d| {
        if crate::devices::is_asio_device(&d) {
            crate::devices::strip_asio_prefix(&d).to_string()
        } else {
            d
        }
    });

    // Select output device. Keep the sanitized device name so recovery can
    // re-resolve the CoreAudio device if the current handle is invalidated.
    let mut device = select_playback_device(&host, output_device.as_deref(), allow_virtual_output)?;
    let mut device_name = device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    #[cfg(target_os = "macos")]
    let mut coreaudio_exclusive_mode = CoreAudioExclusiveModeGuard::inactive();

    #[cfg(target_os = "macos")]
    if output_access_status == OutputAccessStatus::ExclusivePending {
        let activated =
            coreaudio_exclusive_mode.activate_for_device(&device_name, output_access)?;
        output_access_status = activated;
    }

    // Track current channel count (can change dynamically)
    let mut channels = initial_channels;

    // Create stream config - the sample rate has already been verified by the manager
    // via verify_working_sample_rate() before the engine was created.
    let mut config = StreamConfig {
        channels: channels as u16,
        sample_rate,
        buffer_size: initial_buffer_size(output_access_status, frame_size),
    };

    // Detect the best output sample format for this device + config.
    // hw_channels may be less than channels if the device doesn't support
    // the requested channel count (e.g. 6ch file on a 2ch HDMI device).
    let (mut output_format, hw_channels) = choose_output_format(&device, &config);
    if hw_channels != channels as u16 {
        log::info!(
            "[Playback Thread] Adjusting output channels from {} to {} (device limitation)",
            channels,
            hw_channels
        );
        channels = hw_channels as usize;
        config.channels = hw_channels;
    }

    // Query device's native (maximum) channel count for retry fallback.
    let device_native_channels: Option<u16> = device
        .supported_output_configs()
        .ok()
        .and_then(|configs| configs.map(|c| c.channels()).max());

    // Create shared state (ring buffer with ~500ms capacity)
    let mut buffer_capacity = playback_buffer_capacity(sample_rate, channels, buffer_ms);
    let (mut producer, consumer) = RingBuffer::<f32>::new(buffer_capacity);
    let mut state = Arc::new(PlaybackState::new(buffer_capacity));

    // Pre-fill the ring with silence to give cpal a cushion before the producer
    // starts feeding real audio. Without this the cpal callback fires before any
    // AudioFrame has arrived (producer takes a few ms to spin up and the upstream
    // pipeline takes longer when the source is HAL-driven), every callback
    // underruns until the queue stabilises, and steady-state callback timing
    // jitter then keeps poking through to a near-empty queue. Half the ring is
    // ~100 ms of latency at 200 ms buffer_ms — enough cushion to absorb cpal
    // callback variance, small enough not to be perceptible.
    prefill_silence(&mut producer, buffer_capacity / 2);

    // Pre-allocate buffer for channel conversions (fallback downmix/upmix)
    let mut conversion_buffer = Vec::with_capacity(4096);

    // Build cpal stream — retry with device native channel count if the requested
    // count is rejected (some pro interfaces only accept their max channel count).
    let mut stream = match build_output_stream(
        &device,
        &config,
        Arc::clone(&state),
        event_tx.clone(),
        consumer,
        output_format,
    ) {
        Ok(s) => s,
        Err(e) => {
            let native_ch = device_native_channels.unwrap_or(channels as u16);
            if native_ch != channels as u16 {
                log::warn!(
                    "[Playback Thread] Stream build failed with {}ch ({}) — retrying with device native {}ch",
                    channels,
                    e,
                    native_ch
                );
                channels = native_ch as usize;
                config.channels = native_ch;
                buffer_capacity = playback_buffer_capacity(sample_rate, channels, buffer_ms);
                let (mut new_producer, new_consumer) = RingBuffer::<f32>::new(buffer_capacity);
                state = Arc::new(PlaybackState::new(buffer_capacity));
                prefill_silence(&mut new_producer, buffer_capacity / 2);
                producer = new_producer;
                build_output_stream(
                    &device,
                    &config,
                    Arc::clone(&state),
                    event_tx.clone(),
                    new_consumer,
                    output_format,
                )?
            } else {
                return Err(e);
            }
        }
    };

    // Start stream
    stream
        .play()
        .map_err(|e| format!("Failed to start stream: {}", e))?;
    send_playback_event(
        &event_tx,
        ThreadEvent::PlaybackChannelsChanged(channels),
        "initial playback channels",
    );

    let mut coreaudio_device_id = coreaudio_output_device_id(&device_name);
    send_playback_event(
        &event_tx,
        ThreadEvent::PlaybackOutputDeviceChanged(device_name.clone()),
        "initial playback output device",
    );
    send_playback_event(
        &event_tx,
        ThreadEvent::PlaybackOutputAccessChanged(output_access_status),
        "initial playback output access",
    );

    log::info!(
        "[Playback Thread] Started - {}Hz, {} channels, format: {:?}, access: {:?}, device: '{}'",
        sample_rate,
        channels,
        output_format,
        output_access_status,
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
    let mut last_stream_error_count: u64 = 0;
    let mut last_recovery_attempt = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(10))
        .unwrap_or_else(std::time::Instant::now);
    let recovery_retry_interval = std::time::Duration::from_millis(500);
    let mut last_device_identity_check = std::time::Instant::now();
    let device_identity_check_interval = std::time::Duration::from_secs(2);
    let mut last_reported_underruns: u64 = 0;

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
                    log::debug!(
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
                                    send_playback_event(
                                        &event_tx,
                                        ThreadEvent::ProcessingError(format!(
                                            "Playback stream start failed for {} sample rate: {}",
                                            new_sample_rate, e
                                        )),
                                        "sample-rate stream start failure",
                                    );
                                } else {
                                    stream = new_stream;
                                    config = new_config;
                                    state = new_state;
                                    channels = new_channels;
                                    producer = new_producer;
                                    buffer_capacity = new_buffer_capacity;
                                    send_playback_event(
                                        &event_tx,
                                        ThreadEvent::PlaybackChannelsChanged(channels),
                                        "sample-rate rebuild channels",
                                    );

                                    // Final drain
                                    while message_rx.try_recv().is_ok() {}

                                    log::info!(
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
                                send_playback_event(
                                    &event_tx,
                                    ThreadEvent::ProcessingError(format!(
                                        "Playback stream rebuild failed for sample rate {}: {}",
                                        new_sample_rate, e
                                    )),
                                    "sample-rate rebuild failure",
                                );
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
                    log::debug!(
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
                            log::info!(
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
                                        send_playback_event(
                                            &event_tx,
                                            ThreadEvent::ProcessingError(format!(
                                                "Playback stream start failed for {} channels: {}",
                                                new_channels, e
                                            )),
                                            "channel stream start failure",
                                        );
                                    } else {
                                        stream = new_stream;
                                        config = new_config;
                                        state = new_state;
                                        channels = new_channels;
                                        producer = new_producer;
                                        buffer_capacity = new_buffer_capacity;
                                        send_playback_event(
                                            &event_tx,
                                            ThreadEvent::PlaybackChannelsChanged(channels),
                                            "channel rebuild channels",
                                        );

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
                                        send_playback_event(
                                            &event_tx,
                                            ThreadEvent::ProcessingError(format!(
                                                "Playback stream unrecoverable: rebuild failed ({}) \
                                                 and old stream failed to resume ({})",
                                                e, resume_err
                                            )),
                                            "unrecoverable channel rebuild failure",
                                        );
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
                                    send_playback_event(
                                        &event_tx,
                                        ThreadEvent::ProcessingWarning(format!(
                                            "Playback stream rebuild failed for {} channels, falling back to previous configuration",
                                            new_channels
                                        )),
                                        "channel rebuild fallback",
                                    );
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

        // Callback/stream recovery: CoreAudio can invalidate an output stream
        // when another client starts or stops on the HAL device. Re-resolve the
        // device name and rebuild the CPAL stream instead of stopping playback.
        {
            let current_stream_errors = state.stream_error_count.load(Ordering::Relaxed);
            let current_callbacks = state.callback_count.load(Ordering::Relaxed);
            let coreaudio_identity_reason =
                if last_device_identity_check.elapsed() > device_identity_check_interval {
                    last_device_identity_check = std::time::Instant::now();
                    let current_device_id = coreaudio_output_device_id(&device_name);
                    if current_device_id.is_some() && current_device_id != coreaudio_device_id {
                        let previous_device_id = coreaudio_device_id;
                        coreaudio_device_id = current_device_id;
                        Some(format!(
                            "CoreAudio device id changed for '{}' ({:?} -> {:?})",
                            device_name, previous_device_id, current_device_id
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                };
            let recovery_reason = playback_recovery_reason(
                current_stream_errors,
                &mut last_stream_error_count,
                current_callbacks,
                &mut last_callback_count,
                &mut last_callback_check,
                callback_stall_timeout,
                frames_received,
                frames_written,
                coreaudio_identity_reason,
            );

            if let Some(reason) = recovery_reason {
                if last_recovery_attempt.elapsed() < recovery_retry_interval {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                }
                last_recovery_attempt = std::time::Instant::now();

                let mut drained_count = 0;
                while message_rx.try_recv().is_ok() {
                    drained_count += 1;
                }

                let warning = format!(
                    "Audio device '{}' needs playback stream recovery: {}",
                    device_name, reason
                );
                log::warn!(
                    "[Playback Thread] {} (drained {} queued frames)",
                    warning,
                    drained_count
                );
                send_playback_event(
                    &event_tx,
                    ThreadEvent::ProcessingWarning(warning),
                    "stream recovery warning",
                );

                if let Err(e) = stream.pause() {
                    log::warn!(
                        "[Playback Thread] Failed to pause stream before recovery: {}",
                        e
                    );
                }

                #[cfg(target_os = "macos")]
                if output_access.prefers_exclusive() && !backend_exclusive_active {
                    match coreaudio_exclusive_mode.activate_for_device(&device_name, output_access)
                    {
                        Ok(status) => {
                            set_output_access_status(
                                &event_tx,
                                &mut output_access_status,
                                status,
                                "recovered playback output access",
                            );
                        }
                        Err(e) => {
                            log::error!("[Playback Thread] {}", e);
                            send_playback_event(
                                &event_tx,
                                ThreadEvent::ProcessingError(e),
                                "exclusive recovery failure",
                            );
                            break;
                        }
                    }
                }

                match rebuild_playback_stream(
                    &host,
                    RebuildPlaybackParams {
                        output_device: output_device.as_deref(),
                        allow_virtual_output,
                        sample_rate: config.sample_rate,
                        requested_channels: channels,
                        buffer_ms,
                        buffer_size: initial_buffer_size(output_access_status, frame_size),
                        event_tx: event_tx.clone(),
                        old_state: &state,
                    },
                ) {
                    Ok(rebuilt) => {
                        log::info!(
                            "[Playback Thread] Recovered playback stream: device='{}', {}Hz, {}ch, format={:?}",
                            rebuilt.device_name,
                            rebuilt.config.sample_rate,
                            rebuilt.channels,
                            rebuilt.output_format
                        );

                        device = rebuilt.device;
                        device_name = rebuilt.device_name;
                        stream = rebuilt.stream;
                        producer = rebuilt.producer;
                        state = rebuilt.state;
                        config = rebuilt.config;
                        output_format = rebuilt.output_format;
                        channels = rebuilt.channels;
                        buffer_capacity = rebuilt.buffer_capacity;
                        coreaudio_device_id = coreaudio_output_device_id(&device_name);
                        last_device_identity_check = std::time::Instant::now();
                        last_callback_count = 0;
                        last_stream_error_count = 0;
                        last_callback_check = std::time::Instant::now();
                        last_reported_underruns = 0;
                        flush_mode = FlushMode::Normal;
                        end_of_stream = false;
                        drain_start = None;
                        send_playback_event(
                            &event_tx,
                            ThreadEvent::PlaybackChannelsChanged(channels),
                            "playback channels changed",
                        );
                        send_playback_event(
                            &event_tx,
                            ThreadEvent::PlaybackOutputDeviceChanged(device_name.clone()),
                            "playback output device changed",
                        );
                        continue;
                    }
                    Err(e) => {
                        let msg = format!(
                            "Playback stream recovery failed for '{}': {}",
                            device_name, e
                        );
                        log::error!("[Playback Thread] {}", msg);
                        send_playback_event(
                            &event_tx,
                            ThreadEvent::ProcessingWarning(msg),
                            "stream recovery failure",
                        );
                        if let Err(resume_err) = stream.play() {
                            log::warn!(
                                "[Playback Thread] Failed to resume previous stream after recovery failure: {}",
                                resume_err
                            );
                        }
                        last_callback_check = std::time::Instant::now();
                        std::thread::sleep(std::time::Duration::from_millis(250));
                        continue;
                    }
                }
            }
        }

        let current_underruns = state.underrun_count.load(Ordering::Relaxed);
        if current_underruns != last_reported_underruns
            && (current_underruns == 1
                || current_underruns.is_multiple_of(100)
                || last_reported_underruns == 0)
        {
            send_playback_event(
                &event_tx,
                ThreadEvent::PlaybackUnderrun(current_underruns),
                "playback underrun",
            );
            last_reported_underruns = current_underruns;
        }

        // Check if ring buffer has space
        let available_space = producer.slots();

        // Only pull from queue if we have space for at least 2 frames
        let min_space_required = frame_size * channels * 2;

        if available_space < min_space_required {
            // Ring buffer is full, sleep briefly and let the audio callback drain it
            std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_RINGBUFFER));
            frames_blocked += 1;
            continue;
        }

        // Periodic diagnostics: log callback rate and buffer stats every few seconds
        if last_diagnostic_log.elapsed() > diagnostic_interval {
            let elapsed = stream_start_time.elapsed().as_secs_f64();
            let total_cb = state.callback_count.load(Ordering::Relaxed);
            let total_cb_samples = state.total_callback_samples.load(Ordering::Relaxed);
            let effective_hz = if elapsed > 0.0 && channels > 0 {
                (total_cb_samples as f64 / channels as f64 / elapsed) as u64
            } else {
                0
            };
            let fill = {
                let slots = producer.slots();
                ((buffer_capacity - slots) * 100)
                    .checked_div(buffer_capacity)
                    .unwrap_or(0)
            };
            log::debug!(
                "[Playback Thread] PERIODIC: callbacks={}, effective={}Hz (expected {}Hz), \
                 buffer_fill={}%, blocked={}, dropped={}, received={}, format={:?}",
                total_cb,
                effective_hz,
                sample_rate,
                fill,
                frames_blocked,
                frames_dropped,
                frames_received,
                output_format,
            );
            send_playback_event(
                &event_tx,
                ThreadEvent::PlaybackStats {
                    callback_count: total_cb,
                    buffer_fill_percent: fill as u64,
                    stream_error_count: state.stream_error_count.load(Ordering::Relaxed),
                    frames_received,
                    frames_written,
                    frames_dropped,
                    effective_sample_rate: effective_hz,
                },
                "playback stats",
            );
            last_diagnostic_log = std::time::Instant::now();
        }

        // Read from message queue (prioritize draining the queue if we have space)
        let message = message_rx.try_recv();

        match message {
            Ok(ProcessingMessage::Frame(frame)) => {
                if matches!(flush_mode, FlushMode::DroppingUntilFlush) {
                    frames_dropped += 1;
                    recycle_frame_data(&recycle_tx, frame.data, "flush drop");
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
                            let src_base = i * frame.num_channels;
                            let src = &frame.data[src_base..src_base + n];
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
                            recycle_frame_data(&recycle_tx, frame.data, "converted frame drop");
                            std::thread::sleep(std::time::Duration::from_millis(
                                SPIN_MS_RINGBUFFER,
                            ));
                            continue;
                        }
                    };
                    write_chunk_bulk(chunk, &conversion_buffer);
                    recycle_frame_data(&recycle_tx, frame.data, "converted frame written");
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
                        if frames_dropped % 500 == 1 {
                            log::warn!(
                                "[Playback Thread] FRAME DROPPED count: {} (buffer full)",
                                frames_dropped
                            );
                        }
                        recycle_frame_data(&recycle_tx, frame.data, "frame drop");
                        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_RINGBUFFER));
                        continue;
                    }
                };
                write_chunk_bulk(chunk, &frame.data);
                recycle_frame_data(&recycle_tx, frame.data, "frame written");
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
                        send_playback_event(
                            &event_tx,
                            ThreadEvent::PlaybackDrained,
                            "ring buffer drained",
                        );
                        break;
                    }
                    // Safety timeout: if drain takes too long (cpal callback stopped?),
                    // check whether the buffer actually drained or is still full.
                    if let Some(start) = drain_start
                        && start.elapsed() > drain_timeout
                    {
                        let current_slots = producer.slots();
                        let drain_percent = (current_slots * 100)
                            .checked_div(buffer_capacity)
                            .unwrap_or(100);
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
                            send_playback_event(
                                &event_tx,
                                ThreadEvent::ProcessingError(msg),
                                "drain timeout error",
                            );
                        } else {
                            log::warn!(
                                "[Playback Thread] Drain timeout, buffer mostly empty ({}% drained), signaling completion",
                                drain_percent
                            );
                            send_playback_event(
                                &event_tx,
                                ThreadEvent::PlaybackDrained,
                                "drain timeout completion",
                            );
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
                            send_playback_event(
                                &event_tx,
                                ThreadEvent::PlaybackDrained,
                                "post-disconnect drained",
                            );
                            break;
                        }
                        if drain_start.elapsed() > drain_timeout {
                            let current_slots = producer.slots();
                            let drain_percent = (current_slots * 100)
                                .checked_div(buffer_capacity)
                                .unwrap_or(100);
                            if drain_percent < 80 {
                                let msg = format!(
                                    "Playback stalled after disconnect: ring buffer {}% full after drain timeout. Device: '{}'",
                                    100 - drain_percent,
                                    device_name,
                                );
                                log::error!("[Playback Thread] {}", msg);
                                send_playback_event(
                                    &event_tx,
                                    ThreadEvent::ProcessingError(msg),
                                    "post-disconnect drain timeout error",
                                );
                            } else {
                                log::warn!(
                                    "[Playback Thread] Drain timeout after disconnect, buffer mostly empty ({}% drained)",
                                    drain_percent
                                );
                                send_playback_event(
                                    &event_tx,
                                    ThreadEvent::PlaybackDrained,
                                    "post-disconnect drain timeout completion",
                                );
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
    let avg_samples_per_callback = total_samples.checked_div(total_callbacks).unwrap_or(0);
    log::info!(
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
    log::info!(
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

    // Second try: device has a config with ch >= requested that supports this sample rate.
    // Use the requested channel count (not the device's) — CoreAudio/ALSA/WASAPI can
    // typically open a stream with fewer channels than the device maximum. This avoids
    // inflating to e.g. 94 channels on a Fireface UFX+ when only 6 are needed.
    let mut available_channels: Vec<u16> = supported
        .iter()
        .filter(|c| {
            c.min_sample_rate() <= config.sample_rate && c.max_sample_rate() >= config.sample_rate
        })
        .map(|c| c.channels())
        .collect();
    available_channels.sort();
    available_channels.dedup();

    if available_channels.iter().any(|&ch| ch >= config.channels) {
        // Pick format from ANY sample-rate-compatible config (ignoring channel count).
        let fmt = pick_format_any_channels(&candidates, config.sample_rate);
        if let Some(fmt) = fmt {
            log::info!(
                "[Playback Thread] No exact {}ch config; using requested count with {:?} format \
                 (device supports {:?}ch). Device configs: {:?}",
                config.channels,
                fmt,
                available_channels,
                log_configs()
            );
            return (fmt, config.channels);
        }
    }

    // Third try: downmix — pick highest channel count <= requested.
    let alt_ch = available_channels
        .iter()
        .rev()
        .find(|&&ch| ch <= config.channels)
        .copied();

    if let Some(ch) = alt_ch
        && let Some(fmt) = pick_preferred_output_format(&candidates, ch, config.sample_rate)
    {
        log::info!(
            "[Playback Thread] Device doesn't support {}ch; using {}ch {:?} (will downmix). Device configs: {:?}",
            config.channels,
            ch,
            fmt,
            log_configs()
        );
        return (fmt, ch);
    }

    log::info!(
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

/// Pick preferred format from ANY channel count config (for sample-rate compatibility).
/// Used when no exact channel match exists but the device supports >= requested channels.
fn pick_format_any_channels(
    candidates: &[(SampleFormat, u16, cpal::SampleRate, cpal::SampleRate)],
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
        candidates
            .iter()
            .any(|c| c.0 == *fmt && c.2 <= sample_rate && c.3 >= sample_rate)
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

/// Apply volume and mute to f32 scratch buffer without clipping the float path.
#[inline(always)]
fn apply_volume(scratch: &mut [f32], state: &PlaybackState) {
    let volume = f32::from_bits(state.volume.load(Ordering::Relaxed));
    let muted = state.muted.load(Ordering::Relaxed);

    if muted {
        scratch.fill(0.0);
    } else if (volume - 1.0).abs() > 0.001 {
        for sample in scratch.iter_mut() {
            *sample *= volume;
        }
    }
}

#[inline(always)]
fn clamp_samples(scratch: &mut [f32]) {
    for sample in scratch.iter_mut() {
        *sample = sample.clamp(-1.0, 1.0);
    }
}

/// Apply volume/mute and clamp for integer hardware formats.
#[inline(always)]
fn apply_volume_clamp(scratch: &mut [f32], state: &PlaybackState) {
    apply_volume(scratch, state);
    clamp_samples(scratch);
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

#[cfg(test)]
mod tests {
    use super::{
        PlaybackState, apply_volume, fallback_output_format, initial_buffer_size,
        is_virtual_output_device_name, output_access_status_for_device,
        pick_preferred_output_format, playback_buffer_capacity, playback_recovery_reason,
        read_ring_buffer, request_flush,
    };
    use cpal::SampleFormat;
    use rtrb::RingBuffer;
    use sotf_types::{OutputAccessMode, OutputAccessStatus};
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};

    #[test]
    fn playback_stream_error_callbacks_gate_event_formatting() {
        let source = include_str!("playback_thread.rs");

        assert!(
            source.contains("if crate::rate_limit::allow(&EVENT_GATE, 5_000_000_000)"),
            "playback stream-error callbacks must rate-limit event formatting/sending"
        );
    }

    #[test]
    fn macos_playback_recovers_when_coreaudio_device_id_changes() {
        let source = include_str!("playback_thread.rs");

        assert!(
            source.contains("coreaudio_output_device_id(&device_name)")
                && source.contains("CoreAudio device id changed")
                && source.contains("rebuild_playback_stream("),
            "playback should rebuild the output stream when macOS resurrects a named device under a new CoreAudio device id"
        );
    }

    #[test]
    fn macos_exclusive_output_uses_coreaudio_hog_mode_guard() {
        let source = include_str!("playback_thread.rs");

        assert!(
            source.contains("struct CoreAudioExclusiveModeGuard")
                && source.contains("get_hogging_pid(device_id)")
                && source.contains("toggle_hog_mode(device_id)")
                && source.contains("impl Drop for CoreAudioExclusiveModeGuard")
                && source.contains("activate_for_device(&device_name, output_access)")
                && source.contains("PlaybackOutputAccessChanged(new_status)"),
            "macOS exclusive output should acquire CoreAudio hog mode before stream build, publish access changes, and release ownership on drop"
        );
    }

    #[test]
    fn playback_recovery_ignores_coreaudio_identity_change_while_callbacks_advance() {
        let mut last_stream_error_count = 0;
        let mut last_callback_count = 41;
        let mut last_callback_check = Instant::now() - Duration::from_secs(10);

        let recovery = playback_recovery_reason(
            0,
            &mut last_stream_error_count,
            42,
            &mut last_callback_count,
            &mut last_callback_check,
            Duration::from_secs(3),
            100,
            99,
            Some("CoreAudio device id changed".to_string()),
        );

        assert_eq!(recovery, None);
        assert_eq!(last_callback_count, 42);
    }

    #[test]
    fn explicit_output_device_lookup_does_not_silently_fallback() {
        let source = include_str!("playback_thread.rs");
        let explicit_lookup = source
            .split("match crate::devices::find_device(host, device_identifier, false)")
            .nth(1)
            .expect("explicit lookup branch should exist")
            .split("} else {")
            .next()
            .expect("explicit lookup branch should end before default-device branch");

        assert!(
            explicit_lookup.contains("Selected output device")
                && !explicit_lookup.contains("find_fallback()"),
            "an explicit user-selected output must fail loudly instead of falling back to another physical device"
        );
    }

    #[test]
    fn explicit_virtual_output_device_is_rejected_when_not_allowed() {
        let source = include_str!("playback_thread.rs");

        assert!(
            source.contains(
                "is_virtual_output_device_name(device_identifier) && !allow_virtual_output"
            ) && source.contains("is virtual/loopback and cannot be used as speaker output"),
            "explicit virtual output selection must be rejected before device lookup"
        );
    }

    #[test]
    fn default_selection_avoids_opening_virtual_output_when_not_allowed() {
        let source = include_str!("playback_thread.rs");
        let default_branch = source
            .split("} else if !allow_virtual_output {")
            .nth(1)
            .expect("safe default branch should exist")
            .split("} else {")
            .next()
            .expect("safe default branch should end before virtual-allowed branch");

        assert!(
            default_branch.contains("find_physical_output()")
                && !default_branch.contains("default_output_device()"),
            "systemwide default selection must scan physical outputs without opening the virtual default device"
        );
    }

    #[test]
    fn playback_stats_publish_even_before_frames_arrive() {
        let source = include_str!("playback_thread.rs");
        let diagnostics_block = source
            .split("// Periodic diagnostics: log callback rate and buffer stats every few seconds")
            .nth(1)
            .expect("periodic diagnostics block should exist")
            .split("// Read from message queue")
            .next()
            .expect("periodic diagnostics block should end before queue read");

        assert!(
            diagnostics_block.contains("if last_diagnostic_log.elapsed() > diagnostic_interval")
                && !diagnostics_block.contains("frames_received > 0"),
            "playback stats must report callbacks/underruns during upstream starvation"
        );
    }

    #[test]
    fn ios_stub_writes_frame_data_to_ring_buffer_in_bulk() {
        let source = include_str!("playback_thread_stub.rs");

        assert!(
            !source.contains(concat!("fill_from_iter(frame.data.", "iter().copied())")),
            "iOS playback feeder should bulk-copy frame data into ring-buffer chunks"
        );
    }

    #[test]
    fn read_ring_buffer_discards_samples_while_flush_requested() {
        let (mut producer, mut consumer) = RingBuffer::<f32>::new(8);
        let chunk = producer.write_chunk_uninit(4).unwrap();
        chunk.fill_from_iter([0.25, 0.5, 0.75, 1.0]);

        let state = PlaybackState::new(8);
        request_flush(&state);
        let mut scratch = [1.0; 4];

        let underrun = read_ring_buffer(&mut consumer, &mut scratch, 4, &state, 8);

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
        let mut scratch = [1.0; 4];

        read_ring_buffer(&mut consumer, &mut scratch, 4, &state, 8);
        assert_eq!(scratch, [0.0; 4]);
        assert_eq!(consumer.slots(), 4);
        assert!(state.flush_requested.load(Ordering::Relaxed));

        read_ring_buffer(&mut consumer, &mut scratch, 4, &state, 8);
        assert_eq!(scratch, [0.0; 4]);
        assert_eq!(consumer.slots(), 0);
        assert!(!state.flush_requested.load(Ordering::Relaxed));
    }

    #[test]
    fn read_ring_buffer_counts_only_consumed_samples_on_underrun() {
        let (mut producer, mut consumer) = RingBuffer::<f32>::new(8);
        let chunk = producer.write_chunk_uninit(3).unwrap();
        chunk.fill_from_iter([0.25, 0.5, 0.75]);

        let state = PlaybackState::new(8);
        let mut scratch = [1.0; 6];

        let underrun = read_ring_buffer(&mut consumer, &mut scratch, 6, &state, 8);

        assert!(underrun);
        assert_eq!(scratch, [0.25, 0.5, 0.75, 0.0, 0.0, 0.0]);
        assert_eq!(state.total_callback_samples.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn f32_output_volume_at_unity_does_not_clip_samples() {
        let state = PlaybackState::new(8);
        let mut scratch = [-1.25, -0.5, 0.5, 1.25];

        apply_volume(&mut scratch, &state);

        assert_eq!(scratch, [-1.25, -0.5, 0.5, 1.25]);
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
    fn exclusive_preferred_reports_platform_initial_status_for_cpal_devices() {
        #[cfg(target_os = "macos")]
        let expected = OutputAccessStatus::ExclusivePending;
        #[cfg(not(target_os = "macos"))]
        let expected = OutputAccessStatus::FallbackShared;

        assert_eq!(
            output_access_status_for_device(OutputAccessMode::ExclusivePreferred, None),
            expected
        );
    }

    #[test]
    fn exclusive_required_reports_platform_initial_status_without_exclusive_backend() {
        #[cfg(target_os = "macos")]
        let expected = OutputAccessStatus::ExclusivePending;
        #[cfg(not(target_os = "macos"))]
        let expected = OutputAccessStatus::Unsupported;

        assert_eq!(
            output_access_status_for_device(OutputAccessMode::ExclusiveRequired, None),
            expected
        );
    }

    #[test]
    fn asio_output_reports_exclusive_active() {
        assert_eq!(
            output_access_status_for_device(
                OutputAccessMode::ExclusivePreferred,
                Some("ASIO:Focusrite USB ASIO"),
            ),
            OutputAccessStatus::ExclusiveActive
        );
    }

    #[test]
    fn exclusive_active_uses_fixed_initial_buffer_size() {
        assert_eq!(
            initial_buffer_size(OutputAccessStatus::ExclusiveActive, 256),
            cpal::BufferSize::Fixed(256)
        );
        assert_eq!(
            initial_buffer_size(OutputAccessStatus::FallbackShared, 256),
            cpal::BufferSize::Default
        );
        assert_eq!(
            initial_buffer_size(OutputAccessStatus::ExclusivePending, 256),
            cpal::BufferSize::Default
        );
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
        assert!(is_virtual_output_device_name("Soundflower (2ch)"));
        assert!(is_virtual_output_device_name("Background Music"));
        assert!(is_virtual_output_device_name("Audio Bridge"));
        assert!(is_virtual_output_device_name("Generic Virtual Device"));
        assert!(is_virtual_output_device_name("blackhole 2ch"));
        assert!(is_virtual_output_device_name("zoomaudiodevice"));
        assert!(is_virtual_output_device_name("loopback audio"));
    }

    #[test]
    fn is_virtual_output_device_name_allows_regular_physical_outputs() {
        assert!(!is_virtual_output_device_name("Built-in Output"));
    }
}
