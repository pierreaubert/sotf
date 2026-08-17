use super::super::PlayerCommand;
use super::adjust::adjust_recording_field;
use super::adjust::adjust_spl_field;
use super::consts::SPL_FIELD_COUNT;
use super::consts::SPL_FIELD_DURATION;
use super::consts::SPL_FIELD_IN_CH;
use super::consts::SPL_FIELD_OUT_CH;
use super::consts::SPL_FIELD_REF_FREQ;
use super::consts::SPL_FIELD_REPORTED;
use super::consts::SPL_FIELD_RUN;
use super::consts::SPL_FIELD_TONE_AMP;
use super::consts::start_recording_channel;
use super::misc::add_custom_speaker;
use super::misc::commit_save_field_edit;
use super::misc::current_save_field_value;
use super::misc::init_recording_channels;
use super::misc::is_recording_field_numerical_kind;
use super::misc::remove_custom_speaker;
use super::misc::request_bass_anchor_cancel;
use super::misc::request_probe_cancel;
use super::misc::request_spl_cancel;
use super::misc::request_sweep_cancel;
use super::misc::save_recordings;
use super::misc::set_recording_field_from_string;
use super::recording::recording_field_value_string_kind;
use super::recording::recording_step_next_wrap;
use super::recording::recording_step_prev_wrap;
use super::spawn::spawn_bass_anchor_capture;
use super::spawn::spawn_probe_capture;
use super::spawn::spawn_spl_calibration_capture;
use crate::app::{App, FilePickerMode, FilePickerOrigin, InputMode};
use crossterm::event::{KeyCode, KeyEvent};

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
                app.recording.model.step = recording_step_prev_wrap(app.recording.model.step);
                return None;
            }
            KeyCode::Right | KeyCode::Tab => {
                let next = recording_step_next_wrap(app.recording.model.step);
                // Guard: entering Capture requires an output directory.
                // `recording_step_next_wrap(SplCalibration) == Capture`,
                // so the guard fires on the SplCalibration→Capture
                // edge — Config and Capture are not adjacent in the
                // wrap order (Config → SplCalibration → Capture).
                if next == RecordingStep::Capture
                    && app.recording.model.step == RecordingStep::SplCalibration
                {
                    if app.recording.output_directory.is_empty() {
                        app.recording.model.status_message =
                            "Set an output directory first".to_string();
                        return None;
                    }
                    init_recording_channels(app);
                }
                app.recording.model.step = next;
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

    match app.recording.model.step {
        RecordingStep::Config => {
            if app.recording.editing_output_dir {
                match key.code {
                    KeyCode::Enter => {
                        app.recording.model.recording_base_directory =
                            Some(app.recording.output_directory.clone());
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
                    ) && app.recording.model.playback_config.speaker_configuration
                        == sotf_audio_player::recording_types::SpeakerConfiguration::Custom =>
                {
                    add_custom_speaker(app);
                }
                KeyCode::Char('x') | KeyCode::Char('X')
                    if matches!(
                        recording_field_at(&app.recording, app.recording.selected_field),
                        Some(RecordingField::SpeakerConfig)
                    ) && app.recording.model.playback_config.speaker_configuration
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
                match app.recording.model.current_recording_channel {
                    Some(ch) if ch > 0 => {
                        app.recording.model.current_recording_channel = Some(ch - 1);
                    }
                    _ => {
                        app.recording.step_tab_focused = true;
                    }
                }
                None
            }
            KeyCode::Down => {
                if let Some(ch) = app.recording.model.current_recording_channel {
                    if ch + 1 < app.recording.model.channel_recordings.len() {
                        app.recording.model.current_recording_channel = Some(ch + 1);
                    }
                } else if !app.recording.model.channel_recordings.is_empty() {
                    app.recording.model.current_recording_channel = Some(0);
                }
                None
            }
            KeyCode::Enter | KeyCode::Char('r') => {
                // While a capture is in flight the same key requests
                // cancellation (R8) — mirrors the probe/SPL steps where
                // Enter/`r` toggles run/cancel.
                if app.recording.model.capture_in_progress() {
                    request_sweep_cancel(app);
                    return None;
                }
                // B2: Record current channel via engine
                if let Some(ch_idx) = app.recording.model.current_recording_channel {
                    let can_record = app
                        .recording
                        .model
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
                app.recording.model.step = RecordingStep::Config;
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
                    .model
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
                app.recording.model.step = RecordingStep::Capture;
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
/// field begins editing; `Enter` on Run triggers the capture (or requests
/// cancellation while one is running). `+` / `-`
/// or `←` / `→` nudge numeric values. `r` starts (or cancels) the capture
/// from any focused field. `Ctrl+S` never applies here — saving lives on the
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
                            app.recording.model.probe_capture.probe_duration_ms =
                                v.clamp(100.0, 5000.0);
                        }
                    }
                    FIELD_SILENCE_MS => {
                        if let Ok(v) = v {
                            app.recording.model.probe_capture.silence_duration_ms =
                                v.clamp(100.0, 5000.0);
                        }
                    }
                    FIELD_MIC_CHANNEL => {
                        if let Ok(v) = v {
                            app.recording.model.probe_capture.input_channel = v.max(0.0) as u16;
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
        app.recording.model.probe_capture.status,
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
                let v = (app.recording.model.probe_capture.probe_duration_ms + 100.0).min(5000.0);
                app.recording.model.probe_capture.probe_duration_ms = v;
            }
            FIELD_SILENCE_MS => {
                let v = (app.recording.model.probe_capture.silence_duration_ms + 100.0).min(5000.0);
                app.recording.model.probe_capture.silence_duration_ms = v;
            }
            FIELD_MIC_CHANNEL => {
                app.recording.model.probe_capture.input_channel = app
                    .recording
                    .model
                    .probe_capture
                    .input_channel
                    .saturating_add(1);
            }
            _ => {}
        },
        KeyCode::Char('-') | KeyCode::Left => match app.recording.probe_selected_field {
            FIELD_PROBE_MS => {
                let v = (app.recording.model.probe_capture.probe_duration_ms - 100.0).max(100.0);
                app.recording.model.probe_capture.probe_duration_ms = v;
            }
            FIELD_SILENCE_MS => {
                let v = (app.recording.model.probe_capture.silence_duration_ms - 100.0).max(100.0);
                app.recording.model.probe_capture.silence_duration_ms = v;
            }
            FIELD_MIC_CHANNEL => {
                app.recording.model.probe_capture.input_channel = app
                    .recording
                    .model
                    .probe_capture
                    .input_channel
                    .saturating_sub(1);
            }
            _ => {}
        },
        KeyCode::Enter => {
            if app.recording.probe_selected_field == FIELD_RUN {
                if running {
                    request_probe_cancel(app);
                } else {
                    spawn_probe_capture(app);
                }
            } else {
                app.recording.probe_editing_value = true;
                app.recording.edit_buffer.clear();
            }
        }
        KeyCode::Char('r') => {
            if running {
                request_probe_cancel(app);
            } else {
                spawn_probe_capture(app);
            }
        }
        _ => {}
    }
}

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
                let cal = &mut app.recording.model.spl_calibration_capture;
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
        app.recording.model.spl_calibration_capture.status,
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

fn handle_bass_anchor_step_keys(app: &mut App, key: KeyEvent) {
    use sotf_audio_player::recording_types::BassAnchorCaptureStatus;

    let running = matches!(
        app.recording.model.bass_anchor_capture.status,
        BassAnchorCaptureStatus::Running { .. }
    );

    match key.code {
        KeyCode::Enter | KeyCode::Char('r') => {
            if running {
                request_bass_anchor_cancel(app);
            } else {
                spawn_bass_anchor_capture(app);
            }
        }
        _ => {}
    }
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
    use super::super::conf_spinoramaeq::ensure_spinorama_speakers_loading;
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
            app.recording.model.step = RecordingStep::Evaluating;
        }
        KeyCode::Enter => {
            if app.recording.selected_save_field == 4 {
                // Unit toggle is a pure cycle — no edit mode needed.
                app.recording.model.room_dimension_unit =
                    app.recording.model.room_dimension_unit.toggled();
            } else {
                app.recording.editing_save_value = true;
                app.recording.edit_buffer = current_save_field_value(app);
            }
        }
        KeyCode::Char('u') | KeyCode::Char('U') => {
            // Quick keyboard shortcut for the unit toggle from any
            // field — saves the user tabbing to field 4.
            app.recording.model.room_dimension_unit =
                app.recording.model.room_dimension_unit.toggled();
        }
        _ => {}
    }
}
