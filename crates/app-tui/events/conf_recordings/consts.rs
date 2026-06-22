use super::misc::ctc_raw_capture_channel_indices;
use super::types::RecordingResultSlot;
use crate::app::App;
use sotf_audio_player::recording_types::CtcMatrixExportStrategy;
use std::sync::{Arc, Mutex};

/// Kick off the tone-burst probe capture on a background thread.
///
/// The shared-slot pattern mirrors `spawn_room_eq_optimization`
/// (`OnceLock` + `Arc<Mutex>`), drained by [`poll_probe_capture`] on every
/// main loop tick.
#[allow(clippy::type_complexity)]
pub(super) static PROBE_CAPTURE_RESULT: std::sync::OnceLock<
    Arc<
        Mutex<
            Option<
                Result<
                    (
                        sotf_audio_player::recording_types::DelayProbeResults,
                        String,
                    ),
                    String,
                >,
            >,
        >,
    >,
> = std::sync::OnceLock::new();

pub(super) const SPL_FIELD_REF_FREQ: usize = 0;

pub(super) const SPL_FIELD_TONE_AMP: usize = 1;

pub(super) const SPL_FIELD_DURATION: usize = 2;

pub(super) const SPL_FIELD_OUT_CH: usize = 3;

pub(super) const SPL_FIELD_IN_CH: usize = 4;

pub(super) const SPL_FIELD_RUN: usize = 5;

pub(super) const SPL_FIELD_REPORTED: usize = 6;

pub(super) const SPL_FIELD_COUNT: usize = 7;

/// Background slot for the SPL calibration capture.
#[allow(clippy::type_complexity)]
pub(super) static SPL_CAPTURE_RESULT: std::sync::OnceLock<
    Arc<Mutex<Option<Result<sotf_audio::signal_recorder::SplCalibrationResult, String>>>>,
> = std::sync::OnceLock::new();

#[allow(clippy::type_complexity)]
pub(super) static BASS_ANCHOR_CAPTURE_RESULT: std::sync::OnceLock<
    Arc<
        Mutex<
            Option<
                Result<
                    (
                        sotf_audio_player::recording_types::BassAnchorResults,
                        String,
                    ),
                    String,
                >,
            >,
        >,
    >,
> = std::sync::OnceLock::new();

pub(super) static RECORDING_RESULT: std::sync::OnceLock<RecordingResultSlot> =
    std::sync::OnceLock::new();

pub(super) fn start_recording_channel(app: &mut App, channel_idx: usize) {
    use sotf_audio_player::recording_types::ChannelRecordingState;
    use sotf_audio_player::signal_recorder::{
        DEFAULT_MLS_ORDER, SignalParams, SignalType, generate_signal, write_temp_wav,
    };

    let selected = match app.recording.model.channel_recordings.get(channel_idx) {
        Some(ch) => ch.clone(),
        None => return,
    };
    let ctc_strategy = app.recording.model.recording_config.ctc_matrix_strategy;
    let capture_indices = if ctc_strategy == CtcMatrixExportStrategy::RawSweep {
        ctc_raw_capture_channel_indices(app, channel_idx)
    } else {
        vec![channel_idx]
    };
    if ctc_strategy == CtcMatrixExportStrategy::RawSweep && capture_indices.len() < 2 {
        if let Some(ch) = app.recording.model.channel_recordings.get_mut(channel_idx) {
            ch.state = ChannelRecordingState::Error;
        }
        app.recording.model.status_message =
            "Raw-sweep CTC requires two ear input channels for the selected speaker/position"
                .to_string();
        return;
    }

    for idx in &capture_indices {
        if let Some(ch) = app.recording.model.channel_recordings.get_mut(*idx) {
            ch.state = ChannelRecordingState::Recording;
            ch.result = None;
        }
    }
    app.recording.model.status_message = if ctc_strategy == CtcMatrixExportStrategy::RawSweep {
        format!("Recording CTC ear pair for {}...", selected.channel_name)
    } else {
        format!("Recording channel {}...", selected.channel_name)
    };
    let speaker_index = selected.channel_index;
    let mic_index = selected.mic_index;

    // Map signal type
    let signal_type = match app.recording.model.signal_type {
        sotf_audio_player::recording_types::RecordingSignalType::Sweep => SignalType::Sweep,
        sotf_audio_player::recording_types::RecordingSignalType::WhiteNoise => {
            SignalType::WhiteNoise
        }
        sotf_audio_player::recording_types::RecordingSignalType::PinkNoise => SignalType::PinkNoise,
        sotf_audio_player::recording_types::RecordingSignalType::Mls => SignalType::Mls,
        sotf_audio_player::recording_types::RecordingSignalType::Dirac => SignalType::Dirac,
        sotf_audio_player::recording_types::RecordingSignalType::DelayProbe => {
            log::warn!(
                "DelayProbe selected in per-channel mode; use probe_channel_delays() instead. Falling back to Sweep."
            );
            SignalType::Sweep
        }
    };

    let duration_secs = app.recording.model.signal_duration_secs;
    let level_db = app.recording.model.signal_level_db;
    let sweep_start_freq = selected.sweep_start_freq;
    let sweep_end_freq = selected.sweep_end_freq;
    let sample_rate = app.recording.model.playback_config.sample_rate;

    let output_device = app.recording.model.playback_config.device_name.clone();
    let input_device = app.recording.model.recording_config.device_name.clone();

    let output_channel = app
        .recording
        .model
        .playback_config
        .channel_mappings
        .get(speaker_index)
        .map(|m| m.interface_channel())
        .unwrap_or(0) as u16;
    let input_channel = app
        .recording
        .model
        .recording_config
        .channel_mappings
        .get(mic_index)
        .copied()
        .unwrap_or(0) as u16;
    let loopback_input = app
        .recording
        .model
        .recording_config
        .ctc_loopback_input_channel;
    let position_idx = selected.mic_position_index;

    // Per-channel calibration lives in `recording_config.mic_calibration_paths`.
    // The per-channel signal recorder takes a single path and applies it to
    // its one input — pick the calibration for the input channel being used.
    let mic_calibration = app
        .recording
        .model
        .recording_config
        .mic_calibration_paths
        .get(input_channel as usize)
        .and_then(|o| o.clone())
        .filter(|s| !s.is_empty());

    let channel_name = app.recording.model.channel_recordings[channel_idx]
        .channel_name
        .clone();
    let output_directory = app.recording.output_directory.clone();

    // Convert dB level to linear amplitude
    let amplitude = 10.0_f32.powf(level_db / 20.0);

    // Generate signal parameters
    let params = match signal_type {
        SignalType::Sweep => SignalParams::Sweep {
            start_freq: sweep_start_freq,
            end_freq: sweep_end_freq,
            amp: amplitude,
        },
        SignalType::WhiteNoise | SignalType::PinkNoise => SignalParams::Noise { amp: amplitude },
        SignalType::Mls => SignalParams::Mls {
            order: DEFAULT_MLS_ORDER,
            amp: amplitude,
        },
        SignalType::Dirac => SignalParams::Dirac { amp: amplitude },
        _ => SignalParams::Sweep {
            start_freq: sweep_start_freq,
            end_freq: sweep_end_freq,
            amp: amplitude,
        },
    };

    // Generate the test signal
    let signal = match generate_signal(signal_type, &params, duration_secs, sample_rate) {
        Ok(s) => s,
        Err(e) => {
            if let Some(ch) = app.recording.model.channel_recordings.get_mut(channel_idx) {
                ch.state = ChannelRecordingState::Error;
            }
            app.recording.model.status_message = format!("Error generating signal: {}", e);
            return;
        }
    };

    // Write to temp file
    let temp_wav = match write_temp_wav(&signal, sample_rate, 1) {
        Ok(f) => f,
        Err(e) => {
            if let Some(ch) = app.recording.model.channel_recordings.get_mut(channel_idx) {
                ch.state = ChannelRecordingState::Error;
            }
            app.recording.model.status_message = format!("Error writing temp WAV: {}", e);
            return;
        }
    };

    // Create output paths
    let safe_channel_name: String = channel_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let recording_dir = std::path::PathBuf::from(&output_directory);
    let recorded_wav_path = recording_dir.join(format!("{}.wav", safe_channel_name));
    let csv_path = recording_dir.join(format!("{}.csv", safe_channel_name));
    let loopback_wav_path = recording_dir.join(format!("{}_loopback.wav", safe_channel_name));
    let loopback_csv_path = recording_dir.join(format!("{}_loopback.csv", safe_channel_name));

    let capture_entries: Vec<(
        usize,
        std::path::PathBuf,
        std::path::PathBuf,
        u16,
        Option<String>,
    )> = if ctc_strategy == CtcMatrixExportStrategy::RawSweep {
        capture_indices
            .iter()
            .filter_map(|idx| {
                let rec = app.recording.model.channel_recordings.get(*idx)?;
                let safe_name: String = rec
                    .channel_name
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '_' || c == '-' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                let input_ch = app
                    .recording
                    .model
                    .recording_config
                    .channel_mappings
                    .get(rec.mic_index)
                    .copied()
                    .unwrap_or(0) as u16;
                let calibration = app
                    .recording
                    .model
                    .recording_config
                    .mic_calibration_paths
                    .get(input_ch as usize)
                    .and_then(|o| o.clone())
                    .filter(|s| !s.is_empty());
                Some((
                    *idx,
                    recording_dir.join(format!("{}.wav", safe_name)),
                    recording_dir.join(format!("{}.csv", safe_name)),
                    input_ch,
                    calibration,
                ))
            })
            .collect()
    } else {
        vec![(
            channel_idx,
            recorded_wav_path.clone(),
            csv_path.clone(),
            input_channel,
            mic_calibration.clone(),
        )]
    };
    let capture_channel_indices: Vec<usize> = capture_entries.iter().map(|entry| entry.0).collect();
    let capture_wav_paths: Vec<std::path::PathBuf> = capture_entries
        .iter()
        .map(|entry| entry.1.clone())
        .collect();
    let capture_csv_paths: Vec<std::path::PathBuf> = capture_entries
        .iter()
        .map(|entry| entry.2.clone())
        .collect();
    let capture_input_channels: Vec<u16> = capture_entries.iter().map(|entry| entry.3).collect();
    let capture_calibrations: Vec<Option<String>> = capture_entries
        .iter()
        .map(|entry| entry.4.clone())
        .collect();

    // B4: Create output directory before recording
    if let Err(e) = std::fs::create_dir_all(&recording_dir) {
        if let Some(ch) = app.recording.model.channel_recordings.get_mut(channel_idx) {
            ch.state = ChannelRecordingState::Error;
        }
        app.recording.model.status_message = format!("Cannot create directory: {}", e);
        return;
    }

    if ctc_strategy == CtcMatrixExportStrategy::RawSweep {
        let reference_path = recording_dir.join("ctc_reference_sweep.wav");
        if let Err(e) = sotf_audio_player::signal_recorder::write_wav_file(
            &reference_path,
            &signal,
            sample_rate,
            1,
        ) {
            app.recording.model.status_message =
                format!("Could not write CTC reference sweep: {}", e);
        } else {
            app.recording.model.ctc_reference_sweep_path =
                Some(reference_path.to_string_lossy().to_string());
        }
    }

    let result_slot = RECORDING_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    // Clear stale result
    if let Ok(mut g) = result_slot.lock() {
        *g = None;
    }

    let reference_signal = signal;
    let temp_wav_path = temp_wav.path().to_path_buf();

    std::thread::spawn(move || {
        use sotf_audio_player::recording_types::RecordingResult;
        use sotf_audio_player::signal_recorder::{record_and_analyze, record_and_analyze_multi};

        let sweep_range = if signal_type == SignalType::Sweep {
            Some((sweep_start_freq, sweep_end_freq))
        } else {
            None
        };

        let out_dev = if output_device.is_empty() {
            None
        } else {
            Some(output_device.as_str())
        };
        let in_dev = if input_device.is_empty() {
            None
        } else {
            Some(input_device.as_str())
        };

        let result = if ctc_strategy == CtcMatrixExportStrategy::RawSweep {
            let mut wav_paths = capture_wav_paths.clone();
            let mut csv_paths = capture_csv_paths.clone();
            let mut input_channels = capture_input_channels.clone();
            let mut calibrations = capture_calibrations.clone();
            if let Some(loopback_input) = loopback_input {
                wav_paths.push(loopback_wav_path.clone());
                csv_paths.push(loopback_csv_path);
                input_channels.push(loopback_input as u16);
                calibrations.push(None);
            }
            record_and_analyze_multi(
                &temp_wav_path,
                &wav_paths,
                &reference_signal,
                sample_rate,
                &csv_paths,
                output_channel,
                &input_channels,
                out_dev,
                in_dev,
                &calibrations,
                sweep_range,
            )
            .map(|mut results| {
                if loopback_input.is_some() {
                    let _ = results.pop();
                }
                results
            })
        } else {
            record_and_analyze(
                &temp_wav_path,
                &recorded_wav_path,
                &reference_signal,
                sample_rate,
                &csv_path,
                output_channel,
                input_channel,
                out_dev,
                in_dev,
                mic_calibration.as_deref(),
                sweep_range,
            )
            .map(|result| vec![result])
        };

        let mapped = result
            .map(|analysis_results| {
                let rec_results = analysis_results
                    .into_iter()
                    .enumerate()
                    .filter_map(|(idx, analysis_result)| {
                        let ch_idx = *capture_channel_indices.get(idx)?;
                        let wav_path = capture_wav_paths.get(idx)?;
                        let csv_path = capture_csv_paths.get(idx)?;
                        Some((
                            ch_idx,
                            RecordingResult {
                                channel: ch_idx,
                                wav_path: Some(wav_path.to_string_lossy().to_string()),
                                csv_path: Some(csv_path.to_string_lossy().to_string()),
                                frequencies: analysis_result.frequencies,
                                magnitude_db: analysis_result.spl_db,
                                phase_deg: analysis_result.phase_deg,
                                impulse_response: Some(analysis_result.impulse_response),
                                impulse_time_ms: Some(analysis_result.impulse_time_ms),
                                excess_group_delay_ms: Some(analysis_result.excess_group_delay_ms),
                                thd_percent: Some(analysis_result.thd_percent),
                                harmonic_distortion_db: Some(
                                    analysis_result.harmonic_distortion_db,
                                ),
                                rt60_ms: Some(analysis_result.rt60_ms),
                                clarity_c50_db: Some(analysis_result.clarity_c50_db),
                                clarity_c80_db: Some(analysis_result.clarity_c80_db),
                                spectrogram_db: Some(analysis_result.spectrogram_db),
                            },
                        ))
                    })
                    .collect();
                let loopback = if ctc_strategy == CtcMatrixExportStrategy::RawSweep
                    && loopback_input.is_some()
                {
                    Some(
                        sotf_audio_player::recording_types::TransferMatrixLoopbackRecording {
                            speaker_index,
                            mic_position_index: position_idx,
                            wav_path: loopback_wav_path.to_string_lossy().to_string(),
                        },
                    )
                } else {
                    None
                };
                (rec_results, loopback)
            })
            .map_err(|e| e.to_string());

        if let Ok(mut guard) = result_slot.lock() {
            *guard = Some(mapped);
        }

        // Keep temp file alive until recording is done
        drop(temp_wav);
    });
}
