use super::server_state::ServerState;
use crate::federation_config::{self, ServerConfig};
use serde_json::{Value, json};
use sotf_mpd::{MpdAuthMode, MpdPlayState, MpdServerConfig, MpdSongInfo};
use std::sync::Arc;

pub(super) fn mpd_song_json(song: &MpdSongInfo) -> Value {
    json!({
        "file": &song.file,
        "title": &song.title,
        "artist": &song.artist,
        "album": &song.album,
        "track": &song.track,
        "date": &song.date,
        "genre": &song.genre,
        "duration_secs": song.duration,
        "pos": song.pos,
        "id": song.id,
    })
}

pub(super) fn mpd_state_name(state: &MpdPlayState) -> &'static str {
    match state {
        MpdPlayState::Play => "play",
        MpdPlayState::Pause => "pause",
        MpdPlayState::Stop => "stop",
    }
}

/// Convert the persisted `MpdSettings` into the `MpdServerConfig` used by the server.
pub(super) fn mpd_settings_to_config(
    config: &ServerConfig,
    state: &Arc<ServerState>,
) -> MpdServerConfig {
    let settings = &config.mpd;
    MpdServerConfig {
        bind_address: settings.bind_address.clone(),
        port: settings.port,
        tls_enabled: settings.tls_enabled,
        auth_mode: match settings.auth_mode {
            federation_config::MpdAuthMode::Certificate => MpdAuthMode::Certificate,
            federation_config::MpdAuthMode::Password => MpdAuthMode::Password,
        },
        password: settings.password.clone(),
        trusted_client_fingerprints: Arc::clone(&state.trusted_client_fingerprints),
    }
}
