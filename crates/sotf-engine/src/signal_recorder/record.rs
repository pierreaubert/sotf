#[cfg(not(target_os = "ios"))]
use super::misc::capture_capacity;
#[cfg(not(target_os = "ios"))]
use super::misc::drain_capture;
#[cfg(not(target_os = "ios"))]
use super::write::write_selected_channel_to_ring;
use super::write::write_wav_file;
#[cfg(not(target_os = "ios"))]
use crate::signal_analysis::{analyze_recording, write_analysis_csv};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Perform recording and analysis using AudioEngineManager for playback
/// and cpal for recording.
///
/// Plays back a signal to a specific output channel while simultaneously
/// recording from a specific input channel, then analyzes the result.
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
) -> Result<crate::signal_analysis::AnalysisResult, String> {
    use crate::AudioEngineManager;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::thread::sleep;
    use std::time::Duration;

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
        crate::devices::find_device(&host, dev_name, true)?
    } else {
        log::debug!("[record_and_analyze] Using default input device");
        host.default_input_device()
            .ok_or_else(|| "No default input device available".to_string())?
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
    let default_input_config = input_device
        .default_input_config()
        .map_err(|e| format!("Failed to get default input config: {}", e))?;

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
                    recorded_overruns_callback.fetch_add(1, Ordering::Relaxed);
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
        .map_err(|e| format!("Failed to build input stream: {}", e))?;

    // Start recording
    input_stream
        .play()
        .map_err(|e| format!("Failed to start input stream: {}", e))?;
    log::debug!("[record_and_analyze] Recording started");

    // Small delay to let recording buffer fill
    sleep(Duration::from_millis(100));

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
        crate::devices::find_device(&host, dev_name, false)?
    } else {
        log::debug!("[record_and_analyze] Using default output device");
        host.default_output_device()
            .ok_or_else(|| "No default output device available".to_string())?
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
        .map_err(|e| format!("Failed to get supported output configs: {}", e))?
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
        .map_err(|e| format!("Failed to start playback: {}", e))?;

    log::debug!("[record_and_analyze] Playback started, waiting for completion...");

    // Wait for playback to complete
    // Maximum timeout: expected duration + 3 seconds for buffer/latency
    let total_wait = Duration::from_secs_f64(expected_duration + 3.0);
    let check_interval = Duration::from_millis(50);
    let mut elapsed = Duration::ZERO;
    let mut last_sample_count = 0;
    let mut stable_count = 0;

    while elapsed < total_wait {
        sleep(check_interval);
        elapsed += check_interval;

        // Check recording progress
        let current_sample_count = recorded_count.load(Ordering::Relaxed);

        // Print progress every second
        if elapsed.as_millis() % 1000 < check_interval.as_millis() {
            let recorded_duration = current_sample_count as f64 / sample_rate as f64;
            log::info!(
                "[record_and_analyze] Recording progress: {:.2}s / {:.2}s ({} samples)",
                recorded_duration,
                expected_duration,
                current_sample_count
            );
        }

        // Check if recording has stopped growing (playback finished)
        if current_sample_count == last_sample_count && current_sample_count > 0 {
            stable_count += 1;
            // If sample count hasn't changed for 150ms, assume playback is done
            if stable_count >= 3 {
                // 3 * 50ms = 150ms
                log::debug!("[record_and_analyze] Recording stable, playback likely complete");
                break;
            }
        } else {
            stable_count = 0;
        }
        last_sample_count = current_sample_count;

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
    // on high-latency systems (e.g. large buffers, wireless, or complex routing)
    sleep(Duration::from_millis(1000));

    // Stop playback
    manager
        .stop()
        .map_err(|e| format!("Failed to stop playback: {}", e))?;

    // Stop recording
    std::mem::drop(input_stream);
    log::debug!("[record_and_analyze] Recording stopped");

    // Small delay to ensure all buffers are flushed
    sleep(Duration::from_millis(100));

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
        log::warn!("[record_and_analyze] WARNING: Recorded signal is essentially silence!");
    } else if rms_db < -60.0 {
        log::warn!(
            "[record_and_analyze] WARNING: Recorded signal is very quiet ({:.1} dBFS RMS)",
            rms_db
        );
    }

    // Write recorded samples to WAV file as MONO (1 channel)
    log::info!(
        "[record_and_analyze] Writing {} mono samples to WAV file...",
        recorded.len()
    );
    write_wav_file(recorded_wav_path, &recorded, analysis_sample_rate, 1)?;
    log::info!(
        "[record_and_analyze] Wrote {} samples as MONO (1 channel) to {:?}",
        recorded.len(),
        recorded_wav_path
    );

    // Verify the WAV file was written correctly
    use hound::WavReader;
    let reader = WavReader::open(recorded_wav_path)
        .map_err(|e| format!("Failed to verify WAV file: {}", e))?;
    let spec = reader.spec();
    log::info!(
        "[record_and_analyze] WAV file verification: {} channels, {} Hz, {} samples",
        spec.channels,
        spec.sample_rate,
        reader.duration()
    );
    if spec.channels != 1 {
        return Err(format!(
            "ERROR: WAV file has {} channels instead of 1 (mono)!",
            spec.channels
        ));
    }

    // Load microphone compensation if provided
    let compensation = if let Some(comp_path) = microphone_compensation_path {
        log::info!(
            "[record_and_analyze] Loading microphone compensation from {:?}",
            comp_path
        );
        use crate::signal_analysis::MicrophoneCompensation;
        Some(MicrophoneCompensation::from_file(Path::new(comp_path))?)
    } else {
        None
    };

    // Analyze the recording
    log::debug!("[record_and_analyze] Analyzing recording...");
    let analysis = analyze_recording(
        recorded_wav_path,
        reference_signal,
        analysis_sample_rate,
        sweep_range,
    )?;
    write_analysis_csv(&analysis, output_csv_path, compensation.as_ref())?;
    log::info!(
        "[record_and_analyze] Wrote analysis to {:?}",
        output_csv_path
    );

    Ok(analysis)
}

/// Record and analyze capturing multiple input channels simultaneously.
///
/// Plays the signal on `output_channel` and records from all `input_channels` at once.
/// Returns one `AnalysisResult` per input channel (same order as `input_channels`).
/// Each channel's WAV and CSV are written to `recorded_wav_paths` / `csv_paths`.
///
/// `mic_calibrations` must be the same length as `input_channels` (use `None` for uncalibrated).
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
) -> Result<Vec<crate::signal_analysis::AnalysisResult>, String> {
    use crate::AudioEngineManager;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::thread::sleep;
    use std::time::Duration;

    assert_eq!(input_channels.len(), recorded_wav_paths.len());
    assert_eq!(input_channels.len(), csv_paths.len());
    assert_eq!(input_channels.len(), mic_calibrations.len());

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
        crate::devices::find_device(&host, dev_name, true)?
    } else {
        host.default_input_device()
            .ok_or_else(|| "No default input device available".to_string())?
    };

    let default_input_config = input_device
        .default_input_config()
        .map_err(|e| format!("Failed to get default input config: {}", e))?;

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
                        recorded_overruns_callback.fetch_add(1, Ordering::Relaxed);
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
        .map_err(|e| format!("Failed to build input stream: {}", e))?;

    input_stream
        .play()
        .map_err(|e| format!("Failed to start input stream: {}", e))?;

    sleep(Duration::from_millis(100));

    // --- Start playback ---
    let mut manager = AudioEngineManager::new();
    manager
        .load_file(temp_wav_path)
        .map_err(|e| format!("Failed to load file: {}", e))?;

    let output_device = if let Some(dev_name) = output_device_name {
        crate::devices::find_device(&host, dev_name, false)?
    } else {
        host.default_output_device()
            .ok_or_else(|| "No default output device available".to_string())?
    };

    let hardware_channels = output_device
        .supported_output_configs()
        .map_err(|e| format!("Failed to get supported output configs: {}", e))?
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
        .map_err(|e| format!("Failed to start playback: {}", e))?;

    // --- Wait for playback to complete ---
    let total_wait = Duration::from_secs_f64(expected_duration + 3.0);
    let check_interval = Duration::from_millis(50);
    let mut elapsed = Duration::ZERO;
    let mut last_sample_count = 0usize;
    let mut stable_count = 0;

    while elapsed < total_wait {
        sleep(check_interval);
        elapsed += check_interval;

        let current_sample_count = recorded_counts[0].load(Ordering::Relaxed);

        if elapsed.as_millis() % 1000 < check_interval.as_millis() {
            let recorded_duration = current_sample_count as f64 / sample_rate as f64;
            log::info!(
                "[record_and_analyze_multi] Progress: {:.2}s / {:.2}s",
                recorded_duration,
                expected_duration,
            );
        }

        if current_sample_count == last_sample_count && current_sample_count > 0 {
            stable_count += 1;
            if stable_count >= 3 {
                break;
            }
        } else {
            stable_count = 0;
        }
        last_sample_count = current_sample_count;

        manager.try_recv_event();
        if manager.get_state() == crate::StreamingState::Idle {
            break;
        }
    }

    sleep(Duration::from_millis(1000));
    manager
        .stop()
        .map_err(|e| format!("Failed to stop playback: {}", e))?;
    std::mem::drop(input_stream);
    sleep(Duration::from_millis(100));

    // --- Analyze each mic channel independently ---
    let analysis_sample_rate = input_sample_rate;
    let mut results = Vec::with_capacity(num_mics);
    let dropped_samples = recorded_overruns.load(Ordering::Relaxed);
    if dropped_samples > 0 {
        log::warn!(
            "[record_and_analyze_multi] Dropped {} input samples because capture ring buffers filled",
            dropped_samples
        );
    }

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

        // Write WAV
        write_wav_file(
            &recorded_wav_paths[mic_i],
            &recorded,
            analysis_sample_rate,
            1,
        )?;

        // Verify WAV
        let reader = hound::WavReader::open(&recorded_wav_paths[mic_i])
            .map_err(|e| format!("Failed to verify WAV for mic {}: {}", mic_i, e))?;
        if reader.spec().channels != 1 {
            return Err(format!(
                "WAV for mic {} has {} channels instead of 1",
                mic_i,
                reader.spec().channels,
            ));
        }

        // Load mic compensation
        let compensation = if let Some(Some(comp_path)) = mic_calibrations.get(mic_i) {
            use crate::signal_analysis::MicrophoneCompensation;
            Some(MicrophoneCompensation::from_file(Path::new(comp_path))?)
        } else {
            None
        };

        // Analyze
        let analysis = analyze_recording(
            &recorded_wav_paths[mic_i],
            reference_signal,
            analysis_sample_rate,
            sweep_range,
        )?;
        write_analysis_csv(&analysis, &csv_paths[mic_i], compensation.as_ref())?;

        results.push(analysis);
    }

    Ok(results)
}
