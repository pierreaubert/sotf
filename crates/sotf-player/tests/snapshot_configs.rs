//! Snapshot tests for SOTF player configurations and server helpers.

use sotf_audio_player::{
    federation_config::{MpdClientAuthMode, SourceConnectionConfig},
    room_eq_types::RoomEqOptimizerConfig,
    server::api_parse_range_header,
};

#[test]
fn snapshot_source_connection_config_variants() {
    let configs = vec![
        SourceConnectionConfig::Subsonic {
            url: "https://demo.subsonic.org".into(),
            username: "demo".into(),
            password: "demo".into(),
            legacy_auth: false,
        },
        SourceConnectionConfig::Mpd {
            host: "localhost".into(),
            port: 6600,
            auth_mode: MpdClientAuthMode::Password,
            password: Some("secret".into()),
            httpd_port: 6601,
        },
        SourceConnectionConfig::Dlna {
            location_url: Some("http://192.168.1.10:8200".into()),
            friendly_name: Some("Living Room".into()),
        },
        SourceConnectionConfig::Peer {
            host: "peer.example.com".into(),
            port: 8732,
            accepted_fingerprint: Some("ab:cd:ef".into()),
            auth_token: Some("token".into()),
        },
        SourceConnectionConfig::Tidal {
            access_token: "tidal-token".into(),
            quality: "HI_RES".into(),
            country_code: "FR".into(),
        },
        SourceConnectionConfig::Spotify {
            username: "user".into(),
            password: "pass".into(),
            quality: "VeryHigh".into(),
        },
        SourceConnectionConfig::IcyRadio {
            url: "http://stream.example/radio".into(),
            name: "Example Radio".into(),
        },
    ];

    insta::assert_json_snapshot!(configs);
}

#[test]
fn snapshot_room_eq_optimizer_config_default() {
    let config = RoomEqOptimizerConfig::default();
    insta::assert_json_snapshot!(config);
}

#[test]
fn snapshot_api_parse_range_header_results() {
    let cases = [
        ("no header", api_parse_range_header(None, 100)),
        ("full range", api_parse_range_header(Some("bytes=0-99"), 100)),
        ("open ended", api_parse_range_header(Some("bytes=10-"), 100)),
        ("suffix", api_parse_range_header(Some("bytes=-10"), 100)),
        ("end beyond file", api_parse_range_header(Some("bytes=0-999"), 100)),
        ("missing bytes prefix", api_parse_range_header(Some("0-99"), 100)),
        ("multiple ranges", api_parse_range_header(Some("bytes=0-9,10-19"), 100)),
        ("start at length", api_parse_range_header(Some("bytes=100-"), 100)),
        ("end before start", api_parse_range_header(Some("bytes=10-5"), 100)),
        ("zero suffix", api_parse_range_header(Some("bytes=-0"), 100)),
        ("zero file length", api_parse_range_header(Some("bytes=0-"), 0)),
    ];

    insta::assert_debug_snapshot!(cases);
}
