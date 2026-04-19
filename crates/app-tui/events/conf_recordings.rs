//! Recording wizard event handlers

use super::PlayerCommand;
use crate::app::{App, FilePickerMode, FilePickerOrigin, InputMode};
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::{Arc, Mutex};

fn recording_step_prev_wrap(
    s: sotf_audio_player::recording_types::RecordingStep,
) -> sotf_audio_player::recording_types::RecordingStep {
    use sotf_audio_player::recording_types::RecordingStep;
    match s {
        RecordingStep::Config => RecordingStep::Saving,
        RecordingStep::Capture => RecordingStep::Config,
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
        RecordingStep::Config => RecordingStep::Capture,
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
        if app.recording.editing_mic_cal {
            app.recording.editing_mic_cal = false;
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
            if app.recording.editing_mic_cal {
                match key.code {
                    KeyCode::Enter => {
                        app.recording.editing_mic_cal = false;
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
                        app.recording.mic_calibration_path.pop();
                        app.refresh_autocomplete_inline(
                            crate::app::app_autocomplete::get_recording_mic_cal_path,
                            crate::app::app_autocomplete::AutocompleteKind::FilePath,
                        );
                    }
                    KeyCode::F(2) => {
                        let start = app.recording.mic_calibration_path.clone();
                        app.open_file_explorer(
                            FilePickerOrigin::RecordingMicCalibration,
                            FilePickerMode::File,
                            "Select Mic Calibration File",
                            Some(&start),
                            None,
                        );
                    }
                    KeyCode::Char(c) => {
                        app.recording.mic_calibration_path.push(c);
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

            match key.code {
                KeyCode::Up => {
                    if app.recording.selected_field == 0 {
                        app.recording.step_tab_focused = true;
                    } else {
                        app.recording.selected_field -= 1;
                    }
                }
                KeyCode::Down => {
                    if app.recording.selected_field < 9 {
                        app.recording.selected_field += 1;
                    }
                }
                KeyCode::Enter => {
                    let f = app.recording.selected_field;
                    match f {
                        8 => {
                            app.recording.editing_output_dir = true;
                        }
                        9 => {
                            app.recording.editing_mic_cal = true;
                        }
                        _ => {
                            if is_recording_field_numerical(f) {
                                app.recording.edit_buffer = recording_field_value_string(app, f);
                                app.recording.editing_value = true;
                            }
                        }
                    }
                }
                KeyCode::Left | KeyCode::Char('-') => {
                    adjust_recording_field(app, -1);
                }
                KeyCode::Right | KeyCode::Char('+') => {
                    adjust_recording_field(app, 1);
                }
                KeyCode::Tab => {
                    if app.recording.selected_field < 9 {
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
            // GD-Opt v2 Phase GD-1e — display-only in the TUI for now.
            // Navigation keys fall through to the wizard-level handler
            // via the outer loop; no step-specific actions yet.
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

    // Build the channel list from the already-captured sweeps so we
    // probe the same speakers the user recorded.
    let channel_names: Vec<String> = app
        .recording
        .channel_recordings
        .iter()
        .map(|r| r.channel_name.clone())
        .collect();
    let channel_indices: Vec<u16> = (0..channel_names.len() as u16).collect();

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

/// Fields 4-7 are numerical (duration, level, start_freq, end_freq)
fn is_recording_field_numerical(field: usize) -> bool {
    matches!(field, 4..=7)
}

fn recording_field_value_string(app: &App, field: usize) -> String {
    match field {
        4 => format!("{:.1}", app.recording.signal_duration_secs),
        5 => format!("{:.1}", app.recording.signal_level_db),
        6 => format!("{:.0}", app.recording.sweep_start_freq),
        7 => format!("{:.0}", app.recording.sweep_end_freq),
        _ => String::new(),
    }
}

fn set_recording_field_from_string(app: &mut App) {
    let buf = &app.recording.edit_buffer;
    match app.recording.selected_field {
        4 => {
            if let Ok(v) = buf.parse::<f32>() {
                app.recording.signal_duration_secs = v.clamp(1.0, 30.0);
            }
        }
        5 => {
            if let Ok(v) = buf.parse::<f32>() {
                app.recording.signal_level_db = v.clamp(-40.0, 0.0);
            }
        }
        6 => {
            if let Ok(v) = buf.parse::<f32>() {
                app.recording.sweep_start_freq = v.clamp(10.0, 1000.0);
            }
        }
        7 => {
            if let Ok(v) = buf.parse::<f32>() {
                app.recording.sweep_end_freq = v.clamp(1000.0, 24000.0);
            }
        }
        _ => {}
    }
}

fn adjust_recording_field(app: &mut App, delta: i32) {
    use sotf_audio_player::recording_types::{RecordingSignalType, SpeakerConfiguration};

    match app.recording.selected_field {
        0 => {
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
        1 => {
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
        2 => {
            // Cycle speaker config
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
            // Update channel mappings for new config
            update_channel_mappings_for_config(app, new_config);
        }
        3 => {
            // Cycle signal type
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
        4 => {
            app.recording.signal_duration_secs =
                (app.recording.signal_duration_secs + delta as f32).clamp(1.0, 30.0);
        }
        5 => {
            app.recording.signal_level_db =
                (app.recording.signal_level_db + delta as f32).clamp(-40.0, 0.0);
        }
        6 => {
            app.recording.sweep_start_freq =
                (app.recording.sweep_start_freq + delta as f32 * 10.0).clamp(10.0, 1000.0);
        }
        7 => {
            app.recording.sweep_end_freq =
                (app.recording.sweep_end_freq + delta as f32 * 1000.0).clamp(1000.0, 24000.0);
        }
        _ => {}
    }
}

pub(crate) fn update_channel_mappings_for_config(
    app: &mut App,
    config: sotf_audio_player::recording_types::SpeakerConfiguration,
) {
    use sotf_audio_player::recording_types::ChannelMapping;

    let names = config.default_channel_names();
    app.recording.playback_config.channel_mappings = names
        .iter()
        .enumerate()
        .map(|(i, name)| ChannelMapping::single(i, *name))
        .collect();
    app.recording.playback_config.num_channels = names.len();
}

pub(crate) fn init_recording_channels(app: &mut App) {
    use sotf_audio_player::recording_types::ChannelRecording;

    let expected_count = app.recording.playback_config.channel_mappings.len();
    if app.recording.channel_recordings.len() != expected_count {
        app.recording.channel_recordings = app
            .recording
            .playback_config
            .channel_mappings
            .iter()
            .enumerate()
            .map(|(i, mapping)| ChannelRecording::new(i, mapping.group_name.clone()))
            .collect();
        app.recording.current_channel = if expected_count > 0 { Some(0) } else { None };
    }
}

// ---- B2: Actual recording implementation ----

type RecordingResultSlot = Arc<
    Mutex<Option<Result<(usize, sotf_audio_player::recording_types::RecordingResult), String>>>,
>;

static RECORDING_RESULT: std::sync::OnceLock<RecordingResultSlot> = std::sync::OnceLock::new();

fn start_recording_channel(app: &mut App, channel_idx: usize) {
    use sotf_audio_player::recording_types::ChannelRecordingState;
    use sotf_audio_player::signal_recorder::{
        SignalParams, SignalType, generate_signal, write_temp_wav,
    };

    let ch = match app.recording.channel_recordings.get_mut(channel_idx) {
        Some(ch) => ch,
        None => return,
    };
    ch.state = ChannelRecordingState::Recording;
    app.recording.status_message = format!("Recording channel {}...", ch.channel_name);

    // Map signal type
    let signal_type = match app.recording.signal_type {
        sotf_audio_player::recording_types::RecordingSignalType::Sweep => SignalType::Sweep,
        sotf_audio_player::recording_types::RecordingSignalType::WhiteNoise => {
            SignalType::WhiteNoise
        }
        sotf_audio_player::recording_types::RecordingSignalType::PinkNoise => SignalType::PinkNoise,
        sotf_audio_player::recording_types::RecordingSignalType::DelayProbe => {
            log::warn!(
                "DelayProbe selected in per-channel mode; use probe_channel_delays() instead. Falling back to Sweep."
            );
            SignalType::Sweep
        }
    };

    let duration_secs = app.recording.signal_duration_secs;
    let level_db = app.recording.signal_level_db;
    let sweep_start_freq = app.recording.sweep_start_freq;
    let sweep_end_freq = app.recording.sweep_end_freq;
    let sample_rate = app.recording.playback_config.sample_rate;

    let output_device = app.recording.playback_config.device_name.clone();
    let input_device = app.recording.recording_config.device_name.clone();

    let output_channel = app
        .recording
        .playback_config
        .channel_mappings
        .get(channel_idx)
        .map(|m| m.interface_channel())
        .unwrap_or(0) as u16;
    let input_channel = app
        .recording
        .recording_config
        .channel_mappings
        .first()
        .copied()
        .unwrap_or(0) as u16;

    let mic_calibration = if app.recording.mic_calibration_path.is_empty() {
        None
    } else {
        Some(app.recording.mic_calibration_path.clone())
    };

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

    // B4: Create output directory before recording
    if let Err(e) = std::fs::create_dir_all(&recording_dir) {
        if let Some(ch) = app.recording.channel_recordings.get_mut(channel_idx) {
            ch.state = ChannelRecordingState::Error;
        }
        app.recording.status_message = format!("Cannot create directory: {}", e);
        return;
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
        use sotf_audio_player::signal_recorder::record_and_analyze;

        let sweep_range = if signal_type == SignalType::Sweep {
            Some((sweep_start_freq, sweep_end_freq))
        } else {
            None
        };

        let result = record_and_analyze(
            &temp_wav_path,
            &recorded_wav_path,
            &reference_signal,
            sample_rate,
            &csv_path,
            output_channel,
            input_channel,
            if output_device.is_empty() {
                None
            } else {
                Some(output_device.as_str())
            },
            if input_device.is_empty() {
                None
            } else {
                Some(input_device.as_str())
            },
            mic_calibration.as_deref(),
            sweep_range,
        );

        let mapped = result
            .map(|analysis_result| {
                let rec_result = RecordingResult {
                    channel: channel_idx,
                    wav_path: Some(recorded_wav_path.to_string_lossy().to_string()),
                    csv_path: Some(csv_path.to_string_lossy().to_string()),
                    frequencies: analysis_result.frequencies,
                    magnitude_db: analysis_result.spl_db,
                    phase_deg: analysis_result.phase_deg,
                    impulse_response: Some(analysis_result.impulse_response),
                    impulse_time_ms: Some(analysis_result.impulse_time_ms),
                    excess_group_delay_ms: Some(analysis_result.excess_group_delay_ms),
                    thd_percent: Some(analysis_result.thd_percent),
                    harmonic_distortion_db: Some(analysis_result.harmonic_distortion_db),
                    rt60_ms: Some(analysis_result.rt60_ms),
                    clarity_c50_db: Some(analysis_result.clarity_c50_db),
                    clarity_c80_db: Some(analysis_result.clarity_c80_db),
                    spectrogram_db: Some(analysis_result.spectrogram_db),
                };
                (channel_idx, rec_result)
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
            Ok((ch_idx, rec_result)) => {
                if let Some(ch) = app.recording.channel_recordings.get_mut(ch_idx) {
                    ch.state = ChannelRecordingState::Done;
                    let channel_name = ch.channel_name.clone();
                    ch.result = Some(rec_result);
                    app.recording.status_message =
                        format!("Channel {} recording complete", channel_name);
                }
            }
            Err(e) => {
                // Mark the recording channel as error
                for ch in &mut app.recording.channel_recordings {
                    if ch.state == ChannelRecordingState::Recording {
                        ch.state = ChannelRecordingState::Error;
                        break;
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
    use sotf_audio_player::recording_types::ChannelRecordingState;
    use sotf_audio_player::room_eq_types::{
        ChannelMeasurement, RecordingConfiguration, RoomEqMeasurementsFile,
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

    // Build measurements file
    let channels: Vec<ChannelMeasurement> = completed
        .iter()
        .map(|ch| ChannelMeasurement {
            channel_name: ch.channel_name.clone(),
            measurement: ch.result.clone().unwrap(),
            is_group: false,
            group_drivers: Vec::new(),
            multi_mic_measurements: Vec::new(),
        })
        .collect();

    // B6: Build RecordingConfiguration from current state
    let configuration = RecordingConfiguration {
        playback_device_name: app.recording.playback_config.device_name.clone(),
        playback_device_id: app.recording.playback_config.device_id.clone(),
        playback_sample_rate: app.recording.playback_config.sample_rate,
        playback_channels: app.recording.playback_config.num_channels,
        speaker_configuration: app
            .recording
            .playback_config
            .speaker_configuration
            .as_str()
            .to_string(),
        channel_names: app
            .recording
            .playback_config
            .channel_mappings
            .iter()
            .map(|m| m.group_name.clone())
            .collect(),
        recording_device_name: app.recording.recording_config.device_name.clone(),
        recording_device_id: app.recording.recording_config.device_id.clone(),
        recording_sample_rate: app.recording.recording_config.sample_rate,
        recording_channels: app.recording.recording_config.num_channels,
        mic_calibration_path: if app.recording.mic_calibration_path.is_empty() {
            None
        } else {
            Some(app.recording.mic_calibration_path.clone())
        },
        mic_calibration_paths: app.recording.recording_config.mic_calibration_paths.clone(),
        recording_directory: if app.recording.output_directory.is_empty() {
            None
        } else {
            Some(app.recording.output_directory.clone())
        },
        signal_type: app.recording.signal_type.as_str().to_string(),
        signal_duration_secs: app.recording.signal_duration_secs,
        signal_level_db: app.recording.signal_level_db,
        sweep_start_freq: Some(app.recording.sweep_start_freq),
        sweep_end_freq: Some(app.recording.sweep_end_freq),
        // Room info collected on the Save step. Converted to metric,
        // trimmed, or dropped when the user left the fields blank.
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
        // Tone-burst delay-probe results (Recording wizard Probe
        // step). `None` when the user skipped the step.
        probe_results: app.recording.probe_capture.results.clone(),
        probe_wav_relative: app
            .recording
            .probe_capture
            .wav_path
            .as_ref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .map(|f| f.to_string_lossy().to_string()),
    };

    let measurements_file = RoomEqMeasurementsFile::with_configuration(channels, configuration);

    // Determine output path
    let dir = if app.recording.output_directory.is_empty() {
        ".".to_string()
    } else {
        app.recording.output_directory.clone()
    };

    // B4: Create output directory before saving
    if let Err(e) = std::fs::create_dir_all(&dir) {
        app.recording.save_error = Some(format!("Cannot create directory: {}", e));
        return;
    }

    let path = std::path::PathBuf::from(&dir).join(format!("{}.json", name));

    match serde_json::to_string_pretty(&measurements_file) {
        Ok(json) => match std::fs::write(&path, json) {
            Ok(()) => {
                app.recording.save_success = true;
                app.recording.save_error = None;
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
