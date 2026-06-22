use super::consts::SPL_FIELD_DURATION;
use super::consts::SPL_FIELD_IN_CH;
use super::consts::SPL_FIELD_OUT_CH;
use super::consts::SPL_FIELD_REF_FREQ;
use super::consts::SPL_FIELD_REPORTED;
use super::consts::SPL_FIELD_TONE_AMP;
use super::misc::update_channel_mappings_for_config;
use crate::app::App;
use crate::app::RecordingField;
use sotf_audio_player::recording_types::CtcMatrixExportStrategy;

/// Nudge the currently selected SPL form-field by `step` (1 == "one
/// natural unit"). Numeric ranges match the engine's input validation.
pub(super) fn adjust_spl_field(app: &mut App, step: f32) {
    let cal = &mut app.recording.model.spl_calibration_capture;
    match app.recording.spl_selected_field {
        SPL_FIELD_REF_FREQ => {
            cal.reference_freq_hz = (cal.reference_freq_hz + 100.0 * step).clamp(20.0, 20_000.0);
        }
        SPL_FIELD_TONE_AMP => {
            cal.tone_amp = (cal.tone_amp + 0.05 * step).clamp(0.001, 1.0);
        }
        SPL_FIELD_DURATION => {
            cal.duration_s = (cal.duration_s + 0.5 * step).clamp(0.5, 30.0);
        }
        SPL_FIELD_OUT_CH => {
            if step > 0.0 {
                cal.output_channel = cal.output_channel.saturating_add(1);
            } else {
                cal.output_channel = cal.output_channel.saturating_sub(1);
            }
        }
        SPL_FIELD_IN_CH => {
            if step > 0.0 {
                cal.input_channel = cal.input_channel.saturating_add(1);
            } else {
                cal.input_channel = cal.input_channel.saturating_sub(1);
            }
        }
        SPL_FIELD_REPORTED => {
            let cur = cal.reported_db_spl.unwrap_or(80.0);
            cal.reported_db_spl = Some(cur + step);
        }
        _ => {}
    }
}

pub(super) fn adjust_recording_field(app: &mut App, delta: i32) {
    use crate::app::recording_field_at;
    use sotf_audio_player::recording_types::{RecordingSignalType, SpeakerConfiguration};

    let Some(field) = recording_field_at(&app.recording, app.recording.selected_field) else {
        return;
    };
    use RecordingField::*;
    match field {
        PlaybackDevice => {
            // B1: Cycle playback device and populate config
            if !app.recording.available_playback_devices.is_empty() {
                let len = app.recording.available_playback_devices.len();
                app.recording.selected_playback_idx = if delta > 0 {
                    (app.recording.selected_playback_idx + 1) % len
                } else {
                    (app.recording.selected_playback_idx + len - 1) % len
                };
                let (id, name) = app.recording.available_playback_devices
                    [app.recording.selected_playback_idx]
                    .clone();
                app.recording.model.playback_config.device_name = name;
                app.recording.model.playback_config.device_id = id;
            }
        }
        RecordingDevice => {
            // B1: Cycle recording device and populate config
            if !app.recording.available_recording_devices.is_empty() {
                let len = app.recording.available_recording_devices.len();
                app.recording.selected_recording_idx = if delta > 0 {
                    (app.recording.selected_recording_idx + 1) % len
                } else {
                    (app.recording.selected_recording_idx + len - 1) % len
                };
                let (id, name) = app.recording.available_recording_devices
                    [app.recording.selected_recording_idx]
                    .clone();
                app.recording.model.recording_config.device_name = name;
                app.recording.model.recording_config.device_id = id;
            }
        }
        SpeakerConfig => {
            let configs = SpeakerConfiguration::all();
            let idx = configs
                .iter()
                .position(|c| *c == app.recording.model.playback_config.speaker_configuration)
                .unwrap_or(0);
            let new_idx = if delta > 0 {
                (idx + 1) % configs.len()
            } else {
                (idx + configs.len() - 1) % configs.len()
            };
            let new_config = configs[new_idx];
            app.recording.model.playback_config.speaker_configuration = new_config;
            update_channel_mappings_for_config(app, new_config);
        }
        SignalType => {
            let types = RecordingSignalType::all();
            let idx = types
                .iter()
                .position(|t| *t == app.recording.model.signal_type)
                .unwrap_or(0);
            let new_idx = if delta > 0 {
                (idx + 1) % types.len()
            } else {
                (idx + types.len() - 1) % types.len()
            };
            app.recording.model.signal_type = types[new_idx];
        }
        Duration => {
            app.recording.model.signal_duration_secs =
                (app.recording.model.signal_duration_secs + delta as f32).clamp(1.0, 30.0);
        }
        Level => {
            app.recording.model.signal_level_db =
                (app.recording.model.signal_level_db + delta as f32).clamp(-40.0, 0.0);
        }
        SweepStart => {
            app.recording.model.sweep_start_freq =
                (app.recording.model.sweep_start_freq + delta as f32 * 10.0).clamp(10.0, 1000.0);
        }
        SweepEnd => {
            app.recording.model.sweep_end_freq =
                (app.recording.model.sweep_end_freq + delta as f32 * 1000.0).clamp(1000.0, 24000.0);
        }
        NumRecordingChannels => {
            let cur = app.recording.model.recording_config.num_channels as i32;
            let next = (cur + delta).clamp(1, 128) as usize;
            app.recording.model.recording_config.num_channels = next;
            app.recording.sync_recording_channel_vecs();
            let last = crate::app::recording_field_count(&app.recording) - 1;
            if app.recording.selected_field > last {
                app.recording.selected_field = last;
            }
        }
        CtcStrategy => {
            app.recording.model.recording_config.ctc_matrix_strategy =
                match app.recording.model.recording_config.ctc_matrix_strategy {
                    CtcMatrixExportStrategy::ImpulseResponse => CtcMatrixExportStrategy::RawSweep,
                    CtcMatrixExportStrategy::RawSweep => CtcMatrixExportStrategy::ImpulseResponse,
                };
        }
        CtcLoopbackInput => {
            let cur = app
                .recording
                .model
                .recording_config
                .ctc_loopback_input_channel
                .unwrap_or(0) as i32;
            app.recording
                .model
                .recording_config
                .ctc_loopback_input_channel = Some((cur + delta).clamp(0, 127) as usize);
        }
        ChannelInput(i) => {
            if let Some(slot) = app
                .recording
                .model
                .recording_config
                .channel_mappings
                .get_mut(i)
            {
                let cur = *slot as i32;
                let next = (cur + delta).clamp(0, 127) as usize;
                *slot = next;
            }
        }
        OutputDir | MicCal(_) => {
            // Path fields ignore Left/Right adjust; user must Enter to edit.
        }
    }
}
