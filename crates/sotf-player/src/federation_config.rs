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
        #[serde(default = "default_mpd_port")]
        port: u16,
        accepted_fingerprint: Option<String>,
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

fn default_mpd_port() -> u16 {
    6600
}

fn default_httpd_port() -> u16 {
    6601
}

fn default_tidal_quality() -> String {
    "LOSSLESS".to_string()
}

fn default_country_code() -> String {
    "US".to_string()
}

fn default_spotify_quality() -> String {
    "High".to_string()
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
                port: 6600,
                accepted_fingerprint: None,
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
            Self::Peer { .. } => vec!["Host", "Port", "Fingerprint"],
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
            } => match index {
                0 => host.clone(),
                1 => port.to_string(),
                2 => accepted_fingerprint.clone().unwrap_or_default(),
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

/// MPD client authentication mode for federation sources.
#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "auth_type")]
pub enum MpdClientAuthMode {
    #[serde(rename = "none")]
    #[default]
    None,
    #[serde(rename = "password")]
    Password,
    #[serde(rename = "ssl")]
    Ssl,
}

impl MpdClientAuthMode {
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Password => "Password",
            Self::Ssl => "SSL",
        }
    }
}

/// Unified source configuration for UI display and editing.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FederationSourceEntry {
    pub source_id: String,
    pub display_name: String,
    pub priority: i32,
    pub is_enabled: bool,
    pub connection: SourceConnectionConfig,
    /// Persisted reachability state. `None` means never tested,
    /// `Some(true)` means last test/scan succeeded, `Some(false)` means unreachable.
    #[serde(default)]
    pub is_available: Option<bool>,
}

/// Runtime connection status (not persisted).
#[derive(Clone, Debug)]
pub enum ConnectionStatus {
    Untested,
    Testing,
    Connected {
        version: Option<String>,
    },
    Error(String),
    /// Detailed diagnostic results from a structured connection test.
    Diagnostic(ConnectionDiagnostic),
}

impl ConnectionStatus {
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Untested => "untested",
            Self::Testing => "testing...",
            Self::Connected { .. } => "connected",
            Self::Error(_) => "error",
            Self::Diagnostic(d) => {
                if d.is_success() {
                    "connected"
                } else {
                    "error"
                }
            }
        }
    }

    /// Returns true if this is a diagnostic result (vs simple status).
    #[must_use]
    pub fn is_diagnostic(&self) -> bool {
        matches!(self, Self::Diagnostic(_))
    }
}

/// Result of a single diagnostic step.
#[derive(Clone, Debug)]
pub enum StepResult {
    /// Step passed.
    Ok(String),
    /// Step failed — subsequent steps were not attempted.
    Fail(String),
    /// Step was skipped (e.g., TLS not enabled).
    Skipped(String),
}

impl StepResult {
    #[must_use]
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok(_))
    }

    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Ok(m) | Self::Fail(m) | Self::Skipped(m) => m,
        }
    }
}

/// Structured diagnostic that tests each connection layer in order.
#[derive(Clone, Debug)]
pub struct ConnectionDiagnostic {
    pub host: String,
    pub port: u16,
    /// Step 1: DNS resolution / ICMP-level reachability.
    pub dns_resolve: StepResult,
    /// Step 2: TCP port open (connect succeeds within timeout).
    pub tcp_connect: StepResult,
    /// Step 3: TLS handshake (only if TLS is enabled for this source).
    pub tls_handshake: StepResult,
    /// Step 4: Protocol-level check (MPD greeting, Subsonic /rest/ping, etc.).
    pub protocol_hello: StepResult,
}

impl ConnectionDiagnostic {
    /// Returns true if all attempted (non-skipped) steps passed.
    #[must_use]
    pub fn is_success(&self) -> bool {
        [
            &self.dns_resolve,
            &self.tcp_connect,
            &self.tls_handshake,
            &self.protocol_hello,
        ]
        .iter()
        .all(|s| matches!(s, StepResult::Ok(_) | StepResult::Skipped(_)))
    }

    /// Collect all steps with their labels for UI display.
    #[must_use]
    pub fn steps(&self) -> Vec<(&'static str, &StepResult)> {
        vec![
            ("DNS Resolve", &self.dns_resolve),
            ("TCP Connect", &self.tcp_connect),
            ("TLS Handshake", &self.tls_handshake),
            ("Protocol", &self.protocol_hello),
        ]
    }
}

/// Server configuration persisted as `~/.config/sotf/servers.json`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ServerConfig {
    #[serde(default)]
    pub mpd: MpdSettings,
    #[serde(default)]
    pub dlna: DlnaSettings,
}

/// Authentication mode for the MPD server.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub enum MpdAuthMode {
    /// Mutual TLS — clients must present a certificate whose fingerprint is trusted.
    /// No password needed; identity is proven cryptographically.
    #[default]
    Certificate,
    /// Legacy password-based authentication (MPD `password` command).
    Password,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MpdSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_mpd_port")]
    pub port: u16,
    #[serde(default = "default_true")]
    pub tls_enabled: bool,
    /// Authentication mode (default: Certificate/mTLS).
    #[serde(default)]
    pub auth_mode: MpdAuthMode,
    /// Password (only used when auth_mode == Password).
    #[serde(default)]
    pub password: Option<String>,
    /// SHA-256 fingerprints of trusted client certificates (for mTLS auth).
    #[serde(default)]
    pub trusted_client_fingerprints: Vec<String>,
}

impl Default for MpdSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            bind_address: default_bind_address(),
            port: 6600,
            tls_enabled: true,
            auth_mode: MpdAuthMode::Certificate,
            password: None,
            trusted_client_fingerprints: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DlnaSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_dlna_name")]
    pub friendly_name: String,
    #[serde(default = "default_dlna_port")]
    pub port: u16,
}

impl Default for DlnaSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            friendly_name: default_dlna_name(),
            port: default_dlna_port(),
        }
    }
}

fn default_bind_address() -> String {
    "0.0.0.0".to_string()
}

fn default_true() -> bool {
    true
}

fn default_dlna_name() -> String {
    "SOTF Media Server".to_string()
}

fn default_dlna_port() -> u16 {
    8200
}

#[cfg(test)]
mod tests {
    use super::*;

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
                port: 6600,
                accepted_fingerprint: Some("AA:BB:CC".to_string()),
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
        assert_eq!(config.dlna.port, 8200);
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
}
