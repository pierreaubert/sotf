use super::consts::BASS_ANCHOR_CAPTURE_RESULT;
use super::consts::PROBE_CAPTURE_RESULT;
use super::consts::RECORDING_RESULT;
use super::consts::SPL_CAPTURE_RESULT;
use crate::app::App;
use std::sync::{Arc, Mutex};

/// Drain the probe-capture slot into `app.recording.model.probe_capture`.
/// Returns `true` if state changed and the UI should redraw.
pub fn poll_probe_capture(app: &mut App) -> bool {
    use sotf_audio_player::recording_types::ProbeCaptureStatus;

    if !matches!(
        app.recording.model.probe_capture.status,
        ProbeCaptureStatus::Running { .. }
    ) {
        return false;
    }
    let slot = PROBE_CAPTURE_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    let Ok(mut guard) = slot.lock() else {
        return false;
    };
    let Some(outcome) = guard.take() else {
        return false;
    };
    drop(guard);
    match outcome {
        Ok((results, wav_path)) => {
            app.recording
                .model
                .probe_capture
                .apply_results(results, Some(wav_path));
        }
        Err(e) => {
            app.recording.model.probe_capture.status = ProbeCaptureStatus::Failed(e);
        }
    }
    true
}

/// Drain the SPL calibration capture slot. Returns `true` if state
/// changed and the UI should redraw.
pub fn poll_spl_calibration_capture(app: &mut App) -> bool {
    use sotf_audio_player::recording_types::SplCalibrationCaptureStatus;

    if !matches!(
        app.recording.model.spl_calibration_capture.status,
        SplCalibrationCaptureStatus::Running { .. }
    ) {
        return false;
    }
    let slot = SPL_CAPTURE_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    let Ok(mut guard) = slot.lock() else {
        return false;
    };
    let Some(outcome) = guard.take() else {
        return false;
    };
    drop(guard);
    let cal = &mut app.recording.model.spl_calibration_capture;
    match outcome {
        Ok(res) => cal.apply_engine_result(res),
        Err(e) if e == sotf_audio::signal_recorder::CANCELLED_ERR => {
            log::info!("SPL calibration capture cancelled by user");
            cal.status = SplCalibrationCaptureStatus::Idle;
        }
        Err(e) => {
            log::warn!("SPL calibration capture failed: {e}");
            cal.status = SplCalibrationCaptureStatus::Failed(e);
        }
    }
    true
}

/// Drain the bass-anchor capture slot. Returns `true` if state changed
/// and the UI should redraw.
pub fn poll_bass_anchor_capture(app: &mut App) -> bool {
    use sotf_audio_player::recording_types::BassAnchorCaptureStatus;

    if !matches!(
        app.recording.model.bass_anchor_capture.status,
        BassAnchorCaptureStatus::Running { .. }
    ) {
        return false;
    }
    let slot = BASS_ANCHOR_CAPTURE_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    let Ok(mut guard) = slot.lock() else {
        return false;
    };
    let Some(outcome) = guard.take() else {
        return false;
    };
    drop(guard);
    match outcome {
        Ok((results, wav_path)) => {
            app.recording
                .model
                .bass_anchor_capture
                .apply_results(results, Some(wav_path));
        }
        Err(e) => {
            app.recording.model.bass_anchor_capture.status = BassAnchorCaptureStatus::Failed(e);
        }
    }
    true
}

/// B7: Poll for recording completion — call from main tick loop
pub fn poll_recording(app: &mut App) -> bool {
    use sotf_audio_player::recording_types::ChannelRecordingState;

    // Only poll when a recording is active
    let has_active = app
        .recording
        .model
        .channel_recordings
        .iter()
        .any(|ch| ch.state == ChannelRecordingState::Recording);
    if !has_active {
        return false;
    }

    let result_slot = RECORDING_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    if let Ok(mut guard) = result_slot.lock()
        && let Some(result) = guard.take()
    {
        match result {
            Ok((rec_results, loopback)) => {
                let mut completed_names = Vec::new();
                for (ch_idx, rec_result) in rec_results {
                    if let Some(ch) = app.recording.model.channel_recordings.get_mut(ch_idx) {
                        ch.state = ChannelRecordingState::Done;
                        completed_names.push(ch.channel_name.clone());
                        ch.result = Some(rec_result);
                    }
                }
                if let Some(loopback) = loopback {
                    app.recording.model.transfer_matrix_loopbacks.retain(|r| {
                        r.speaker_index != loopback.speaker_index
                            || r.mic_position_index != loopback.mic_position_index
                    });
                    app.recording.model.transfer_matrix_loopbacks.push(loopback);
                }
                if !completed_names.is_empty() {
                    if completed_names.len() == 1 {
                        app.recording.model.status_message =
                            format!("Channel {} recording complete", completed_names[0]);
                    } else {
                        app.recording.model.status_message =
                            format!("Recorded {} CTC ear channels", completed_names.len());
                    }
                }
            }
            Err(e) => {
                // Mark the recording channel as error
                for ch in &mut app.recording.model.channel_recordings {
                    if ch.state == ChannelRecordingState::Recording {
                        ch.state = ChannelRecordingState::Error;
                    }
                }
                app.recording.model.status_message = format!("Recording failed: {}", e);
            }
        }
        return true;
    }

    false
}

/// Drain the background save thread's result, if any. Returns true
/// when state changed (forces a redraw via the tick handler).
pub fn poll_save_recordings(app: &mut App) -> bool {
    let rx = match app.recording.save.receiver.as_ref() {
        Some(rx) => rx,
        None => return false,
    };
    match rx.try_recv() {
        Ok(Ok(())) => {
            app.recording.save.success = true;
            app.recording.save.error = None;
            app.recording.save.in_progress = false;
            app.recording.save.receiver = None;
            true
        }
        Ok(Err(msg)) => {
            app.recording.save.error = Some(msg);
            app.recording.save.success = false;
            app.recording.save.in_progress = false;
            app.recording.save.receiver = None;
            true
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => false,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            // Worker dropped its sender without sending — treat as
            // failure rather than a silent hang.
            app.recording.save.error = Some("Save thread terminated without result".to_string());
            app.recording.save.in_progress = false;
            app.recording.save.receiver = None;
            true
        }
    }
}
