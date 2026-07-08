use super::mpd_client_auth_mode::MpdClientAuthMode;
use super::source_connection_config::SourceConnectionConfig;
use super::types::{MpdAuthMode, ServerConfig};

#[test]
fn test_source_connection_roundtrip() {
    let configs = vec![
        SourceConnectionConfig::Subsonic {
            url: "https://music.example.com".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            legacy_auth: false,
        },
        SourceConnectionConfig::Mpd {
            host: "192.168.1.5".to_string(),
            port: 6600,
            auth_mode: MpdClientAuthMode::Ssl,
            password: None,
            httpd_port: 6601,
        },
        SourceConnectionConfig::Dlna {
            location_url: Some("http://192.168.1.10:8200/description.xml".to_string()),
            friendly_name: Some("Living Room".to_string()),
        },
        SourceConnectionConfig::Peer {
            host: "10.0.0.5".to_string(),
            port: 8732,
            accepted_fingerprint: Some("AA:BB:CC".to_string()),
            auth_token: Some("secret-token".to_string()),
        },
    ];

    for config in &configs {
        let json = serde_json::to_string(config).expect("serialize");
        let back: SourceConnectionConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, &back);
    }
}

#[test]
fn test_server_config_defaults() {
    let json = "{}";
    let config: ServerConfig = serde_json::from_str(json).expect("deserialize");
    assert!(!config.mpd.enabled);
    assert!(config.mpd.tls_enabled);
    assert_eq!(config.mpd.port, 6600);
    assert!(!config.dlna.enabled);
    assert_eq!(config.dlna.bind_address, "0.0.0.0");
    assert_eq!(config.dlna.port, 8200);
    assert!(!config.api.enabled);
    assert_eq!(config.api.bind_address, "0.0.0.0");
    assert_eq!(config.api.port, 8732);
    assert_eq!(config.api.friendly_name, "SOTF Player");
    assert!(config.api.tls_enabled);
    assert!(config.api.auth_token.is_none());
}

#[test]
fn test_field_get_set() {
    let mut config = SourceConnectionConfig::Subsonic {
        url: "https://example.com".to_string(),
        username: "user".to_string(),
        password: "pass".to_string(),
        legacy_auth: false,
    };

    assert_eq!(config.field_value(0), "https://example.com");
    config.set_field_value(0, "https://new.com");
    assert_eq!(config.field_value(0), "https://new.com");

    assert_eq!(config.field_names().len(), 4);
}

#[test]
fn test_default_for_type() {
    let sub = SourceConnectionConfig::default_for_type("subsonic");
    assert_eq!(sub.type_name(), "Subsonic");

    let mpd = SourceConnectionConfig::default_for_type("mpd");
    assert_eq!(mpd.type_name(), "MPD");

    let dlna = SourceConnectionConfig::default_for_type("dlna");
    assert_eq!(dlna.type_name(), "DLNA");

    let peer = SourceConnectionConfig::default_for_type("peer");
    assert_eq!(peer.type_name(), "Peer");
    assert_eq!(
        peer.field_names(),
        vec!["Host", "Port", "Fingerprint", "API Token"]
    );
    assert_eq!(peer.field_value(1), "8732");

    let tidal = SourceConnectionConfig::default_for_type("tidal");
    assert_eq!(tidal.type_name(), "Tidal");

    let spotify = SourceConnectionConfig::default_for_type("spotify");
    assert_eq!(spotify.type_name(), "Spotify");

    let radio = SourceConnectionConfig::default_for_type("icy_radio");
    assert_eq!(radio.type_name(), "Radio");
}

#[test]
fn test_tidal_config_roundtrip() {
    let config = SourceConnectionConfig::Tidal {
        access_token: "tok_abc123".to_string(),
        quality: "LOSSLESS".to_string(),
        country_code: "FR".to_string(),
    };
    let json = serde_json::to_string(&config).expect("serialize");
    let back: SourceConnectionConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(config, back);
}

#[test]
fn test_spotify_config_roundtrip() {
    let config = SourceConnectionConfig::Spotify {
        username: "user@example.com".to_string(),
        password: "s3cret".to_string(),
        quality: "High".to_string(),
    };
    let json = serde_json::to_string(&config).expect("serialize");
    let back: SourceConnectionConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(config, back);
}

#[test]
fn test_icy_radio_config_roundtrip() {
    let config = SourceConnectionConfig::IcyRadio {
        url: "http://radio.example.com:8000/stream".to_string(),
        name: "Jazz FM".to_string(),
    };
    let json = serde_json::to_string(&config).expect("serialize");
    let back: SourceConnectionConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(config, back);
}

#[test]
fn test_tidal_field_get_set() {
    let mut config = SourceConnectionConfig::default_for_type("tidal");
    assert_eq!(config.field_names().len(), 3);
    assert_eq!(config.field_names()[0], "Access Token");

    config.set_field_value(0, "my_token");
    // Tokens are masked in display
    assert_eq!(config.field_value(0), "********");

    config.set_field_value(1, "HI_RES_LOSSLESS");
    assert_eq!(config.field_value(1), "HI_RES_LOSSLESS");

    config.set_field_value(2, "GB");
    assert_eq!(config.field_value(2), "GB");
}

#[test]
fn test_spotify_field_get_set() {
    let mut config = SourceConnectionConfig::default_for_type("spotify");
    assert_eq!(config.field_names().len(), 3);

    config.set_field_value(0, "myuser");
    assert_eq!(config.field_value(0), "myuser");

    config.set_field_value(1, "mypass");
    // Password is masked
    assert_eq!(config.field_value(1), "******");

    config.set_field_value(2, "Normal");
    assert_eq!(config.field_value(2), "Normal");
}

#[test]
fn test_icy_radio_field_get_set() {
    let mut config = SourceConnectionConfig::default_for_type("icy_radio");
    assert_eq!(config.field_names().len(), 2);
    assert_eq!(config.field_names()[0], "Stream URL");
    assert_eq!(config.field_names()[1], "Station Name");

    config.set_field_value(0, "http://radio.example.com:8000/stream");
    assert_eq!(
        config.field_value(0),
        "http://radio.example.com:8000/stream"
    );

    config.set_field_value(1, "My Radio");
    assert_eq!(config.field_value(1), "My Radio");
}

// =========================================================================
// set_field_value comprehensive coverage
// =========================================================================

#[test]
fn test_set_field_value_subsonic() {
    let mut config = SourceConnectionConfig::default_for_type("subsonic");
    config.set_field_value(0, "https://new.example.com");
    assert_eq!(config.field_value(0), "https://new.example.com");

    config.set_field_value(1, "newuser");
    assert_eq!(config.field_value(1), "newuser");

    config.set_field_value(2, "secret");
    assert_eq!(config.field_value(2), "******");

    config.set_field_value(3, "true");
    assert_eq!(config.field_value(3), "true");

    config.set_field_value(3, "false");
    assert_eq!(config.field_value(3), "false");
}

#[test]
fn test_set_field_value_mpd() {
    let mut config = SourceConnectionConfig::default_for_type("mpd");

    config.set_field_value(0, "  192.168.1.5  ");
    assert_eq!(config.field_value(0), "192.168.1.5");

    config.set_field_value(1, "6601");
    assert_eq!(config.field_value(1), "6601");

    config.set_field_value(2, "Password");
    assert_eq!(config.field_value(2), "Password");

    config.set_field_value(3, "mypassword");
    assert_eq!(config.field_value(3), "********");

    config.set_field_value(3, "");
    assert_eq!(config.field_value(3), "");

    config.set_field_value(4, "6602");
    assert_eq!(config.field_value(4), "6602");
}

#[test]
fn test_set_field_value_mpd_port_parse_error() {
    let mut config = SourceConnectionConfig::default_for_type("mpd");
    let original_port = config.field_value(1);
    config.set_field_value(1, "not_a_number");
    assert_eq!(config.field_value(1), original_port);
}

#[test]
fn test_set_field_value_mpd_auth_mode_variants() {
    let mut config = SourceConnectionConfig::Mpd {
        host: "localhost".to_string(),
        port: 6600,
        auth_mode: MpdClientAuthMode::None,
        password: None,
        httpd_port: 6601,
    };

    config.set_field_value(2, "Password");
    assert_eq!(config.field_value(2), "Password");

    config.set_field_value(2, "SSL");
    assert_eq!(config.field_value(2), "SSL");

    config.set_field_value(2, "None");
    assert_eq!(config.field_value(2), "None");

    config.set_field_value(2, "Unknown");
    assert_eq!(config.field_value(2), "None");
}

#[test]
fn test_set_field_value_dlna_empty_becomes_none() {
    let mut config = SourceConnectionConfig::default_for_type("dlna");

    config.set_field_value(0, "http://example.com");
    assert_eq!(config.field_value(0), "http://example.com");

    config.set_field_value(0, "");
    assert_eq!(config.field_value(0), "");

    config.set_field_value(1, "Living Room");
    assert_eq!(config.field_value(1), "Living Room");

    config.set_field_value(1, "");
    assert_eq!(config.field_value(1), "");
}

#[test]
fn test_set_field_value_peer() {
    let mut config = SourceConnectionConfig::default_for_type("peer");

    config.set_field_value(0, "  10.0.0.5  ");
    assert_eq!(config.field_value(0), "10.0.0.5");

    config.set_field_value(1, "8733");
    assert_eq!(config.field_value(1), "8733");

    config.set_field_value(2, "AA:BB:CC");
    assert_eq!(config.field_value(2), "AA:BB:CC");

    config.set_field_value(2, "");
    assert_eq!(config.field_value(2), "");

    config.set_field_value(2, "  DD:EE:FF  ");
    assert_eq!(config.field_value(2), "DD:EE:FF");

    config.set_field_value(3, "mytoken");
    assert_eq!(config.field_value(3), "*******");

    config.set_field_value(3, "");
    assert_eq!(config.field_value(3), "");

    config.set_field_value(3, "  ");
    assert_eq!(config.field_value(3), "");
}

#[test]
fn test_set_field_value_tidal() {
    let mut config = SourceConnectionConfig::default_for_type("tidal");

    config.set_field_value(0, "token123");
    assert_eq!(config.field_value(0), "********");

    config.set_field_value(1, "HI_RES_LOSSLESS");
    assert_eq!(config.field_value(1), "HI_RES_LOSSLESS");

    config.set_field_value(2, "US");
    assert_eq!(config.field_value(2), "US");
}

#[test]
fn test_set_field_value_spotify() {
    let mut config = SourceConnectionConfig::default_for_type("spotify");

    config.set_field_value(0, "user123");
    assert_eq!(config.field_value(0), "user123");

    config.set_field_value(1, "pass123");
    assert_eq!(config.field_value(1), "*******");

    config.set_field_value(2, "Very High");
    assert_eq!(config.field_value(2), "Very High");
}

#[test]
fn test_set_field_value_icy_radio() {
    let mut config = SourceConnectionConfig::default_for_type("icy_radio");

    config.set_field_value(0, "http://radio.example.com:8000/stream");
    assert_eq!(
        config.field_value(0),
        "http://radio.example.com:8000/stream"
    );

    config.set_field_value(1, "My Station");
    assert_eq!(config.field_value(1), "My Station");
}

#[test]
fn test_set_field_value_out_of_bounds() {
    let mut config = SourceConnectionConfig::default_for_type("subsonic");
    // Setting an out-of-bounds index should be a no-op (not panic)
    config.set_field_value(99, "value");
    // Values should remain unchanged
    assert_eq!(config.field_value(0), "https://");
}

// =========================================================================
// ServerConfig schema / version compatibility tests (QA-CORE-001)
// =========================================================================

#[test]
fn server_config_empty_json_defaults() {
    let json = "{}";
    let config: ServerConfig = serde_json::from_str(json).unwrap();
    assert!(!config.mpd.enabled);
    assert_eq!(config.mpd.bind_address, "0.0.0.0");
    assert_eq!(config.mpd.port, 6600);
    assert!(config.mpd.tls_enabled);
    assert_eq!(config.mpd.auth_mode, MpdAuthMode::Certificate);
    assert!(config.mpd.password.is_none());
    assert!(config.mpd.trusted_client_fingerprints.is_empty());
    assert!(!config.dlna.enabled);
    assert_eq!(config.dlna.bind_address, "0.0.0.0");
    assert_eq!(config.dlna.friendly_name, "SOTF Media Server");
    assert_eq!(config.dlna.port, 8200);
    assert!(!config.api.enabled);
    assert_eq!(config.api.bind_address, "0.0.0.0");
    assert_eq!(config.api.port, 8732);
    assert_eq!(config.api.friendly_name, "SOTF Player");
    assert!(config.api.tls_enabled);
    assert!(config.api.auth_token.is_none());
}

#[test]
fn server_config_ignores_unknown_fields() {
    let json = r#"{
        "mpd": {"enabled": true},
        "dlna": {"enabled": true},
        "api": {"enabled": true},
        "future_field": "ignored",
        "unknown_nested": {"x": 1}
    }"#;

    let config: ServerConfig = serde_json::from_str(json).unwrap();
    assert!(config.mpd.enabled);
    assert!(config.dlna.enabled);
    assert!(config.api.enabled);
}

#[test]
fn server_config_serde_roundtrip() {
    let config = ServerConfig {
        mpd: super::mpd_settings::MpdSettings {
            enabled: true,
            bind_address: "127.0.0.1".into(),
            port: 6601,
            tls_enabled: false,
            auth_mode: MpdAuthMode::Password,
            password: Some("secret".into()),
            trusted_client_fingerprints: vec!["aa:bb".into()],
        },
        dlna: super::dlna_settings::DlnaSettings {
            enabled: true,
            bind_address: "127.0.0.1".into(),
            friendly_name: "Test".into(),
            port: 8201,
        },
        api: super::sotf_api_settings::SotfApiSettings {
            enabled: true,
            bind_address: "127.0.0.1".into(),
            port: 8733,
            friendly_name: "Test API".into(),
            tls_enabled: false,
            auth_token: Some("token".into()),
        },
    };

    let json = serde_json::to_string(&config).unwrap();
    let decoded: ServerConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.mpd.enabled, config.mpd.enabled);
    assert_eq!(decoded.mpd.port, config.mpd.port);
    assert_eq!(decoded.dlna.port, config.dlna.port);
    assert_eq!(decoded.api.port, config.api.port);
    assert_eq!(decoded.api.auth_token, config.api.auth_token);
}
