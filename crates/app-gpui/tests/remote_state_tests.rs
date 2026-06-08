use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use sotf_audio_player::lan_discovery::DiscoveredSotfApiServer;
use sotf_audio_player::sotf_api_client::{SotfApiAlbum, SotfApiAlbumList};
use sotf_audio_player_gpui::app::state::app::{
    RemoteAlbumCache, RemoteCacheRefreshError, RemoteRefreshRequests, RemoteServerProbeStatus,
    RemoteState,
};

fn discovered_server() -> DiscoveredSotfApiServer {
    DiscoveredSotfApiServer {
        instance_name: "Listening Room._sotf._tcp.local".to_string(),
        friendly_name: "Listening Room".to_string(),
        host_name: "listening-room.local".to_string(),
        address: Ipv4Addr::new(192, 168, 1, 23),
        port: 8732,
        protocol: "http".to_string(),
        api_path: "/api/v1".to_string(),
        auth: "bearer".to_string(),
        origin_url: "http://192.168.1.23:8732".to_string(),
        api_base_url: "http://192.168.1.23:8732/api/v1".to_string(),
        txt_records: BTreeMap::new(),
    }
}

#[test]
fn merges_discovered_servers_into_non_secret_store() {
    let mut state = RemoteState::default();
    let merged = state.merge_discovered_servers(vec![discovered_server()]);

    assert_eq!(merged, 1);
    assert_eq!(state.discovered_servers.len(), 1);
    assert!(state.server_store.selected_server().is_some());

    let json = serde_json::to_string(&state.server_store).unwrap();
    assert!(json.contains("Listening Room"));
    assert!(!json.contains("auth_token"));
    assert!(!json.contains("bearer-token"));
}

#[test]
fn manual_server_record_is_selected_and_non_secret() {
    let mut state = RemoteState::default();
    let id = state
        .add_manual_server_record("Desk", "http://desk.local:8732")
        .unwrap();

    assert_eq!(
        state.server_store.selected_server_id.as_deref(),
        Some(id.as_str())
    );
    assert_eq!(
        state.server_store.selected_server().unwrap().api_base_url,
        "http://desk.local:8732/api/v1"
    );

    let json = serde_json::to_string(&state.server_store).unwrap();
    assert!(json.contains("Desk"));
    assert!(!json.contains("auth_token"));
    assert!(!json.contains("bearer-token"));
}

#[test]
fn manual_input_add_clears_fields_and_keeps_json_non_secret() {
    let mut state = RemoteState::default();
    state.set_manual_server_name(" Desk ");
    state.set_manual_api_base_url(" http://desk.local:8732 ");
    state.set_manual_auth_token(" very-secret-token ");

    let id = state.add_manual_server_from_inputs().unwrap();

    assert_eq!(
        state.server_store.selected_server_id.as_deref(),
        Some(id.as_str())
    );
    assert!(state.manual_server_name.is_empty());
    assert!(state.manual_api_base_url.is_empty());
    assert!(state.manual_auth_token.is_empty());
    assert_eq!(
        state.server_tokens.get(&id).map(String::as_str),
        Some("very-secret-token")
    );

    let json = serde_json::to_string(&state.server_store).unwrap();
    assert!(json.contains("Desk"));
    assert!(!json.contains("auth_token"));
    assert!(!json.contains("bearer-token"));
    assert!(!json.contains("very-secret-token"));
}

#[test]
fn manual_input_add_accepts_host_port_without_scheme() {
    let mut state = RemoteState::default();
    state.set_manual_api_base_url("192.168.1.102:8732");
    state.set_manual_auth_token("secret");

    state.add_manual_server_from_inputs().unwrap();

    assert_eq!(
        state.server_store.selected_server().unwrap().api_base_url,
        "http://192.168.1.102:8732/api/v1"
    );
}

#[test]
fn manual_input_add_requires_auth_token() {
    let mut state = RemoteState::default();
    state.set_manual_api_base_url("192.168.1.102:8732");

    let err = state.add_manual_server_from_inputs().unwrap_err();

    assert_eq!(err, "remote API token must not be empty");
    assert!(state.server_store.servers.is_empty());
}

#[test]
fn remote_probe_status_labels_are_user_readable() {
    assert_eq!(RemoteServerProbeStatus::Testing.label(), "testing");
    assert_eq!(
        RemoteServerProbeStatus::Reachable {
            friendly_name: "Desk".to_string(),
            version: "0.6.7".to_string(),
            auth_required: true,
            api_version: 1,
            media_range: true,
            events: true,
        }
        .label(),
        "reachable, auth required (0.6.7, media, events)"
    );
    assert_eq!(
        RemoteServerProbeStatus::Failed("connection refused".to_string()).label(),
        "failed: connection refused"
    );
}

fn remote_album(id: &str, title: &str) -> SotfApiAlbum {
    SotfApiAlbum {
        id: id.to_string(),
        title: title.to_string(),
        artist: "Artist".to_string(),
        year: Some(2024),
        track_count: 1,
        edition: None,
        dynamic_range: None,
        is_favorite: false,
        play_count: 0,
    }
}

#[test]
fn remote_album_cache_is_bounded_to_recent_metadata() {
    let mut cache = RemoteAlbumCache::with_limit(2);
    cache.upsert_metadata_page(
        "server-a",
        1,
        &[
            remote_album("one", "One"),
            remote_album("two", "Two"),
            remote_album("three", "Three"),
        ],
    );

    assert_eq!(cache.max_albums(), 2);
    assert_eq!(cache.metadata_len(), 2);
    assert!(cache.metadata("server-a", 1, "one").is_none());
    assert_eq!(
        cache.metadata("server-a", 1, "three").unwrap().title,
        "Three"
    );
}

#[test]
fn remote_album_cache_is_bounded_to_recent_artwork() {
    let mut cache = RemoteAlbumCache::with_limit(2);
    cache.upsert_artwork("server-a", 1, "one", vec![1]);
    cache.upsert_artwork("server-a", 1, "two", vec![2]);
    cache.upsert_artwork("server-a", 1, "three", vec![3]);

    assert_eq!(cache.artwork_len(), 2);
    assert!(cache.artwork("server-a", 1, "one").is_none());
    assert_eq!(cache.artwork("server-a", 1, "three"), Some(&[3][..]));
}

#[test]
fn remote_album_cache_is_not_persisted_with_server_store() {
    let mut state = RemoteState::default();
    state
        .album_cache
        .upsert_metadata_page("server-a", 1, &[remote_album("one", "One")]);

    let json = serde_json::to_string(&state.server_store).unwrap();
    assert!(!json.contains("One"));
    assert!(!json.contains("album_cache"));
}

#[test]
fn remote_album_cache_invalidates_selected_server() {
    let mut cache = RemoteAlbumCache::with_limit(10);
    cache.upsert_metadata_page("server-a", 1, &[remote_album("one", "One")]);
    cache.upsert_metadata_page("server-b", 1, &[remote_album("two", "Two")]);
    cache.upsert_artwork("server-a", 1, "one", vec![1]);

    cache.invalidate_server("server-a");

    assert!(cache.metadata("server-a", 1, "one").is_none());
    assert!(cache.artwork("server-a", 1, "one").is_none());
    assert!(cache.metadata("server-b", 1, "two").is_some());
}

#[test]
fn remote_state_applies_visible_album_page_without_local_db() {
    let mut state = RemoteState::default();
    state.apply_remote_album_page(
        "server-a",
        SotfApiAlbumList {
            albums: vec![remote_album("one", "One")],
            total: 1,
            offset: 0,
            limit: 50,
            library_version: 7,
        },
    );

    assert_eq!(state.current_album_page.as_ref().unwrap().total, 1);
    assert_eq!(
        state.current_album_page_server_id.as_deref(),
        Some("server-a")
    );
    assert!(state.album_cache.metadata("server-a", 7, "one").is_some());
    assert!(state.server_store.servers.is_empty());
}

#[test]
fn remote_cache_refresh_failure_requeues_until_threshold() {
    let mut state = RemoteState::default();
    let requests = RemoteRefreshRequests {
        state: true,
        queue: false,
        visible_album_page: true,
    };

    state.record_remote_cache_refresh_failure(RemoteCacheRefreshError {
        requests,
        message: "timeout".to_string(),
    });

    assert_eq!(state.cache_refresh_failures, 1);
    assert!(!state.cache_updates_disabled);
    assert!(state.refresh_requests.state);
    assert!(state.refresh_requests.visible_album_page);
}

#[test]
fn remote_cache_refresh_disables_after_repeated_failures() {
    let mut state = RemoteState::default();
    let requests = RemoteRefreshRequests {
        state: true,
        queue: true,
        visible_album_page: true,
    };

    for _ in 0..RemoteState::CACHE_REFRESH_FAILURE_DISABLE_THRESHOLD {
        state.record_remote_cache_refresh_failure(RemoteCacheRefreshError {
            requests,
            message: "network unstable".to_string(),
        });
    }

    assert!(state.cache_updates_disabled);
    assert!(state.refresh_requests.is_empty());
    assert_eq!(
        state.cache_refresh_failures,
        RemoteState::CACHE_REFRESH_FAILURE_DISABLE_THRESHOLD
    );
}

#[test]
fn remote_cache_refresh_reset_reenables_background_updates() {
    let mut state = RemoteState::default();
    state.cache_updates_disabled = true;
    state.cache_refresh_failures = RemoteState::CACHE_REFRESH_FAILURE_DISABLE_THRESHOLD;
    state.cache_last_error = Some("network unstable".to_string());

    state.reset_remote_cache_updater();

    assert!(!state.cache_updates_disabled);
    assert_eq!(state.cache_refresh_failures, 0);
    assert!(state.cache_last_error.is_none());
}
