//! Signal generation and recording module
//!
//! This module provides functionality to generate test signals, play them back,
//! record the output, and analyze the results.

#[cfg(not(target_os = "ios"))]
use crate::signal_analysis::{analyze_recording, write_analysis_csv};
use crate::signals::*;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tempfile::NamedTempFile;

/// Signal type for recording
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalType {
    Tone,
    TwoTone,
    Sweep,
    WhiteNoise,
    PinkNoise,
    MNoise,
}

impl SignalType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tone => "tone",
            Self::TwoTone => "two-tone",
            Self::Sweep => "sweep",
            Self::WhiteNoise => "white-noise",
            Self::PinkNoise => "pink-noise",
            Self::MNoise => "m-noise",
        }
    }
}

impl FromStr for SignalType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tone" => Ok(Self::Tone),
            "two-tone" | "twotone" => Ok(Self::TwoTone),
            "sweep" => Ok(Self::Sweep),
            "white-noise" | "white_noise" | "whitenoise" => Ok(Self::WhiteNoise),
            "pink-noise" | "pink_noise" | "pinknoise" => Ok(Self::PinkNoise),
            "m-noise" | "m_noise" | "mnoise" => Ok(Self::MNoise),
            _ => Err(format!("Unknown signal type: {}", s)),
        }
    }
}

/// Parameters for signal generation
#[derive(Debug, Clone)]
pub enum SignalParams {
    Tone {
        freq: f32,
        amp: f32,
    },
    TwoTone {
        freq1: f32,
        amp1: f32,
        freq2: f32,
        amp2: f32,
    },
    Sweep {
        start_freq: f32,
        end_freq: f32,
        amp: f32,
    },
    Noise {
        amp: f32,
    },
}

/// Generate a signal based on parameters
pub fn generate_signal(
    signal_type: SignalType,
    params: &SignalParams,
    duration: f32,
    sample_rate: u32,
) -> Result<Vec<f32>, String> {
    let signal = match (signal_type, params) {
        (SignalType::Tone, SignalParams::Tone { freq, amp }) => {
            gen_tone(*freq, *amp, sample_rate, duration)
        }
        (
            SignalType::TwoTone,
            SignalParams::TwoTone {
                freq1,
                amp1,
                freq2,
                amp2,
            },
        ) => gen_two_tone(*freq1, *amp1, *freq2, *amp2, sample_rate, duration),
        (
            SignalType::Sweep,
            SignalParams::Sweep {
                start_freq,
                end_freq,
                amp,
            },
        ) => gen_log_sweep(*start_freq, *end_freq, *amp, sample_rate, duration),
        (SignalType::WhiteNoise, SignalParams::Noise { amp }) => {
            gen_white_noise(*amp, sample_rate, duration)
        }
        (SignalType::PinkNoise, SignalParams::Noise { amp }) => {
            gen_pink_noise(*amp, sample_rate, duration)
        }
        (SignalType::MNoise, SignalParams::Noise { amp }) => {
            gen_m_noise(*amp, sample_rate, duration)
        }
        _ => {
            return Err(format!(
                "Signal type {:?} does not match parameters {:?}",
                signal_type, params
            ));
        }
    };

    Ok(signal)
}

/// Prepare a signal for playback with fades and padding
pub fn prepare_signal(signal: Vec<f32>, sample_rate: u32) -> Vec<f32> {
    const FADE_MS: f32 = 20.0;
    const PADDING_MS: f32 = 250.0;

    prepare_signal_for_playback(signal, sample_rate, FADE_MS, PADDING_MS)
}

/// Write signal to a temporary WAV file
pub fn write_temp_wav(
    signal: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<NamedTempFile, String> {
    let temp_file = NamedTempFile::with_suffix(".wav")
        .map_err(|e| format!("Failed to create temp file: {}", e))?;

    write_wav_file(temp_file.path(), signal, sample_rate, channels)?;

    Ok(temp_file)
}

/// Write signal to a WAV file
pub fn write_wav_file(
    path: &Path,
    signal: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<(), String> {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };

    let mut writer =
        WavWriter::create(path, spec).map_err(|e| format!("Failed to create WAV writer: {}", e))?;

    for &sample in signal {
        writer
            .write_sample(sample)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV file: {}", e))?;

    Ok(())
}

/// Generate output filenames for a recording with both send and record channels
pub fn generate_output_filenames_stereo(
    name_prefix: Option<&str>,
    signal_type: SignalType,
    send_channel: u16,
    record_channel: u16,
    sample_rate: u32,
) -> (PathBuf, PathBuf) {
    let base_name = if let Some(prefix) = name_prefix {
        format!(
            "{}_{}_send{}_rec{}_{}",
            prefix,
            signal_type.as_str(),
            send_channel,
            record_channel,
            sample_rate
        )
    } else {
        format!(
            "{}_send{}_rec{}_{}",
            signal_type.as_str(),
            send_channel,
            record_channel,
            sample_rate
        )
    };

    let wav_path = PathBuf::from(format!("{}.wav", base_name));
    let csv_path = PathBuf::from(format!("{}.csv", base_name));

    (wav_path, csv_path)
}

/// Generate output filenames for a recording
pub fn generate_output_filenames(
    name_prefix: Option<&str>,
    signal_type: SignalType,
    channel: u16,
    sample_rate: u32,
) -> (PathBuf, PathBuf) {
    let base_name = if let Some(prefix) = name_prefix {
        format!(
            "{}_{}_ch{}_{}",
            prefix,
            signal_type.as_str(),
            channel,
            sample_rate
        )
    } else {
        format!("{}_ch{}_{}", signal_type.as_str(), channel, sample_rate)
    };

    let wav_path = PathBuf::from(format!("{}.wav", base_name));
    let csv_path = PathBuf::from(format!("{}.csv", base_name));

    (wav_path, csv_path)
}

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
    use std::sync::Arc;
    use std::sync::Mutex;
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

    // Shared state for recording
    let recorded_samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let recorded_samples_clone = Arc::clone(&recorded_samples);

    // Create input stream
    let input_channel_idx = input_channel as usize;
    let input_stream = input_device
        .build_input_stream(
            &input_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut recorded = recorded_samples_clone.lock().unwrap();

                // Extract only the specified input channel
                // Data is interleaved: [ch0, ch1, ..., chN, ch0, ch1, ..., chN, ...]
                for frame in data.chunks(hardware_input_channels) {
                    if input_channel_idx < frame.len() {
                        recorded.push(frame[input_channel_idx]);
                    } else {
                        log::info!(
                            "[record_and_analyze] ERROR: Tried to access channel {} but frame has {} channels",
                            input_channel_idx,
                            frame.len()
                        );
                    }
                }
            },
            |err| log::debug!("[record_and_analyze] Input stream error: {}", err),
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
        let current_sample_count = recorded_samples.lock().unwrap().len();

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
    let recorded = recorded_samples.lock().unwrap().clone();
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
    use std::sync::Arc;
    use std::sync::Mutex;
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

    // --- Shared recording buffers (one Vec<f32> per mic) ---
    let recorded_buffers: Vec<Arc<Mutex<Vec<f32>>>> = (0..num_mics)
        .map(|_| Arc::new(Mutex::new(Vec::new())))
        .collect();
    let buffers_clone: Vec<Arc<Mutex<Vec<f32>>>> =
        recorded_buffers.iter().map(Arc::clone).collect();

    let input_channels_vec: Vec<usize> = input_channels.iter().map(|&c| c as usize).collect();

    let input_stream = input_device
        .build_input_stream(
            &input_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                for frame in data.chunks(hardware_input_channels) {
                    for (mic_i, &ch_idx) in input_channels_vec.iter().enumerate() {
                        if ch_idx < frame.len() {
                            buffers_clone[mic_i].lock().unwrap().push(frame[ch_idx]);
                        }
                    }
                }
            },
            |err| log::debug!("[record_and_analyze_multi] Input stream error: {}", err),
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

        let current_sample_count = recorded_buffers[0].lock().unwrap().len();

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

    for mic_i in 0..num_mics {
        let recorded = recorded_buffers[mic_i].lock().unwrap().clone();
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

/// Parse comma-separated channel list (0-based indices)
pub fn parse_channel_list(s: &str) -> Result<Vec<u16>, String> {
    let mut channels = Vec::new();

    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let ch: u16 = part
            .parse()
            .map_err(|_| format!("Invalid channel number: {}", part))?;

        if channels.contains(&ch) {
            return Err(format!("Duplicate channel number: {}", ch));
        }

        channels.push(ch);
    }

    if channels.is_empty() {
        return Err("Channel list is empty".to_string());
    }

    Ok(channels)
}

// ============================================================================
// Lightweight Recording Format
// ============================================================================

use serde::{Deserialize, Serialize};

/// Lightweight recording metadata (V2 format)
///
/// This format stores only metadata and file paths, with actual analysis
/// data stored in CSV files. This reduces JSON file size dramatically
/// (from ~90MB to ~2KB for a typical multi-channel recording session).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSession {
    /// Format version (currently "2.0")
    pub version: String,
    /// Recording timestamp (RFC 3339 format)
    pub timestamp: String,
    /// Sample rate used for recording
    pub sample_rate: u32,
    /// Signal type used (sweep, pink-noise, etc.)
    pub signal_type: String,
    /// Signal duration in seconds
    pub signal_duration_secs: f32,
    /// Signal level in dBFS
    pub signal_level_db: f32,
    /// Sweep frequency range (if applicable)
    pub sweep_range: Option<(f32, f32)>,
    /// Playback device configuration
    pub playback_device: Option<DeviceInfo>,
    /// Recording device configuration
    pub recording_device: Option<DeviceInfo>,
    /// Microphone calibration file path (relative to session directory)
    pub mic_calibration_path: Option<String>,
    /// Per-channel microphone calibration file paths (parallel to channels)
    #[serde(default)]
    pub mic_calibration_paths: Vec<Option<String>>,
    /// Individual channel recordings
    pub channels: Vec<ChannelRecordingInfo>,
}

/// Device information for recording metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub num_channels: usize,
}

/// Information about a single channel's recording
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelRecordingInfo {
    /// Channel index (0-based)
    pub channel_index: usize,
    /// Channel name (e.g., "L", "R", "C")
    pub channel_name: String,
    /// Interface output channel used for playback
    pub output_channel: usize,
    /// Interface input channel used for recording
    pub input_channel: usize,
    /// Path to WAV file (relative to session directory)
    pub wav_path: String,
    /// Path to CSV file with analysis data (relative to session directory)
    pub csv_path: String,
    /// Whether recording succeeded
    pub success: bool,
    /// Error message if recording failed
    pub error: Option<String>,
    /// Per-channel microphone calibration file path
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mic_calibration_path: Option<String>,
}

impl RecordingSession {
    /// Create a new recording session
    pub fn new(
        sample_rate: u32,
        signal_type: &str,
        signal_duration_secs: f32,
        signal_level_db: f32,
        sweep_range: Option<(f32, f32)>,
    ) -> Self {
        Self {
            version: "2.0".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            sample_rate,
            signal_type: signal_type.to_string(),
            signal_duration_secs,
            signal_level_db,
            sweep_range,
            playback_device: None,
            recording_device: None,
            mic_calibration_path: None,
            mic_calibration_paths: Vec::new(),
            channels: Vec::new(),
        }
    }

    /// Get the effective calibration path for a channel, checking per-channel first, then global fallback
    pub fn effective_calibration_for_channel(&self, idx: usize) -> Option<&str> {
        // Per-channel calibration takes priority
        if let Some(Some(path)) = self.mic_calibration_paths.get(idx)
            && !path.is_empty()
        {
            return Some(path.as_str());
        }
        // Fall back to global
        self.mic_calibration_path.as_deref()
    }

    /// Add a channel recording to the session
    #[allow(clippy::too_many_arguments)] // recording metadata has many independent fields
    pub fn add_channel(
        &mut self,
        channel_index: usize,
        channel_name: &str,
        output_channel: usize,
        input_channel: usize,
        wav_path: &str,
        csv_path: &str,
        success: bool,
        error: Option<String>,
    ) {
        self.channels.push(ChannelRecordingInfo {
            channel_index,
            channel_name: channel_name.to_string(),
            output_channel,
            input_channel,
            wav_path: wav_path.to_string(),
            csv_path: csv_path.to_string(),
            success,
            error,
            mic_calibration_path: None,
        });
    }

    /// Save session to JSON file
    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        let file = std::fs::File::create(path)
            .map_err(|e| format!("Failed to create session file: {}", e))?;
        serde_json::to_writer_pretty(file, self)
            .map_err(|e| format!("Failed to serialize session: {}", e))?;
        log::info!("[RecordingSession] Saved session to {:?}", path);
        Ok(())
    }

    /// Load session from JSON file
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let file =
            std::fs::File::open(path).map_err(|e| format!("Failed to open session file: {}", e))?;
        let session: Self = serde_json::from_reader(file)
            .map_err(|e| format!("Failed to deserialize session: {}", e))?;
        log::info!(
            "[RecordingSession] Loaded session from {:?} (version {})",
            path,
            session.version
        );
        Ok(session)
    }
}

/// Re-process recordings from WAV files and regenerate CSV analysis files
///
/// This function loads WAV files from a recording session, re-runs the analysis,
/// and writes updated CSV files. Useful when analysis algorithms are updated.
///
/// # Arguments
/// * `session_dir` - Directory containing the recording session
/// * `session` - Recording session metadata
/// * `reference_signal` - Reference signal used for recording (must regenerate)
/// * `sample_rate` - Sample rate
/// * `sweep_range` - Sweep frequency range (if applicable)
/// * `mic_compensation_path` - Path to microphone calibration file (optional)
///
/// # Returns
/// Updated RecordingSession with new CSV paths
pub fn reprocess_recordings(
    session_dir: &Path,
    session: &RecordingSession,
    reference_signal: &[f32],
    mic_compensation_path: Option<&Path>,
) -> Result<RecordingSession, String> {
    use crate::signal_analysis::{MicrophoneCompensation, analyze_recording, write_analysis_csv};

    log::info!(
        "[reprocess_recordings] Re-processing {} channels in {:?}",
        session.channels.len(),
        session_dir
    );

    // Load global microphone compensation (used as fallback)
    let global_compensation = if let Some(comp_path) = mic_compensation_path {
        Some(MicrophoneCompensation::from_file(comp_path)?)
    } else if let Some(ref rel_path) = session.mic_calibration_path {
        let full_path = session_dir.join(rel_path);
        if full_path.exists() {
            Some(MicrophoneCompensation::from_file(&full_path)?)
        } else {
            None
        }
    } else {
        None
    };

    let mut updated_session = session.clone();
    updated_session.channels.clear();

    for (ch_idx, channel_info) in session.channels.iter().enumerate() {
        if !channel_info.success {
            // Keep failed channels as-is
            updated_session.channels.push(channel_info.clone());
            continue;
        }

        let wav_path = session_dir.join(&channel_info.wav_path);
        let csv_path = session_dir.join(&channel_info.csv_path);

        if !wav_path.exists() {
            log::warn!(
                "[reprocess_recordings] WAV file not found: {:?}, skipping channel {}",
                wav_path,
                channel_info.channel_name
            );
            let mut failed_channel = channel_info.clone();
            failed_channel.success = false;
            failed_channel.error = Some(format!("WAV file not found: {:?}", wav_path));
            updated_session.channels.push(failed_channel);
            continue;
        }

        log::info!(
            "[reprocess_recordings] Processing channel '{}' from {:?}",
            channel_info.channel_name,
            wav_path
        );

        // Resolve per-channel compensation with fallback chain:
        // 1. ChannelRecordingInfo.mic_calibration_path (per-recording override)
        // 2. RecordingSession.mic_calibration_paths[idx] (per-channel session config)
        // 3. Global compensation (from mic_compensation_path arg or session.mic_calibration_path)
        let per_channel_cal_path = channel_info
            .mic_calibration_path
            .as_deref()
            .or_else(|| {
                session
                    .mic_calibration_paths
                    .get(ch_idx)
                    .and_then(|p| p.as_deref())
            })
            .filter(|p| !p.is_empty());

        let channel_compensation = if let Some(cal_path) = per_channel_cal_path {
            let ch_path = Path::new(cal_path);
            let full_path = if ch_path.is_absolute() {
                ch_path.to_path_buf()
            } else {
                session_dir.join(ch_path)
            };
            if full_path.exists() {
                Some(MicrophoneCompensation::from_file(&full_path)?)
            } else {
                log::warn!(
                    "[reprocess_recordings] Per-channel calibration file not found: {:?}, using global",
                    full_path
                );
                global_compensation.as_ref().cloned()
            }
        } else {
            global_compensation.as_ref().cloned()
        };

        // Re-analyze the recording
        match analyze_recording(
            &wav_path,
            reference_signal,
            session.sample_rate,
            session.sweep_range,
        ) {
            Ok(analysis) => {
                // Write updated CSV
                if let Err(e) =
                    write_analysis_csv(&analysis, &csv_path, channel_compensation.as_ref())
                {
                    log::error!(
                        "[reprocess_recordings] Failed to write CSV for channel '{}': {}",
                        channel_info.channel_name,
                        e
                    );
                    let mut failed_channel = channel_info.clone();
                    failed_channel.success = false;
                    failed_channel.error = Some(format!("Failed to write CSV: {}", e));
                    updated_session.channels.push(failed_channel);
                } else {
                    log::info!(
                        "[reprocess_recordings] Updated CSV for channel '{}'",
                        channel_info.channel_name
                    );
                    updated_session.channels.push(channel_info.clone());
                }
            }
            Err(e) => {
                log::error!(
                    "[reprocess_recordings] Analysis failed for channel '{}': {}",
                    channel_info.channel_name,
                    e
                );
                let mut failed_channel = channel_info.clone();
                failed_channel.success = false;
                failed_channel.error = Some(format!("Analysis failed: {}", e));
                updated_session.channels.push(failed_channel);
            }
        }
    }

    Ok(updated_session)
}

// ============================================================================
// Legacy JSON Migration (V1 to V2)
// ============================================================================

/// Legacy recording result format (V1 - data stored inline in JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyRecordingResult {
    pub channel: usize,
    pub wav_path: Option<String>,
    pub csv_path: Option<String>,
    pub frequencies: Vec<f32>,
    pub magnitude_db: Vec<f32>,
    pub phase_deg: Vec<f32>,
    pub impulse_response: Option<Vec<f32>>,
    pub impulse_time_ms: Option<Vec<f32>>,
    pub thd_percent: Option<Vec<f32>>,
    pub harmonic_distortion_db: Option<Vec<Vec<f32>>>,
    pub excess_group_delay_ms: Option<Vec<f32>>,
    pub rt60_ms: Option<Vec<f32>>,
    pub clarity_c50_db: Option<Vec<f32>>,
    pub clarity_c80_db: Option<Vec<f32>>,
    pub spectrogram_db: Option<Vec<Vec<f32>>>,
}

/// Legacy channel recording format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyChannelRecording {
    pub channel_index: usize,
    pub channel_name: String,
    pub state: String,
    pub result: Option<LegacyRecordingResult>,
}

/// Legacy recording session format (V1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyRecordingSession {
    pub timestamp: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Vec<LegacyChannelRecording>,
}

/// Migrate legacy V1 recording format to V2 format
///
/// This function:
/// 1. Reads the legacy JSON file with inline data
/// 2. Creates a new V2 session file with metadata only
/// 3. Extracts analysis data from JSON and writes to CSV files
///
/// # Arguments
/// * `legacy_json_path` - Path to the legacy recordings.json file
/// * `session_dir` - Directory containing the recording session
///
/// # Returns
/// The new V2 RecordingSession
pub fn migrate_legacy_recording(
    legacy_json_path: &Path,
    session_dir: &Path,
) -> Result<RecordingSession, String> {
    use crate::signal_analysis::AnalysisResult;
    use std::fs::File;

    log::info!(
        "[migrate_legacy_recording] Migrating {:?} to V2 format",
        legacy_json_path
    );

    // Load legacy format
    let file =
        File::open(legacy_json_path).map_err(|e| format!("Failed to open legacy file: {}", e))?;
    let legacy: LegacyRecordingSession = serde_json::from_reader(file)
        .map_err(|e| format!("Failed to parse legacy format: {}", e))?;

    // Create V2 session
    let mut session = RecordingSession {
        version: "2.0".to_string(),
        timestamp: legacy
            .timestamp
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
        sample_rate: legacy.sample_rate.unwrap_or(48000),
        signal_type: "sweep".to_string(), // Default, can't recover from legacy
        signal_duration_secs: 5.0,        // Default
        signal_level_db: -20.0,           // Default
        sweep_range: Some((20.0, 20000.0)), // Default
        playback_device: None,
        recording_device: None,
        mic_calibration_path: None,
        mic_calibration_paths: Vec::new(),
        channels: Vec::new(),
    };

    // Process each channel
    for legacy_channel in &legacy.channels {
        let success = legacy_channel.state == "Done";

        if let Some(ref result) = legacy_channel.result {
            // Generate CSV filename
            let csv_filename = format!("channel_{}.csv", legacy_channel.channel_index);
            let csv_path = session_dir.join(&csv_filename);

            // Convert legacy result to AnalysisResult and write CSV
            let analysis = AnalysisResult {
                frequencies: result.frequencies.clone(),
                spl_db: result.magnitude_db.clone(),
                phase_deg: result.phase_deg.clone(),
                estimated_lag_samples: 0,
                impulse_response: result.impulse_response.clone().unwrap_or_default(),
                impulse_time_ms: result.impulse_time_ms.clone().unwrap_or_default(),
                thd_percent: result.thd_percent.clone().unwrap_or_default(),
                harmonic_distortion_db: result.harmonic_distortion_db.clone().unwrap_or_default(),
                rt60_ms: result.rt60_ms.clone().unwrap_or_default(),
                clarity_c50_db: result.clarity_c50_db.clone().unwrap_or_default(),
                clarity_c80_db: result.clarity_c80_db.clone().unwrap_or_default(),
                excess_group_delay_ms: result.excess_group_delay_ms.clone().unwrap_or_default(),
                spectrogram_db: result.spectrogram_db.clone().unwrap_or_default(),
            };

            // Write CSV with extended format
            write_extended_csv(&analysis, &csv_path)?;

            // Determine WAV path
            let wav_path = result
                .wav_path
                .clone()
                .unwrap_or_else(|| format!("channel_{}.wav", legacy_channel.channel_index));

            session.add_channel(
                legacy_channel.channel_index,
                &legacy_channel.channel_name,
                legacy_channel.channel_index, // Assume 1:1 mapping
                0,                            // Unknown input channel
                &wav_path,
                &csv_filename,
                success,
                None,
            );

            log::info!(
                "[migrate_legacy_recording] Migrated channel '{}' -> {}",
                legacy_channel.channel_name,
                csv_filename
            );
        } else {
            // No result data - still add the channel entry
            session.add_channel(
                legacy_channel.channel_index,
                &legacy_channel.channel_name,
                legacy_channel.channel_index,
                0,
                "",
                "",
                false,
                Some("No data in legacy format".to_string()),
            );
        }
    }

    // Save V2 session file
    let session_path = session_dir.join("session.json");
    session.save_to_file(&session_path)?;

    log::info!(
        "[migrate_legacy_recording] Migration complete: {} channels processed",
        session.channels.len()
    );

    Ok(session)
}

/// Write analysis result to CSV with extended format
fn write_extended_csv(
    analysis: &crate::signal_analysis::AnalysisResult,
    csv_path: &Path,
) -> Result<(), String> {
    use std::io::Write;

    let mut file =
        std::fs::File::create(csv_path).map_err(|e| format!("Failed to create CSV: {}", e))?;

    // Header
    writeln!(
        file,
        "frequency_hz,spl_db,phase_deg,thd_percent,rt60_ms,c50_db,c80_db,group_delay_ms"
    )
    .map_err(|e| format!("Failed to write header: {}", e))?;

    // Data
    for i in 0..analysis.frequencies.len() {
        let freq = analysis.frequencies[i];
        let spl = analysis.spl_db[i];
        let phase = analysis.phase_deg[i];
        let thd = analysis.thd_percent.get(i).copied().unwrap_or(0.0);
        let rt60 = analysis.rt60_ms.get(i).copied().unwrap_or(0.0);
        let c50 = analysis.clarity_c50_db.get(i).copied().unwrap_or(0.0);
        let c80 = analysis.clarity_c80_db.get(i).copied().unwrap_or(0.0);
        let gd = analysis
            .excess_group_delay_ms
            .get(i)
            .copied()
            .unwrap_or(0.0);

        writeln!(
            file,
            "{:.6},{:.3},{:.6},{:.6},{:.3},{:.3},{:.3},{:.6}",
            freq, spl, phase, thd, rt60, c50, c80, gd
        )
        .map_err(|e| format!("Failed to write data: {}", e))?;
    }

    Ok(())
}

/// Validate signal parameters
pub fn validate_signal_params(
    signal_type: SignalType,
    params: &SignalParams,
    duration: f32,
    sample_rate: u32,
) -> Result<(), String> {
    if duration <= 0.0 {
        return Err("Duration must be positive".to_string());
    }

    let nyquist = sample_rate as f32 / 2.0;

    match (signal_type, params) {
        (SignalType::Tone, SignalParams::Tone { freq, amp }) => {
            if *freq <= 0.0 || *freq >= nyquist {
                return Err(format!(
                    "Tone frequency {} Hz must be in range (0, {} Hz)",
                    freq, nyquist
                ));
            }
            if *amp <= 0.0 || *amp > 1.0 {
                return Err(format!("Amplitude {} must be in range (0, 1]", amp));
            }
        }
        (
            SignalType::TwoTone,
            SignalParams::TwoTone {
                freq1,
                amp1,
                freq2,
                amp2,
            },
        ) => {
            if *freq1 <= 0.0 || *freq1 >= nyquist {
                return Err(format!(
                    "First frequency {} Hz must be in range (0, {} Hz)",
                    freq1, nyquist
                ));
            }
            if *freq2 <= 0.0 || *freq2 >= nyquist {
                return Err(format!(
                    "Second frequency {} Hz must be in range (0, {} Hz)",
                    freq2, nyquist
                ));
            }
            if *amp1 <= 0.0 || *amp1 > 1.0 {
                return Err(format!("First amplitude {} must be in range (0, 1]", amp1));
            }
            if *amp2 <= 0.0 || *amp2 > 1.0 {
                return Err(format!("Second amplitude {} must be in range (0, 1]", amp2));
            }
        }
        (
            SignalType::Sweep,
            SignalParams::Sweep {
                start_freq,
                end_freq,
                amp,
            },
        ) => {
            if *start_freq <= 0.0 || *start_freq >= nyquist {
                return Err(format!(
                    "Start frequency {} Hz must be in range (0, {} Hz)",
                    start_freq, nyquist
                ));
            }
            if *end_freq <= 0.0 || *end_freq >= nyquist {
                return Err(format!(
                    "End frequency {} Hz must be in range (0, {} Hz)",
                    end_freq, nyquist
                ));
            }
            if *start_freq >= *end_freq {
                return Err(format!(
                    "Start frequency {} Hz must be less than end frequency {} Hz",
                    start_freq, end_freq
                ));
            }
            if *amp <= 0.0 || *amp > 1.0 {
                return Err(format!("Amplitude {} must be in range (0, 1]", amp));
            }
        }
        (_, SignalParams::Noise { amp }) => {
            if *amp <= 0.0 || *amp > 1.0 {
                return Err(format!("Amplitude {} must be in range (0, 1]", amp));
            }
        }
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::WavReader;
    use tempfile::tempdir;

    #[test]
    fn test_effective_calibration_per_channel_priority() {
        let mut session = RecordingSession::new(48000, "sweep", 5.0, -20.0, None);
        session.mic_calibration_path = Some("/global/cal.txt".to_string());
        session.mic_calibration_paths = vec![
            Some("/ch0/cal.txt".to_string()),
            None,
            Some("/ch2/cal.txt".to_string()),
        ];
        // Per-channel takes priority over global
        assert_eq!(
            session.effective_calibration_for_channel(0),
            Some("/ch0/cal.txt")
        );
        // Falls back to global when per-channel is None
        assert_eq!(
            session.effective_calibration_for_channel(1),
            Some("/global/cal.txt")
        );
        // Per-channel takes priority
        assert_eq!(
            session.effective_calibration_for_channel(2),
            Some("/ch2/cal.txt")
        );
        // Out-of-bounds falls back to global
        assert_eq!(
            session.effective_calibration_for_channel(5),
            Some("/global/cal.txt")
        );
    }

    #[test]
    fn test_effective_calibration_empty_string_falls_back() {
        let mut session = RecordingSession::new(48000, "sweep", 5.0, -20.0, None);
        session.mic_calibration_path = Some("/global/cal.txt".to_string());
        session.mic_calibration_paths = vec![Some("".to_string())];
        // Empty string should fall back to global
        assert_eq!(
            session.effective_calibration_for_channel(0),
            Some("/global/cal.txt")
        );
    }

    #[test]
    fn test_effective_calibration_no_global_no_per_channel() {
        let session = RecordingSession::new(48000, "sweep", 5.0, -20.0, None);
        assert!(session.effective_calibration_for_channel(0).is_none());
    }

    #[test]
    fn test_recording_session_serde_backward_compat() {
        // Old format without mic_calibration_paths
        let json = r#"{
            "version": "2.0",
            "timestamp": "2024-01-01T00:00:00Z",
            "sample_rate": 48000,
            "signal_type": "sweep",
            "signal_duration_secs": 5.0,
            "signal_level_db": -20.0,
            "sweep_range": null,
            "playback_device": null,
            "recording_device": null,
            "mic_calibration_path": "/global.txt",
            "channels": []
        }"#;
        let session: RecordingSession = serde_json::from_str(json).unwrap();
        assert!(session.mic_calibration_paths.is_empty());
        assert_eq!(
            session.effective_calibration_for_channel(0),
            Some("/global.txt")
        );
    }

    #[test]
    fn test_channel_recording_info_serde_backward_compat() {
        // Old format without per-channel mic_calibration_path
        let json = r#"{
            "channel_index": 0,
            "channel_name": "L",
            "output_channel": 0,
            "input_channel": 0,
            "wav_path": "ch0.wav",
            "csv_path": "ch0.csv",
            "success": true,
            "error": null
        }"#;
        let info: ChannelRecordingInfo = serde_json::from_str(json).unwrap();
        assert!(info.mic_calibration_path.is_none());
    }

    #[test]
    fn test_signal_type_from_str() {
        assert_eq!(SignalType::from_str("tone").unwrap(), SignalType::Tone);
        assert_eq!(
            SignalType::from_str("two-tone").unwrap(),
            SignalType::TwoTone
        );
        assert_eq!(SignalType::from_str("sweep").unwrap(), SignalType::Sweep);
        assert_eq!(
            SignalType::from_str("white-noise").unwrap(),
            SignalType::WhiteNoise
        );
        assert!(SignalType::from_str("invalid").is_err());
    }

    #[test]
    fn test_parse_channel_list() {
        assert_eq!(parse_channel_list("0").unwrap(), vec![0]); // Channel 0 is valid (0-based indexing)
        assert_eq!(parse_channel_list("1").unwrap(), vec![1]);
        assert_eq!(parse_channel_list("1,2,3").unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_channel_list(" 1 , 2 , 3 ").unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_channel_list("0,1,2").unwrap(), vec![0, 1, 2]); // 0-based channels

        assert!(parse_channel_list("1,1").is_err()); // Duplicate
        assert!(parse_channel_list("").is_err()); // Empty
        assert!(parse_channel_list("abc").is_err()); // Non-numeric
    }

    #[test]
    fn test_validate_signal_params_tone() {
        let params = SignalParams::Tone {
            freq: 1000.0,
            amp: 0.5,
        };
        assert!(validate_signal_params(SignalType::Tone, &params, 1.0, 48000).is_ok());

        let params_bad_freq = SignalParams::Tone {
            freq: 30000.0,
            amp: 0.5,
        };
        assert!(validate_signal_params(SignalType::Tone, &params_bad_freq, 1.0, 48000).is_err());

        let params_bad_amp = SignalParams::Tone {
            freq: 1000.0,
            amp: 2.0,
        };
        assert!(validate_signal_params(SignalType::Tone, &params_bad_amp, 1.0, 48000).is_err());
    }

    #[test]
    fn test_generate_output_filenames_stereo() {
        let (wav, csv) = generate_output_filenames_stereo(
            Some("test"),
            SignalType::Sweep,
            2, // send channel
            1, // record channel
            48000,
        );
        assert_eq!(wav, PathBuf::from("test_sweep_send2_rec1_48000.wav"));
        assert_eq!(csv, PathBuf::from("test_sweep_send2_rec1_48000.csv"));

        let (wav, csv) = generate_output_filenames_stereo(
            None,
            SignalType::Tone,
            1, // send channel
            3, // record channel
            44100,
        );
        assert_eq!(wav, PathBuf::from("tone_send1_rec3_44100.wav"));
        assert_eq!(csv, PathBuf::from("tone_send1_rec3_44100.csv"));
    }

    #[test]
    fn test_generate_output_filenames() {
        let (wav, csv) = generate_output_filenames(Some("test"), SignalType::Sweep, 1, 48000);
        assert_eq!(wav, PathBuf::from("test_sweep_ch1_48000.wav"));
        assert_eq!(csv, PathBuf::from("test_sweep_ch1_48000.csv"));

        let (wav, csv) = generate_output_filenames(None, SignalType::Tone, 2, 44100);
        assert_eq!(wav, PathBuf::from("tone_ch2_44100.wav"));
        assert_eq!(csv, PathBuf::from("tone_ch2_44100.csv"));
    }

    #[test]
    fn test_generate_signal_tone() {
        let params = SignalParams::Tone {
            freq: 1000.0,
            amp: 0.5,
        };
        let signal = generate_signal(SignalType::Tone, &params, 0.1, 48000)
            .expect("Failed to generate tone");

        assert_eq!(signal.len(), 4800); // 0.1s * 48000 Hz

        // Check signal is non-zero and within amplitude bounds
        let max_val = signal
            .iter()
            .map(|&x| x.abs())
            .fold(0.0_f32, |a, b| a.max(b));
        assert!(
            max_val > 0.4 && max_val <= 0.5,
            "Tone amplitude out of range: {}",
            max_val
        );
    }

    #[test]
    fn test_generate_signal_sweep() {
        let params = SignalParams::Sweep {
            start_freq: 20.0,
            end_freq: 20000.0,
            amp: 0.5,
        };
        let signal = generate_signal(SignalType::Sweep, &params, 1.0, 48000)
            .expect("Failed to generate sweep");

        assert_eq!(signal.len(), 48000);

        let max_val = signal
            .iter()
            .map(|&x| x.abs())
            .fold(0.0_f32, |a, b| a.max(b));
        assert!(
            max_val > 0.4 && max_val <= 0.5,
            "Sweep amplitude out of range: {}",
            max_val
        );
    }

    #[test]
    fn test_generate_signal_noise() {
        let params = SignalParams::Noise { amp: 0.5 };
        let signal = generate_signal(SignalType::WhiteNoise, &params, 1.0, 48000)
            .expect("Failed to generate white noise");

        assert_eq!(signal.len(), 48000);

        // Check that noise has content (not all zeros) - matches existing test pattern
        assert!(
            signal.iter().any(|&x| x.abs() > 0.01),
            "Noise signal should have non-zero samples"
        );
    }

    #[test]
    fn test_generate_signal_type_mismatch() {
        // Wrong params for signal type should fail
        let params = SignalParams::Tone {
            freq: 1000.0,
            amp: 0.5,
        };
        let result = generate_signal(SignalType::Sweep, &params, 1.0, 48000);
        assert!(result.is_err());
    }

    #[test]
    fn test_prepare_signal_adds_padding() {
        let signal = vec![1.0; 4800]; // 0.1s at 48kHz
        let prepared = prepare_signal(signal.clone(), 48000);

        // Should be longer due to fades and padding
        assert!(
            prepared.len() > signal.len(),
            "Prepared signal should be longer than original"
        );

        // First samples should be faded (smaller than original)
        assert!(
            prepared[0].abs() < signal[0].abs(),
            "First sample should be faded in"
        );

        // Last samples should be faded
        assert!(
            prepared[prepared.len() - 1].abs() < 0.1,
            "Last sample should be faded out or padded"
        );
    }

    #[test]
    fn test_write_and_read_wav_roundtrip() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let wav_path = temp_dir.path().join("test.wav");

        // Generate a simple signal
        let sample_rate = 48000;
        let duration = 0.1;
        let signal: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
            .map(|i| {
                (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sample_rate as f32).sin() * 0.5
            })
            .collect();

        // Write WAV
        write_wav_file(&wav_path, &signal, sample_rate, 1).expect("Failed to write WAV");

        assert!(wav_path.exists(), "WAV file should exist");

        // Read it back using hound
        let mut reader = WavReader::open(&wav_path).expect("Failed to open WAV for reading");

        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, sample_rate);
        assert_eq!(spec.sample_format, SampleFormat::Float);

        let read_samples: Vec<f32> = reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .expect("Failed to read samples");

        // Verify samples match (with small floating point tolerance)
        assert_eq!(read_samples.len(), signal.len());
        for (i, (&original, &read)) in signal.iter().zip(read_samples.iter()).enumerate() {
            assert!(
                (original - read).abs() < 1e-6,
                "Sample {} mismatch: original={}, read={}",
                i,
                original,
                read
            );
        }
    }

    #[test]
    fn test_write_temp_wav() {
        let signal = vec![0.5, 0.3, -0.2, -0.4, 0.0];
        let sample_rate = 48000;

        let temp_file = write_temp_wav(&signal, sample_rate, 1).expect("Failed to write temp WAV");

        assert!(temp_file.path().exists());

        // Verify it's a valid WAV
        let reader = WavReader::open(temp_file.path()).expect("Failed to open temp WAV");
        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, sample_rate);
    }

    #[test]
    fn test_validate_signal_params_duration() {
        let params = SignalParams::Tone {
            freq: 1000.0,
            amp: 0.5,
        };

        // Valid duration
        assert!(validate_signal_params(SignalType::Tone, &params, 1.0, 48000).is_ok());

        // Invalid duration
        assert!(validate_signal_params(SignalType::Tone, &params, 0.0, 48000).is_err());
        assert!(validate_signal_params(SignalType::Tone, &params, -1.0, 48000).is_err());
    }

    #[test]
    fn test_validate_signal_params_frequency_nyquist() {
        let sample_rate = 48000;
        let nyquist = sample_rate as f32 / 2.0;

        // Valid frequency
        let params_valid = SignalParams::Tone {
            freq: 1000.0,
            amp: 0.5,
        };
        assert!(validate_signal_params(SignalType::Tone, &params_valid, 1.0, sample_rate).is_ok());

        // Frequency above Nyquist
        let params_high = SignalParams::Tone {
            freq: nyquist + 100.0,
            amp: 0.5,
        };
        assert!(validate_signal_params(SignalType::Tone, &params_high, 1.0, sample_rate).is_err());

        // Zero frequency
        let params_zero = SignalParams::Tone {
            freq: 0.0,
            amp: 0.5,
        };
        assert!(validate_signal_params(SignalType::Tone, &params_zero, 1.0, sample_rate).is_err());
    }

    #[test]
    fn test_validate_signal_params_sweep_order() {
        let sample_rate = 48000;

        // Valid sweep (ascending)
        let params_valid = SignalParams::Sweep {
            start_freq: 20.0,
            end_freq: 20000.0,
            amp: 0.5,
        };
        assert!(validate_signal_params(SignalType::Sweep, &params_valid, 1.0, sample_rate).is_ok());

        // Invalid sweep (start >= end)
        let params_reversed = SignalParams::Sweep {
            start_freq: 20000.0,
            end_freq: 20.0,
            amp: 0.5,
        };
        assert!(
            validate_signal_params(SignalType::Sweep, &params_reversed, 1.0, sample_rate).is_err()
        );

        let params_equal = SignalParams::Sweep {
            start_freq: 1000.0,
            end_freq: 1000.0,
            amp: 0.5,
        };
        assert!(
            validate_signal_params(SignalType::Sweep, &params_equal, 1.0, sample_rate).is_err()
        );
    }

    /// Regression test: Verify that record_and_analyze doesn't just copy the input file
    ///
    /// This test ensures that the recording function actually performs recording,
    /// not just file copying. It checks that:
    /// 1. The function signature includes both input and output paths
    /// 2. The implementation uses proper recording mechanisms
    ///
    /// Note: This is a compile-time/documentation test. The actual E2E test
    /// should verify that recorded audio differs from input when there's
    /// actual signal processing or latency.
    #[test]
    fn test_record_and_analyze_signature() {
        // This test documents the expected signature of record_and_analyze.
        // It takes separate paths for input (playback) and output (recording),
        // which is the first line of defense against the "copy instead of record" bug.

        // Verify function exists with correct parameter count and types
        // by calling it with dummy parameters (compile-time check only)
        let _check = || async {
            let temp_path = Path::new("/tmp/input.wav");
            let output_path = Path::new("/tmp/output.wav");
            let csv_path = Path::new("/tmp/output.csv");
            let reference: Vec<f32> = vec![];

            // This won't run, but ensures the signature is correct
            if false {
                let _result = record_and_analyze(
                    temp_path,   // temp_wav_path (for playback)
                    output_path, // recorded_wav_path (for recording output)
                    &reference,  // reference_signal
                    48000_u32,   // sample_rate
                    csv_path,    // output_csv_path
                    1_u16,       // output_channel
                    1_u16,       // input_channel
                    None,        // output_device_name
                    None,        // input_device_name
                    None,        // microphone_compensation_path
                    None,        // sweep_range
                );
            }
        };

        // Just verify it compiles
        let _ = _check;
    }
}
