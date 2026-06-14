use super::consts::BASS_ANCHOR_CAPTURE_RESULT;
use super::consts::PROBE_CAPTURE_RESULT;
use super::consts::SPL_CAPTURE_RESULT;
use super::misc::now_ms;
use crate::app::App;
use std::sync::{Arc, Mutex};

pub(super) fn spawn_probe_capture(app: &mut App) {
    use sotf_audio_player::recording_types::ProbeCaptureStatus;

    if app.recording.channel_recordings.is_empty() {
        app.recording.probe_capture.status =
            ProbeCaptureStatus::Failed("Record sweeps first (Capture step)".to_string());
        return;
    }

    // Probe one signal per *speaker output channel*, not per
    // (speaker × position × mic) entry in `channel_recordings`.
    // The latter multiplies the channel count well beyond the physical
    // layout (e.g. 9.1.6 × 2 mic positions × 1 mic = 32 entries for a
    // 16-speaker setup) and tries to address hardware outputs that
    // don't exist.
    let mappings = &app.recording.playback_config.channel_mappings;
    let channel_names: Vec<String> = mappings.iter().map(|m| m.group_name.clone()).collect();
    let channel_indices: Vec<u16> = mappings
        .iter()
        .map(|m| m.interface_channel() as u16)
        .collect();

    // Build the output WAV path under the same directory the sweeps
    // landed in so everything travels together at save time.
    let wav_path_str = {
        let base_dir = if app.recording.output_directory.is_empty() {
            ".".to_string()
        } else {
            app.recording.output_directory.clone()
        };
        format!("{}/probe_all_channels.wav", base_dir)
    };

    let probe_ms = app.recording.probe_capture.probe_duration_ms;
    let silence_ms = app.recording.probe_capture.silence_duration_ms;
    let sample_rate = app.recording.probe_capture.sample_rate;
    let input_channel = app.recording.probe_capture.input_channel;
    let signal_level_db = app.recording.signal_level_db;
    let output_device = Some(app.recording.playback_config.device_name.clone());
    let input_device = Some(app.recording.recording_config.device_name.clone());

    app.recording.probe_capture.status = ProbeCaptureStatus::Running {
        started_at_ms: now_ms(),
    };
    app.recording.probe_capture.results = None;

    let slot = PROBE_CAPTURE_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    if let Ok(mut g) = slot.lock() {
        *g = None;
    }

    std::thread::spawn(move || {
        let wav_path = std::path::PathBuf::from(&wav_path_str);
        let result = sotf_audio::signal_recorder::probe_channel_delays_with_recording(
            &channel_indices,
            &channel_names,
            sample_rate,
            probe_ms,
            silence_ms,
            output_device.as_deref(),
            input_device.as_deref(),
            input_channel,
            &wav_path,
            signal_level_db,
            None,
        )
        .map(|r| (r, wav_path_str));
        if let Ok(mut g) = slot.lock() {
            *g = Some(result);
        }
    });
}

/// Spawn the SPL calibration capture on a background thread (mirrors
/// `spawn_probe_capture`).
pub(super) fn spawn_spl_calibration_capture(app: &mut App) {
    use sotf_audio_player::recording_types::SplCalibrationCaptureStatus;

    let cal = &mut app.recording.spl_calibration_capture;
    let reference_freq_hz = cal.reference_freq_hz;
    let tone_amp = cal.tone_amp;
    let duration_s = cal.duration_s;
    let sample_rate = cal.sample_rate;
    let output_channel = cal.output_channel;
    let input_channel = cal.input_channel;
    let output_device = Some(app.recording.playback_config.device_name.clone());
    let input_device = Some(app.recording.recording_config.device_name.clone());

    // Reset the cancel flag and capture status so the new run starts
    // clean. `engine_result` is cleared on every fresh capture.
    app.recording
        .spl_cancel_requested
        .store(false, std::sync::atomic::Ordering::Relaxed);
    let cancel_flag = app.recording.spl_cancel_requested.clone();

    let cal = &mut app.recording.spl_calibration_capture;
    cal.status = SplCalibrationCaptureStatus::Running {
        started_at_ms: now_ms(),
    };
    cal.engine_result = None;

    let slot = SPL_CAPTURE_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    if let Ok(mut g) = slot.lock() {
        *g = None;
    }

    std::thread::spawn(move || {
        let result = sotf_audio::signal_recorder::run_spl_calibration(
            output_channel,
            sample_rate,
            reference_freq_hz,
            tone_amp,
            duration_s,
            output_device.as_deref(),
            input_device.as_deref(),
            input_channel,
            Some(cancel_flag),
        );
        if let Ok(mut g) = slot.lock() {
            *g = Some(result);
        }
    });
}

pub(super) fn spawn_bass_anchor_capture(app: &mut App) {
    use sotf_audio_player::recording_types::BassAnchorCaptureStatus;

    let mappings = &app.recording.playback_config.channel_mappings;
    if mappings.is_empty() {
        app.recording.bass_anchor_capture.status =
            BassAnchorCaptureStatus::Failed("Configure speakers first (Config step)".to_string());
        return;
    }
    let channel_names: Vec<String> = mappings.iter().map(|m| m.group_name.clone()).collect();
    let channel_indices: Vec<u16> = mappings
        .iter()
        .map(|m| m.interface_channel() as u16)
        .collect();

    let wav_path_str = {
        let base_dir = if app.recording.output_directory.is_empty() {
            ".".to_string()
        } else {
            app.recording.output_directory.clone()
        };
        format!("{}/bass_anchor_all_channels.wav", base_dir)
    };

    let bass_freq_hz = app.recording.bass_anchor_capture.bass_freq_hz;
    let bass_duration_s = app.recording.bass_anchor_capture.bass_duration_s;
    let fade_ms = app.recording.bass_anchor_capture.fade_ms;
    let num_windows = app.recording.bass_anchor_capture.num_windows;
    let silence_ms = app.recording.bass_anchor_capture.silence_duration_ms;
    let sample_rate = app.recording.bass_anchor_capture.sample_rate;
    let input_channel = app.recording.bass_anchor_capture.input_channel;
    let loopback_input_channel =
        app.recording
            .recording_config
            .ctc_loopback_input_channel
            .and_then(|c| match u16::try_from(c) {
                Ok(v) => Some(v),
                Err(_) => {
                    log::warn!(
                        "Loopback input channel {c} exceeds u16::MAX — bass anchor will run without loopback reference",
                    );
                    None
                }
            });
    let signal_level_db = app.recording.signal_level_db;
    let output_device = Some(app.recording.playback_config.device_name.clone());
    let input_device = Some(app.recording.recording_config.device_name.clone());

    app.recording.bass_anchor_capture.status = BassAnchorCaptureStatus::Running {
        started_at_ms: now_ms(),
    };
    app.recording.bass_anchor_capture.results = None;

    let slot = BASS_ANCHOR_CAPTURE_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    if let Ok(mut g) = slot.lock() {
        *g = None;
    }

    std::thread::spawn(move || {
        let wav_path = std::path::PathBuf::from(&wav_path_str);
        let result = sotf_audio::signal_recorder::run_bass_anchor_with_recording(
            &channel_indices,
            &channel_names,
            sample_rate,
            bass_freq_hz,
            bass_duration_s,
            fade_ms,
            num_windows,
            silence_ms,
            output_device.as_deref(),
            input_device.as_deref(),
            input_channel,
            loopback_input_channel,
            &wav_path,
            signal_level_db,
            None,
        )
        .map(|r| (r, wav_path_str));
        if let Ok(mut g) = slot.lock() {
            *g = Some(result);
        }
    });
}
