//! Recording wizard event handlers

use super::PlayerCommand;
use crate::app::{App, FilePickerMode, FilePickerOrigin, InputMode};
use crossterm::event::{KeyCode, KeyEvent};
use sotf_audio_player::recording_types::CtcMatrixExportStrategy;
use std::sync::{Arc, Mutex};

fn recording_step_prev_wrap(
    s: sotf_audio_player::recording_types::RecordingStep,
) -> sotf_audio_player::recording_types::RecordingStep {
    use sotf_audio_player::recording_types::RecordingStep;
    match s {
        RecordingStep::Config => RecordingStep::Saving,
        RecordingStep::SplCalibration => RecordingStep::Config,
        RecordingStep::Capture => RecordingStep::SplCalibration,
        RecordingStep::Probe => RecordingStep::Capture,
        RecordingStep::BassAnchor => RecordingStep::Probe,
        RecordingStep::Evaluating => RecordingStep::BassAnchor,
        RecordingStep::Saving => RecordingStep::Evaluating,
    }
}

fn recording_step_next_wrap(
    s: sotf_audio_player::recording_types::RecordingStep,
) -> sotf_audio_player::recording_types::RecordingStep {
    use sotf_audio_player::recording_types::RecordingStep;
    match s {
        RecordingStep::Config => RecordingStep::SplCalibration,
        RecordingStep::SplCalibration => RecordingStep::Capture,
        RecordingStep::Capture => RecordingStep::Probe,
        RecordingStep::Probe => RecordingStep::BassAnchor,
        RecordingStep::BassAnchor => RecordingStep::Evaluating,
        RecordingStep::Evaluating => RecordingStep::Saving,
        RecordingStep::Saving => RecordingStep::Config,
    }
}

pub fn handle_recording_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use sotf_audio_player::recording_types::{ChannelRecordingState, RecordingStep};

    // Esc: exit editing if active, then two-level focus (content → step tab → configure tab)
    if key.code == KeyCode::Esc {
        // First dismiss numerical direct-edit mode
        if app.recording.editing_value {
            app.recording.editing_value = false;
            app.recording.edit_buffer.clear();
            return None;
        }
        // Then dismiss text editing
        if app.recording.editing_output_dir {
            app.recording.editing_output_dir = false;
            app.clear_autocomplete();
            return None;
        }
        if app.recording.editing_mic_cal_channel.is_some() {
            app.recording.editing_mic_cal_channel = None;
            app.clear_autocomplete();
            return None;
        }
        if app.recording.editing_save_name {
            app.recording.editing_save_name = false;
            return None;
        }
        // Two-level: content → step tab bar → configure tab bar
        if app.recording.step_tab_focused {
            app.recording.step_tab_focused = false;
            app.input_mode = InputMode::Configure;
        } else {
            app.recording.step_tab_focused = true;
        }
        return None;
    }

    // When the step tab bar has focus, Left/Right change step, Up goes to
    // the top-level configure tab bar, Down/Enter returns to step content.
    if app.recording.step_tab_focused {
        match key.code {
            KeyCode::Left | KeyCode::BackTab => {
                app.recording.step = recording_step_prev_wrap(app.recording.step);
                return None;
            }
            KeyCode::Right | KeyCode::Tab => {
                let next = recording_step_next_wrap(app.recording.step);
                // Guard: entering Capture requires an output directory
                if app.recording.step == RecordingStep::Config && next == RecordingStep::Capture {
                    if app.recording.output_directory.is_empty() {
                        app.recording.status_message = "Set an output directory first".to_string();
                        return None;
                    }
                    init_recording_channels(app);
                }
                app.recording.step = next;
                return None;
            }
            KeyCode::Up => {
                app.recording.step_tab_focused = false;
                app.input_mode = InputMode::Configure;
                return None;
            }
            KeyCode::Down | KeyCode::Enter => {
                app.recording.step_tab_focused = false;
                return None;
            }
            _ => return None,
        }
    }

    match app.recording.step {
        RecordingStep::Config => {
            if app.recording.editing_output_dir {
                match key.code {
                    KeyCode::Enter => {
                        app.recording.editing_output_dir = false;
                        app.clear_autocomplete();
                    }
                    KeyCode::Tab => {
                        app.zsh_tab_complete(
                            crate::app::app_autocomplete::get_recording_output_dir,
                            crate::app::app_autocomplete::set_recording_output_dir,
                            crate::app::app_autocomplete::AutocompleteKind::FilePath,
                        );
                    }
                    KeyCode::BackTab => {
                        app.zsh_backtab_complete(
                            crate::app::app_autocomplete::set_recording_output_dir,
                        );
                    }
                    KeyCode::Down => {
                        app.autocomplete_down(
                            crate::app::app_autocomplete::set_recording_output_dir,
                        );
                    }
                    KeyCode::Up => {
                        app.autocomplete_up(crate::app::app_autocomplete::set_recording_output_dir);
                    }
                    KeyCode::Backspace => {
                        app.recording.output_directory.pop();
                        app.refresh_autocomplete_inline(
                            crate::app::app_autocomplete::get_recording_output_dir,
                            crate::app::app_autocomplete::AutocompleteKind::FilePath,
                        );
                    }
                    KeyCode::F(2) => {
                        let start = app.recording.output_directory.clone();
                        app.open_file_explorer(
                            FilePickerOrigin::RecordingOutputDir,
                            FilePickerMode::Directory,
                            "Select Output Directory",
                            Some(&start),
                            None,
                        );
                    }
                    KeyCode::Char(c) => {
                        app.recording.output_directory.push(c);
                        app.refresh_autocomplete_inline(
                            crate::app::app_autocomplete::get_recording_output_dir,
                            crate::app::app_autocomplete::AutocompleteKind::FilePath,
                        );
                    }
                    _ => {}
                }
                return None;
            }
            if app.recording.editing_mic_cal_channel.is_some() {
                match key.code {
                    KeyCode::Enter => {
                        app.recording.editing_mic_cal_channel = None;
                        app.clear_autocomplete();
                    }
                    KeyCode::Tab => {
                        app.zsh_tab_complete(
                            crate::app::app_autocomplete::get_recording_mic_cal_path,
                            crate::app::app_autocomplete::set_recording_mic_cal_path,
                            crate::app::app_autocomplete::AutocompleteKind::FilePath,
                        );
                    }
                    KeyCode::BackTab => {
                        app.zsh_backtab_complete(
                            crate::app::app_autocomplete::set_recording_mic_cal_path,
                        );
                    }
                    KeyCode::Down => {
                        app.autocomplete_down(
                            crate::app::app_autocomplete::set_recording_mic_cal_path,
                        );
                    }
                    KeyCode::Up => {
                        app.autocomplete_up(
                            crate::app::app_autocomplete::set_recording_mic_cal_path,
                        );
                    }
                    KeyCode::Backspace => {
                        if let Some(s) = app.recording.active_mic_cal_path_mut() {
                            s.pop();
                        }
                        app.refresh_autocomplete_inline(
                            crate::app::app_autocomplete::get_recording_mic_cal_path,
                            crate::app::app_autocomplete::AutocompleteKind::FilePath,
                        );
                    }
                    KeyCode::F(2) => {
                        let start = app.recording.active_mic_cal_path().to_string();
                        app.open_file_explorer(
                            FilePickerOrigin::RecordingMicCalibration,
                            FilePickerMode::File,
                            "Select Mic Calibration File",
                            Some(&start),
                            None,
                        );
                    }
                    KeyCode::Char(c) => {
                        if let Some(s) = app.recording.active_mic_cal_path_mut() {
                            s.push(c);
                        }
                        app.refresh_autocomplete_inline(
                            crate::app::app_autocomplete::get_recording_mic_cal_path,
                            crate::app::app_autocomplete::AutocompleteKind::FilePath,
                        );
                    }
                    _ => {}
                }
                return None;
            }
            // Numerical direct-edit mode
            if app.recording.editing_value {
                match key.code {
                    KeyCode::Enter => {
                        set_recording_field_from_string(app);
                        app.recording.editing_value = false;
                        app.recording.edit_buffer.clear();
                    }
                    KeyCode::Esc => {
                        app.recording.editing_value = false;
                        app.recording.edit_buffer.clear();
                    }
                    KeyCode::Backspace => {
                        app.recording.edit_buffer.pop();
                    }
                    KeyCode::Char(c) if c.is_ascii_digit() || c == '.' || c == '-' => {
                        app.recording.edit_buffer.push(c);
                    }
                    _ => {}
                }
                return None;
            }

            use crate::app::{RecordingField, recording_field_at, recording_field_count};
            let total_fields = recording_field_count(&app.recording);
            match key.code {
                KeyCode::Up => {
                    if app.recording.selected_field == 0 {
                        app.recording.step_tab_focused = true;
                    } else {
                        app.recording.selected_field -= 1;
                    }
                }
                KeyCode::Down if app.recording.selected_field + 1 < total_fields => {
                    app.recording.selected_field += 1;
                }
                KeyCode::Enter => {
                    match recording_field_at(&app.recording, app.recording.selected_field) {
                        Some(RecordingField::OutputDir) => {
                            app.recording.editing_output_dir = true;
                        }
                        Some(RecordingField::MicCal(ch)) => {
                            // Make sure the channel slot exists so the
                            // autocomplete getter can read it as `&str`.
                            app.recording.sync_recording_channel_vecs();
                            app.recording.editing_mic_cal_channel = Some(ch);
                        }
                        Some(field) if is_recording_field_numerical_kind(&field) => {
                            app.recording.edit_buffer =
                                recording_field_value_string_kind(app, &field);
                            app.recording.editing_value = true;
                        }
                        _ => {}
                    }
                }
                KeyCode::Left | KeyCode::Char('-') => {
                    adjust_recording_field(app, -1);
                }
                KeyCode::Right | KeyCode::Char('+') => {
                    adjust_recording_field(app, 1);
                }
                KeyCode::Char('a') | KeyCode::Char('A')
                    if matches!(
                        recording_field_at(&app.recording, app.recording.selected_field),
                        Some(RecordingField::SpeakerConfig)
                    ) && app.recording.playback_config.speaker_configuration
                        == sotf_audio_player::recording_types::SpeakerConfiguration::Custom =>
                {
                    add_custom_speaker(app);
                }
                KeyCode::Char('x') | KeyCode::Char('X')
                    if matches!(
                        recording_field_at(&app.recording, app.recording.selected_field),
                        Some(RecordingField::SpeakerConfig)
                    ) && app.recording.playback_config.speaker_configuration
                        == sotf_audio_player::recording_types::SpeakerConfiguration::Custom =>
                {
                    remove_custom_speaker(app);
                }
                KeyCode::Tab => {
                    if app.recording.selected_field + 1 < total_fields {
                        app.recording.selected_field += 1;
                    } else {
                        app.recording.selected_field = 0;
                    }
                }
                _ => {}
            }
            None
        }

        RecordingStep::Capture => match key.code {
            KeyCode::Up => {
                match app.recording.current_channel {
                    Some(ch) if ch > 0 => {
                        app.recording.current_channel = Some(ch - 1);
                    }
                    _ => {
                        app.recording.step_tab_focused = true;
                    }
                }
                None
            }
            KeyCode::Down => {
                if let Some(ch) = app.recording.current_channel {
                    if ch + 1 < app.recording.channel_recordings.len() {
                        app.recording.current_channel = Some(ch + 1);
                    }
                } else if !app.recording.channel_recordings.is_empty() {
                    app.recording.current_channel = Some(0);
                }
                None
            }
            KeyCode::Enter => {
                // B2: Record current channel via engine
                if let Some(ch_idx) = app.recording.current_channel {
                    let can_record = app
                        .recording
                        .channel_recordings
                        .get(ch_idx)
                        .map(|ch| {
                            ch.state == ChannelRecordingState::Empty
                                || ch.state == ChannelRecordingState::Error
                        })
                        .unwrap_or(false);
                    if can_record {
                        start_recording_channel(app, ch_idx);
                    }
                }
                None
            }
            KeyCode::BackTab => {
                app.recording.step = RecordingStep::Config;
                None
            }
            _ => None,
        },

        RecordingStep::Evaluating => match key.code {
            KeyCode::Up => {
                if app.recording.selected_channel_view > 0 {
                    app.recording.selected_channel_view -= 1;
                } else {
                    app.recording.step_tab_focused = true;
                }
                None
            }
            KeyCode::Down => {
                let completed = app
                    .recording
                    .channel_recordings
                    .iter()
                    .filter(|ch| ch.state == ChannelRecordingState::Done)
                    .count();
                if app.recording.selected_channel_view + 1 < completed {
                    app.recording.selected_channel_view += 1;
                }
                None
            }
            KeyCode::BackTab => {
                app.recording.step = RecordingStep::Capture;
                None
            }
            _ => None,
        },

        RecordingStep::Probe => {
            handle_probe_step_keys(app, key);
            None
        }

        RecordingStep::BassAnchor => {
            handle_bass_anchor_step_keys(app, key);
            None
        }

        RecordingStep::SplCalibration => {
            handle_spl_calibration_step_keys(app, key);
            None
        }

        RecordingStep::Saving => {
            handle_saving_step_keys(app, key);
            None
        }
    }
}

/// Key handler for the Probe step of the Recording wizard.
///
/// Cursor fields:
///   0  probe duration (ms)
///   1  silence gap (ms)
///   2  mic input channel
///   3  Run button
///
/// `Tab` / `↓` advance; `Shift-Tab` / `↑` retreat. `Enter` on a numeric
/// field begins editing; `Enter` on Run triggers the capture. `+` / `-`
/// or `←` / `→` nudge numeric values. `r` starts the capture from any
/// focused field. `Ctrl+S` never applies here — saving lives on the
/// Save step.
fn handle_probe_step_keys(app: &mut App, key: KeyEvent) {
    use sotf_audio_player::recording_types::ProbeCaptureStatus;

    const FIELD_COUNT: usize = 4;
    const FIELD_PROBE_MS: usize = 0;
    const FIELD_SILENCE_MS: usize = 1;
    const FIELD_MIC_CHANNEL: usize = 2;
    const FIELD_RUN: usize = 3;

    if app.recording.probe_editing_value {
        match key.code {
            KeyCode::Esc => {
                app.recording.probe_editing_value = false;
                app.recording.edit_buffer.clear();
            }
            KeyCode::Enter => {
                let v = app.recording.edit_buffer.trim().parse::<f32>();
                match app.recording.probe_selected_field {
                    FIELD_PROBE_MS => {
                        if let Ok(v) = v {
                            app.recording.probe_capture.probe_duration_ms = v.clamp(100.0, 5000.0);
                        }
                    }
                    FIELD_SILENCE_MS => {
                        if let Ok(v) = v {
                            app.recording.probe_capture.silence_duration_ms =
                                v.clamp(100.0, 5000.0);
                        }
                    }
                    FIELD_MIC_CHANNEL => {
                        if let Ok(v) = v {
                            app.recording.probe_capture.input_channel = v.max(0.0) as u16;
                        }
                    }
                    _ => {}
                }
                app.recording.probe_editing_value = false;
                app.recording.edit_buffer.clear();
            }
            KeyCode::Char(c) if c.is_ascii_digit() || c == '.' || c == '-' => {
                app.recording.edit_buffer.push(c);
            }
            KeyCode::Backspace => {
                app.recording.edit_buffer.pop();
            }
            _ => {}
        }
        return;
    }

    let running = matches!(
        app.recording.probe_capture.status,
        ProbeCaptureStatus::Running { .. }
    );

    match key.code {
        KeyCode::Tab | KeyCode::Down => {
            app.recording.probe_selected_field =
                (app.recording.probe_selected_field + 1) % FIELD_COUNT;
        }
        KeyCode::BackTab | KeyCode::Up => {
            app.recording.probe_selected_field =
                (app.recording.probe_selected_field + FIELD_COUNT - 1) % FIELD_COUNT;
        }
        KeyCode::Char('+') | KeyCode::Right => match app.recording.probe_selected_field {
            FIELD_PROBE_MS => {
                let v = (app.recording.probe_capture.probe_duration_ms + 100.0).min(5000.0);
                app.recording.probe_capture.probe_duration_ms = v;
            }
            FIELD_SILENCE_MS => {
                let v = (app.recording.probe_capture.silence_duration_ms + 100.0).min(5000.0);
                app.recording.probe_capture.silence_duration_ms = v;
            }
            FIELD_MIC_CHANNEL => {
                app.recording.probe_capture.input_channel =
                    app.recording.probe_capture.input_channel.saturating_add(1);
            }
            _ => {}
        },
        KeyCode::Char('-') | KeyCode::Left => match app.recording.probe_selected_field {
            FIELD_PROBE_MS => {
                let v = (app.recording.probe_capture.probe_duration_ms - 100.0).max(100.0);
                app.recording.probe_capture.probe_duration_ms = v;
            }
            FIELD_SILENCE_MS => {
                let v = (app.recording.probe_capture.silence_duration_ms - 100.0).max(100.0);
                app.recording.probe_capture.silence_duration_ms = v;
            }
            FIELD_MIC_CHANNEL => {
                app.recording.probe_capture.input_channel =
                    app.recording.probe_capture.input_channel.saturating_sub(1);
            }
            _ => {}
        },
        KeyCode::Enter => {
            if app.recording.probe_selected_field == FIELD_RUN && !running {
                spawn_probe_capture(app);
            } else if app.recording.probe_selected_field != FIELD_RUN {
                app.recording.probe_editing_value = true;
                app.recording.edit_buffer.clear();
            }
        }
        KeyCode::Char('r') if !running => {
            spawn_probe_capture(app);
        }
        _ => {}
    }
}

/// Kick off the tone-burst probe capture on a background thread.
///
/// The shared-slot pattern mirrors `spawn_room_eq_optimization`
/// (`OnceLock` + `Arc<Mutex>`), drained by [`poll_probe_capture`] on every
/// main loop tick.
#[allow(clippy::type_complexity)]
static PROBE_CAPTURE_RESULT: std::sync::OnceLock<
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

fn spawn_probe_capture(app: &mut App) {
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

/// Drain the probe-capture slot into `app.recording.probe_capture`.
/// Returns `true` if state changed and the UI should redraw.
pub fn poll_probe_capture(app: &mut App) -> bool {
    use sotf_audio_player::recording_types::ProbeCaptureStatus;

    if !matches!(
        app.recording.probe_capture.status,
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
                .probe_capture
                .apply_results(results, Some(wav_path));
        }
        Err(e) => {
            app.recording.probe_capture.status = ProbeCaptureStatus::Failed(e);
        }
    }
    true
}

/// Wall-clock millis since the Unix epoch — used by the Probe step's
/// `started_at_ms` so the status banner can render an elapsed-time
/// progress estimate.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// SPL Calibration step (GD-Opt v2 Phase GD-1e.5)
// ---------------------------------------------------------------------------
//
// Form-field cursor semantics — kept in lockstep with
// `draw_recording_spl_calibration_step` in `ui/draw_configure.rs`:
//
//   0  Reference frequency (Hz)
//   1  Tone amplitude (0..1)
//   2  Duration (s)
//   3  Output channel
//   4  Mic input channel
//   5  Run / Cancel pseudo-button
//   6  Reported dBSPL meter reading
//
// Mirrors the GPUI surface: pressing `r` (or Enter on the Run row)
// kicks off the capture; while running, the same key cancels via the
// shared `spl_cancel_requested` flag the engine polls every frame.

const SPL_FIELD_REF_FREQ: usize = 0;
const SPL_FIELD_TONE_AMP: usize = 1;
const SPL_FIELD_DURATION: usize = 2;
const SPL_FIELD_OUT_CH: usize = 3;
const SPL_FIELD_IN_CH: usize = 4;
const SPL_FIELD_RUN: usize = 5;
const SPL_FIELD_REPORTED: usize = 6;
const SPL_FIELD_COUNT: usize = 7;

fn handle_spl_calibration_step_keys(app: &mut App, key: KeyEvent) {
    use sotf_audio_player::recording_types::SplCalibrationCaptureStatus;

    if app.recording.spl_editing_value {
        match key.code {
            KeyCode::Esc => {
                app.recording.spl_editing_value = false;
                app.recording.edit_buffer.clear();
            }
            KeyCode::Enter => {
                let parsed = app.recording.edit_buffer.trim().parse::<f32>();
                let cal = &mut app.recording.spl_calibration_capture;
                match app.recording.spl_selected_field {
                    SPL_FIELD_REF_FREQ => {
                        if let Ok(v) = parsed
                            && v.is_finite()
                            && v > 0.0
                        {
                            cal.reference_freq_hz = v.clamp(20.0, 20_000.0);
                        }
                    }
                    SPL_FIELD_TONE_AMP => {
                        if let Ok(v) = parsed
                            && v.is_finite()
                        {
                            cal.tone_amp = v.clamp(0.001, 1.0);
                        }
                    }
                    SPL_FIELD_DURATION => {
                        if let Ok(v) = parsed
                            && v.is_finite()
                        {
                            cal.duration_s = v.clamp(0.5, 30.0);
                        }
                    }
                    SPL_FIELD_OUT_CH => {
                        if let Ok(v) = parsed
                            && v.is_finite()
                            && v >= 0.0
                        {
                            cal.output_channel = v as u16;
                        }
                    }
                    SPL_FIELD_IN_CH => {
                        if let Ok(v) = parsed
                            && v.is_finite()
                            && v >= 0.0
                        {
                            cal.input_channel = v as u16;
                        }
                    }
                    SPL_FIELD_REPORTED => {
                        if let Ok(v) = parsed
                            && v.is_finite()
                        {
                            cal.reported_db_spl = Some(v);
                        }
                    }
                    _ => {}
                }
                app.recording.spl_editing_value = false;
                app.recording.edit_buffer.clear();
            }
            KeyCode::Char(c) if c.is_ascii_digit() || c == '.' || c == '-' => {
                app.recording.edit_buffer.push(c);
            }
            KeyCode::Backspace => {
                app.recording.edit_buffer.pop();
            }
            _ => {}
        }
        return;
    }

    let running = matches!(
        app.recording.spl_calibration_capture.status,
        SplCalibrationCaptureStatus::Running { .. }
    );

    match key.code {
        KeyCode::Tab | KeyCode::Down => {
            app.recording.spl_selected_field =
                (app.recording.spl_selected_field + 1) % SPL_FIELD_COUNT;
        }
        KeyCode::BackTab | KeyCode::Up => {
            app.recording.spl_selected_field =
                (app.recording.spl_selected_field + SPL_FIELD_COUNT - 1) % SPL_FIELD_COUNT;
        }
        KeyCode::Char('+') | KeyCode::Right => {
            adjust_spl_field(app, 1.0);
        }
        KeyCode::Char('-') | KeyCode::Left => {
            adjust_spl_field(app, -1.0);
        }
        KeyCode::Enter => {
            if app.recording.spl_selected_field == SPL_FIELD_RUN {
                if running {
                    request_spl_cancel(app);
                } else {
                    spawn_spl_calibration_capture(app);
                }
            } else {
                app.recording.spl_editing_value = true;
                app.recording.edit_buffer.clear();
            }
        }
        KeyCode::Char('r') => {
            if running {
                request_spl_cancel(app);
            } else {
                spawn_spl_calibration_capture(app);
            }
        }
        _ => {}
    }
}

/// Nudge the currently selected SPL form-field by `step` (1 == "one
/// natural unit"). Numeric ranges match the engine's input validation.
fn adjust_spl_field(app: &mut App, step: f32) {
    let cal = &mut app.recording.spl_calibration_capture;
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

fn request_spl_cancel(app: &mut App) {
    log::info!("Cancel requested for SPL calibration capture");
    app.recording
        .spl_cancel_requested
        .store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Background slot for the SPL calibration capture.
#[allow(clippy::type_complexity)]
static SPL_CAPTURE_RESULT: std::sync::OnceLock<
    Arc<Mutex<Option<Result<sotf_audio::signal_recorder::SplCalibrationResult, String>>>>,
> = std::sync::OnceLock::new();

/// Spawn the SPL calibration capture on a background thread (mirrors
/// `spawn_probe_capture`).
fn spawn_spl_calibration_capture(app: &mut App) {
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

/// Drain the SPL calibration capture slot. Returns `true` if state
/// changed and the UI should redraw.
pub fn poll_spl_calibration_capture(app: &mut App) -> bool {
    use sotf_audio_player::recording_types::SplCalibrationCaptureStatus;

    if !matches!(
        app.recording.spl_calibration_capture.status,
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
    let cal = &mut app.recording.spl_calibration_capture;
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

// ---------------------------------------------------------------------------
// Bass Anchor step (GD-Opt v2 Phase GD-1e)
// ---------------------------------------------------------------------------

#[allow(clippy::type_complexity)]
static BASS_ANCHOR_CAPTURE_RESULT: std::sync::OnceLock<
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

fn handle_bass_anchor_step_keys(app: &mut App, key: KeyEvent) {
    use sotf_audio_player::recording_types::BassAnchorCaptureStatus;

    let running = matches!(
        app.recording.bass_anchor_capture.status,
        BassAnchorCaptureStatus::Running { .. }
    );

    match key.code {
        KeyCode::Enter | KeyCode::Char('r') => {
            if running {
                // No cancel flag wired in the TUI yet — just leave it
                // running; the engine call will return when it finishes.
            } else {
                spawn_bass_anchor_capture(app);
            }
        }
        _ => {}
    }
}

fn spawn_bass_anchor_capture(app: &mut App) {
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

/// Drain the bass-anchor capture slot. Returns `true` if state changed
/// and the UI should redraw.
pub fn poll_bass_anchor_capture(app: &mut App) -> bool {
    use sotf_audio_player::recording_types::BassAnchorCaptureStatus;

    if !matches!(
        app.recording.bass_anchor_capture.status,
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
                .bass_anchor_capture
                .apply_results(results, Some(wav_path));
        }
        Err(e) => {
            app.recording.bass_anchor_capture.status = BassAnchorCaptureStatus::Failed(e);
        }
    }
    true
}

/// Key handler for the Save step of the Recording wizard.
///
/// Field cursor semantics match `draw_recording_saving_step`:
///   0        session name (text)
///   1..=3    room width / depth / height (numeric)
///   4        unit toggle (Metric / Imperial)
///   5        setup description (text)
///   6..      per-channel speaker entries (text, autocomplete-backed)
///
/// Tab / ↓ advance; Shift-Tab / ↑ retreat. Enter starts editing the
/// current field (or toggles the unit); Esc cancels an in-flight edit.
/// `Ctrl+S` saves from any state.
fn handle_saving_step_keys(app: &mut App, key: KeyEvent) {
    use super::conf_spinoramaeq::ensure_spinorama_speakers_loading;
    use crossterm::event::KeyModifiers;
    use sotf_audio_player::recording_types::RecordingStep;

    // Pre-warm the spinorama speaker catalog so the per-channel
    // autocomplete has something to show. Idempotent — returns
    // immediately once the fetch is in flight or cached.
    ensure_spinorama_speakers_loading(app);

    // Ctrl+S saves regardless of mode.
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S'))
    {
        app.recording.editing_save_value = false;
        app.recording.edit_buffer.clear();
        save_recordings(app);
        return;
    }

    // Keep the speaker vec in sync with the channel list so the
    // cursor never indexes a short vec.
    app.recording.sync_channel_speakers_length();
    let field_count = app.recording.save_field_count();

    // --- Edit-in-progress sub-mode ---------------------------------
    if app.recording.editing_save_value {
        match key.code {
            KeyCode::Esc => {
                app.recording.editing_save_value = false;
                app.recording.edit_buffer.clear();
            }
            KeyCode::Enter => commit_save_field_edit(app),
            KeyCode::Backspace => {
                app.recording.edit_buffer.pop();
            }
            KeyCode::Char(c) => {
                app.recording.edit_buffer.push(c);
            }
            _ => {}
        }
        return;
    }

    // --- Navigation / activation ------------------------------------
    match key.code {
        KeyCode::Up => {
            if app.recording.selected_save_field == 0 {
                app.recording.step_tab_focused = true;
            } else {
                app.recording.selected_save_field -= 1;
            }
        }
        KeyCode::Down | KeyCode::Tab => {
            app.recording.selected_save_field =
                (app.recording.selected_save_field + 1) % field_count.max(1);
        }
        KeyCode::BackTab => {
            app.recording.step = RecordingStep::Evaluating;
        }
        KeyCode::Enter => {
            if app.recording.selected_save_field == 4 {
                // Unit toggle is a pure cycle — no edit mode needed.
                app.recording.save_room_unit = app.recording.save_room_unit.toggled();
            } else {
                app.recording.editing_save_value = true;
                app.recording.edit_buffer = current_save_field_value(app);
            }
        }
        KeyCode::Char('u') | KeyCode::Char('U') => {
            // Quick keyboard shortcut for the unit toggle from any
            // field — saves the user tabbing to field 4.
            app.recording.save_room_unit = app.recording.save_room_unit.toggled();
        }
        _ => {}
    }
}

/// Snapshot the current field's display value into `edit_buffer` so
/// editing an existing value doesn't start from an empty buffer.
fn current_save_field_value(app: &App) -> String {
    match app.recording.selected_save_field {
        0 => app.recording.save_name.clone(),
        1 => fmt_opt_dim(app.recording.save_room_width),
        2 => fmt_opt_dim(app.recording.save_room_depth),
        3 => fmt_opt_dim(app.recording.save_room_height),
        5 => app.recording.setup_description.clone(),
        n if n >= 6 => {
            let row = n - 6;
            app.recording
                .channel_speakers
                .get(row)
                .cloned()
                .unwrap_or_default()
        }
        _ => String::new(),
    }
}

fn fmt_opt_dim(v: f64) -> String {
    if v > 0.0 {
        format!("{:.2}", v)
    } else {
        String::new()
    }
}

/// Commit the `edit_buffer` into the currently-selected field.
fn commit_save_field_edit(app: &mut App) {
    let buf = app.recording.edit_buffer.clone();
    match app.recording.selected_save_field {
        0 => app.recording.save_name = buf,
        1 => {
            app.recording.save_room_width = buf.trim().parse::<f64>().unwrap_or(0.0).max(0.0);
        }
        2 => {
            app.recording.save_room_depth = buf.trim().parse::<f64>().unwrap_or(0.0).max(0.0);
        }
        3 => {
            app.recording.save_room_height = buf.trim().parse::<f64>().unwrap_or(0.0).max(0.0);
        }
        5 => app.recording.setup_description = buf,
        n if n >= 6 => {
            let row = n - 6;
            app.recording.sync_channel_speakers_length();
            if let Some(slot) = app.recording.channel_speakers.get_mut(row) {
                *slot = buf;
            }
        }
        _ => {}
    }
    app.recording.editing_save_value = false;
    app.recording.edit_buffer.clear();
}

use crate::app::RecordingField;

/// True for fields whose value is edited as a typed number string.
pub(crate) fn is_recording_field_numerical_kind(field: &RecordingField) -> bool {
    use RecordingField::*;
    matches!(
        field,
        Duration
            | Level
            | SweepStart
            | SweepEnd
            | NumRecordingChannels
            | CtcLoopbackInput
            | ChannelInput(_)
    )
}

pub(crate) fn recording_field_value_string_kind(app: &App, field: &RecordingField) -> String {
    use RecordingField::*;
    match field {
        Duration => format!("{:.1}", app.recording.signal_duration_secs),
        Level => format!("{:.1}", app.recording.signal_level_db),
        SweepStart => format!("{:.0}", app.recording.sweep_start_freq),
        SweepEnd => format!("{:.0}", app.recording.sweep_end_freq),
        NumRecordingChannels => app.recording.recording_config.num_channels.to_string(),
        CtcLoopbackInput => app
            .recording
            .recording_config
            .ctc_loopback_input_channel
            .map(|c| (c + 1).to_string())
            .unwrap_or_else(|| "1".to_string()),
        ChannelInput(i) => app
            .recording
            .recording_config
            .channel_mappings
            .get(*i)
            .map(|c| (c + 1).to_string())
            .unwrap_or_else(|| "1".to_string()),
        _ => String::new(),
    }
}

fn set_recording_field_from_string(app: &mut App) {
    use crate::app::recording_field_at;
    let buf = app.recording.edit_buffer.clone();
    let Some(field) = recording_field_at(&app.recording, app.recording.selected_field) else {
        return;
    };
    use RecordingField::*;
    match field {
        Duration => {
            if let Ok(v) = buf.parse::<f32>() {
                app.recording.signal_duration_secs = v.clamp(1.0, 30.0);
            }
        }
        Level => {
            if let Ok(v) = buf.parse::<f32>() {
                app.recording.signal_level_db = v.clamp(-40.0, 0.0);
            }
        }
        SweepStart => {
            if let Ok(v) = buf.parse::<f32>() {
                app.recording.sweep_start_freq = v.clamp(10.0, 1000.0);
            }
        }
        SweepEnd => {
            if let Ok(v) = buf.parse::<f32>() {
                app.recording.sweep_end_freq = v.clamp(1000.0, 24000.0);
            }
        }
        NumRecordingChannels => {
            if let Ok(v) = buf.parse::<usize>() {
                app.recording.recording_config.num_channels = v.clamp(1, 128);
                app.recording.sync_recording_channel_vecs();
                // Clamp cursor in case the field count just shrank.
                let last = crate::app::recording_field_count(&app.recording) - 1;
                if app.recording.selected_field > last {
                    app.recording.selected_field = last;
                }
            }
        }
        CtcLoopbackInput => {
            if let Ok(v) = buf.parse::<usize>() {
                app.recording.recording_config.ctc_loopback_input_channel =
                    Some(v.saturating_sub(1).min(127));
            }
        }
        ChannelInput(i) => {
            if let Ok(v) = buf.parse::<usize>() {
                let v = v.saturating_sub(1).min(127);
                if let Some(slot) = app.recording.recording_config.channel_mappings.get_mut(i) {
                    *slot = v;
                }
            }
        }
        _ => {}
    }
}

fn adjust_recording_field(app: &mut App, delta: i32) {
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
                app.recording.playback_config.device_name = name;
                app.recording.playback_config.device_id = id;
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
                app.recording.recording_config.device_name = name;
                app.recording.recording_config.device_id = id;
            }
        }
        SpeakerConfig => {
            let configs = SpeakerConfiguration::all();
            let idx = configs
                .iter()
                .position(|c| *c == app.recording.playback_config.speaker_configuration)
                .unwrap_or(0);
            let new_idx = if delta > 0 {
                (idx + 1) % configs.len()
            } else {
                (idx + configs.len() - 1) % configs.len()
            };
            let new_config = configs[new_idx];
            app.recording.playback_config.speaker_configuration = new_config;
            update_channel_mappings_for_config(app, new_config);
        }
        SignalType => {
            let types = RecordingSignalType::all();
            let idx = types
                .iter()
                .position(|t| *t == app.recording.signal_type)
                .unwrap_or(0);
            let new_idx = if delta > 0 {
                (idx + 1) % types.len()
            } else {
                (idx + types.len() - 1) % types.len()
            };
            app.recording.signal_type = types[new_idx];
        }
        Duration => {
            app.recording.signal_duration_secs =
                (app.recording.signal_duration_secs + delta as f32).clamp(1.0, 30.0);
        }
        Level => {
            app.recording.signal_level_db =
                (app.recording.signal_level_db + delta as f32).clamp(-40.0, 0.0);
        }
        SweepStart => {
            app.recording.sweep_start_freq =
                (app.recording.sweep_start_freq + delta as f32 * 10.0).clamp(10.0, 1000.0);
        }
        SweepEnd => {
            app.recording.sweep_end_freq =
                (app.recording.sweep_end_freq + delta as f32 * 1000.0).clamp(1000.0, 24000.0);
        }
        NumRecordingChannels => {
            let cur = app.recording.recording_config.num_channels as i32;
            let next = (cur + delta).clamp(1, 128) as usize;
            app.recording.recording_config.num_channels = next;
            app.recording.sync_recording_channel_vecs();
            let last = crate::app::recording_field_count(&app.recording) - 1;
            if app.recording.selected_field > last {
                app.recording.selected_field = last;
            }
        }
        CtcStrategy => {
            app.recording.recording_config.ctc_matrix_strategy =
                match app.recording.recording_config.ctc_matrix_strategy {
                    CtcMatrixExportStrategy::ImpulseResponse => CtcMatrixExportStrategy::RawSweep,
                    CtcMatrixExportStrategy::RawSweep => CtcMatrixExportStrategy::ImpulseResponse,
                };
        }
        CtcLoopbackInput => {
            let cur = app
                .recording
                .recording_config
                .ctc_loopback_input_channel
                .unwrap_or(0) as i32;
            app.recording.recording_config.ctc_loopback_input_channel =
                Some((cur + delta).clamp(0, 127) as usize);
        }
        ChannelInput(i) => {
            if let Some(slot) = app.recording.recording_config.channel_mappings.get_mut(i) {
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

pub(crate) fn update_channel_mappings_for_config(
    app: &mut App,
    config: sotf_audio_player::recording_types::SpeakerConfiguration,
) {
    use sotf_audio_player::recording_types::{ChannelMapping, SpeakerConfiguration};

    // For Custom, keep the existing channel mappings (renamed to generic Ch labels
    // when transitioning into Custom from a preset) so the user can edit them.
    if config == SpeakerConfiguration::Custom {
        let len = app.recording.playback_config.channel_mappings.len().max(2);
        app.recording.playback_config.channel_mappings = (0..len)
            .map(|i| ChannelMapping::single(i, format!("Ch{}", i + 1)))
            .collect();
        app.recording.playback_config.num_channels = len;
        return;
    }

    let names = config.default_channel_names();
    app.recording.playback_config.channel_mappings = names
        .iter()
        .enumerate()
        .map(|(i, name)| ChannelMapping::single(i, *name))
        .collect();
    app.recording.playback_config.num_channels = names.len();
}

pub(crate) fn add_custom_speaker(app: &mut App) {
    use sotf_audio_player::recording_types::ChannelMapping;
    let cfg = &mut app.recording.playback_config;
    let next_ch = cfg
        .channel_mappings
        .iter()
        .map(|m| m.channel_count())
        .sum::<usize>();
    let idx = cfg.channel_mappings.len() + 1;
    cfg.channel_mappings
        .push(ChannelMapping::single(next_ch, format!("Ch{}", idx)));
    cfg.sync_channel_count();
}

pub(crate) fn remove_custom_speaker(app: &mut App) {
    let cfg = &mut app.recording.playback_config;
    if cfg.channel_mappings.len() > 1 {
        cfg.channel_mappings.pop();
        cfg.sync_channel_count();
    }
}

pub(crate) fn init_recording_channels(app: &mut App) {
    use sotf_audio_player::recording_types::ChannelRecording;

    let num_speakers = app.recording.playback_config.channel_mappings.len();
    let num_mics = app.recording.recording_config.channel_mappings.len().max(1);
    let num_positions = app.recording.recording_config.num_positions.max(1);
    let expected_count = num_speakers * num_mics * num_positions;
    if app.recording.channel_recordings.len() != expected_count {
        let mut recordings = Vec::with_capacity(expected_count);
        for position_idx in 0..num_positions {
            for (speaker_idx, mapping) in app
                .recording
                .playback_config
                .channel_mappings
                .iter()
                .enumerate()
            {
                for mic_idx in 0..num_mics {
                    let mut name = mapping.group_name.clone();
                    if num_positions > 1 && num_mics > 1 {
                        name = format!("{} (Pos {} / Mic {})", name, position_idx + 1, mic_idx + 1);
                    } else if num_positions > 1 {
                        name = format!("{} (Pos {})", name, position_idx + 1);
                    } else if num_mics > 1 {
                        name = format!("{} (Mic {})", name, mic_idx + 1);
                    }
                    recordings.push(ChannelRecording::with_mic_position(
                        speaker_idx,
                        name,
                        mic_idx,
                        position_idx,
                    ));
                }
            }
        }
        app.recording.channel_recordings = recordings;
        app.recording.transfer_matrix_loopbacks.clear();
        app.recording.ctc_reference_sweep_path = None;
        app.recording.current_channel = if expected_count > 0 { Some(0) } else { None };
    }
}

// ---- B2: Actual recording implementation ----

type RecordingResultSlot = Arc<
    Mutex<
        Option<
            Result<
                (
                    Vec<(usize, sotf_audio_player::recording_types::RecordingResult)>,
                    Option<sotf_audio_player::recording_types::TransferMatrixLoopbackRecording>,
                ),
                String,
            >,
        >,
    >,
>;

static RECORDING_RESULT: std::sync::OnceLock<RecordingResultSlot> = std::sync::OnceLock::new();

fn ctc_raw_capture_channel_indices(app: &App, channel_idx: usize) -> Vec<usize> {
    let Some(selected) = app.recording.channel_recordings.get(channel_idx) else {
        return Vec::new();
    };
    let mut indices: Vec<usize> = app
        .recording
        .channel_recordings
        .iter()
        .enumerate()
        .filter(|(_, rec)| {
            rec.channel_index == selected.channel_index
                && rec.mic_position_index == selected.mic_position_index
                && rec.mic_index <= 1
        })
        .map(|(idx, _)| idx)
        .collect();
    indices.sort_by_key(|idx| {
        app.recording
            .channel_recordings
            .get(*idx)
            .map(|rec| rec.mic_index)
            .unwrap_or(usize::MAX)
    });
    indices
}

fn start_recording_channel(app: &mut App, channel_idx: usize) {
    use sotf_audio_player::recording_types::ChannelRecordingState;
    use sotf_audio_player::signal_recorder::{
        DEFAULT_MLS_ORDER, SignalParams, SignalType, generate_signal, write_temp_wav,
    };

    let selected = match app.recording.channel_recordings.get(channel_idx) {
        Some(ch) => ch.clone(),
        None => return,
    };
    let ctc_strategy = app.recording.recording_config.ctc_matrix_strategy;
    let capture_indices = if ctc_strategy == CtcMatrixExportStrategy::RawSweep {
        ctc_raw_capture_channel_indices(app, channel_idx)
    } else {
        vec![channel_idx]
    };
    if ctc_strategy == CtcMatrixExportStrategy::RawSweep && capture_indices.len() < 2 {
        if let Some(ch) = app.recording.channel_recordings.get_mut(channel_idx) {
            ch.state = ChannelRecordingState::Error;
        }
        app.recording.status_message =
            "Raw-sweep CTC requires two ear input channels for the selected speaker/position"
                .to_string();
        return;
    }

    for idx in &capture_indices {
        if let Some(ch) = app.recording.channel_recordings.get_mut(*idx) {
            ch.state = ChannelRecordingState::Recording;
            ch.result = None;
        }
    }
    app.recording.status_message = if ctc_strategy == CtcMatrixExportStrategy::RawSweep {
        format!("Recording CTC ear pair for {}...", selected.channel_name)
    } else {
        format!("Recording channel {}...", selected.channel_name)
    };
    let speaker_index = selected.channel_index;
    let mic_index = selected.mic_index;

    // Map signal type
    let signal_type = match app.recording.signal_type {
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

    let duration_secs = app.recording.signal_duration_secs;
    let level_db = app.recording.signal_level_db;
    let sweep_start_freq = selected.sweep_start_freq;
    let sweep_end_freq = selected.sweep_end_freq;
    let sample_rate = app.recording.playback_config.sample_rate;

    let output_device = app.recording.playback_config.device_name.clone();
    let input_device = app.recording.recording_config.device_name.clone();

    let output_channel = app
        .recording
        .playback_config
        .channel_mappings
        .get(speaker_index)
        .map(|m| m.interface_channel())
        .unwrap_or(0) as u16;
    let input_channel = app
        .recording
        .recording_config
        .channel_mappings
        .get(mic_index)
        .copied()
        .unwrap_or(0) as u16;
    let loopback_input = app.recording.recording_config.ctc_loopback_input_channel;
    let position_idx = selected.mic_position_index;

    // Per-channel calibration lives in `recording_config.mic_calibration_paths`.
    // The per-channel signal recorder takes a single path and applies it to
    // its one input — pick the calibration for the input channel being used.
    let mic_calibration = app
        .recording
        .recording_config
        .mic_calibration_paths
        .get(input_channel as usize)
        .and_then(|o| o.clone())
        .filter(|s| !s.is_empty());

    let channel_name = app.recording.channel_recordings[channel_idx]
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
            if let Some(ch) = app.recording.channel_recordings.get_mut(channel_idx) {
                ch.state = ChannelRecordingState::Error;
            }
            app.recording.status_message = format!("Error generating signal: {}", e);
            return;
        }
    };

    // Write to temp file
    let temp_wav = match write_temp_wav(&signal, sample_rate, 1) {
        Ok(f) => f,
        Err(e) => {
            if let Some(ch) = app.recording.channel_recordings.get_mut(channel_idx) {
                ch.state = ChannelRecordingState::Error;
            }
            app.recording.status_message = format!("Error writing temp WAV: {}", e);
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
                let rec = app.recording.channel_recordings.get(*idx)?;
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
                    .recording_config
                    .channel_mappings
                    .get(rec.mic_index)
                    .copied()
                    .unwrap_or(0) as u16;
                let calibration = app
                    .recording
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
        if let Some(ch) = app.recording.channel_recordings.get_mut(channel_idx) {
            ch.state = ChannelRecordingState::Error;
        }
        app.recording.status_message = format!("Cannot create directory: {}", e);
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
            app.recording.status_message = format!("Could not write CTC reference sweep: {}", e);
        } else {
            app.recording.ctc_reference_sweep_path =
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

/// B7: Poll for recording completion — call from main tick loop
pub fn poll_recording(app: &mut App) -> bool {
    use sotf_audio_player::recording_types::ChannelRecordingState;

    // Only poll when a recording is active
    let has_active = app
        .recording
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
                    if let Some(ch) = app.recording.channel_recordings.get_mut(ch_idx) {
                        ch.state = ChannelRecordingState::Done;
                        completed_names.push(ch.channel_name.clone());
                        ch.result = Some(rec_result);
                    }
                }
                if let Some(loopback) = loopback {
                    app.recording.transfer_matrix_loopbacks.retain(|r| {
                        r.speaker_index != loopback.speaker_index
                            || r.mic_position_index != loopback.mic_position_index
                    });
                    app.recording.transfer_matrix_loopbacks.push(loopback);
                }
                if !completed_names.is_empty() {
                    if completed_names.len() == 1 {
                        app.recording.status_message =
                            format!("Channel {} recording complete", completed_names[0]);
                    } else {
                        app.recording.status_message =
                            format!("Recorded {} CTC ear channels", completed_names.len());
                    }
                }
            }
            Err(e) => {
                // Mark the recording channel as error
                for ch in &mut app.recording.channel_recordings {
                    if ch.state == ChannelRecordingState::Recording {
                        ch.state = ChannelRecordingState::Error;
                    }
                }
                app.recording.status_message = format!("Recording failed: {}", e);
            }
        }
        return true;
    }

    false
}

pub(crate) fn save_recordings(app: &mut App) {
    use autoeq::{OptimizerConfig, RecordingConfiguration, RoomConfig};
    use sotf_audio_player::recording_types::ChannelRecordingState;
    use sotf_audio_player::room_eq_types::{
        DEFAULT_BASS_MANAGEMENT_CROSSOVER_KEY, RoomEqMeasurementsFile,
        build_speakers_from_recordings, ctc_system_config_for_speaker_names,
        default_bass_management_crossovers, room_eq_channel_is_bass_output,
    };

    // Validate save name early (before any I/O)
    let name = if app.recording.save_name.is_empty() {
        "recordings".to_string()
    } else {
        app.recording.save_name.clone()
    };
    if name.contains('/') || name.contains('\\') {
        app.recording.save_error = Some("Save name must not contain path separators".to_string());
        return;
    }

    let completed: Vec<_> = app
        .recording
        .channel_recordings
        .iter()
        .filter(|ch| ch.state == ChannelRecordingState::Done && ch.result.is_some())
        .collect();

    if completed.is_empty() {
        app.recording.save_error = Some("No completed recordings to save".to_string());
        return;
    }

    let dir = if app.recording.output_directory.is_empty() {
        ".".to_string()
    } else {
        app.recording.output_directory.clone()
    };

    // B4: Create output directory before saving or exporting matrix WAVs.
    if let Err(e) = std::fs::create_dir_all(&dir) {
        app.recording.save_error = Some(format!("Cannot create directory: {}", e));
        return;
    }

    let channel_names: Vec<String> = app
        .recording
        .playback_config
        .channel_mappings
        .iter()
        .map(|m| m.group_name.clone())
        .collect();
    let mic_names = vec!["left_ear".to_string(), "right_ear".to_string()];
    let ctc_strategy = app.recording.recording_config.ctc_matrix_strategy;
    let mut ctc_reference_sweep = None;
    let mut ctc_raw_sweep_range = None;
    let mut ctc_raw_fallback = false;
    let mut ctc_measurements = if ctc_strategy == CtcMatrixExportStrategy::RawSweep {
        ctc_reference_sweep = app.recording.ctc_reference_sweep_path.as_ref().map(|path| {
            std::path::Path::new(path)
                .strip_prefix(&dir)
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|_| std::path::PathBuf::from(path))
        });
        match RoomEqMeasurementsFile::build_ctc_measurements_from_recordings_with_strategy(
            &app.recording.channel_recordings,
            &channel_names,
            &mic_names,
            app.recording.recording_config.sample_rate,
            std::path::Path::new(&dir),
            ctc_strategy,
            app.recording.recording_config.ctc_loopback_input_channel,
            &app.recording.transfer_matrix_loopbacks,
        ) {
            Ok(Some(measurements)) => {
                match sotf_audio_player::room_eq_types::ctc_uniform_sweep_range_for_measurements(
                    &app.recording.channel_recordings,
                    &channel_names,
                    &measurements,
                ) {
                    Some(range) => {
                        ctc_raw_sweep_range = Some(range);
                        Some(measurements)
                    }
                    None => {
                        ctc_raw_fallback = true;
                        app.recording.status_message =
                            "Raw-sweep CTC mixes sweep ranges; falling back to measured CTC"
                                .to_string();
                        None
                    }
                }
            }
            Ok(None) => {
                ctc_raw_fallback = true;
                app.recording.status_message =
                    "Raw-sweep CTC incomplete; falling back to measured CTC".to_string();
                None
            }
            Err(e) => {
                ctc_raw_fallback = true;
                app.recording.status_message =
                    format!("Could not export raw-sweep CTC transfer matrix: {}", e);
                None
            }
        }
    } else {
        None
    };
    if ctc_measurements.is_none() {
        ctc_measurements = match RoomEqMeasurementsFile::build_ctc_measurements_from_recordings(
            &app.recording.channel_recordings,
            &channel_names,
            &mic_names,
            app.recording.recording_config.sample_rate,
            std::path::Path::new(&dir),
        ) {
            Ok(measurements) => measurements,
            Err(e) => {
                app.recording.status_message =
                    format!("Could not export CTC transfer matrix: {}", e);
                None
            }
        };
        ctc_reference_sweep = None;
    }

    // Build per-channel speaker entries — every (channel × mic ×
    // position) recording for the same output channel folds into one
    // SpeakerConfig so roomeq emits one EQ chain per channel.
    let speakers = build_speakers_from_recordings(
        &app.recording.channel_recordings,
        &channel_names,
        app.recording.channel_speakers_map_for_save().as_ref(),
    );

    if speakers.is_empty() {
        app.recording.save_error = Some("No completed recordings to save".to_string());
        return;
    }

    let mic_calibration_paths_value =
        app.recording.recording_config.mic_calibration_paths.clone();

    let configuration = RecordingConfiguration {
        playback_device_name: Some(app.recording.playback_config.device_name.clone()),
        playback_device_id: Some(app.recording.playback_config.device_id.clone()),
        playback_sample_rate: Some(app.recording.playback_config.sample_rate),
        playback_channels: Some(app.recording.playback_config.num_channels),
        speaker_configuration: Some(
            app.recording
                .playback_config
                .speaker_configuration
                .as_str()
                .to_string(),
        ),
        channel_names: Some(channel_names.clone()),
        recording_device_name: Some(app.recording.recording_config.device_name.clone()),
        recording_device_id: Some(app.recording.recording_config.device_id.clone()),
        recording_sample_rate: Some(app.recording.recording_config.sample_rate),
        recording_channels: Some(app.recording.recording_config.num_channels),
        mic_calibration_path: mic_calibration_paths_value
            .first()
            .and_then(|o| o.clone())
            .filter(|s| !s.is_empty()),
        mic_calibration_paths: if mic_calibration_paths_value.is_empty() {
            None
        } else {
            Some(mic_calibration_paths_value)
        },
        recording_directory: if app.recording.output_directory.is_empty() {
            None
        } else {
            Some(app.recording.output_directory.clone())
        },
        signal_type: Some(app.recording.signal_type.as_str().to_string()),
        signal_duration_secs: Some(app.recording.signal_duration_secs),
        signal_level_db: Some(app.recording.signal_level_db),
        sweep_start_freq: Some(app.recording.sweep_start_freq),
        sweep_end_freq: Some(app.recording.sweep_end_freq),
        room_dimensions: app.recording.room_dimensions_for_save(),
        setup_description: {
            let s = app.recording.setup_description.trim();
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        },
        channel_speakers: app.recording.channel_speakers_map_for_save(),
        probe_results: app.recording.probe_capture.results.as_ref().map(|r| {
            autoeq::roomeq::ProbeResultsLegacy {
                channels: r
                    .channels
                    .iter()
                    .map(|c| autoeq::roomeq::ProbeChannelResultLegacy {
                        channel_name: c.channel_name.clone(),
                        channel_index: c.channel_index,
                        arrival_ms: c.arrival_ms,
                        gain_db: c.gain_db,
                        snr_db: c.snr_db,
                    })
                    .collect(),
                sample_rate: r.sample_rate,
                alignment_delays_ms: r.alignment_delays_ms.clone(),
            }
        }),
        probe_wav_relative: app
            .recording
            .probe_capture
            .wav_path
            .as_ref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .map(|f| f.to_string_lossy().to_string()),
        num_positions: {
            let n = app.recording.recording_config.num_positions.max(1);
            if n > 1 { Some(n) } else { None }
        },
        bass_probe_freq_hz: Some(app.recording.bass_anchor_capture.bass_freq_hz),
        bass_probe_duration_s: Some(app.recording.bass_anchor_capture.bass_duration_s),
        ..Default::default()
    };

    let ctc = ctc_measurements.map(|measurements| {
        let raw = ctc_reference_sweep.is_some();
        autoeq::roomeq::CtcConfig {
            // Off by default — binaural CTC is opt-in. The stanza is
            // written so the user can flip it on later without
            // re-recording, but roomeq must not run the CTC solver
            // until they do.
            enabled: false,
            matrix_source: if raw { "raw_sweep" } else { "measured" }.to_string(),
            measurements: Some(measurements),
            reference_sweep: ctc_reference_sweep,
            sweep_duration_s: if raw {
                Some(app.recording.signal_duration_secs as f64)
            } else {
                None
            },
            sweep_start_hz: if raw {
                ctc_raw_sweep_range.map(|(start, _)| start as f64)
            } else {
                None
            },
            sweep_end_hz: if raw {
                ctc_raw_sweep_range.map(|(_, end)| end as f64)
            } else {
                None
            },
            ..Default::default()
        }
    });
    // Always emit the system (logical role) map from the recorded
    // speakers, independent of CTC enable state. roomeq uses it to
    // interpret the layout (LFE/sub detection, bass-management
    // routing). Flipping CTC on later does not require re-recording.
    let has_bass_output = speakers
        .keys()
        .any(|name| room_eq_channel_is_bass_output(name));
    let bass_management_crossover =
        has_bass_output.then(|| DEFAULT_BASS_MANAGEMENT_CROSSOVER_KEY.to_string());
    let system = ctc_system_config_for_speaker_names(
        speakers.keys().map(String::as_str),
        bass_management_crossover,
    );
    let crossovers = has_bass_output.then(default_bass_management_crossovers);

    let room_config = RoomConfig {
        version: "1.1.0".to_string(),
        system,
        speakers,
        crossovers,
        target_curve: None,
        optimizer: OptimizerConfig::default(),
        recording_config: Some(configuration),
        ctc,
        cea2034_cache: None,
    };

    let path = std::path::PathBuf::from(&dir).join(format!("{}.json", name));

    match serde_json::to_string_pretty(&room_config) {
        Ok(json) => match std::fs::write(&path, json) {
            Ok(()) => {
                app.recording.save_success = true;
                app.recording.save_error = None;
                if ctc_raw_fallback {
                    app.recording.status_message =
                        "Saved with measured CTC fallback; raw-sweep CTC was incomplete"
                            .to_string();
                }
            }
            Err(e) => {
                app.recording.save_error = Some(format!("Write error: {}", e));
            }
        },
        Err(e) => {
            app.recording.save_error = Some(format!("Serialize error: {}", e));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::tests::make_app;
    use sotf_audio_player::recording_types::{ChannelMapping, RecordingStep, SpeakerConfiguration};

    #[test]
    fn init_recording_channels_creates_channels() {
        let mut app = make_app();
        app.recording.playback_config.channel_mappings = vec![
            ChannelMapping::single(0, "FL"),
            ChannelMapping::single(1, "FR"),
        ];
        app.recording.playback_config.num_channels = 2;

        init_recording_channels(&mut app);
        assert_eq!(app.recording.channel_recordings.len(), 2);
        assert_eq!(app.recording.current_channel, Some(0));
        assert_eq!(app.recording.channel_recordings[0].channel_name, "FL");
        assert_eq!(app.recording.channel_recordings[1].channel_name, "FR");
    }

    #[test]
    fn init_recording_channels_expands_speaker_mic_position_matrix() {
        let mut app = make_app();
        app.recording.playback_config.channel_mappings = vec![
            ChannelMapping::single(0, "FL"),
            ChannelMapping::single(1, "FR"),
        ];
        app.recording.recording_config.channel_mappings = vec![0, 1];
        app.recording.recording_config.num_positions = 2;

        init_recording_channels(&mut app);

        assert_eq!(app.recording.channel_recordings.len(), 8);
        assert_eq!(
            app.recording.channel_recordings[0].channel_name,
            "FL (Pos 1 / Mic 1)"
        );
        assert_eq!(
            app.recording.channel_recordings[1].channel_name,
            "FL (Pos 1 / Mic 2)"
        );
        assert_eq!(
            app.recording.channel_recordings[2].channel_name,
            "FR (Pos 1 / Mic 1)"
        );
        assert_eq!(
            app.recording.channel_recordings[4].channel_name,
            "FL (Pos 2 / Mic 1)"
        );
        assert_eq!(app.recording.channel_recordings[4].channel_index, 0);
        assert_eq!(app.recording.channel_recordings[4].mic_position_index, 1);
    }

    #[test]
    fn ctc_raw_capture_selects_both_ears_for_same_speaker_position() {
        let mut app = make_app();
        app.recording.playback_config.channel_mappings = vec![
            ChannelMapping::single(0, "FL"),
            ChannelMapping::single(1, "FR"),
        ];
        app.recording.recording_config.channel_mappings = vec![0, 1];
        app.recording.recording_config.num_positions = 2;
        init_recording_channels(&mut app);

        assert_eq!(ctc_raw_capture_channel_indices(&app, 1), vec![0, 1]);
        assert_eq!(ctc_raw_capture_channel_indices(&app, 4), vec![4, 5]);
    }

    #[test]
    fn init_recording_channels_reinits_on_config_change() {
        let mut app = make_app();
        // Start with 2 channels
        app.recording.playback_config.channel_mappings = vec![
            ChannelMapping::single(0, "FL"),
            ChannelMapping::single(1, "FR"),
        ];
        init_recording_channels(&mut app);
        assert_eq!(app.recording.channel_recordings.len(), 2);

        // Change to 3 channels
        app.recording.playback_config.channel_mappings = vec![
            ChannelMapping::single(0, "FL"),
            ChannelMapping::single(1, "FR"),
            ChannelMapping::single(2, "C"),
        ];
        init_recording_channels(&mut app);
        assert_eq!(app.recording.channel_recordings.len(), 3);
        assert_eq!(app.recording.channel_recordings[2].channel_name, "C");
    }

    #[test]
    fn init_recording_channels_handles_empty_config() {
        let mut app = make_app();
        app.recording.playback_config.channel_mappings = vec![];
        init_recording_channels(&mut app);
        assert_eq!(app.recording.channel_recordings.len(), 0);
        assert_eq!(app.recording.current_channel, None);
    }

    #[test]
    fn save_recordings_rejects_path_separators_in_name() {
        let mut app = make_app();
        app.recording.save_name = "../../evil".to_string();
        save_recordings(&mut app);
        assert!(app.recording.save_error.is_some());
        assert!(
            app.recording
                .save_error
                .as_ref()
                .unwrap()
                .contains("path separators")
        );
    }

    #[test]
    fn save_recordings_rejects_backslash_in_name() {
        let mut app = make_app();
        app.recording.save_name = "foo\\bar".to_string();
        save_recordings(&mut app);
        assert!(app.recording.save_error.is_some());
        assert!(
            app.recording
                .save_error
                .as_ref()
                .unwrap()
                .contains("path separators")
        );
    }

    #[test]
    fn save_recordings_requires_completed_channels() {
        let mut app = make_app();
        app.recording.save_name = "test".to_string();
        // No completed recordings
        save_recordings(&mut app);
        assert!(app.recording.save_error.is_some());
        assert!(
            app.recording
                .save_error
                .as_ref()
                .unwrap()
                .contains("No completed")
        );
    }

    #[test]
    fn recording_step_default_is_config() {
        assert_eq!(RecordingStep::default(), RecordingStep::Config);
    }

    #[test]
    fn update_channel_mappings_creates_correct_channels() {
        let mut app = make_app();
        update_channel_mappings_for_config(&mut app, SpeakerConfiguration::Stereo);
        assert_eq!(app.recording.playback_config.num_channels, 2);
        assert_eq!(app.recording.playback_config.channel_mappings.len(), 2);
    }

    #[test]
    fn adjust_device_populates_config() {
        let mut app = make_app();
        app.recording.available_playback_devices = vec![
            ("id0".to_string(), "Device 0".to_string()),
            ("id1".to_string(), "Device 1".to_string()),
        ];
        app.recording.available_recording_devices = vec![
            ("rid0".to_string(), "Mic 0".to_string()),
            ("rid1".to_string(), "Mic 1".to_string()),
        ];
        app.recording.selected_field = 0;
        adjust_recording_field(&mut app, 1);
        assert_eq!(app.recording.playback_config.device_name, "Device 1");
        assert_eq!(app.recording.playback_config.device_id, "id1");

        app.recording.selected_field = 1;
        adjust_recording_field(&mut app, 1);
        assert_eq!(app.recording.recording_config.device_name, "Mic 1");
        assert_eq!(app.recording.recording_config.device_id, "rid1");
    }

    #[test]
    fn tab_on_config_cycles_fields() {
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

        let mut app = make_app();
        app.recording.selected_field = 0;

        let tab_key = KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        handle_recording_keys(&mut app, tab_key);
        assert_eq!(app.recording.selected_field, 1);
        assert_eq!(
            app.recording.step,
            sotf_audio_player::recording_types::RecordingStep::Config
        );
    }

    #[test]
    fn right_on_config_adjusts_field() {
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

        let mut app = make_app();
        app.recording.selected_field = 4; // signal_duration_secs (numerical)
        let before = app.recording.signal_duration_secs;
        let right_key = KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        handle_recording_keys(&mut app, right_key);
        assert_eq!(app.recording.signal_duration_secs, before + 1.0);
        // Should stay on Config step
        assert_eq!(
            app.recording.step,
            sotf_audio_player::recording_types::RecordingStep::Config
        );
    }
}
