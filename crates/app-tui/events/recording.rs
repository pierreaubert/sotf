//! Recording wizard event handlers

use super::PlayerCommand;
use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_recording_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use sotf_audio_player::recording_types::RecordingStep;

    // Esc goes up one level
    if key.code == KeyCode::Esc {
        match app.recording.step {
            RecordingStep::Config => {
                if app.recording.editing_output_dir {
                    app.recording.editing_output_dir = false;
                } else if app.recording.editing_mic_cal {
                    app.recording.editing_mic_cal = false;
                } else {
                    app.configure_tab_focused = true;
                }
            }
            RecordingStep::Capture => {
                app.recording.step = RecordingStep::Config;
            }
            RecordingStep::Evaluating => {
                app.recording.step = RecordingStep::Capture;
            }
            RecordingStep::Saving => {
                if app.recording.editing_save_name {
                    app.recording.editing_save_name = false;
                } else {
                    app.recording.step = RecordingStep::Evaluating;
                }
            }
        }
        return None;
    }

    match app.recording.step {
        RecordingStep::Config => {
            if app.recording.editing_output_dir {
                match key.code {
                    KeyCode::Enter => { app.recording.editing_output_dir = false; }
                    KeyCode::Backspace => { app.recording.output_directory.pop(); }
                    KeyCode::Char(c) => { app.recording.output_directory.push(c); }
                    _ => {}
                }
                return None;
            }
            if app.recording.editing_mic_cal {
                match key.code {
                    KeyCode::Enter => { app.recording.editing_mic_cal = false; }
                    KeyCode::Backspace => { app.recording.mic_calibration_path.pop(); }
                    KeyCode::Char(c) => { app.recording.mic_calibration_path.push(c); }
                    _ => {}
                }
                return None;
            }

            match key.code {
                KeyCode::Up => {
                    if app.recording.selected_field == 0 {
                        app.configure_tab_focused = true;
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
                    match app.recording.selected_field {
                        8 => { app.recording.editing_output_dir = true; }
                        9 => { app.recording.editing_mic_cal = true; }
                        _ => {}
                    }
                }
                KeyCode::Left | KeyCode::Right => {
                    let delta = if key.code == KeyCode::Right { 1i32 } else { -1 };
                    adjust_recording_field(app, delta);
                }
                KeyCode::Tab => {
                    init_recording_channels(app);
                    app.recording.step = RecordingStep::Capture;
                }
                _ => {}
            }
            None
        }

        RecordingStep::Capture => match key.code {
            KeyCode::Up => {
                if let Some(ch) = app.recording.current_channel {
                    if ch > 0 {
                        app.recording.current_channel = Some(ch - 1);
                    }
                }
                None
            }
            KeyCode::Down => {
                if let Some(ch) = app.recording.current_channel {
                    if ch + 1 < app.recording.channel_recordings.len() {
                        app.recording.current_channel = Some(ch + 1);
                    }
                }
                None
            }
            KeyCode::Enter => {
                if let Some(idx) = app.recording.current_channel {
                    if let Some(rec) = app.recording.channel_recordings.get_mut(idx) {
                        use sotf_audio_player::recording_types::ChannelRecordingState;
                        rec.state = match rec.state {
                            ChannelRecordingState::Empty => ChannelRecordingState::Recording,
                            ChannelRecordingState::Recording => ChannelRecordingState::Done,
                            ChannelRecordingState::Done => ChannelRecordingState::Empty,
                            ChannelRecordingState::Error => ChannelRecordingState::Empty,
                        };
                    }
                }
                None
            }
            KeyCode::Tab => {
                app.recording.step = RecordingStep::Evaluating;
                None
            }
            _ => None,
        },

        RecordingStep::Evaluating => match key.code {
            KeyCode::Tab => {
                if can_save_recordings(app) {
                    app.recording.editing_save_name = false;
                    app.recording.step = RecordingStep::Saving;
                }
                None
            }
            _ => None,
        },

        RecordingStep::Saving => match key.code {
            KeyCode::Up => {
                if app.recording.selected_save_field > 0 {
                    app.recording.selected_save_field -= 1;
                }
                None
            }
            KeyCode::Down => {
                if app.recording.selected_save_field < 1 {
                    app.recording.selected_save_field += 1;
                }
                None
            }
            KeyCode::Enter => {
                match app.recording.selected_save_field {
                    0 => { app.recording.editing_save_name = true; }
                    1 => {
                        save_recordings(app);
                    }
                    _ => {}
                }
                None
            }
            KeyCode::Char(c) if app.recording.editing_save_name => {
                app.recording.save_name.push(c);
                None
            }
            KeyCode::Backspace if app.recording.editing_save_name => {
                app.recording.save_name.pop();
                None
            }
            _ => None,
        },
    }
}

fn adjust_recording_field(app: &mut App, delta: i32) {
    use sotf_audio_player::recording_types::{RecordingSignalType, SpeakerConfiguration};
    match app.recording.selected_field {
        0 => {
            // Playback device
            let count = app.recording.available_playback_devices.len();
            if count > 0 {
                let idx = app.recording.selected_playback_idx as i32 + delta;
                app.recording.selected_playback_idx = idx.max(0).min((count - 1) as i32) as usize;
            }
        }
        1 => {
            // Recording device
            let count = app.recording.available_recording_devices.len();
            if count > 0 {
                let idx = app.recording.selected_recording_idx as i32 + delta;
                app.recording.selected_recording_idx = idx.max(0).min((count - 1) as i32) as usize;
            }
        }
        2 => {
            // Speaker configuration
            let configs = SpeakerConfiguration::all();
            if let Some(idx) = configs
                .iter()
                .position(|&c| c == app.recording.playback_config.speaker_configuration)
            {
                let new_idx = (idx as i32 + delta).max(0).min((configs.len() - 1) as i32) as usize;
                app.recording.playback_config.speaker_configuration = configs[new_idx];
                // Update channel mappings to match new config
                let names = app
                    .recording
                    .playback_config
                    .speaker_configuration
                    .default_channel_names();
                app.recording.playback_config.channel_mappings = names
                    .iter()
                    .enumerate()
                    .map(|(i, name)| {
                        sotf_audio_player::recording_types::ChannelMapping::single(i, *name)
                    })
                    .collect();
                app.recording.playback_config.sync_channel_count();
            }
        }
        3 => {
            // Signal type
            let types = RecordingSignalType::all();
            if let Some(idx) = types.iter().position(|&t| t == app.recording.signal_type) {
                let new_idx = (idx as i32 + delta).max(0).min((types.len() - 1) as i32) as usize;
                app.recording.signal_type = types[new_idx];
            }
        }
        4 => {
            // Duration
            app.recording.signal_duration_secs =
                (app.recording.signal_duration_secs + delta as f32 * 0.5).max(0.5);
        }
        5 => {
            // Level dB
            app.recording.signal_level_db =
                (app.recording.signal_level_db + delta as f32).clamp(-60.0, 0.0);
        }
        6 => {
            // Sweep start freq
            app.recording.sweep_start_freq =
                (app.recording.sweep_start_freq + delta as f32 * 5.0).max(5.0);
        }
        7 => {
            // Sweep end freq
            app.recording.sweep_end_freq =
                (app.recording.sweep_end_freq + delta as f32 * 500.0).max(100.0);
        }
        _ => {}
    }
}

pub fn init_recording_channels(app: &mut App) {
    use sotf_audio_player::recording_types::{ChannelRecording, ChannelRecordingState};

    let names = app
        .recording
        .playback_config
        .speaker_configuration
        .default_channel_names();

    app.recording.channel_recordings = names
        .iter()
        .enumerate()
        .map(|(i, name)| ChannelRecording {
            channel_index: i,
            channel_name: name.to_string(),
            state: ChannelRecordingState::Empty,
            result: None,
        })
        .collect();

    app.recording.current_channel = if !names.is_empty() { Some(0) } else { None };
}

pub fn can_save_recordings(app: &App) -> bool {
    use sotf_audio_player::recording_types::ChannelRecordingState;

    if app.recording.channel_recordings.is_empty() {
        return false;
    }

    if app.recording.save_name.contains('/') || app.recording.save_name.contains('\\') {
        return false;
    }

    app.recording
        .channel_recordings
        .iter()
        .all(|ch| ch.state == ChannelRecordingState::Done)
}

fn save_recordings(app: &mut App) {
    use sotf_audio_player::recording_types::RecordingStep;

    let output_dir = &app.recording.output_directory;
    let save_name = &app.recording.save_name;

    if output_dir.is_empty() || save_name.is_empty() {
        app.recording.save_error = Some("Output dir and session name required".to_string());
        return;
    }

    match std::fs::create_dir_all(output_dir) {
        Ok(_) => {
            app.recording.step = RecordingStep::Config;
            app.recording.save_error = None;
        }
        Err(e) => {
            app.recording.save_error = Some(e.to_string());
        }
    }
}
