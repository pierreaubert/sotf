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
                    if app.recording.selected_field > 0 {
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
                        rec.toggle();
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
                app.recording.session_name.push(c);
                None
            }
            KeyCode::Backspace if app.recording.editing_save_name => {
                app.recording.session_name.pop();
                None
            }
            _ => None,
        },
    }
}

fn adjust_recording_field(app: &mut App, delta: i32) {
    use sotf_audio_player::recording_types::SpeakerConfiguration;
    match app.recording.selected_field {
        0 => {
            // Channel count
            let configs = [
                SpeakerConfiguration::Mono,
                SpeakerConfiguration::Stereo,
                SpeakerConfiguration::ThreePoint0,
                SpeakerConfiguration::FiveOne,
                SpeakerConfiguration::SevenOne,
            ];
            if let Some(idx) = configs.iter().position(|&c| c == app.recording.config) {
                let new_idx = (idx as i32 + delta).max(0).min((configs.len() - 1) as i32) as usize;
                app.recording.config = configs[new_idx];
            }
        }
        1..=7 => {
            // Channel mappings - adjust indices
            if let Some(rec) = app.recording.channel_recordings.get_mut(app.recording.selected_field - 1) {
                rec.adjust_input_index(delta);
            }
        }
        _ => {}
    }
}

pub fn update_channel_mappings_for_config(
    app: &mut App,
    config: sotf_audio_player::recording_types::SpeakerConfiguration,
) {
    app.recording.config = config;
    init_recording_channels(app);
}

pub fn init_recording_channels(app: &mut App) {
    use sotf_audio_player::recording_types::{ChannelRecordingState, SpeakerConfiguration};

    let channel_count = match app.recording.config {
        SpeakerConfiguration::Empty => 0,
        SpeakerConfiguration::Mono => 1,
        SpeakerConfiguration::Stereo => 2,
        SpeakerConfiguration::ThreePoint0 => 3,
        SpeakerConfiguration::FiveOne => 6,
        SpeakerConfiguration::SevenOne => 8,
    };

    app.recording.channels.clear();
    for i in 0..channel_count {
        app.recording.channels.push(sotf_audio_player::recording_types::ChannelMapping {
            index: i,
            input_index: i,
            label: format!("Ch {}", i + 1),
            state: ChannelRecordingState::NotStarted,
        });
    }

    app.recording.channel_recordings.clear();
    app.recording.current_channel = if channel_count > 0 { Some(0) } else { None };
}

pub fn can_save_recordings(app: &App) -> bool {
    use sotf_audio_player::recording_types::ChannelRecordingState;

    if app.recording.channels.is_empty() {
        return false;
    }

    if app.recording.session_name.contains('/') || app.recording.session_name.contains('\\') {
        return false;
    }

    app.recording.channels.iter().all(|ch| ch.state == ChannelRecordingState::Completed)
}

fn save_recordings(app: &mut App) {
    use sotf_audio_player::recording_types::RecordingStep;

    let output_dir = &app.recording.output_directory;
    let session_name = &app.recording.session_name;

    if output_dir.is_empty() || session_name.is_empty() {
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
