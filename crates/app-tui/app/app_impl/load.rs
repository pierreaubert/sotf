use super::super::types::ServersTuiState;

pub(super) fn load_server_tui_state() -> ServersTuiState {
    load_server_tui_state_with(sotf_audio_player::config::load_server_config)
}

pub(super) fn load_server_tui_state_with<E>(
    load_config: impl FnOnce() -> Result<sotf_audio_player::federation_config::ServerConfig, E>,
) -> ServersTuiState
where
    E: std::fmt::Display,
{
    match load_config() {
        Ok(config) => ServersTuiState::with_config(config),
        Err(err) => {
            log::warn!("Failed to load server config: {err}");
            ServersTuiState::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::types::ServerSection;
    use super::*;

    use sotf_audio_player::federation_config::ServerConfig;

    #[test]
    fn server_tui_state_uses_loaded_server_config() {
        let mut config = ServerConfig::default();
        config.mpd.enabled = true;
        config.mpd.port = 6601;
        config.dlna.enabled = true;
        config.dlna.friendly_name = "SOTF Test Server".to_string();

        let state = load_server_tui_state_with(|| Ok::<_, &str>(config));

        assert!(state.config.mpd.enabled);
        assert_eq!(state.config.mpd.port, 6601);
        assert!(state.config.dlna.enabled);
        assert_eq!(state.config.dlna.friendly_name, "SOTF Test Server");
        assert_eq!(state.selected_section, ServerSection::Api);
        assert_eq!(state.selected_field, 0);
        assert!(!state.editing_value);
    }

    #[test]
    fn server_tui_state_falls_back_to_defaults_when_config_load_fails() {
        let state = load_server_tui_state_with(|| Err("invalid server config"));

        assert!(!state.config.mpd.enabled);
        assert!(!state.config.dlna.enabled);
        assert_eq!(state.selected_section, ServerSection::Api);
        assert_eq!(state.selected_field, 0);
        assert!(!state.editing_value);
    }
}
