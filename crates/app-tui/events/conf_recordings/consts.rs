use super::misc::ctc_raw_capture_channel_indices;
use super::types::{
    BassAnchorCaptureResultSlot, ProbeCaptureResultSlot, RecordingResultSlot, SplCaptureResultSlot,
};
use crate::app::App;
use sotf_audio_player::recording_types::CtcMatrixExportStrategy;
use std::sync::{Arc, Mutex};

/// Kick off the tone-burst probe capture on a background thread.
///
/// The shared-slot pattern mirrors `spawn_room_eq_optimization`
/// (`OnceLock` + `Arc<Mutex>`), drained by [`poll_probe_capture`] on every
/// main loop tick.
pub(super) static PROBE_CAPTURE_RESULT: std::sync::OnceLock<ProbeCaptureResultSlot> =
    std::sync::OnceLock::new();

pub(super) const SPL_FIELD_REF_FREQ: usize = 0;

pub(super) const SPL_FIELD_TONE_AMP: usize = 1;

pub(super) const SPL_FIELD_DURATION: usize = 2;

pub(super) const SPL_FIELD_OUT_CH: usize = 3;

pub(super) const SPL_FIELD_IN_CH: usize = 4;

pub(super) const SPL_FIELD_RUN: usize = 5;

pub(super) const SPL_FIELD_REPORTED: usize = 6;

pub(super) const SPL_FIELD_COUNT: usize = 7;

/// Background slot for the SPL calibration capture.
pub(super) static SPL_CAPTURE_RESULT: std::sync::OnceLock<SplCaptureResultSlot> =
    std::sync::OnceLock::new();

pub(super) static BASS_ANCHOR_CAPTURE_RESULT: std::sync::OnceLock<BassAnchorCaptureResultSlot> =
    std::sync::OnceLock::new();

pub(super) static RECORDING_RESULT: std::sync::OnceLock<RecordingResultSlot> =
    std::sync::OnceLock::new();

#[expect(
    clippy::too_many_lines,
    reason = "Keep the ordered UI workflow and its state updates in one operation"
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
pub(super) fn start_recording_channel(app: &mut App, channel_idx: usize) {
    use sotf_audio_player::recording_helpers::{
        capture_signal_params, measurement_amplitude, resolve_mic_calibration,
        sanitize_recording_name, signal_type_for,
    };
    use sotf_audio_player::recording_types::ChannelRecordingState;
    use sotf_audio_player::signal_recorder::{
        SignalType, prepare_measurement_signal, write_temp_wav,
    };

    let selected = match app.recording.model.channel_recordings.get(channel_idx) {
        Some(ch) => ch.clone(),
        None => return,
    };
    let channels = (|| {
        let output = app
            .recording
            .model
            .playback_config
            .channel_mappings
            .iter()
            .map(|mapping| u16::try_from(mapping.interface_channel()))
            .collect::<Result<Vec<_>, _>>()?;
        let input = app
            .recording
            .model
            .recording_config
            .channel_mappings
            .iter()
            .copied()
            .map(u16::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let loopback = app
            .recording
            .model
            .recording_config
            .ctc_loopback_input_channel
            .map(u16::try_from)
            .transpose()?;
        Ok::<_, std::num::TryFromIntError>((output, input, loopback))
    })();
    let Ok((mapped_output_channels, mapped_input_channels, loopback_input)) = channels else {
        if let Some(channel) = app.recording.model.channel_recordings.get_mut(channel_idx) {
            channel.state = ChannelRecordingState::Error;
        }
        app.recording.model.status_message =
            "Channel exceeds the capture channel range".to_string();
        return;
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

    // Map signal type via the shared helper (DelayProbe falls back to Sweep).
    let signal_type = signal_type_for(app.recording.model.signal_type);

    let duration_secs = app.recording.model.signal_duration_secs;
    let level_db = app.recording.model.signal_level_db;
    let sweep_start_freq = selected.sweep_start_freq;
    let sweep_end_freq = selected.sweep_end_freq;
    // GD-Opt sweep shaping (B1): the same model values persisted by
    // `build_recording_configuration`, so the saved metadata describes the
    // stimulus actually played.
    let bass_octave_duration_s = app.recording.model.bass_octave_duration_s;
    let pre_silence_s = app.recording.model.pre_silence_s;
    let post_silence_s = app.recording.model.post_silence_s;
    let sample_rate = app.recording.model.playback_config.sample_rate;

    let output_device = app.recording.model.playback_config.device_name.clone();
    let input_device = app.recording.model.recording_config.device_name.clone();

    let output_channel = mapped_output_channels
        .get(speaker_index)
        .copied()
        .unwrap_or(0);
    let input_channel = mapped_input_channels.get(mic_index).copied().unwrap_or(0);
    let position_idx = selected.mic_position_index;

    // Per-mic calibration lives in the model-level `mic_calibration_paths`,
    // indexed by mic slot (parallel to `recording_config.channel_mappings`),
    // NOT by hardware input channel (B3). Falls back to the session-global
    // `mic_calibration_path` when the per-mic slot is unset.
    let mic_calibration = resolve_mic_calibration(
        &app.recording.model.mic_calibration_paths,
        mic_index,
        app.recording.model.mic_calibration_path.as_deref(),
    );

    let channel_name = app.recording.model.channel_recordings[channel_idx]
        .channel_name
        .clone();
    let output_directory = app.recording.output_directory.clone();

    // Convert dB level to linear amplitude (clamped to ≤ 0 dBFS, R3).
    let amplitude = measurement_amplitude(f64::from(level_db));

    // Generate signal parameters. The sweep path routes through the shared
    // helper (B1): with `bass_octave_duration_s` set, the stimulus becomes
    // the self-timed octave-scaled sweep with silence windows — the same
    // values `build_recording_configuration` persists at save time.
    let params = capture_signal_params(
        signal_type,
        sweep_start_freq,
        sweep_end_freq,
        amplitude,
        Some(bass_octave_duration_s),
        Some(pre_silence_s),
        post_silence_s,
    );

    // Prepare the test signal through the shared entry point (R4):
    // validates the parameters (Nyquist, start < end, amplitude) and
    // applies the standard 20 ms fades + 250 ms pre/post padding. The
    // padding is lag-neutral for the whole-signal cross-correlation
    // analysis. Validation failures (e.g. sweep end above Nyquist at
    // 44.1 kHz) surface as a status message and an Error channel state.
    let signal = match prepare_measurement_signal(signal_type, &params, duration_secs, sample_rate)
    {
        Ok(s) => s,
        Err(e) => {
            for idx in &capture_indices {
                if let Some(ch) = app.recording.model.channel_recordings.get_mut(*idx) {
                    ch.state = ChannelRecordingState::Error;
                }
            }
            app.recording.model.status_message = format!("Error generating signal: {e}");
            return;
        }
    };
    let stimulus_config = match sotf_audio_player::recording_helpers::effective_acquisition_config(
        signal_type,
        &params,
        duration_secs,
        sample_rate,
    ) {
        Ok(config) => config,
        Err(error) => {
            for idx in &capture_indices {
                if let Some(ch) = app.recording.model.channel_recordings.get_mut(*idx) {
                    ch.state = ChannelRecordingState::Error;
                }
            }
            app.recording.model.status_message =
                format!("Error describing capture signal: {error}");
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
            app.recording.model.status_message = format!("Error writing temp WAV: {e}");
            return;
        }
    };

    // Create output paths
    let safe_channel_name = sanitize_recording_name(&channel_name);
    let recording_dir = std::path::PathBuf::from(&output_directory);
    let recorded_wav_path = recording_dir.join(format!("{safe_channel_name}.wav"));
    let csv_path = recording_dir.join(format!("{safe_channel_name}.csv"));
    let loopback_wav_path = recording_dir.join(format!("{safe_channel_name}_loopback.wav"));
    let loopback_csv_path = recording_dir.join(format!("{safe_channel_name}_loopback.csv"));

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
                let safe_name = sanitize_recording_name(&rec.channel_name);
                let input_ch = mapped_input_channels
                    .get(rec.mic_index)
                    .copied()
                    .unwrap_or(0);
                let calibration = resolve_mic_calibration(
                    &app.recording.model.mic_calibration_paths,
                    rec.mic_index,
                    app.recording.model.mic_calibration_path.as_deref(),
                );
                Some((
                    *idx,
                    recording_dir.join(format!("{safe_name}.wav")),
                    recording_dir.join(format!("{safe_name}.csv")),
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
        app.recording.model.status_message = format!("Cannot create directory: {e}");
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
                format!("Could not write CTC reference sweep: {e}");
        } else {
            // Task 10: the octave-scaled sweep is self-timed, so persist the
            // *actual* stimulus duration alongside the reference path —
            // `signal_duration_secs` no longer describes this WAV.
            let actual_duration_s = signal.len() as f32 / sample_rate as f32;
            app.recording.model.ctc_reference_sweep_path =
                Some(reference_path.to_string_lossy().to_string());
            app.recording.model.ctc_reference_sweep_duration_s = Some(actual_duration_s);
        }
    }

    let result_slot =
        std::sync::Arc::clone(RECORDING_RESULT.get_or_init(|| Arc::new(Mutex::new(None))));

    // F1: bump the capture generation so any OLD capture thread that
    // completes after this spawn (e.g. inside the ~50 ms cancel-poll
    // window, or orphaned by a mid-session NumPositions rebuild) has its
    // result discarded by `poll_recording` instead of overwriting the new
    // capture's channels.
    let capture_generation = app.recording.model.next_capture_generation();

    // Clear stale result
    if let Ok(mut g) = result_slot.lock() {
        *g = None;
    }

    let reference_signal = signal;
    let temp_wav_path = temp_wav.path().to_path_buf();
    let num_sweeps = app.recording.model.num_sweeps;
    #[cfg(feature = "dev-api")]
    let virtual_capture_profile = app.recording.virtual_capture_profile.clone();

    // R8: reset the cancel flag so a stale request cannot abort this run,
    // then hand a clone to the capture thread (mirrors
    // `spawn_probe_capture`).
    app.recording.model.reset_sweep_cancel();
    let cancel_flag = std::sync::Arc::clone(&app.recording.model.sweep_cancel_requested);

    std::thread::spawn(move || {
        use sotf_audio_player::EnginePlayback;
        use sotf_audio_player::recording_helpers::summarize_take_quality;
        use sotf_audio_player::recording_types::RecordingResult;
        use sotf_audio_player::signal_recorder::{
            record_and_analyze_with, record_and_analyze_with_multi,
        };
        #[cfg(feature = "dev-api")]
        use sotf_capture::signal_recorder::{
            record_and_analyze_with_multi_recorder, record_and_analyze_with_recorder,
        };

        let sweep_range =
            (signal_type == SignalType::Sweep).then_some((sweep_start_freq, sweep_end_freq));

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

        #[cfg(feature = "dev-api")]
        let fake_profile = virtual_capture_profile
            .as_deref()
            .and_then(|profile| match profile {
                "clean" => Some(sotf_capture::signal_recorder::FakeAudioProfile::Clean),
                "high-thd" => Some(sotf_capture::signal_recorder::FakeAudioProfile::HighThd),
                "low-snr" => Some(sotf_capture::signal_recorder::FakeAudioProfile::LowSnr),
                "unknown-evidence" => {
                    Some(sotf_capture::signal_recorder::FakeAudioProfile::UnknownEvidence)
                }
                "clipping" => Some(sotf_capture::signal_recorder::FakeAudioProfile::Clipping),
                _ => None,
            });

        let result = if ctc_strategy == CtcMatrixExportStrategy::RawSweep {
            let mut wav_paths = capture_wav_paths.clone();
            let mut csv_paths = capture_csv_paths.clone();
            let mut input_channels = capture_input_channels.clone();
            let mut calibrations = capture_calibrations.clone();
            if let Some(loopback_input) = loopback_input {
                wav_paths.push(loopback_wav_path.clone());
                csv_paths.push(loopback_csv_path);
                input_channels.push(loopback_input);
                calibrations.push(None);
            }
            #[cfg(feature = "dev-api")]
            let captures = if let Some(profile) = fake_profile {
                let wire = sotf_capture::signal_recorder::VirtualWire::for_profile(profile);
                let mut playback = wire.playback("tui_record_and_analyze_multi");
                let mut recorder = wire.recorder("tui_record_and_analyze_multi");
                record_and_analyze_with_multi_recorder(
                    &mut playback,
                    &mut recorder,
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
                    num_sweeps,
                    Some(std::sync::Arc::clone(&cancel_flag)),
                )
            } else {
                record_and_analyze_with_multi(
                    &mut EnginePlayback::new(false, "record_and_analyze_multi"),
                    &temp_wav_path,
                    &wav_paths,
                    &reference_signal,
                    sample_rate,
                    &csv_paths,
                    output_channel,
                    &input_channels,
                    None,
                    None,
                    &calibrations,
                    sweep_range,
                    num_sweeps,
                    Some(std::sync::Arc::clone(&cancel_flag)),
                )
            };
            #[cfg(not(feature = "dev-api"))]
            let captures = record_and_analyze_with_multi(
                &mut EnginePlayback::new(false, "record_and_analyze_multi"),
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
                num_sweeps,
                Some(cancel_flag.clone()),
            );
            captures.map(|mut results| {
                if loopback_input.is_some() {
                    let _ = results.pop();
                }
                results
            })
        } else {
            #[cfg(feature = "dev-api")]
            let capture = if let Some(profile) = fake_profile {
                let wire = sotf_capture::signal_recorder::VirtualWire::for_profile(profile);
                let mut playback = wire.playback("tui_record_and_analyze");
                let mut recorder = wire.recorder("tui_record_and_analyze");
                record_and_analyze_with_recorder(
                    &mut playback,
                    &mut recorder,
                    &temp_wav_path,
                    &recorded_wav_path,
                    &reference_signal,
                    sample_rate,
                    &csv_path,
                    output_channel,
                    input_channel,
                    None,
                    None,
                    mic_calibration.as_deref(),
                    sweep_range,
                    num_sweeps,
                    Some(cancel_flag),
                )
            } else {
                record_and_analyze_with(
                    &mut EnginePlayback::new(true, "record_and_analyze"),
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
                    num_sweeps,
                    Some(cancel_flag),
                )
            };
            #[cfg(not(feature = "dev-api"))]
            let capture = record_and_analyze_with(
                &mut EnginePlayback::new(true, "record_and_analyze"),
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
                num_sweeps,
                Some(cancel_flag),
            );
            capture.map(|result| vec![result])
        };

        let mapped = result
            .map(|analysis_results| {
                let rec_results = analysis_results
                    .into_iter()
                    .enumerate()
                    .filter_map(|(idx, capture)| {
                        let ch_idx = *capture_channel_indices.get(idx)?;
                        let wav_path = capture_wav_paths.get(idx)?;
                        let csv_path = capture_csv_paths.get(idx)?;
                        // The engine returns CaptureAnalysis; summarize the
                        // per-take quality before moving the analysis result.
                        let quality = summarize_take_quality(&capture);
                        let analysis_result = capture.result;
                        Some((
                            ch_idx,
                            RecordingResult {
                                sample_rate_hz: Some(sample_rate),
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
                                quality: Some(quality),
                                stimulus_config: Some(stimulus_config.clone()),
                            },
                        ))
                    })
                    .collect();
                let loopback = (ctc_strategy == CtcMatrixExportStrategy::RawSweep
                    && loopback_input.is_some())
                .then(
                    || sotf_audio_player::recording_types::TransferMatrixLoopbackRecording {
                        speaker_index,
                        mic_position_index: position_idx,
                        wav_path: loopback_wav_path.to_string_lossy().to_string(),
                    },
                );
                (rec_results, loopback)
            })
            .map_err(|e| e.clone());

        if let Ok(mut guard) = result_slot.lock() {
            *guard = Some((capture_generation, mapped));
        }

        // Keep temp file alive until recording is done
        drop(temp_wav);
    });
}
