use super::default::default_country_code;
use super::default::default_httpd_port;
use super::default::default_mpd_port;
use super::default::default_sotf_api_port;
use super::default::default_spotify_quality;
use super::default::default_tidal_quality;
use super::mpd_client_auth_mode::MpdClientAuthMode;
use serde::{Deserialize, Serialize};

/// Per-source connection configuration.
///
/// Serialized as JSON into the `library_sources.config_json` column.
/// The `type` tag discriminant maps to `SourceType` in `sotf-federation`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum SourceConnectionConfig {
    Subsonic {
        url: String,
        username: String,
        password: String,
        #[serde(default)]
        legacy_auth: bool,
    },
    Mpd {
        host: String,
        #[serde(default = "default_mpd_port")]
        port: u16,
        #[serde(default)]
        auth_mode: MpdClientAuthMode,
        /// Password for password authentication.
        password: Option<String>,
        /// Port for MPD's httpd streaming output (default 6601).
        /// SOTF streams audio from this port during playback.
        #[serde(default = "default_httpd_port")]
        httpd_port: u16,
    },
    Dlna {
        location_url: Option<String>,
        friendly_name: Option<String>,
    },
    Peer {
        host: String,
        #[serde(default = "default_sotf_api_port")]
        port: u16,
        accepted_fingerprint: Option<String>,
        #[serde(default)]
        auth_token: Option<String>,
    },
    Tidal {
        #[serde(default)]
        access_token: String,
        #[serde(default = "default_tidal_quality")]
        quality: String,
        #[serde(default = "default_country_code")]
        country_code: String,
    },
    Spotify {
        #[serde(default)]
        username: String,
        #[serde(default)]
        password: String,
        #[serde(default = "default_spotify_quality")]
        quality: String,
    },
    IcyRadio {
        #[serde(default)]
        url: String,
        #[serde(default)]
        name: String,
    },
}

impl SourceConnectionConfig {
    /// Human-readable type name for UI display.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Subsonic { .. } => "Subsonic",
            Self::Mpd { .. } => "MPD",
            Self::Dlna { .. } => "DLNA",
            Self::Peer { .. } => "Peer",
            Self::Tidal { .. } => "Tidal",
            Self::Spotify { .. } => "Spotify",
            Self::IcyRadio { .. } => "Radio",
        }
    }

    /// Database/serde key for this source type (lowercase, used for storage).
    #[must_use]
    pub fn source_type_key(&self) -> &'static str {
        match self {
            Self::Subsonic { .. } => "subsonic",
            Self::Mpd { .. } => "mpd",
            Self::Dlna { .. } => "dlna",
            Self::Peer { .. } => "peer",
            Self::Tidal { .. } => "tidal",
            Self::Spotify { .. } => "spotify",
            Self::IcyRadio { .. } => "icy_radio",
        }
    }

    /// Default config for a new source of the given type.
    #[must_use]
    pub fn default_for_type(type_name: &str) -> Self {
        match type_name {
            "subsonic" => Self::Subsonic {
                url: "https://".to_string(),
                username: String::new(),
                password: String::new(),
                legacy_auth: false,
            },
            "mpd" => Self::Mpd {
                host: "localhost".to_string(),
                port: 6600,
                auth_mode: MpdClientAuthMode::default(),
                password: None,
                httpd_port: 6601,
            },
            "dlna" => Self::Dlna {
                location_url: None,
                friendly_name: None,
            },
            "peer" => Self::Peer {
                host: String::new(),
                port: default_sotf_api_port(),
                accepted_fingerprint: None,
                auth_token: None,
            },
            "tidal" => Self::Tidal {
                access_token: String::new(),
                quality: default_tidal_quality(),
                country_code: default_country_code(),
            },
            "spotify" => Self::Spotify {
                username: String::new(),
                password: String::new(),
                quality: default_spotify_quality(),
            },
            "icy_radio" => Self::IcyRadio {
                url: String::new(),
                name: String::new(),
            },
            _ => Self::Mpd {
                host: "localhost".to_string(),
                port: 6600,
                auth_mode: MpdClientAuthMode::default(),
                password: None,
                httpd_port: 6601,
            },
        }
    }

    /// Editable field names for this source type.
    #[must_use]
    pub fn field_names(&self) -> Vec<&'static str> {
        match self {
            Self::Subsonic { .. } => vec!["URL", "Username", "Password", "Legacy Auth"],
            Self::Mpd { .. } => vec!["Host", "Port", "Auth Mode", "Password", "HTTP Stream Port"],
            Self::Dlna { .. } => vec!["Location URL", "Friendly Name"],
            Self::Peer { .. } => vec!["Host", "Port", "Fingerprint", "API Token"],
            Self::Tidal { .. } => vec!["Access Token", "Quality", "Country Code"],
            Self::Spotify { .. } => vec!["Username", "Password", "Quality"],
            Self::IcyRadio { .. } => vec!["Stream URL", "Station Name"],
        }
    }

    /// Get field value by index for UI display.
    #[must_use]
    pub fn field_value(&self, index: usize) -> String {
        match self {
            Self::Subsonic {
                url,
                username,
                password,
                legacy_auth,
            } => match index {
                0 => url.clone(),
                1 => username.clone(),
                2 => "*".repeat(password.len().min(8)),
                3 => legacy_auth.to_string(),
                _ => String::new(),
            },
            Self::Mpd {
                host,
                port,
                auth_mode,
                password,
                httpd_port,
            } => match index {
                0 => host.clone(),
                1 => port.to_string(),
                2 => auth_mode.label().to_string(),
                3 => password
                    .as_ref()
                    .map_or_else(String::new, |p| "*".repeat(p.len().min(8))),
                4 => httpd_port.to_string(),
                _ => String::new(),
            },
            Self::Dlna {
                location_url,
                friendly_name,
            } => match index {
                0 => location_url.clone().unwrap_or_default(),
                1 => friendly_name.clone().unwrap_or_default(),
                _ => String::new(),
            },
            Self::Peer {
                host,
                port,
                accepted_fingerprint,
                auth_token,
            } => match index {
                0 => host.clone(),
                1 => port.to_string(),
                2 => accepted_fingerprint.clone().unwrap_or_default(),
                3 => auth_token
                    .as_ref()
                    .map_or_else(String::new, |token| "*".repeat(token.len().min(8))),
                _ => String::new(),
            },
            Self::Tidal {
                access_token,
                quality,
                country_code,
            } => match index {
                0 => "*".repeat(access_token.len().min(8)),
                1 => quality.clone(),
                2 => country_code.clone(),
                _ => String::new(),
            },
            Self::Spotify {
                username,
                password,
                quality,
            } => match index {
                0 => username.clone(),
                1 => "*".repeat(password.len().min(8)),
                2 => quality.clone(),
                _ => String::new(),
            },
            Self::IcyRadio { url, name } => match index {
                0 => url.clone(),
                1 => name.clone(),
                _ => String::new(),
            },
        }
    }

    /// Set field value by index from UI input.
    pub fn set_field_value(&mut self, index: usize, value: &str) {
        match self {
            Self::Subsonic {
                url,
                username,
                password,
                legacy_auth,
            } => match index {
                0 => *url = value.to_string(),
                1 => *username = value.to_string(),
                2 => *password = value.to_string(),
                3 => *legacy_auth = value == "true",
                _ => {}
            },
            Self::Mpd {
                host,
                port,
                auth_mode,
                password,
                httpd_port,
            } => match index {
                0 => *host = value.trim().to_string(),
                1 => {
                    if let Ok(p) = value.trim().parse() {
                        *port = p;
                    }
                }
                2 => {
                    *auth_mode = match value {
                        "Password" => MpdClientAuthMode::Password,
                        "SSL" => MpdClientAuthMode::Ssl,
                        _ => MpdClientAuthMode::None,
                    }
                }
                3 => {
                    *password = if value.is_empty() {
                        None
                    } else {
                        Some(value.to_string())
                    };
                }
                4 => {
                    if let Ok(p) = value.trim().parse() {
                        *httpd_port = p;
                    }
                }
                _ => {}
            },
            Self::Dlna {
                location_url,
                friendly_name,
            } => match index {
                0 => {
                    *location_url = if value.is_empty() {
                        None
                    } else {
                        Some(value.to_string())
                    };
                }
                1 => {
                    *friendly_name = if value.is_empty() {
                        None
                    } else {
                        Some(value.to_string())
                    };
                }
                _ => {}
            },
            Self::Peer {
                host,
                port,
                accepted_fingerprint,
                auth_token,
            } => match index {
                0 => *host = value.trim().to_string(),
                1 => {
                    if let Ok(p) = value.trim().parse() {
                        *port = p;
                    }
                }
                2 => {
                    *accepted_fingerprint = if value.is_empty() {
                        None
                    } else {
                        Some(value.trim().to_string())
                    };
                }
                3 => {
                    *auth_token = if value.trim().is_empty() {
                        None
                    } else {
                        Some(value.trim().to_string())
                    };
                }
                _ => {}
            },
            Self::Tidal {
                access_token,
                quality,
                country_code,
            } => match index {
                0 => *access_token = value.to_string(),
                1 => *quality = value.to_string(),
                2 => *country_code = value.to_string(),
                _ => {}
            },
            Self::Spotify {
                username,
                password,
                quality,
            } => match index {
                0 => *username = value.to_string(),
                1 => *password = value.to_string(),
                2 => *quality = value.to_string(),
                _ => {}
            },
            Self::IcyRadio { url, name } => match index {
                0 => *url = value.to_string(),
                1 => *name = value.to_string(),
                _ => {}
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_configs() -> Vec<SourceConnectionConfig> {
        vec![
            SourceConnectionConfig::Subsonic {
                url: "https://".to_string(),
                username: String::new(),
                password: String::new(),
                legacy_auth: false,
            },
            SourceConnectionConfig::Mpd {
                host: "localhost".to_string(),
                port: 6600,
                auth_mode: MpdClientAuthMode::default(),
                password: None,
                httpd_port: 6601,
            },
            SourceConnectionConfig::Dlna {
                location_url: None,
                friendly_name: None,
            },
            SourceConnectionConfig::Peer {
                host: String::new(),
                port: default_sotf_api_port(),
                accepted_fingerprint: None,
                auth_token: None,
            },
            SourceConnectionConfig::Tidal {
                access_token: String::new(),
                quality: default_tidal_quality(),
                country_code: default_country_code(),
            },
            SourceConnectionConfig::Spotify {
                username: String::new(),
                password: String::new(),
                quality: default_spotify_quality(),
            },
            SourceConnectionConfig::IcyRadio {
                url: String::new(),
                name: String::new(),
            },
        ]
    }

    fn source_keys() -> Vec<&'static str> {
        vec![
            "subsonic", "mpd", "dlna", "peer", "tidal", "spotify", "icy_radio",
        ]
    }

    fn test_value(config: &SourceConnectionConfig, index: usize) -> &'static str {
        match config {
            SourceConnectionConfig::Subsonic { .. } => match index {
                0 => "https://example.com",
                1 => "user",
                2 => "secret123",
                3 => "true",
                _ => "",
            },
            SourceConnectionConfig::Mpd { .. } => match index {
                0 => "192.168.1.1",
                1 => "6601",
                2 => "Password",
                3 => "hunter2",
                4 => "6602",
                _ => "",
            },
            SourceConnectionConfig::Dlna { .. } => match index {
                0 => "http://dlna.local",
                1 => "My DLNA",
                _ => "",
            },
            SourceConnectionConfig::Peer { .. } => match index {
                0 => "peer.local",
                1 => "8733",
                2 => "ab:cd",
                3 => "api-token",
                _ => "",
            },
            SourceConnectionConfig::Tidal { .. } => match index {
                0 => "tidal-token",
                1 => "HI_RES",
                2 => "FR",
                _ => "",
            },
            SourceConnectionConfig::Spotify { .. } => match index {
                0 => "spotify-user",
                1 => "spotify-pass",
                2 => "VeryHigh",
                _ => "",
            },
            SourceConnectionConfig::IcyRadio { .. } => match index {
                0 => "http://stream.local",
                1 => "Station",
                _ => "",
            },
        }
    }

    fn expected_display(config: &SourceConnectionConfig, index: usize, set_value: &str) -> String {
        let masked = |s: &str| "*".repeat(s.len().min(8));

        match config {
            SourceConnectionConfig::Subsonic { .. } => match index {
                0 | 1 => set_value.to_string(),
                2 => masked(set_value),
                3 => set_value.to_string(),
                _ => String::new(),
            },
            SourceConnectionConfig::Mpd { .. } => match index {
                0 => set_value.trim().to_string(),
                1 => set_value.trim().to_string(),
                2 => set_value.to_string(),
                3 => masked(set_value),
                4 => set_value.trim().to_string(),
                _ => String::new(),
            },
            SourceConnectionConfig::Dlna { .. } => match index {
                0 | 1 => set_value.to_string(),
                _ => String::new(),
            },
            SourceConnectionConfig::Peer { .. } => match index {
                0 => set_value.trim().to_string(),
                1 => set_value.trim().to_string(),
                2 => set_value.trim().to_string(),
                3 => masked(set_value.trim()),
                _ => String::new(),
            },
            SourceConnectionConfig::Tidal { .. } => match index {
                0 => masked(set_value),
                1 | 2 => set_value.to_string(),
                _ => String::new(),
            },
            SourceConnectionConfig::Spotify { .. } => match index {
                0 => set_value.to_string(),
                1 => masked(set_value),
                2 => set_value.to_string(),
                _ => String::new(),
            },
            SourceConnectionConfig::IcyRadio { .. } => match index {
                0 | 1 => set_value.to_string(),
                _ => String::new(),
            },
        }
    }

    #[test]
    fn default_for_type_matches_source_type_keys() {
        for key in source_keys() {
            let config = SourceConnectionConfig::default_for_type(key);
            assert_eq!(
                config.source_type_key(),
                key,
                "default_for_type({key:?}) returned wrong variant"
            );
            assert!(!config.field_names().is_empty(), "{key:?} has no fields");
        }
    }

    #[test]
    fn default_for_type_unknown_falls_back_to_mpd() {
        let config = SourceConnectionConfig::default_for_type("unknown");
        assert_eq!(config.source_type_key(), "mpd");
        assert_eq!(config.type_name(), "MPD");
    }

    #[test]
    fn field_names_len_matches_field_value_and_set_field_value() {
        for config in all_configs() {
            let names = config.field_names();
            let len = names.len();
            assert!(len > 0, "{} has no field names", config.source_type_key());

            // field_value must return a value for every valid index.
            for i in 0..len {
                let _ = config.field_value(i);
            }

            // set_field_value must accept every valid index without panicking.
            let mut clone = config.clone();
            for i in 0..len {
                clone.set_field_value(i, "test");
            }
        }
    }

    #[test]
    fn field_value_round_trips_after_set() {
        for config in all_configs() {
            let len = config.field_names().len();
            for i in 0..len {
                let mut clone = config.clone();
                let value = test_value(&config, i);
                clone.set_field_value(i, value);
                let expected = expected_display(&config, i, value);
                let actual = clone.field_value(i);
                assert_eq!(
                    actual, expected,
                    "{} field {i} round-trip failed (set {value:?}, expected {expected:?})",
                    config.source_type_key()
                );
            }
        }
    }

    #[test]
    fn empty_string_clears_optional_fields() {
        let mut mpd = SourceConnectionConfig::Mpd {
            host: "localhost".to_string(),
            port: 6600,
            auth_mode: MpdClientAuthMode::None,
            password: Some("secret".to_string()),
            httpd_port: 6601,
        };
        mpd.set_field_value(3, "");
        assert_eq!(mpd.field_value(3), String::new());

        let mut dlna = SourceConnectionConfig::Dlna {
            location_url: Some("url".to_string()),
            friendly_name: Some("name".to_string()),
        };
        dlna.set_field_value(0, "");
        dlna.set_field_value(1, "");
        assert_eq!(dlna.field_value(0), String::new());
        assert_eq!(dlna.field_value(1), String::new());

        let mut peer = SourceConnectionConfig::Peer {
            host: String::new(),
            port: 8732,
            accepted_fingerprint: Some("fp".to_string()),
            auth_token: Some("token".to_string()),
        };
        peer.set_field_value(2, "");
        peer.set_field_value(3, "");
        assert_eq!(peer.field_value(2), String::new());
        assert_eq!(peer.field_value(3), String::new());
    }

    #[test]
    fn serde_json_round_trips() {
        for config in all_configs() {
            let json = serde_json::to_string(&config).expect("serialize");
            let decoded: SourceConnectionConfig = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(decoded, config, "{} JSON round-trip failed", config.source_type_key());
        }
    }

    #[test]
    fn invalid_field_indices_are_handled_gracefully() {
        for config in all_configs() {
            let len = config.field_names().len();
            let out_of_range = len + 10;

            // Reading returns an empty string.
            assert_eq!(config.field_value(out_of_range), String::new());

            // Writing is a no-op and does not panic.
            let mut clone = config.clone();
            clone.set_field_value(out_of_range, "should-not-change-anything");
            assert_eq!(clone, config);
        }
    }

    mod property_tests {
        use super::super::{MpdClientAuthMode, SourceConnectionConfig};
        use proptest::prelude::*;

        fn host_strategy() -> BoxedStrategy<String> {
            proptest::string::string_regex("[a-zA-Z0-9_.:-]+")
                .unwrap()
                .boxed()
        }

        fn token_strategy() -> BoxedStrategy<String> {
            proptest::string::string_regex("[a-zA-Z0-9_]+")
                .unwrap()
                .boxed()
        }

        fn maybe_empty_token_strategy() -> BoxedStrategy<Option<String>> {
            prop::option::of(
                proptest::string::string_regex("[a-zA-Z0-9_ ./:-]*")
                    .unwrap()
                    .boxed(),
            )
            .boxed()
        }

        fn mpd_config_strategy() -> BoxedStrategy<SourceConnectionConfig> {
            (
                host_strategy(),
                1u16..65535u16,
                prop::bool::ANY.prop_map(|b| {
                    if b {
                        MpdClientAuthMode::Password
                    } else {
                        MpdClientAuthMode::None
                    }
                }),
                maybe_empty_token_strategy(),
                1u16..65535u16,
            )
                .prop_map(|(host, port, auth_mode, password, httpd_port)| {
                    SourceConnectionConfig::Mpd {
                        host,
                        port,
                        auth_mode,
                        password,
                        httpd_port,
                    }
                })
                .boxed()
        }

        fn subsonic_config_strategy() -> BoxedStrategy<SourceConnectionConfig> {
            (
                host_strategy(),
                token_strategy(),
                token_strategy(),
                prop::bool::ANY,
            )
                .prop_map(|(url, username, password, legacy_auth)| {
                    SourceConnectionConfig::Subsonic {
                        url,
                        username,
                        password,
                        legacy_auth,
                    }
                })
                .boxed()
        }

        fn dlna_config_strategy() -> BoxedStrategy<SourceConnectionConfig> {
            (maybe_empty_token_strategy(), maybe_empty_token_strategy())
                .prop_map(|(location_url, friendly_name)| SourceConnectionConfig::Dlna {
                    location_url,
                    friendly_name,
                })
                .boxed()
        }

        fn peer_config_strategy() -> BoxedStrategy<SourceConnectionConfig> {
            (
                host_strategy(),
                1u16..65535u16,
                maybe_empty_token_strategy(),
                maybe_empty_token_strategy(),
            )
                .prop_map(|(host, port, accepted_fingerprint, auth_token)| {
                    SourceConnectionConfig::Peer {
                        host,
                        port,
                        accepted_fingerprint,
                        auth_token,
                    }
                })
                .boxed()
        }

        fn tidal_config_strategy() -> BoxedStrategy<SourceConnectionConfig> {
            (
                token_strategy(),
                proptest::string::string_regex("[A-Z_]+").unwrap().boxed(),
                proptest::string::string_regex("[A-Z]{2}").unwrap().boxed(),
            )
                .prop_map(|(access_token, quality, country_code)| SourceConnectionConfig::Tidal {
                    access_token,
                    quality,
                    country_code,
                })
                .boxed()
        }

        fn spotify_config_strategy() -> BoxedStrategy<SourceConnectionConfig> {
            (
                token_strategy(),
                token_strategy(),
                proptest::string::string_regex("[A-Za-z]+").unwrap().boxed(),
            )
                .prop_map(|(username, password, quality)| SourceConnectionConfig::Spotify {
                    username,
                    password,
                    quality,
                })
                .boxed()
        }

        fn icy_radio_config_strategy() -> BoxedStrategy<SourceConnectionConfig> {
            (
                host_strategy(),
                proptest::string::string_regex("[a-zA-Z0-9_ ./:-]*")
                    .unwrap()
                    .boxed(),
            )
                .prop_map(|(url, name)| SourceConnectionConfig::IcyRadio { url, name })
                .boxed()
        }

        fn source_connection_config_strategy() -> BoxedStrategy<SourceConnectionConfig> {
            prop_oneof![
                subsonic_config_strategy(),
                mpd_config_strategy(),
                dlna_config_strategy(),
                peer_config_strategy(),
                tidal_config_strategy(),
                spotify_config_strategy(),
                icy_radio_config_strategy(),
            ]
            .boxed()
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

            /// INVARIANT: every SourceConnectionConfig variant round-trips through
            /// JSON serialization without losing field values or changing variant.
            #[test]
            fn serde_json_round_trip_all_variants(config in source_connection_config_strategy()) {
                let json = serde_json::to_string(&config).expect("serialize");
                let decoded: SourceConnectionConfig = serde_json::from_str(&json).expect("deserialize");
                prop_assert_eq!(
                    decoded.clone(), config.clone(),
                    "JSON round-trip failed for {}: json={}",
                    config.source_type_key(),
                    json
                );
                prop_assert_eq!(
                    decoded.source_type_key(),
                    config.source_type_key(),
                    "variant key must round-trip"
                );
            }

            /// INVARIANT: `type_name` and `source_type_key` are consistent for all variants.
            #[test]
            fn type_name_and_key_are_consistent(config in source_connection_config_strategy()) {
                let key = config.source_type_key();
                prop_assert!(
                    matches!(
                        (key, config.type_name()),
                        ("subsonic", "Subsonic")
                            | ("mpd", "MPD")
                            | ("dlna", "DLNA")
                            | ("peer", "Peer")
                            | ("tidal", "Tidal")
                            | ("spotify", "Spotify")
                            | ("icy_radio", "Radio")
                    ),
                    "unexpected key/name pair: {:?} / {:?}",
                    key,
                    config.type_name()
                );
            }
        }
    }
}
