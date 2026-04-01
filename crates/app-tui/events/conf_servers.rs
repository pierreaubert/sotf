use super::PlayerCommand;
use crate::app::{App, InputMode, ServerSection};
use crossterm::event::{KeyCode, KeyEvent};

pub(super) fn handle_server_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    let state = &mut app.server_state;

    if state.editing_value {
        match key.code {
            KeyCode::Enter => {
                apply_edit(state);
                state.editing_value = false;
            }
            KeyCode::Esc => {
                state.editing_value = false;
            }
            KeyCode::Backspace => {
                state.edit_buffer.pop();
            }
            KeyCode::Char(c) => {
                state.edit_buffer.push(c);
            }
            _ => {}
        }
        // Save on every change
        save_server_config(app);
        return None;
    }

    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Configure;
        }
        KeyCode::Left | KeyCode::Right => {
            state.selected_section = match state.selected_section {
                ServerSection::Mpd => ServerSection::Dlna,
                ServerSection::Dlna => ServerSection::Mpd,
            };
            state.selected_field = 0;
        }
        KeyCode::Tab => {
            state.selected_section = match state.selected_section {
                ServerSection::Mpd => ServerSection::Dlna,
                ServerSection::Dlna => ServerSection::Mpd,
            };
            state.selected_field = 0;
        }
        KeyCode::Up => {
            if state.selected_field > 0 {
                state.selected_field -= 1;
            }
        }
        KeyCode::Down => {
            let max = field_count(state.selected_section);
            if state.selected_field + 1 < max {
                state.selected_field += 1;
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            // Toggle booleans, enter edit mode for text/numbers
            match state.selected_section {
                ServerSection::Mpd => match state.selected_field {
                    0 => {
                        // enabled toggle
                        state.config.mpd.enabled = !state.config.mpd.enabled;
                        save_server_config(app);
                    }
                    1 => {
                        // bind_address
                        state.edit_buffer = state.config.mpd.bind_address.clone();
                        state.editing_value = true;
                    }
                    2 => {
                        // port
                        state.edit_buffer = state.config.mpd.port.to_string();
                        state.editing_value = true;
                    }
                    3 => {
                        // tls_enabled toggle
                        state.config.mpd.tls_enabled = !state.config.mpd.tls_enabled;
                        save_server_config(app);
                    }
                    4 => {
                        // password
                        state.edit_buffer = state.config.mpd.password.clone().unwrap_or_default();
                        state.editing_value = true;
                    }
                    _ => {}
                },
                ServerSection::Dlna => match state.selected_field {
                    0 => {
                        state.config.dlna.enabled = !state.config.dlna.enabled;
                        save_server_config(app);
                    }
                    1 => {
                        state.edit_buffer = state.config.dlna.friendly_name.clone();
                        state.editing_value = true;
                    }
                    2 => {
                        state.edit_buffer = state.config.dlna.port.to_string();
                        state.editing_value = true;
                    }
                    _ => {}
                },
            }
        }
        _ => {}
    }
    None
}

fn apply_edit(state: &mut crate::app::ServersTuiState) {
    let value = state.edit_buffer.clone();
    match state.selected_section {
        ServerSection::Mpd => match state.selected_field {
            1 => state.config.mpd.bind_address = value,
            2 => {
                if let Ok(p) = value.parse() {
                    state.config.mpd.port = p;
                }
            }
            4 => {
                state.config.mpd.password = if value.is_empty() { None } else { Some(value) };
            }
            _ => {}
        },
        ServerSection::Dlna => match state.selected_field {
            1 => state.config.dlna.friendly_name = value,
            2 => {
                if let Ok(p) = value.parse() {
                    state.config.dlna.port = p;
                }
            }
            _ => {}
        },
    }
}

fn field_count(section: ServerSection) -> usize {
    match section {
        ServerSection::Mpd => 5,  // enabled, bind, port, tls, password
        ServerSection::Dlna => 3, // enabled, name, port
    }
}

fn save_server_config(app: &App) {
    let _ = sotf_audio_player::config::save_server_config(&app.server_state.config);
}
