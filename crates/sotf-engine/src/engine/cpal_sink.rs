// ============================================================================
// CpalSink - Hardware audio output via cpal
// ============================================================================
//
// Implements AudioSink for local audio hardware using cpal.
// Extracted from playback_thread.rs to allow alternative output sinks.

use super::ThreadEvent;
use super::audio_sink::{AudioSink, SinkConfig, SinkOpenResult};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::Sender;

/// Max input channels for the stack-allocated downmix coefficient arrays.
#[allow(dead_code)]
const MAX_DOWNMIX_CH: usize = 16;

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

/// Shared state between the playback thread and cpal callback.
/// All fields are lock-free atomics for real-time safety.
pub(crate) struct CpalPlaybackState {
    pub capacity: usize,
    pub volume: Arc<AtomicU32>,
    pub muted: Arc<AtomicBool>,
    pub flush_requested: Arc<AtomicBool>,
    pub underrun_count: Arc<AtomicU64>,
    pub last_buffer_level: Arc<AtomicU64>,
    pub total_callback_samples: Arc<AtomicU64>,
    pub callback_count: Arc<AtomicU64>,
}

impl CpalPlaybackState {
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

/// cpal-based audio output sink.
pub struct CpalSink {
    device: Option<Device>,
    device_name: String,
    stream: Option<Stream>,
    producer: Option<Producer<f32>>,
    state: Option<Arc<CpalPlaybackState>>,
    config: Option<StreamConfig>,
    output_format: SampleFormat,
    buffer_capacity: usize,
    channels: usize,
    event_tx: Option<Sender<ThreadEvent>>,
    allow_virtual_output: bool,
    /// Callback count at last stall check
    last_callback_count: u64,
    last_callback_check: std::time::Instant,
}

impl Default for CpalSink {
    fn default() -> Self {
        Self::new()
    }
}

impl CpalSink {
    pub fn new() -> Self {
        Self {
            device: None,
            device_name: String::new(),
            stream: None,
            producer: None,
            state: None,
            config: None,
            output_format: SampleFormat::F32,
            buffer_capacity: 0,
            channels: 0,
            event_tx: None,
            allow_virtual_output: false,
            last_callback_count: 0,
            last_callback_check: std::time::Instant::now(),
        }
    }

    /// Select the output device based on the config.
    fn select_device(
        device_name: Option<&str>,
        allow_virtual: bool,
    ) -> Result<(Device, String), String> {
        let host = crate::devices::get_host_for_device(device_name);

        let device_name_stripped = device_name.map(|d| {
            if crate::devices::is_asio_device(d) {
                crate::devices::strip_asio_prefix(d).to_string()
            } else {
                d.to_string()
            }
        });

        let find_fallback = |host: &cpal::Host| -> Result<Device, String> {
            let devices = host.output_devices().map_err(|e| e.to_string())?;
            let get_name = |d: &Device| -> String {
                d.description()
                    .map(|desc| desc.name().to_string())
                    .unwrap_or_else(|_| "Unknown".to_string())
            };
            let physical = devices
                .into_iter()
                .find(|d| !is_virtual_output_device_name(&get_name(d)));

            physical
                .or_else(|| host.default_output_device())
                .ok_or_else(|| "No output device available".to_string())
        };

        let device = if let Some(device_id) = device_name_stripped {
            if is_virtual_output_device_name(&device_id) && !allow_virtual {
                log::warn!(
                    "[CpalSink] Virtual output device '{}' requested - forcing fallback",
                    device_id
                );
                find_fallback(&host)?
            } else {
                match crate::devices::find_device(&host, &device_id, false) {
                    Ok(dev) => dev,
                    Err(e) => {
                        log::info!(
                            "[CpalSink] Device '{}' not found ({}), using fallback",
                            device_id,
                            e
                        );
                        find_fallback(&host)?
                    }
                }
            }
        } else {
            let default_dev = host
                .default_output_device()
                .ok_or("No output device available")?;
            let name = default_dev
                .description()
                .map(|d| d.name().to_string())
                .unwrap_or_else(|_| "Unknown".to_string());

            if is_virtual_output_device_name(&name) && !allow_virtual {
                log::warn!(
                    "[CpalSink] Default device '{}' is virtual - finding fallback",
                    name
                );
                let devices = host
                    .output_devices()
                    .map_err(|e| format!("Failed to list devices: {}", e))?;
                let get_name = |d: &Device| -> String {
                    d.description()
                        .map(|desc| desc.name().to_string())
                        .unwrap_or_else(|_| "Unknown".to_string())
                };
                devices
                    .into_iter()
                    .find(|d| !is_virtual_output_device_name(&get_name(d)))
                    .unwrap_or(default_dev)
            } else {
                default_dev
            }
        };

        let name = device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "Unknown".to_string());

        Ok((device, name))
    }

    /// Build the cpal stream and ring buffer for the given device/config.
    fn build_stream(
        device: &Device,
        config: &StreamConfig,
        buffer_capacity: usize,
        output_format: SampleFormat,
        event_tx: Sender<ThreadEvent>,
    ) -> Result<(Producer<f32>, Arc<CpalPlaybackState>, Stream), String> {
        let (producer, consumer) = RingBuffer::<f32>::new(buffer_capacity);
        let state = Arc::new(CpalPlaybackState::new(buffer_capacity));

        let stream = build_output_stream(
            device,
            config,
            Arc::clone(&state),
            event_tx,
            consumer,
            output_format,
        )?;

        stream
            .play()
            .map_err(|e| format!("Failed to start stream: {}", e))?;

        Ok((producer, state, stream))
    }
}

impl AudioSink for CpalSink {
    fn open(
        &mut self,
        config: SinkConfig,
        event_tx: Sender<ThreadEvent>,
    ) -> Result<SinkOpenResult, String> {
        self.allow_virtual_output = config.allow_virtual_output;
        self.event_tx = Some(event_tx.clone());

        let (device, name) =
            Self::select_device(config.device.as_deref(), config.allow_virtual_output)?;
        self.device_name = name;

        let mut stream_config = StreamConfig {
            channels: config.channels as u16,
            sample_rate: config.sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };

        let (output_format, hw_channels) = choose_output_format(&device, &stream_config);
        if hw_channels != config.channels as u16 {
            log::warn!(
                "[CpalSink] Adjusting channels from {} to {} (device limitation)",
                config.channels,
                hw_channels
            );
            stream_config.channels = hw_channels;
        }

        let channels = hw_channels as usize;
        let buffer_capacity =
            playback_buffer_capacity(config.sample_rate, channels, config.buffer_ms);

        let (producer, state, stream) = Self::build_stream(
            &device,
            &stream_config,
            buffer_capacity,
            output_format,
            event_tx.clone(),
        )?;

        event_tx
            .send(ThreadEvent::PlaybackChannelsChanged(channels))
            .ok();

        log::info!(
            "[CpalSink] Opened - {}Hz, {}ch, format: {:?}, device: '{}'",
            config.sample_rate,
            channels,
            output_format,
            self.device_name,
        );

        self.device = Some(device);
        self.stream = Some(stream);
        self.producer = Some(producer);
        self.state = Some(state);
        self.config = Some(stream_config);
        self.output_format = output_format;
        self.buffer_capacity = buffer_capacity;
        self.channels = channels;
        self.last_callback_count = 0;
        self.last_callback_check = std::time::Instant::now();

        Ok(SinkOpenResult {
            channels,
            buffer_capacity,
        })
    }

    fn write(&mut self, data: &[f32]) -> Result<usize, String> {
        let producer = self.producer.as_mut().ok_or("Sink not open")?;

        match producer.write_chunk_uninit(data.len()) {
            Ok(chunk) => {
                chunk.fill_from_iter(data.iter().copied());
                Ok(data.len())
            }
            Err(_) => Ok(0), // Buffer full
        }
    }

    fn available_slots(&self) -> usize {
        self.producer.as_ref().map_or(0, |p| p.slots())
    }

    fn capacity(&self) -> usize {
        self.buffer_capacity
    }

    fn flush(&mut self) {
        if let Some(state) = &self.state {
            state.flush_requested.store(true, Ordering::Release);
        }
    }

    fn is_flush_complete(&self) -> bool {
        let Some(state) = &self.state else {
            return true;
        };
        let Some(producer) = &self.producer else {
            return true;
        };

        if state.flush_requested.load(Ordering::Acquire) && producer.slots() >= self.buffer_capacity
        {
            state.flush_requested.store(false, Ordering::Release);
        }

        !state.flush_requested.load(Ordering::Acquire)
    }

    fn set_volume(&mut self, volume: f32) {
        if let Some(state) = &self.state {
            state.volume.store(volume.to_bits(), Ordering::Relaxed);
        }
    }

    fn set_muted(&mut self, muted: bool) {
        if let Some(state) = &self.state {
            state.muted.store(muted, Ordering::Relaxed);
        }
    }

    fn reconfigure(&mut self, config: SinkConfig) -> Result<SinkOpenResult, String> {
        let event_tx = self.event_tx.clone().ok_or("No event channel")?;
        let device = self.device.as_ref().ok_or("No device")?;

        // Pause old stream
        if let Some(ref stream) = self.stream
            && let Err(e) = stream.pause()
        {
            log::warn!("[CpalSink] Failed to pause old stream: {}", e);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));

        let mut stream_config = StreamConfig {
            channels: config.channels as u16,
            sample_rate: config.sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };

        let (output_format, hw_channels) = choose_output_format(device, &stream_config);
        if hw_channels != config.channels as u16 {
            log::warn!(
                "[CpalSink] Adjusting rebuild channels from {} to {}",
                config.channels,
                hw_channels
            );
            stream_config.channels = hw_channels;
        }

        let channels = hw_channels as usize;
        let buffer_capacity =
            playback_buffer_capacity(config.sample_rate, channels, config.buffer_ms);

        let (producer, state, stream) = Self::build_stream(
            device,
            &stream_config,
            buffer_capacity,
            output_format,
            event_tx.clone(),
        )?;

        event_tx
            .send(ThreadEvent::PlaybackChannelsChanged(channels))
            .ok();

        log::info!(
            "[CpalSink] Reconfigured - {}Hz, {}ch, format: {:?}",
            config.sample_rate,
            channels,
            output_format,
        );

        // Drop old stream before replacing
        self.stream = None;
        self.producer = Some(producer);
        self.state = Some(state);
        self.stream = Some(stream);
        self.config = Some(stream_config);
        self.output_format = output_format;
        self.buffer_capacity = buffer_capacity;
        self.channels = channels;
        self.last_callback_count = 0;
        self.last_callback_check = std::time::Instant::now();

        Ok(SinkOpenResult {
            channels,
            buffer_capacity,
        })
    }

    fn is_stalled(&self) -> bool {
        let Some(state) = &self.state else {
            return false;
        };
        let current = state.callback_count.load(Ordering::Relaxed);
        // We can't update last_callback_count through &self, so we check
        // if the callback count hasn't changed since the last check by
        // comparing against the stored value. The playback thread loop
        // calls update_stall_check() to advance the timer.
        current == self.last_callback_count
            && self.last_callback_check.elapsed() > std::time::Duration::from_secs(3)
    }

    fn device_name(&self) -> &str {
        &self.device_name
    }

    fn close(&mut self) {
        self.stream = None;
        self.producer = None;
        self.state = None;
        log::debug!("[CpalSink] Closed");
    }
}

impl CpalSink {
    /// Update the stall detection timer. Called from the playback thread loop.
    pub fn update_stall_check(&mut self) {
        if let Some(state) = &self.state {
            let current = state.callback_count.load(Ordering::Relaxed);
            if current != self.last_callback_count {
                self.last_callback_count = current;
                self.last_callback_check = std::time::Instant::now();
            }
        }
    }

    /// Get diagnostic info for periodic logging.
    pub fn diagnostics(&self) -> Option<SinkDiagnostics> {
        let state = self.state.as_ref()?;
        Some(SinkDiagnostics {
            callback_count: state.callback_count.load(Ordering::Relaxed),
            total_callback_samples: state.total_callback_samples.load(Ordering::Relaxed),
        })
    }
}

pub struct SinkDiagnostics {
    pub callback_count: u64,
    pub total_callback_samples: u64,
}

// ============================================================================
// Helper functions (extracted from playback_thread.rs)
// ============================================================================

fn playback_buffer_capacity(sample_rate: u32, channels: usize, buffer_ms: u32) -> usize {
    (((sample_rate as u64 * buffer_ms as u64) / 1000) as usize) * channels
}

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
                "[CpalSink] Cannot query supported formats: {}, falling back to {:?}/{}ch",
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

    // First try: exact channel count match
    if let Some(fmt) =
        pick_preferred_output_format(&candidates, config.channels, config.sample_rate)
    {
        return (fmt, config.channels);
    }

    // Second try: find the best alternative channel count
    let mut available_channels: Vec<u16> = supported
        .iter()
        .filter(|c| {
            c.min_sample_rate() <= config.sample_rate && c.max_sample_rate() >= config.sample_rate
        })
        .map(|c| c.channels())
        .collect();
    available_channels.sort();
    available_channels.dedup();

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
            "[CpalSink] Device doesn't support {}ch; using {}ch {:?}",
            config.channels,
            ch,
            fmt,
        );
        return (fmt, ch);
    }

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
#[inline(always)]
fn read_ring_buffer(
    consumer: &mut Consumer<f32>,
    scratch: &mut [f32],
    requested: usize,
    state: &CpalPlaybackState,
    capacity: usize,
) -> bool {
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
        if available < requested {
            scratch[available..requested].fill(0.0);
        }
        underrun = true;
        state.underrun_count.fetch_add(1, Ordering::Relaxed);
    }

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

/// Apply volume and mute to f32 scratch buffer without clipping the float path.
#[inline(always)]
fn apply_volume(scratch: &mut [f32], state: &CpalPlaybackState) {
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

#[inline(always)]
fn apply_volume_clamp(scratch: &mut [f32], state: &CpalPlaybackState) {
    apply_volume(scratch, state);
    clamp_samples(scratch);
}

fn build_output_stream(
    device: &Device,
    config: &StreamConfig,
    state: Arc<CpalPlaybackState>,
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

fn build_output_stream_f32(
    device: &Device,
    config: &StreamConfig,
    state: Arc<CpalPlaybackState>,
    event_tx: Sender<ThreadEvent>,
    mut consumer: Consumer<f32>,
) -> Result<Stream, String> {
    let state_clone = Arc::clone(&state);
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
                log::warn!("[CpalSink] Stream error: {}", err);
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

fn build_output_stream_int<T>(
    device: &Device,
    config: &StreamConfig,
    state: Arc<CpalPlaybackState>,
    event_tx: Sender<ThreadEvent>,
    mut consumer: Consumer<f32>,
) -> Result<Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let state_clone = Arc::clone(&state);
    let capacity = state.capacity;
    let mut scratch = vec![0.0f32; 16384];

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                state_clone.callback_count.fetch_add(1, Ordering::Relaxed);
                let requested = data.len();
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
                log::warn!("[CpalSink] Stream error: {}", err);
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
    use super::*;

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
        assert!(is_virtual_output_device_name("Soundflower (2ch)"));
        assert!(is_virtual_output_device_name("Background Music"));
        assert!(is_virtual_output_device_name("Audio Bridge"));
        assert!(is_virtual_output_device_name("Generic Virtual Device"));
    }

    #[test]
    fn is_virtual_output_device_name_allows_regular_physical_outputs() {
        assert!(!is_virtual_output_device_name("Built-in Output"));
    }
}
