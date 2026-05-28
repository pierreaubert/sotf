use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use sotf_audio_player::lan_discovery::DiscoveredSotfApiServer;
use sotf_audio_player_gpui::app::state::app::RemoteState;

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
