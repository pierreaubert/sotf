#![allow(clippy::field_reassign_with_default)]
use super::PlayerCommand;
use crate::app::{App, InputMode, ServerSection};
use crossterm::event::{KeyCode, KeyEvent};
use sotf_audio_player::federation_config::MpdAuthMode;
use sotf_audio_player::server::normalize_certificate_fingerprint;

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
            state.selected_section = next_section(state.selected_section, key.code);
            state.selected_field = 0;
        }
        KeyCode::Tab => {
            state.selected_section = next_section(state.selected_section, KeyCode::Right);
            state.selected_field = 0;
        }
        KeyCode::Up if state.selected_field > 0 => {
            state.selected_field -= 1;
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
                        // auth_mode toggle
                        toggle_mpd_auth_mode(state);
                        save_server_config(app);
                    }
                    5 => {
                        // password
                        state.edit_buffer = state.config.mpd.password.clone().unwrap_or_default();
                        state.editing_value = true;
                    }
                    6 => {
                        // trusted client fingerprints
                        state.edit_buffer = state.config.mpd.trusted_client_fingerprints.join(", ");
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
                        state.edit_buffer = state.config.dlna.bind_address.clone();
                        state.editing_value = true;
                    }
                    2 => {
                        state.edit_buffer = state.config.dlna.friendly_name.clone();
                        state.editing_value = true;
                    }
                    3 => {
                        state.edit_buffer = state.config.dlna.port.to_string();
                        state.editing_value = true;
                    }
                    _ => {}
                },
                ServerSection::Api => match state.selected_field {
                    0 => {
                        state.config.api.enabled = !state.config.api.enabled;
                        if state.config.api.enabled
                            && state
                                .config
                                .api
                                .auth_token
                                .as_deref()
                                .unwrap_or_default()
                                .is_empty()
                        {
                            state.config.api.auth_token =
                                Some(sotf_audio_player::server::generate_api_auth_token());
                        }
                        save_server_config(app);
                    }
                    1 => {
                        state.edit_buffer = state.config.api.bind_address.clone();
                        state.editing_value = true;
                    }
                    2 => {
                        state.edit_buffer = state.config.api.port.to_string();
                        state.editing_value = true;
                    }
                    3 => {
                        state.edit_buffer = state.config.api.friendly_name.clone();
                        state.editing_value = true;
                    }
                    4 => {
                        state.edit_buffer = state.config.api.auth_token.clone().unwrap_or_default();
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

fn next_section(section: ServerSection, key: KeyCode) -> ServerSection {
    match (section, key) {
        (ServerSection::Mpd, KeyCode::Left) => ServerSection::Api,
        (ServerSection::Mpd, _) => ServerSection::Dlna,
        (ServerSection::Dlna, KeyCode::Left) => ServerSection::Mpd,
        (ServerSection::Dlna, _) => ServerSection::Api,
        (ServerSection::Api, KeyCode::Left) => ServerSection::Dlna,
        (ServerSection::Api, _) => ServerSection::Mpd,
    }
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
            5 => {
                state.config.mpd.password = if value.is_empty() { None } else { Some(value) };
            }
            6 => {
                state.config.mpd.trusted_client_fingerprints =
                    parse_trusted_client_fingerprints(&value);
            }
            _ => {}
        },
        ServerSection::Dlna => match state.selected_field {
            1 => state.config.dlna.bind_address = value,
            2 => state.config.dlna.friendly_name = value,
            3 => {
                if let Ok(p) = value.parse() {
                    state.config.dlna.port = p;
                }
            }
            _ => {}
        },
        ServerSection::Api => match state.selected_field {
            1 => state.config.api.bind_address = value,
            2 => {
                if let Ok(p) = value.parse() {
                    state.config.api.port = p;
                }
            }
            3 => state.config.api.friendly_name = value,
            4 => {
                state.config.api.auth_token = if value.trim().is_empty() {
                    None
                } else {
                    Some(value)
                };
            }
            _ => {}
        },
    }
}

fn field_count(section: ServerSection) -> usize {
    match section {
        ServerSection::Mpd => 7, // enabled, bind, port, tls, auth, password, fingerprints
        ServerSection::Dlna => 4, // enabled, bind, name, port
        ServerSection::Api => 5, // enabled, bind, port, name, token
    }
}

fn toggle_mpd_auth_mode(state: &mut crate::app::ServersTuiState) {
    state.config.mpd.auth_mode = match state.config.mpd.auth_mode {
        MpdAuthMode::Certificate => MpdAuthMode::Password,
        MpdAuthMode::Password => MpdAuthMode::Certificate,
    };
}

fn parse_trusted_client_fingerprints(value: &str) -> Vec<String> {
    value
        .split(|ch: char| ch == ',' || ch == ';' || ch.is_ascii_whitespace())
        .map(str::trim)
        .filter(|fingerprint| !fingerprint.is_empty())
        .map(|fingerprint| {
            normalize_certificate_fingerprint(fingerprint)
                .unwrap_or_else(|_| fingerprint.to_string())
        })
        .collect()
}

fn save_server_config(app: &App) {
    if let Err(err) = sotf_audio_player::config::save_server_config(&app.server_state.config) {
        log::warn!("Failed to save server config: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ServersTuiState;

    #[test]
    fn mpd_auth_mode_toggle_switches_between_certificate_and_password() {
        let mut state = ServersTuiState::default();
        assert_eq!(state.config.mpd.auth_mode, MpdAuthMode::Certificate);

        toggle_mpd_auth_mode(&mut state);
        assert_eq!(state.config.mpd.auth_mode, MpdAuthMode::Password);

        toggle_mpd_auth_mode(&mut state);
        assert_eq!(state.config.mpd.auth_mode, MpdAuthMode::Certificate);
    }

    #[test]
    fn trusted_client_fingerprint_edit_parses_comma_separated_values() {
        let mut state = ServersTuiState::default();
        state.selected_section = ServerSection::Mpd;
        state.selected_field = 6;
        let first = "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899";
        let second = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        state.edit_buffer = format!(" {first} , , {second} ");

        apply_edit(&mut state);

        assert_eq!(
            state.config.mpd.trusted_client_fingerprints,
            vec![
                "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99".to_string(),
                "00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF".to_string(),
            ]
        );
    }

    #[test]
    fn trusted_client_fingerprint_edit_preserves_invalid_values_for_startup_error() {
        let mut state = ServersTuiState::default();
        state.selected_section = ServerSection::Mpd;
        state.selected_field = 6;
        state.edit_buffer = "not-a-fingerprint".to_string();

        apply_edit(&mut state);

        assert_eq!(
            state.config.mpd.trusted_client_fingerprints,
            vec!["not-a-fingerprint".to_string()]
        );
    }

    #[test]
    fn dlna_bind_address_edit_updates_config() {
        let mut state = ServersTuiState::default();
        state.selected_section = ServerSection::Dlna;
        state.selected_field = 1;
        state.edit_buffer = "192.168.1.42".to_string();

        apply_edit(&mut state);

        assert_eq!(state.config.dlna.bind_address, "192.168.1.42");
    }

    #[test]
    fn server_section_navigation_cycles_through_api_mpd_and_dlna() {
        assert_eq!(
            next_section(ServerSection::Api, KeyCode::Right),
            ServerSection::Mpd
        );
        assert_eq!(
            next_section(ServerSection::Mpd, KeyCode::Right),
            ServerSection::Dlna
        );
        assert_eq!(
            next_section(ServerSection::Dlna, KeyCode::Right),
            ServerSection::Api
        );
        assert_eq!(
            next_section(ServerSection::Api, KeyCode::Left),
            ServerSection::Dlna
        );
        assert_eq!(
            next_section(ServerSection::Dlna, KeyCode::Left),
            ServerSection::Mpd
        );
        assert_eq!(
            next_section(ServerSection::Mpd, KeyCode::Left),
            ServerSection::Api
        );
    }

    #[test]
    fn api_edits_update_config() {
        let mut state = ServersTuiState::default();
        state.selected_section = ServerSection::Api;

        state.selected_field = 1;
        state.edit_buffer = "192.168.1.42".to_string();
        apply_edit(&mut state);
        assert_eq!(state.config.api.bind_address, "192.168.1.42");

        state.selected_field = 2;
        state.edit_buffer = "9876".to_string();
        apply_edit(&mut state);
        assert_eq!(state.config.api.port, 9876);

        state.selected_field = 3;
        state.edit_buffer = "Listening Room".to_string();
        apply_edit(&mut state);
        assert_eq!(state.config.api.friendly_name, "Listening Room");

        state.selected_field = 4;
        state.edit_buffer = "secret-token".to_string();
        apply_edit(&mut state);
        assert_eq!(state.config.api.auth_token.as_deref(), Some("secret-token"));

        state.edit_buffer = "   ".to_string();
        apply_edit(&mut state);
        assert_eq!(state.config.api.auth_token, None);
    }
}
