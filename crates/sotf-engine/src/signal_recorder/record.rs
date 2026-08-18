#[cfg(not(target_os = "ios"))]
use super::consts::CANCELLED_ERR;
#[cfg(not(target_os = "ios"))]
use super::consts::MIN_REPEAT_SWEEPS;
#[cfg(not(target_os = "ios"))]
use super::misc::actionable_capture_error;
#[cfg(not(target_os = "ios"))]
use super::misc::capture_capacity;
#[cfg(not(target_os = "ios"))]
use super::misc::check_capture_clipping;
#[cfg(not(target_os = "ios"))]
use super::misc::drain_capture;
#[cfg(not(target_os = "ios"))]
use super::quality::CaptureAnalysis;
#[cfg(not(target_os = "ios"))]
use super::quality::{
    DriftAction, build_capture_quality, check_lag_lock, drift_action, log_capture_quality,
    normalize_clock_drift_ppm,
};
#[cfg(not(target_os = "ios"))]
use super::types::{CancelFlag, cancel_requested};
#[cfg(not(target_os = "ios"))]
use super::write::write_selected_channel_to_ring;
use super::write::write_wav_file;
#[cfg(not(target_os = "ios"))]
use super::write::{interpolate_log_frequency_grid, write_analysis_csv_extended};
#[cfg(not(target_os = "ios"))]
use crate::signal_analysis::{
    ClockDriftEstimate, LagEstimate, MicrophoneCompensation, analyze_recording,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Pause between repeat-sweep takes, letting the room decay and the device
/// streams settle between play/record cycles.
#[cfg(not(target_os = "ios"))]
const INTER_TAKE_SETTLE_MS: u64 = 300;

/// Minimum pre-sweep room-noise window (samples) for a usable noise-floor
/// estimate (~43 ms at 48 kHz). Below this the column/report input is
/// omitted rather than fabricated from a handful of samples.
#[cfg(not(target_os = "ios"))]
const MIN_NOISE_FLOOR_WINDOW_SAMPLES: usize = 2048;

#[cfg(not(target_os = "ios"))]
pub(super) fn resample_reference_signal(
    signal: &[f32],
    source_rate: u32,
    target_rate: u32,
) -> Result<Vec<f32>, String> {
    use sotf_plugins::{Plugin, ProcessContext, ResamplerPlugin};

    if source_rate == target_rate {
        return Ok(signal.to_vec());
    }
    if signal.is_empty() {
        return Ok(Vec::new());
    }

    const CHUNK_SIZE: usize = 1_024;
    let mut resampler = ResamplerPlugin::new(1, source_rate, target_rate, CHUNK_SIZE)?;
    let expected_len =
        (signal.len() as f64 * target_rate as f64 / source_rate as f64).ceil() as usize;
    let filter_delay = resampler.output_delay_frames();
    let aligned_end = filter_delay.saturating_add(expected_len);
    let mut resampled = Vec::with_capacity(aligned_end + CHUNK_SIZE);

    for chunk in signal.chunks(CHUNK_SIZE) {
        let max_output_frames = resampler.output_frames_for_input(chunk.len());
        let mut output = vec![0.0f32; max_output_frames];
        let produced = resampler.process(
            chunk,
            &mut output,
            &ProcessContext::new(source_rate, chunk.len()),
        )?;
        resampled.extend_from_slice(&output[..produced]);
    }

    // Feed silence until rubato has emitted the complete valid tail. Trimming
    // its output-domain filter delay then aligns sample zero while retaining
    // exactly the duration represented by the original reference.
    let zero_input = vec![0.0f32; CHUNK_SIZE];
    while resampled.len() < aligned_end {
        let max_output_frames = resampler.output_frames_for_input(CHUNK_SIZE);
        let mut output = vec![0.0f32; max_output_frames];
        let produced = resampler.process(
            &zero_input,
            &mut output,
            &ProcessContext::new(source_rate, CHUNK_SIZE),
        )?;
        if produced == 0 {
            return Err("Resampler made no progress while draining its filter tail".to_string());
        }
        resampled.extend_from_slice(&output[..produced]);
    }

    Ok(resampled[filter_delay..aligned_end].to_vec())
}

/// One raw play/record cycle of the sweep-capture machinery: the captured
/// mono buffer plus the capture-side diagnostics needed downstream.
#[cfg(not(target_os = "ios"))]
struct RawSweepTake {
    /// Raw mono capture (NOT drift-corrected) at `analysis_sample_rate`.
    recorded: Vec<f32>,
    /// The input sample rate actually negotiated with the device.
    analysis_sample_rate: u32,
    /// Input samples dropped this take because the capture ring buffer
    /// filled (R6).
    dropped_samples: u64,
}

/// One play/record cycle capturing all mics at once (multi-mic variant of
/// [`RawSweepTake`]).
#[cfg(not(target_os = "ios"))]
struct RawSweepTakeMulti {
    /// Raw mono capture per mic (same order as the requested input channels).
    recorded_per_mic: Vec<Vec<f32>>,
    /// The input sample rate actually negotiated with the device.
    analysis_sample_rate: u32,
    /// Shared overrun counter across all mic ring buffers (R6).
    dropped_samples: u64,
}

/// Apply a playback-stop result: a stop failure is fatal only when the
/// capture was NOT cancelled. On cancel, the error is logged and ignored so
/// the caller's `CANCELLED_ERR` always wins over a secondary "Failed to stop
/// playback" (Task 10).
#[cfg(not(target_os = "ios"))]
pub(super) fn cancel_aware_stop(
    stop_result: Result<(), impl std::fmt::Display>,
    cancelled: bool,
) -> Result<(), String> {
    match stop_result {
        Ok(()) => Ok(()),
        Err(e) if cancelled => {
            log::warn!("Ignoring playback-stop failure after cancellation: {e}");
            Ok(())
        }
        Err(e) => Err(format!("Failed to stop playback: {e}")),
    }
}

/// Perform one play/record capture cycle using AudioEngineManager for
/// playback and cpal for recording.
///
/// Plays back a signal to a specific output channel while simultaneously
/// recording from a specific input channel, and returns the raw captured
/// buffer after the capture gates (cancel, silence, clipping). Analysis and
/// the per-take quality pipeline live in [`analyze_sweep_takes`].
///
/// `cancel` is a cooperative cancellation flag (see [`CancelFlag`]): when
/// set, the capture stops the streams and returns
/// `Err(CANCELLED_ERR)` ("cancelled").
#[cfg(not(target_os = "ios"))]
#[allow(clippy::too_many_arguments)]
fn capture_sweep_take(
    temp_wav_path: &Path,
    reference_signal: &[f32],
    sample_rate: u32,
    output_channel: u16,
    input_channel: u16,
    output_device_name: Option<&str>,
    input_device_name: Option<&str>,
    cancel: Option<&CancelFlag>,
) -> Result<RawSweepTake, String> {
    use crate::AudioEngineManager;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::thread::sleep;
    use std::time::Duration;

    if cancel_requested(cancel) {
        return Err(CANCELLED_ERR.to_string());
    }

    log::debug!("[record_and_analyze] Starting playback and recording...");
    log::debug!("[record_and_analyze]   Playback file: {:?}", temp_wav_path);
    log::debug!("[record_and_analyze]   Output channel: {}", output_channel);
    log::debug!("[record_and_analyze]   Input channel: {}", input_channel);
    log::debug!("[record_and_analyze]   Sample rate: {}", sample_rate);

    // Calculate expected duration
    let expected_duration = reference_signal.len() as f64 / sample_rate as f64;
    log::info!(
        "[record_and_analyze]   Expected duration: {:.2}s",
        expected_duration
    );

    // Set up recording stream
    let host = cpal::default_host();

    // Get input device (either by name or default)
    let input_device = if let Some(dev_name) = input_device_name {
        log::info!(
            "[record_and_analyze] Looking for input device: {}",
            dev_name
        );
        crate::devices::find_device(&host, dev_name, true)
            .map_err(|e| actionable_capture_error("[record_and_analyze] Input device not usable", &e))?
    } else {
        log::debug!("[record_and_analyze] Using default input device");
        host.default_input_device().ok_or_else(|| {
            actionable_capture_error("[record_and_analyze]", &"No default input device available")
        })?
    };

    log::info!(
        "[record_and_analyze] Input device: {}",
        input_device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "Unknown Device".to_string())
    );

    // Find a supported input config that has enough channels for our input_channel
    // and supports the requested sample rate. Use default config as primary choice
    // since it's known to work with the device.
    let default_input_config = input_device.default_input_config().map_err(|e| {
        actionable_capture_error(
            "[record_and_analyze] Failed to get default input config",
            &e,
        )
    })?;

    let default_input_channels = default_input_config.channels() as usize;
    let default_input_sample_rate = default_input_config.sample_rate();

    log::info!(
        "[record_and_analyze] Input device default config: {}ch, {}Hz",
        default_input_channels,
        default_input_sample_rate,
    );

    // Find the best supported config: prefer one that matches our requested sample rate
    // and has enough channels, falling back to default
    let min_channels_needed = (input_channel as usize) + 1;

    let best_config = input_device
        .supported_input_configs()
        .ok()
        .and_then(|configs| {
            configs
                .filter(|c| {
                    let ch = c.channels() as usize;
                    ch >= min_channels_needed
                        && c.min_sample_rate() <= sample_rate
                        && c.max_sample_rate() >= sample_rate
                })
                // Prefer fewer channels (less data to process)
                .min_by_key(|c| c.channels())
        });

    let (hardware_input_channels, input_sample_rate) = if let Some(config) = best_config {
        let ch = config.channels() as usize;
        log::info!(
            "[record_and_analyze] Using supported config: {}ch, {}Hz (requested {}Hz)",
            ch,
            sample_rate,
            sample_rate,
        );
        (ch, sample_rate)
    } else {
        // Fall back to default config
        log::warn!(
            "[record_and_analyze] No supported config for {}ch at {}Hz, using default ({}ch, {}Hz)",
            min_channels_needed,
            sample_rate,
            default_input_channels,
            default_input_sample_rate,
        );
        (default_input_channels, default_input_sample_rate)
    };

    if input_sample_rate != sample_rate {
        log::warn!(
            "[record_and_analyze] INPUT SAMPLE RATE MISMATCH: recording at {}Hz but sweep/reference at {}Hz",
            input_sample_rate,
            sample_rate,
        );
    }

    // Validate that input_channel is within hardware capabilities
    if (input_channel as usize) >= hardware_input_channels {
        return Err(format!(
            "Input channel {} exceeds hardware channel count {} (channels are 0-indexed)",
            input_channel, hardware_input_channels
        ));
    }

    // Configure input stream
    let input_config = cpal::StreamConfig {
        channels: hardware_input_channels as u16,
        sample_rate: input_sample_rate,
        buffer_size: cpal::BufferSize::Default,
    };

    log::info!(
        "[record_and_analyze] Recording from input channel {} (0-indexed) out of {} total channels at {}Hz",
        input_channel,
        hardware_input_channels,
        input_sample_rate,
    );

    let capture_capacity = capture_capacity(input_sample_rate, expected_duration, 4.5);
    let (mut recorded_producer, mut recorded_consumer) =
        rtrb::RingBuffer::<f32>::new(capture_capacity);
    let recorded_count = Arc::new(AtomicUsize::new(0));
    let recorded_count_callback = Arc::clone(&recorded_count);
    let recorded_overruns = Arc::new(AtomicUsize::new(0));
    let recorded_overruns_callback = Arc::clone(&recorded_overruns);

    // Create input stream
    let input_channel_idx = input_channel as usize;
    let input_stream = input_device
        .build_input_stream(
            &input_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // Extract only the specified input channel
                // Data is interleaved: [ch0, ch1, ..., chN, ch0, ch1, ..., chN, ...]
                let frames = data.len() / hardware_input_channels;
                let written = write_selected_channel_to_ring(
                    &mut recorded_producer,
                    data,
                    hardware_input_channels,
                    input_channel_idx,
                );
                recorded_count_callback.fetch_add(written, Ordering::Relaxed);
                if written < frames {
                    // Count the actual dropped samples, not just the
                    // number of callbacks that saw a shortfall.
                    recorded_overruns_callback.fetch_add(frames - written, Ordering::Relaxed);
                    crate::rate_limited_log!(
                        warn,
                        5,
                        "[record_and_analyze] Input capture ring buffer overrun"
                    );
                }
            },
            |err| {
                crate::rate_limited_log!(
                    warn,
                    5,
                    "[record_and_analyze] Input stream error: {}",
                    err
                )
            },
            None,
        )
        .map_err(|e| {
            actionable_capture_error("[record_and_analyze] Failed to build input stream", &e)
        })?;

    // Start recording
    input_stream.play().map_err(|e| {
        actionable_capture_error("[record_and_analyze] Failed to start input stream", &e)
    })?;
    log::debug!("[record_and_analyze] Recording started");

    // Small delay to let recording buffer fill
    sleep(Duration::from_millis(100));

    // Honor cancellation before starting playback (mirrors the aux path).
    if cancel_requested(cancel) {
        std::mem::drop(input_stream);
        return Err(CANCELLED_ERR.to_string());
    }

    // Start playback using AudioEngineManager
    // Allow virtual output devices (BlackHole, loopback) — recording intentionally
    // sends signal through a loopback or to real speakers for mic capture.
    let mut manager = AudioEngineManager::new();
    manager.set_allow_virtual_output(true);
    manager
        .load_file(temp_wav_path)
        .map_err(|e| format!("Failed to load file: {}", e))?;

    // Get output device configuration to determine hardware channel count
    let output_device = if let Some(dev_name) = output_device_name {
        log::info!(
            "[record_and_analyze] Looking for output device: {}",
            dev_name
        );
        crate::devices::find_device(&host, dev_name, false).map_err(|e| {
            actionable_capture_error("[record_and_analyze] Output device not usable", &e)
        })?
    } else {
        log::debug!("[record_and_analyze] Using default output device");
        host.default_output_device().ok_or_else(|| {
            actionable_capture_error("[record_and_analyze]", &"No default output device available")
        })?
    };

    log::info!(
        "[record_and_analyze] Output device: {}",
        output_device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "Unknown Device".to_string())
    );

    // Get the maximum number of output channels supported by the device
    // (not just the default, which might be less than the hardware capability)
    let hardware_channels = output_device
        .supported_output_configs()
        .map_err(|e| {
            actionable_capture_error(
                "[record_and_analyze] Failed to get supported output configs",
                &e,
            )
        })?
        .map(|config| config.channels() as usize)
        .max()
        .unwrap_or_else(|| {
            // Fallback to default config if we can't query supported configs
            output_device
                .default_output_config()
                .map(|cfg| cfg.channels() as usize)
                .unwrap_or(2) // Ultimate fallback to stereo
        });

    log::info!(
        "[record_and_analyze] Hardware output channels: {}",
        hardware_channels
    );

    // Validate that output_channel is within hardware capabilities
    if (output_channel as usize) >= hardware_channels {
        return Err(format!(
            "Output channel {} exceeds hardware channel count {} (channels are 0-indexed)",
            output_channel, hardware_channels
        ));
    }

    // Create matrix plugin config to route mono signal to specific output channel
    // Use dense mapping: 1 input channel to hardware_channels output channels
    // Matrix will have all zeros except 1.0 at the target output channel
    log::info!(
        "[record_and_analyze] Routing mono input (channel 0) to hardware output channel {} (0-indexed)",
        output_channel
    );

    // Create matrix: 1 input x hardware_channels outputs
    // All zeros except position [output_channel * 1 + 0] = 1.0
    let mut matrix = vec![0.0_f32; hardware_channels];
    matrix[output_channel as usize] = 1.0;

    let matrix_params = serde_json::json!({
        "input_channels": 1,
        "output_channels": hardware_channels,
        "matrix": matrix,
    });

    use crate::engine::PluginConfig;
    let plugins = vec![PluginConfig::new("matrix", matrix_params)];

    log::info!(
        "[record_and_analyze] Matrix: 1 input -> {} outputs, channel {} active (rest silent)",
        hardware_channels,
        output_channel
    );

    // Check what output sample rate the engine will actually use
    let actual_output_rate = crate::manager::select_output_sample_rate_for_channels(
        sample_rate,
        output_device_name,
        hardware_channels,
    );
    if actual_output_rate != sample_rate {
        log::warn!(
            "[record_and_analyze] OUTPUT SAMPLE RATE MISMATCH: engine will use {}Hz but sweep is at {}Hz (engine will resample)",
            actual_output_rate,
            sample_rate,
        );
    }

    manager
        .start_playback(
            output_device_name.map(|s| s.to_string()),
            plugins,
            hardware_channels,
        )
        .map_err(|e| actionable_capture_error("[record_and_analyze] Failed to start playback", &e))?;

    log::debug!("[record_and_analyze] Playback started, waiting for completion...");

    // Wait for playback to complete
    // Maximum timeout: expected duration + 3 seconds for buffer/latency
    let total_wait = Duration::from_secs_f64(expected_duration + 3.0);
    let check_interval = Duration::from_millis(50);
    let mut elapsed = Duration::ZERO;
    let mut cancelled = false;

    while elapsed < total_wait {
        sleep(check_interval);
        elapsed += check_interval;

        // Honor cancellation between polls (worst-case ~50 ms latency).
        if cancel_requested(cancel) {
            cancelled = true;
            log::info!("[record_and_analyze] Cancellation requested — aborting capture");
            break;
        }

        // Check recording progress
        let current_sample_count = recorded_count.load(Ordering::Relaxed);

        // Print progress every second
        if elapsed.as_millis() % 1000 < check_interval.as_millis() {
            let recorded_duration = current_sample_count as f64 / input_sample_rate as f64;
            log::info!(
                "[record_and_analyze] Recording progress: {:.2}s / {:.2}s ({} samples)",
                recorded_duration,
                expected_duration,
                current_sample_count
            );
        }

        // Check for events
        manager.try_recv_event();
        let state = manager.get_state();

        if state == crate::StreamingState::Idle {
            log::debug!("[record_and_analyze] Playback state changed to Idle");
            break;
        }
    }

    // Add a buffer after playback finishes to capture any tail/latency
    // 1 second is generous but ensures we don't cut off the end of the sweep
    // on high-latency systems (e.g. large buffers, wireless, or complex routing).
    // On cancel, skip the tail capture so the UI gets snappy feedback.
    if !cancelled {
        sleep(Duration::from_millis(1000));
    }

    // Stop playback. On cancel, a stop failure must not mask the
    // cancellation: log-and-ignore so `CANCELLED_ERR` below always wins.
    cancel_aware_stop(manager.stop(), cancelled)?;

    // Stop recording
    std::mem::drop(input_stream);
    log::debug!("[record_and_analyze] Recording stopped");

    // Small delay to ensure all buffers are flushed
    sleep(Duration::from_millis(100));

    if cancelled {
        return Err(CANCELLED_ERR.to_string());
    }

    // Get recorded samples
    let recorded = drain_capture(
        &mut recorded_consumer,
        recorded_count.load(Ordering::Relaxed),
    );
    let dropped_samples = recorded_overruns.load(Ordering::Relaxed);
    if dropped_samples > 0 {
        log::warn!(
            "[record_and_analyze] Dropped {} input samples because the capture ring buffer filled",
            dropped_samples
        );
    }
    // Use the actual input sample rate for duration/WAV/analysis calculations
    let analysis_sample_rate = input_sample_rate;
    let recorded_duration = recorded.len() as f64 / analysis_sample_rate as f64;
    log::info!(
        "[record_and_analyze] Total recorded: {} samples ({:.2}s at {}Hz)",
        recorded.len(),
        recorded_duration,
        analysis_sample_rate,
    );

    if recorded.is_empty() {
        return Err("No samples were recorded".to_string());
    }

    // Diagnostic: check recorded signal statistics
    let max_amplitude = recorded.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    let rms = (recorded.iter().map(|s| s * s).sum::<f32>() / recorded.len() as f32).sqrt();
    let rms_db = if rms > 0.0 {
        20.0 * rms.log10()
    } else {
        f32::NEG_INFINITY
    };
    log::info!(
        "[record_and_analyze] Recorded signal: max={:.6}, RMS={:.6} ({:.1} dBFS)",
        max_amplitude,
        rms,
        rms_db,
    );
    if max_amplitude < 1e-6 {
        // Consistent with the aux capture path (`play_per_channel_and_record_mono`):
        // a silent take must not silently succeed and feed garbage to roomeq.
        return Err(format!(
            "[record_and_analyze] Recording appears silent (peak {:.6}). Check mic, input channel, and output device availability.",
            max_amplitude
        ));
    }
    if rms_db < -60.0 {
        log::warn!(
            "[record_and_analyze] WARNING: Recorded signal is very quiet ({:.1} dBFS RMS)",
            rms_db
        );
    }
    // Refuse hard-clipped takes; warn on moderate clipping.
    check_capture_clipping(&recorded, "record_and_analyze")?;

    Ok(RawSweepTake {
        recorded,
        analysis_sample_rate,
        dropped_samples: dropped_samples as u64,
    })
}

/// One play/record cycle capturing all `input_channels` simultaneously
/// (multi-mic variant of [`capture_sweep_take`]). Applies the capture gates
/// (cancel, silence, clipping) per mic; analysis lives in
/// [`analyze_sweep_takes`].
#[cfg(not(target_os = "ios"))]
#[allow(clippy::too_many_arguments)]
fn capture_sweep_take_multi(
    temp_wav_path: &Path,
    reference_signal: &[f32],
    sample_rate: u32,
    output_channel: u16,
    input_channels: &[u16],
    output_device_name: Option<&str>,
    input_device_name: Option<&str>,
    cancel: Option<&CancelFlag>,
) -> Result<RawSweepTakeMulti, String> {
    use crate::AudioEngineManager;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::thread::sleep;
    use std::time::Duration;

    if cancel_requested(cancel) {
        return Err(CANCELLED_ERR.to_string());
    }

    let num_mics = input_channels.len();
    log::info!(
        "[record_and_analyze_multi] Starting: {} mics, output_ch={}, input_chs={:?}",
        num_mics,
        output_channel,
        input_channels,
    );

    let expected_duration = reference_signal.len() as f64 / sample_rate as f64;
    log::info!(
        "[record_and_analyze_multi] Expected duration: {:.2}s",
        expected_duration,
    );

    // --- Set up input device ---
    let host = cpal::default_host();

    let input_device = if let Some(dev_name) = input_device_name {
        crate::devices::find_device(&host, dev_name, true).map_err(|e| {
            actionable_capture_error("[record_and_analyze_multi] Input device not usable", &e)
        })?
    } else {
        host.default_input_device().ok_or_else(|| {
            actionable_capture_error(
                "[record_and_analyze_multi]",
                &"No default input device available",
            )
        })?
    };

    let default_input_config = input_device.default_input_config().map_err(|e| {
        actionable_capture_error(
            "[record_and_analyze_multi] Failed to get default input config",
            &e,
        )
    })?;

    let max_input_ch = input_channels.iter().copied().max().unwrap_or(0) as usize;
    let min_channels_needed = max_input_ch + 1;

    let best_config = input_device
        .supported_input_configs()
        .ok()
        .and_then(|configs| {
            configs
                .filter(|c| {
                    let ch = c.channels() as usize;
                    ch >= min_channels_needed
                        && c.min_sample_rate() <= sample_rate
                        && c.max_sample_rate() >= sample_rate
                })
                .min_by_key(|c| c.channels())
        });

    let (hardware_input_channels, input_sample_rate) = if let Some(config) = best_config {
        (config.channels() as usize, sample_rate)
    } else {
        (
            default_input_config.channels() as usize,
            default_input_config.sample_rate(),
        )
    };

    if input_sample_rate != sample_rate {
        log::warn!(
            "[record_and_analyze_multi] INPUT SAMPLE RATE MISMATCH: {}Hz vs {}Hz",
            input_sample_rate,
            sample_rate,
        );
    }

    for &ch in input_channels {
        if (ch as usize) >= hardware_input_channels {
            return Err(format!(
                "Input channel {} exceeds hardware channel count {}",
                ch, hardware_input_channels,
            ));
        }
    }

    let input_config = cpal::StreamConfig {
        channels: hardware_input_channels as u16,
        sample_rate: input_sample_rate,
        buffer_size: cpal::BufferSize::Default,
    };

    // --- Lock-free recording buffers (one SPSC ring per mic) ---
    let capture_capacity = capture_capacity(input_sample_rate, expected_duration, 4.5);
    let mut recorded_producers = Vec::with_capacity(num_mics);
    let mut recorded_consumers = Vec::with_capacity(num_mics);
    for _ in 0..num_mics {
        let (producer, consumer) = rtrb::RingBuffer::<f32>::new(capture_capacity);
        recorded_producers.push(producer);
        recorded_consumers.push(consumer);
    }
    let recorded_counts: Vec<Arc<AtomicUsize>> = (0..num_mics)
        .map(|_| Arc::new(AtomicUsize::new(0)))
        .collect();
    let counts_callback: Vec<Arc<AtomicUsize>> = recorded_counts.iter().map(Arc::clone).collect();
    let recorded_overruns = Arc::new(AtomicUsize::new(0));
    let recorded_overruns_callback = Arc::clone(&recorded_overruns);

    let input_channels_vec: Vec<usize> = input_channels.iter().map(|&c| c as usize).collect();

    let input_stream = input_device
        .build_input_stream(
            &input_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let frames = data.len() / hardware_input_channels;
                for (mic_i, &ch_idx) in input_channels_vec.iter().enumerate() {
                    let written = write_selected_channel_to_ring(
                        &mut recorded_producers[mic_i],
                        data,
                        hardware_input_channels,
                        ch_idx,
                    );
                    counts_callback[mic_i].fetch_add(written, Ordering::Relaxed);
                    if ch_idx < hardware_input_channels && written < frames {
                        // Count the actual dropped samples, not just the
                        // number of callbacks that saw a shortfall.
                        recorded_overruns_callback.fetch_add(frames - written, Ordering::Relaxed);
                        crate::rate_limited_log!(
                            warn,
                            5,
                            "[record_and_analyze_multi] Input capture ring buffer overrun"
                        );
                    }
                }
            },
            |err| {
                crate::rate_limited_log!(
                    warn,
                    5,
                    "[record_and_analyze_multi] Input stream error: {}",
                    err
                )
            },
            None,
        )
        .map_err(|e| {
            actionable_capture_error(
                "[record_and_analyze_multi] Failed to build input stream",
                &e,
            )
        })?;

    input_stream.play().map_err(|e| {
        actionable_capture_error(
            "[record_and_analyze_multi] Failed to start input stream",
            &e,
        )
    })?;

    sleep(Duration::from_millis(100));

    // Honor cancellation before starting playback (mirrors the aux path).
    if cancel_requested(cancel) {
        std::mem::drop(input_stream);
        return Err(CANCELLED_ERR.to_string());
    }

    // --- Start playback ---
    let mut manager = AudioEngineManager::new();
    manager
        .load_file(temp_wav_path)
        .map_err(|e| format!("Failed to load file: {}", e))?;

    let output_device = if let Some(dev_name) = output_device_name {
        crate::devices::find_device(&host, dev_name, false).map_err(|e| {
            actionable_capture_error("[record_and_analyze_multi] Output device not usable", &e)
        })?
    } else {
        host.default_output_device().ok_or_else(|| {
            actionable_capture_error(
                "[record_and_analyze_multi]",
                &"No default output device available",
            )
        })?
    };

    let hardware_channels = output_device
        .supported_output_configs()
        .map_err(|e| {
            actionable_capture_error(
                "[record_and_analyze_multi] Failed to get supported output configs",
                &e,
            )
        })?
        .map(|config| config.channels() as usize)
        .max()
        .unwrap_or_else(|| {
            output_device
                .default_output_config()
                .map(|cfg| cfg.channels() as usize)
                .unwrap_or(2)
        });

    if (output_channel as usize) >= hardware_channels {
        return Err(format!(
            "Output channel {} exceeds hardware channel count {}",
            output_channel, hardware_channels,
        ));
    }

    let mut matrix = vec![0.0_f32; hardware_channels];
    matrix[output_channel as usize] = 1.0;
    let matrix_params = serde_json::json!({
        "input_channels": 1,
        "output_channels": hardware_channels,
        "matrix": matrix,
    });

    use crate::engine::PluginConfig;
    let plugins = vec![PluginConfig::new("matrix", matrix_params)];

    let actual_output_rate = crate::manager::select_output_sample_rate_for_channels(
        sample_rate,
        output_device_name,
        hardware_channels,
    );
    if actual_output_rate != sample_rate {
        log::warn!(
            "[record_and_analyze_multi] OUTPUT SAMPLE RATE MISMATCH: engine {}Hz vs sweep {}Hz",
            actual_output_rate,
            sample_rate,
        );
    }

    manager
        .start_playback(
            output_device_name.map(|s| s.to_string()),
            plugins,
            hardware_channels,
        )
        .map_err(|e| {
            actionable_capture_error("[record_and_analyze_multi] Failed to start playback", &e)
        })?;

    // --- Wait for playback to complete ---
    let total_wait = Duration::from_secs_f64(expected_duration + 3.0);
    let check_interval = Duration::from_millis(50);
    let mut elapsed = Duration::ZERO;
    let mut cancelled = false;

    while elapsed < total_wait {
        sleep(check_interval);
        elapsed += check_interval;

        // Honor cancellation between polls (worst-case ~50 ms latency).
        if cancel_requested(cancel) {
            cancelled = true;
            log::info!("[record_and_analyze_multi] Cancellation requested — aborting capture");
            break;
        }

        let current_sample_count = recorded_counts[0].load(Ordering::Relaxed);

        if elapsed.as_millis() % 1000 < check_interval.as_millis() {
            let recorded_duration = current_sample_count as f64 / input_sample_rate as f64;
            log::info!(
                "[record_and_analyze_multi] Progress: {:.2}s / {:.2}s",
                recorded_duration,
                expected_duration,
            );
        }

        manager.try_recv_event();
        if manager.get_state() == crate::StreamingState::Idle {
            break;
        }
    }

    // On cancel, skip the tail-capture sleep so the UI gets snappy feedback.
    if !cancelled {
        sleep(Duration::from_millis(1000));
    }
    // On cancel, a stop failure must not mask the cancellation:
    // log-and-ignore so `CANCELLED_ERR` below always wins.
    cancel_aware_stop(manager.stop(), cancelled)?;
    std::mem::drop(input_stream);
    sleep(Duration::from_millis(100));

    if cancelled {
        return Err(CANCELLED_ERR.to_string());
    }

    // --- Drain and gate each mic channel ---
    let analysis_sample_rate = input_sample_rate;
    let dropped_samples = recorded_overruns.load(Ordering::Relaxed);
    if dropped_samples > 0 {
        log::warn!(
            "[record_and_analyze_multi] Dropped {} input samples because capture ring buffers filled",
            dropped_samples
        );
    }

    let mut recorded_per_mic = Vec::with_capacity(num_mics);
    for mic_i in 0..num_mics {
        let recorded = drain_capture(
            &mut recorded_consumers[mic_i],
            recorded_counts[mic_i].load(Ordering::Relaxed),
        );
        let recorded_duration = recorded.len() as f64 / analysis_sample_rate as f64;
        log::info!(
            "[record_and_analyze_multi] Mic {}: {} samples ({:.2}s)",
            mic_i,
            recorded.len(),
            recorded_duration,
        );

        if recorded.is_empty() {
            return Err(format!("No samples recorded on mic {}", mic_i));
        }

        // Consistent with the single-mic sweep path and the aux capture
        // path: a silent take must not silently succeed.
        let peak = recorded.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        if peak < 1e-6 {
            return Err(format!(
                "[record_and_analyze_multi] Mic {} recording appears silent (peak {:.6}). Check mic, input channel, and output device availability.",
                mic_i, peak
            ));
        }
        // Refuse hard-clipped takes; warn on moderate clipping.
        check_capture_clipping(&recorded, &format!("record_and_analyze_multi mic {mic_i}"))?;

        recorded_per_mic.push(recorded);
    }

    Ok(RawSweepTakeMulti {
        recorded_per_mic,
        analysis_sample_rate,
        dropped_samples: dropped_samples as u64,
    })
}

/// Per-take clock-drift bookkeeping: the ppm-normalized estimate, whether
/// correction was applied, and the severe-drift advisory with its |ppm|.
#[cfg(not(target_os = "ios"))]
type TakeDrift = (Option<ClockDriftEstimate>, bool, Option<(f64, String)>);

/// Estimate clock drift on one take and, when warranted, time-rescale it
/// before averaging/analysis. Returns the (possibly corrected) take and its
/// [`TakeDrift`] bookkeeping. Drift alone never fails the take.
///
/// Two guards against garbage estimates (task-8 review A1):
/// - estimates above [`super::quality::DRIFT_IMPLAUSIBLE_PPM`] are physically
///   implausible for an audio clock and are never applied;
/// - after a successful `correct_clock_drift` the corrected take must still
///   lock onto the reference ([`correction_keeps_lock`]); if the lock
///   collapses the estimate was wrong, not the clock, and the correction is
///   discarded (the raw take is kept, the advisory still stands).
#[cfg(not(target_os = "ios"))]
fn correct_take_clock_drift(
    recorded: &[f32],
    analysis_reference: &[f32],
    analysis_sample_rate: u32,
    log_tag: &str,
) -> (Vec<f32>, TakeDrift) {
    // The estimator windows the ends of the reference, so hand it the active
    // sweep rather than the silence-padded playback reference.
    let drift_reference = super::quality::active_reference_span(analysis_reference);
    let drift = crate::signal_analysis::estimate_clock_drift(
        drift_reference,
        recorded,
        analysis_sample_rate,
    )
    .ok()
    .map(|raw| normalize_clock_drift_ppm(raw, analysis_sample_rate));
    match (drift_action(drift.as_ref()), drift) {
        (DriftAction::None, _) | (_, None) => (recorded.to_vec(), (drift, false, None)),
        (DriftAction::Implausible, Some(estimate)) => {
            log::warn!(
                "[{log_tag}] Clock-drift estimate {:.0} ppm is physically implausible for an \
                 audio clock — skipping correction",
                estimate.ppm,
            );
            let advisory = (
                estimate.ppm.abs(),
                format!(
                    "clock drift estimate of {:.0} ppm is physically implausible for an audio \
                     clock — correction skipped; check the capture for dropouts or a wrong \
                     reference signal",
                    estimate.ppm,
                ),
            );
            (recorded.to_vec(), (Some(estimate), false, Some(advisory)))
        }
        (action, Some(estimate)) => {
            log::warn!(
                "[{log_tag}] Clock drift {:.1} ppm detected (split DAC/ADC clocks, e.g. USB mic) — correcting capture before analysis",
                estimate.ppm,
            );
            let advisory = (action == DriftAction::CorrectAndAdvise).then(|| {
                (
                    estimate.ppm.abs(),
                    format!(
                        "clock drift {:.1} ppm exceeds {:.0} ppm — long sweeps on split-clock \
                         setups (separate DAC/ADC, e.g. USB mic) smear HF phase even after \
                         correction; prefer a single audio device or a loopback reference",
                        estimate.ppm,
                        super::quality::DRIFT_SEVERE_PPM,
                    ),
                )
            });
            match crate::signal_analysis::correct_clock_drift(recorded, &estimate) {
                Ok(corrected) => {
                    if correction_keeps_lock(
                        analysis_reference,
                        recorded,
                        &corrected,
                        analysis_sample_rate,
                    ) {
                        (corrected, (Some(estimate), true, advisory))
                    } else {
                        log::warn!(
                            "[{log_tag}] Clock-drift correction collapsed the lag lock — \
                             discarding the correction and keeping the raw capture"
                        );
                        (recorded.to_vec(), (Some(estimate), false, advisory))
                    }
                }
                Err(e) => {
                    log::warn!(
                        "[{log_tag}] Clock-drift correction failed ({e}) — using the uncorrected capture"
                    );
                    (recorded.to_vec(), (Some(estimate), false, advisory))
                }
            }
        }
    }
}

/// Correct-then-verify (task-8 review A1): a correction is kept only when the
/// corrected take still locks onto the reference like the raw take did:
///
/// - the lag-lock confidence must not collapse (less than half the raw
///   take's) nor fall below the quality gate's minimum while the raw take
///   passed it;
/// - the bulk lag must stay put: a drift correction preserves the capture's
///   start, so the lock point can move by at most a small tolerance
///   (20 ms). A wild shift means the "correction" mangled the take.
///
/// A garbage drift estimate can stretch a clean take out of lock; this check
/// catches that without relying on downstream MAD rejection. Confidence alone
/// is not sufficient — the estimator reports spuriously high confidence on
/// pure noise, but at a meaningless lag.
#[cfg(not(target_os = "ios"))]
pub(super) fn correction_keeps_lock(
    reference: &[f32],
    raw: &[f32],
    corrected: &[f32],
    sample_rate: u32,
) -> bool {
    use crate::signal_analysis::MeasurementQualityConfig;

    // If the raw take never locked there is no good lock to destroy — leave
    // the verdict to the downstream gates (single-take lag gate, MAD).
    let Ok(raw_lock) = crate::signal_analysis::estimate_lag_with_confidence(reference, raw) else {
        return true;
    };
    let corrected_lock =
        match crate::signal_analysis::estimate_lag_with_confidence(reference, corrected) {
            Ok(lock) => lock,
            // The corrected take no longer locks at all — discard.
            Err(_) => return false,
        };
    let minimum = MeasurementQualityConfig::default().minimum_lag_confidence;
    if corrected_lock.confidence < raw_lock.confidence * 0.5 {
        return false;
    }
    if corrected_lock.confidence < minimum && raw_lock.confidence >= minimum {
        return false;
    }
    let lag_tolerance = (sample_rate / 50).max(1) as isize; // 20 ms
    (corrected_lock.lag_samples - raw_lock.lag_samples).abs() <= lag_tolerance
}

/// REW-style synchronous averaging: align each accepted take at its lag,
/// zero-pad to the common length, and average in the time domain. The result
/// is what gets written to disk and analyzed, so the final curve is
/// consistent with the averaged-complex response used for coherence.
#[cfg(not(target_os = "ios"))]
fn average_aligned_takes(
    takes: &[Vec<f32>],
    lag_estimates: &[LagEstimate],
    accepted_indices: &[usize],
) -> Result<Vec<f32>, String> {
    let mut common_len = 0;
    for &index in accepted_indices {
        let lag = lag_estimates[index].lag_samples.max(0) as usize;
        common_len = common_len.max(takes[index].len().saturating_sub(lag));
    }
    if common_len == 0 {
        return Err(
            "repeat-sweep averaging: no accepted take has samples past its lag".to_string(),
        );
    }
    let mut sum = vec![0.0_f64; common_len];
    for &index in accepted_indices {
        let lag = lag_estimates[index].lag_samples.max(0) as usize;
        for (out, &sample) in sum.iter_mut().zip(takes[index][lag..].iter()) {
            *out += sample as f64;
        }
    }
    let scale = 1.0 / accepted_indices.len() as f64;
    Ok(sum.iter().map(|value| (value * scale) as f32).collect())
}

/// Sibling path preserving a repeat capture's raw (pre-correction) take,
/// e.g. `L.wav` → `L.take2.wav` for the second take.
#[cfg(not(target_os = "ios"))]
fn raw_take_wav_path(recorded_wav_path: &Path, take_index: usize) -> PathBuf {
    let stem = recorded_wav_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "recording".to_string());
    recorded_wav_path.with_file_name(format!("{stem}.take{}.wav", take_index + 1))
}

/// Remove pre-existing `{stem}.take*.wav` siblings of a capture WAV (task-8
/// review A4): rerunning a session with fewer sweeps would otherwise leave
/// higher-N raw takes from the previous run next to the new averaged WAV.
/// Removal failures are logged and ignored.
#[cfg(not(target_os = "ios"))]
fn remove_stale_take_wavs(recorded_wav_path: &Path, log_tag: &str) {
    let stem = recorded_wav_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "recording".to_string());
    let prefix = format!("{stem}.take");
    let Some(parent) = recorded_wav_path.parent() else {
        return;
    };
    let entries = match std::fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(e) => {
            log::warn!(
                "[{log_tag}] Could not list {:?} for stale take-WAV cleanup: {e}",
                parent
            );
            return;
        }
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // Match exactly `{stem}.take{N}.wav`: a bare prefix match would also
        // hit unrelated user files like `{stem}.takeaway.wav` (task-10
        // review).
        let is_take_wav = name
            .strip_prefix(&prefix)
            .and_then(|rest| rest.strip_suffix(".wav"))
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
        if is_take_wav
            && let Err(e) = std::fs::remove_file(entry.path())
        {
            log::warn!(
                "[{log_tag}] Could not remove stale take WAV {:?}: {e}",
                entry.path()
            );
        }
    }
}

/// Write a mono capture WAV and verify the round-trip channel count.
#[cfg(not(target_os = "ios"))]
fn write_and_verify_mono_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    write_wav_file(path, samples, sample_rate, 1)?;
    let reader =
        hound::WavReader::open(path).map_err(|e| format!("Failed to verify WAV file: {}", e))?;
    if reader.spec().channels != 1 {
        return Err(format!(
            "ERROR: WAV file has {} channels instead of 1 (mono)!",
            reader.spec().channels
        ));
    }
    Ok(())
}

/// Frequency of each bin of a one-sided FFT spectrum (`fft_size/2 + 1` bins,
/// matching math-dsp's `deconvolve_sweep` / noise-floor grids).
#[cfg(not(target_os = "ios"))]
fn fft_bin_frequencies(bin_count: usize, sample_rate: u32) -> Vec<f32> {
    let fft_size = bin_count.saturating_sub(1) * 2;
    if fft_size == 0 {
        return Vec::new();
    }
    (0..bin_count)
        .map(|bin| bin as f32 * sample_rate as f32 / fft_size as f32)
        .collect()
}

/// Pre-sweep room-noise window of one take.
///
/// The capture lead-in before the reference's first sample arrives
/// (`take[..lag]`) is room noise; when the reference itself starts with
/// pre-silence (OctaveSweep's default 2 s), that silence is part of the
/// aligned reference content, so the take's noise region extends through
/// `lag + active_reference_start`. Both regions are contiguous, so the
/// window is simply `take[..lag + active_start]` (clamped, and shrunk for
/// negative lags).
#[cfg(not(target_os = "ios"))]
fn pre_silence_window<'a>(take: &'a [f32], reference: &[f32], lag: &LagEstimate) -> &'a [f32] {
    let active_start = super::quality::active_reference_start(reference) as isize;
    let noise_len = (active_start + lag.lag_samples).max(0) as usize;
    &take[..noise_len.min(take.len())]
}

/// Longest pre-sweep room-noise window across the accepted takes. The
/// longest single window is the cleanest estimator input — concatenating
/// windows would invent discontinuities at the joins.
#[cfg(not(target_os = "ios"))]
fn longest_pre_silence<'a>(
    takes: &'a [Vec<f32>],
    reference: &[f32],
    lag_estimates: &[LagEstimate],
    accepted_indices: &[usize],
) -> &'a [f32] {
    let mut best: &[f32] = &[];
    for &index in accepted_indices {
        let window = pre_silence_window(&takes[index], reference, &lag_estimates[index]);
        if window.len() > best.len() {
            best = window;
        }
    }
    best
}

/// Post-process the raw takes of one channel: per-take clock-drift
/// correction, robust multi-take averaging for repeat captures, WAV/CSV
/// output, and the per-take quality report.
///
/// Order of operations for repeat captures (Task 8): **per-take drift-correct
/// → align → average → analyze**. Drift correction must precede averaging so
/// a drifting take does not smear the synchronous average; lag alignment
/// comes from math-dsp's `average_ess_recordings`, whose per-take
/// `LagEstimate`s also feed the Task-7 lag-lock gate. A take that cannot
/// lock (or fails the confidence gate) aborts the whole set — no retry,
/// mirroring REW's abort semantics: a disconnected mic or a noisy room will
/// not fix itself between takes, and the actionable error reaches the user
/// immediately.
///
/// The WAV at `recorded_wav_path` is exactly what was analyzed: the
/// corrected take for single-sweep captures (Task-7 behavior), or the
/// drift-corrected, lag-aligned, synchronously averaged accepted takes for
/// repeats. Repeat captures additionally preserve each raw (pre-correction)
/// take in a `*.take{N}.wav` sibling so per-take raw data survives.
#[cfg(not(target_os = "ios"))]
#[allow(clippy::too_many_arguments)]
pub(super) fn analyze_sweep_takes(
    recorded_wav_path: &Path,
    output_csv_path: &Path,
    raw_takes: &[Vec<f32>],
    reference_signal: &[f32],
    sample_rate: u32,
    analysis_sample_rate: u32,
    sweep_range: Option<(f32, f32)>,
    compensation: Option<&MicrophoneCompensation>,
    dropped_samples: u64,
    log_tag: &str,
) -> Result<CaptureAnalysis, String> {
    if raw_takes.is_empty() {
        return Err(format!("[{log_tag}] No takes were captured"));
    }

    // Bring the reference onto the capture's sample rate before the
    // correlation-based gates (lag lock, clock drift) and the analysis.
    let resampled_reference = (analysis_sample_rate != sample_rate)
        .then(|| resample_reference_signal(reference_signal, sample_rate, analysis_sample_rate))
        .transpose()?;
    let analysis_reference = resampled_reference.as_deref().unwrap_or(reference_signal);
    let single_take = raw_takes.len() == 1;

    // Per-take clock-drift handling (R5), before averaging. Single-take
    // captures keep the Task-7 gate order: the lag lock runs on the raw take,
    // before drift handling.
    let mut corrected_takes = Vec::with_capacity(raw_takes.len());
    let mut per_take_drift: Vec<TakeDrift> = Vec::with_capacity(raw_takes.len());
    let mut single_lag = None;
    for (take_index, raw) in raw_takes.iter().enumerate() {
        let take_tag = if single_take {
            log_tag.to_string()
        } else {
            format!("{log_tag} take {}/{}", take_index + 1, raw_takes.len())
        };
        if single_take {
            // Hard gate: no confident cross-correlation peak means the sweep
            // never locked onto the capture — analyzing it would proceed at
            // an arbitrary lag and feed garbage to roomeq (same spirit as
            // the silence gate).
            let lag = super::quality::estimate_lag_or_advise(analysis_reference, raw, &take_tag)?;
            check_lag_lock(&lag, &take_tag)?;
            single_lag = Some(lag);
        }
        let (corrected, take_drift) =
            correct_take_clock_drift(raw, analysis_reference, analysis_sample_rate, &take_tag);
        per_take_drift.push(take_drift);
        corrected_takes.push(corrected);
    }

    let lag_for_quality: LagEstimate;
    let analysis_capture: Vec<f32>;
    let accepted_count: usize;
    let rejected_count: usize;
    let relevant_takes: Vec<usize>;
    // (frequencies, values) on the deconvolution FFT grid; repeats only.
    let mut coherence_grid: Option<(Vec<f32>, Vec<f32>)> = None;
    let mut measured_grid: Option<(Vec<f32>, Vec<f32>)> = None;
    let noise_floor_grid: Option<(Vec<f32>, Vec<f32>)>;

    if single_take {
        lag_for_quality =
            single_lag.expect("single-take path always gates the lag before this point");
        analysis_capture = corrected_takes
            .into_iter()
            .next()
            .expect("single-take path has exactly one take");
        accepted_count = 1;
        rejected_count = 0;
        relevant_takes = vec![0];

        let silence = pre_silence_window(&analysis_capture, analysis_reference, &lag_for_quality);
        noise_floor_grid = (silence.len() >= MIN_NOISE_FLOOR_WINDOW_SAMPLES).then(|| {
            let values = crate::signal_analysis::estimate_noise_floor_db_from_silence(
                silence,
                analysis_sample_rate,
            );
            (
                fft_bin_frequencies(values.len(), analysis_sample_rate),
                values,
            )
        });
    } else {
        // Robustly align, deconvolve, and median/MAD-average the takes. Any
        // take that cannot lock aborts the set (REW abort semantics).
        let averaged = crate::signal_analysis::average_ess_recordings(
            &corrected_takes,
            analysis_reference,
            analysis_sample_rate,
        )
        .map_err(|e| {
            format!(
                "[{log_tag}] Repeat-sweep set unusable ({e}) — check mic connection, \
                 input channel, and playback level; background noise may be too high"
            )
        })?;

        // Task-7 lag-lock gate per take, on the estimates of the
        // drift-corrected takes actually used for alignment.
        for (take_index, lag) in averaged.lag_estimates.iter().enumerate() {
            check_lag_lock(
                lag,
                &format!("{log_tag} take {}/{}", take_index + 1, raw_takes.len()),
            )?;
        }

        let accepted = &averaged.averaged.accepted_indices;
        let rejected = &averaged.averaged.rejected_indices;
        if !rejected.is_empty() {
            log::warn!(
                "[{log_tag}] Rejected {} of {} takes as median/MAD outliers: {:?}",
                rejected.len(),
                raw_takes.len(),
                rejected,
            );
        }

        analysis_capture =
            average_aligned_takes(&corrected_takes, &averaged.lag_estimates, accepted)?;
        accepted_count = accepted.len();
        rejected_count = rejected.len();
        relevant_takes = accepted.clone();
        lag_for_quality = averaged.lag_estimates[accepted[0]];

        // Measured spectrum + coherence on the shared deconvolution FFT grid.
        let response = &averaged.averaged.response;
        let measured_db: Vec<f32> = response
            .iter()
            .map(|value| {
                let magnitude = value.norm();
                if magnitude > 1e-10 {
                    20.0 * magnitude.log10()
                } else {
                    -200.0
                }
            })
            .collect();
        measured_grid = Some((
            fft_bin_frequencies(response.len(), analysis_sample_rate),
            measured_db,
        ));
        if !averaged.averaged.coherence.is_empty() {
            coherence_grid = Some((
                fft_bin_frequencies(averaged.averaged.coherence.len(), analysis_sample_rate),
                averaged.averaged.coherence.clone(),
            ));
        } else {
            log::info!(
                "[{log_tag}] Coherence unavailable ({accepted_count} accepted takes < 4) — \
                 the CSV column is omitted and autoeq's gate will degrade as designed"
            );
        }

        let silence = longest_pre_silence(
            &corrected_takes,
            analysis_reference,
            &averaged.lag_estimates,
            accepted,
        );
        noise_floor_grid = (silence.len() >= MIN_NOISE_FLOOR_WINDOW_SAMPLES).then(|| {
            let values = crate::signal_analysis::estimate_noise_floor_db_from_silence(
                silence,
                analysis_sample_rate,
            );
            (
                fft_bin_frequencies(values.len(), analysis_sample_rate),
                values,
            )
        });
    }

    // Drift fields/advisory consider only takes that made it into the
    // analysis: a rejected take's pathologies must not flag the session.
    let first_drift = relevant_takes.iter().find_map(|&i| per_take_drift[i].0);
    let any_drift_corrected = relevant_takes.iter().any(|&i| per_take_drift[i].1);
    let mut quality_issues: Vec<String> = Vec::new();
    if let Some((_, message)) = relevant_takes
        .iter()
        .filter_map(|&i| per_take_drift[i].2.as_ref())
        .max_by(|l, r| l.0.total_cmp(&r.0))
    {
        quality_issues.push(message.clone());
    }

    // The WAV on disk is exactly what gets analyzed.
    log::info!(
        "[{log_tag}] Writing {} mono samples to WAV file...",
        analysis_capture.len()
    );
    write_and_verify_mono_wav(recorded_wav_path, &analysis_capture, analysis_sample_rate)?;
    log::info!(
        "[{log_tag}] Wrote {} samples as MONO (1 channel) to {:?}",
        analysis_capture.len(),
        recorded_wav_path
    );

    // Repeat captures preserve every raw (pre-drift-correction) take next to
    // the averaged WAV so per-take raw data survives for post-mortem (the
    // Task-7 single-take path intentionally keeps its old behavior). Stale
    // `*.takeN.wav` siblings from a previous run with a higher take count
    // are removed first so the directory cannot mix takes from two sessions
    // (task-8 review A4); a failing raw-take write only warns — the averaged
    // WAV/CSV/analysis are already valid at this point (task-8 review A3).
    if !single_take {
        remove_stale_take_wavs(recorded_wav_path, log_tag);
        for (take_index, raw) in raw_takes.iter().enumerate() {
            let raw_path = raw_take_wav_path(recorded_wav_path, take_index);
            if let Err(e) = write_wav_file(&raw_path, raw, analysis_sample_rate, 1) {
                log::warn!(
                    "[{log_tag}] Failed to preserve raw take WAV {:?} ({e}) — continuing; the averaged measurement is unaffected",
                    raw_path
                );
            }
        }
        log::info!(
            "[{log_tag}] Preserved {} raw take WAV(s) next to {:?} (*.takeN.wav)",
            raw_takes.len(),
            recorded_wav_path
        );
    }

    // Analyze the recording
    log::debug!("[{log_tag}] Analyzing recording...");
    let analysis = analyze_recording(
        recorded_wav_path,
        analysis_reference,
        analysis_sample_rate,
        sweep_range,
    )?;

    // Extended CSV: the FFT-grid curves are log-frequency interpolated onto
    // the analysis grid and appended as `coherence` / `noise_floor_db`
    // columns. Coherence is omitted (not fabricated) when no real multi-take
    // coherence exists.
    let coherence_column = coherence_grid.as_ref().map(|(freqs, values)| {
        interpolate_log_frequency_grid(freqs, values, &analysis.frequencies)
    });
    let noise_floor_column = noise_floor_grid.as_ref().map(|(freqs, values)| {
        interpolate_log_frequency_grid(freqs, values, &analysis.frequencies)
    });
    write_analysis_csv_extended(
        &analysis,
        output_csv_path,
        compensation,
        coherence_column.as_deref(),
        noise_floor_column.as_deref(),
    )?;
    log::info!("[{log_tag}] Wrote analysis to {:?}", output_csv_path);

    // Per-take quality report. Advisory only — the silence/clip/lag gates
    // above already hard-failed unusable takes. Repeat captures supply the
    // real coherence and the measured-spectrum/noise-floor pair (both on the
    // deconvolution grid); single takes keep the Task-7 None-metrics.
    let (quality_coherence, quality_measured, quality_noise) = if single_take {
        (None, None, None)
    } else {
        let noise_on_measured_grid = match (&measured_grid, &noise_floor_grid) {
            (Some((measured_freqs, _)), Some((noise_freqs, noise_values))) => Some(
                interpolate_log_frequency_grid(noise_freqs, noise_values, measured_freqs),
            ),
            _ => None,
        };
        match noise_on_measured_grid {
            Some(noise) => (
                coherence_grid.as_ref().map(|(_, values)| values.as_slice()),
                measured_grid.as_ref().map(|(_, values)| values.as_slice()),
                Some(noise),
            ),
            // The SNR pair must be supplied together or not at all (see
            // build_capture_quality).
            None => (
                coherence_grid.as_ref().map(|(_, values)| values.as_slice()),
                None,
                None,
            ),
        }
    };
    let quality = build_capture_quality(
        &analysis_capture,
        &lag_for_quality,
        quality_coherence,
        quality_measured,
        quality_noise.as_deref(),
        quality_issues,
    );
    log_capture_quality(&quality, log_tag);

    Ok(CaptureAnalysis {
        result: analysis,
        quality,
        drift: first_drift,
        drift_corrected: any_drift_corrected,
        dropped_samples,
        accepted_count,
        rejected_count,
    })
}

/// Perform recording and analysis using AudioEngineManager for playback
/// and cpal for recording.
///
/// Plays back a signal to a specific output channel while simultaneously
/// recording from a specific input channel, then analyzes the result.
///
/// `num_sweeps` selects the repeat-sweep path (Task 8): N sequential
/// play/record takes are captured (see [`super::consts::DEFAULT_NUM_SWEEPS`]
/// for the recommended default), per-take clock-drift-corrected, robustly
/// averaged (median/MAD outlier rejection + coherence via math-dsp's
/// `average_ess_recordings`), and the averaged capture is analyzed and
/// written to `recorded_wav_path` / `output_csv_path` (the CSV gains real
/// `coherence` and `noise_floor_db` columns). `num_sweeps <= 1` keeps the
/// legacy single-sweep behavior; `num_sweeps == 2` is bumped to
/// [`super::consts::MIN_REPEAT_SWEEPS`] (3) with a warning because two
/// takes cannot reject outliers. A cancelled or gated-out take aborts the
/// whole set (no retry — REW abort semantics).
///
/// After the capture passes the silence/clipping gates it goes through the
/// per-take quality pipeline (see [`super::quality`]): a hard lag-lock gate,
/// clock-drift estimation with optional correction, and a quality report —
/// all returned in [`CaptureAnalysis`].
///
/// `cancel` is a cooperative cancellation flag (see [`CancelFlag`]): when
/// set, the capture stops the streams and returns
/// `Err(CANCELLED_ERR)` ("cancelled") instead of analyzing.
#[cfg(not(target_os = "ios"))]
#[allow(clippy::too_many_arguments)]
pub fn record_and_analyze(
    temp_wav_path: &Path,
    recorded_wav_path: &Path,
    reference_signal: &[f32],
    sample_rate: u32,
    output_csv_path: &Path,
    output_channel: u16,
    input_channel: u16,
    output_device_name: Option<&str>,
    input_device_name: Option<&str>,
    microphone_compensation_path: Option<&str>,
    sweep_range: Option<(f32, f32)>,
    num_sweeps: u16,
    cancel: Option<CancelFlag>,
) -> Result<CaptureAnalysis, String> {
    use std::thread::sleep;
    use std::time::Duration;

    // N=2 gives math-dsp's median/MAD rejection zero breakdown power (both
    // takes always pass), so one corrupt take would poison the average —
    // bump to the smallest count that can reject a single bad take.
    let num_sweeps = match num_sweeps.max(1) {
        1 => 1_usize,
        2 => {
            log::warn!(
                "[record_and_analyze] num_sweeps=2 has no outlier rejection; using {MIN_REPEAT_SWEEPS}"
            );
            MIN_REPEAT_SWEEPS as usize
        }
        n => n as usize,
    };
    let mut takes = Vec::with_capacity(num_sweeps);
    let mut dropped_samples = 0_u64;
    let mut analysis_sample_rate = sample_rate;
    for take_index in 0..num_sweeps {
        if take_index > 0 {
            sleep(Duration::from_millis(INTER_TAKE_SETTLE_MS));
        }
        if num_sweeps > 1 {
            log::info!(
                "[record_and_analyze] Sweep take {}/{}",
                take_index + 1,
                num_sweeps
            );
        }
        let take = capture_sweep_take(
            temp_wav_path,
            reference_signal,
            sample_rate,
            output_channel,
            input_channel,
            output_device_name,
            input_device_name,
            cancel.as_ref(),
        )?;
        dropped_samples = dropped_samples.saturating_add(take.dropped_samples);
        // Guard against the device renegotiating its sample rate mid-set
        // (task-8 review A5): takes at mixed rates would be averaged and
        // deconvolved as if at one rate.
        if take_index == 0 {
            analysis_sample_rate = take.analysis_sample_rate;
        } else if take.analysis_sample_rate != analysis_sample_rate {
            return Err(format!(
                "[record_and_analyze] Input device renegotiated its sample rate mid-capture \
                 ({} Hz → {} Hz at take {}) — aborting the set; check the device connection",
                analysis_sample_rate,
                take.analysis_sample_rate,
                take_index + 1,
            ));
        }
        takes.push(take.recorded);
    }

    // Load microphone compensation if provided
    let compensation = if let Some(comp_path) = microphone_compensation_path {
        log::info!(
            "[record_and_analyze] Loading microphone compensation from {:?}",
            comp_path
        );
        Some(MicrophoneCompensation::from_file(Path::new(comp_path))?)
    } else {
        None
    };

    analyze_sweep_takes(
        recorded_wav_path,
        output_csv_path,
        &takes,
        reference_signal,
        sample_rate,
        analysis_sample_rate,
        sweep_range,
        compensation.as_ref(),
        dropped_samples,
        "record_and_analyze",
    )
}

/// Record and analyze capturing multiple input channels simultaneously.
///
/// Plays the signal on `output_channel` and records from all `input_channels` at once.
/// Returns one [`CaptureAnalysis`] per input channel (same order as `input_channels`):
/// the math-dsp analysis plus a per-mic quality report (lag-confidence gate,
/// clock-drift estimate/correction, capture diagnostics — see [`super::quality`]).
/// Each channel's WAV and CSV are written to `recorded_wav_paths` / `csv_paths`.
///
/// `num_sweeps` selects the repeat-sweep path (Task 8): each take is one
/// play/record cycle capturing ALL mics simultaneously; per mic the takes
/// are then drift-corrected, robustly averaged, and analyzed exactly like
/// [`record_and_analyze`]. `num_sweeps <= 1` keeps the legacy single-sweep
/// behavior; `num_sweeps == 2` is bumped to 3 (see [`record_and_analyze`]).
/// A cancelled or gated-out take aborts the whole set.
///
/// `mic_calibrations` must be the same length as `input_channels` (use `None` for uncalibrated).
///
/// `cancel` is a cooperative cancellation flag (see [`CancelFlag`]): when
/// set, the capture stops the streams and returns
/// `Err(CANCELLED_ERR)` ("cancelled") instead of analyzing.
#[cfg(not(target_os = "ios"))]
#[allow(clippy::too_many_arguments)]
pub fn record_and_analyze_multi(
    temp_wav_path: &Path,
    recorded_wav_paths: &[PathBuf],
    reference_signal: &[f32],
    sample_rate: u32,
    csv_paths: &[PathBuf],
    output_channel: u16,
    input_channels: &[u16],
    output_device_name: Option<&str>,
    input_device_name: Option<&str>,
    mic_calibrations: &[Option<String>],
    sweep_range: Option<(f32, f32)>,
    num_sweeps: u16,
    cancel: Option<CancelFlag>,
) -> Result<Vec<CaptureAnalysis>, String> {
    use std::thread::sleep;
    use std::time::Duration;

    assert_eq!(input_channels.len(), recorded_wav_paths.len());
    assert_eq!(input_channels.len(), csv_paths.len());
    assert_eq!(input_channels.len(), mic_calibrations.len());

    let num_mics = input_channels.len();
    // See `record_and_analyze`: 2 takes have no outlier-rejection power.
    let num_sweeps = match num_sweeps.max(1) {
        1 => 1_usize,
        2 => {
            log::warn!(
                "[record_and_analyze_multi] num_sweeps=2 has no outlier rejection; using {MIN_REPEAT_SWEEPS}"
            );
            MIN_REPEAT_SWEEPS as usize
        }
        n => n as usize,
    };

    // Take loop: one play/record cycle per take, all mics simultaneously.
    let mut takes_per_mic: Vec<Vec<Vec<f32>>> = vec![Vec::with_capacity(num_sweeps); num_mics];
    let mut dropped_samples = 0_u64;
    let mut analysis_sample_rate = sample_rate;
    for take_index in 0..num_sweeps {
        if take_index > 0 {
            sleep(Duration::from_millis(INTER_TAKE_SETTLE_MS));
        }
        if num_sweeps > 1 {
            log::info!(
                "[record_and_analyze_multi] Sweep take {}/{}",
                take_index + 1,
                num_sweeps
            );
        }
        let take = capture_sweep_take_multi(
            temp_wav_path,
            reference_signal,
            sample_rate,
            output_channel,
            input_channels,
            output_device_name,
            input_device_name,
            cancel.as_ref(),
        )?;
        dropped_samples = dropped_samples.saturating_add(take.dropped_samples);
        // See `record_and_analyze`: guard against the device renegotiating
        // its sample rate mid-set (task-8 review A5).
        if take_index == 0 {
            analysis_sample_rate = take.analysis_sample_rate;
        } else if take.analysis_sample_rate != analysis_sample_rate {
            return Err(format!(
                "[record_and_analyze_multi] Input device renegotiated its sample rate \
                 mid-capture ({} Hz → {} Hz at take {}) — aborting the set; check the \
                 device connection",
                analysis_sample_rate,
                take.analysis_sample_rate,
                take_index + 1,
            ));
        }
        for (mic_i, recorded) in take.recorded_per_mic.into_iter().enumerate() {
            takes_per_mic[mic_i].push(recorded);
        }
    }

    // --- Analyze each mic channel independently ---
    let mut results = Vec::with_capacity(num_mics);
    for mic_i in 0..num_mics {
        // Load mic compensation
        let compensation = if let Some(Some(comp_path)) = mic_calibrations.get(mic_i) {
            Some(MicrophoneCompensation::from_file(Path::new(comp_path))?)
        } else {
            None
        };

        let lag_tag = format!("record_and_analyze_multi mic {mic_i}");
        results.push(analyze_sweep_takes(
            &recorded_wav_paths[mic_i],
            &csv_paths[mic_i],
            &takes_per_mic[mic_i],
            reference_signal,
            sample_rate,
            analysis_sample_rate,
            sweep_range,
            compensation.as_ref(),
            dropped_samples,
            &lag_tag,
        )?);
    }

    Ok(results)
}
