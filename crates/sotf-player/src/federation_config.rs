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
        password: Option<String>,
        /// Use client certificate for mTLS authentication (default: true).
        #[serde(default = "default_true")]
        use_client_cert: bool,
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
}

fn default_mpd_port() -> u16 {
    6600
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
                password: None,
                use_client_cert: true,
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
            _ => Self::Mpd {
                host: "localhost".to_string(),
                port: 6600,
                password: None,
                use_client_cert: true,
            },
        }
    }

    /// Editable field names for this source type.
    #[must_use]
    pub fn field_names(&self) -> Vec<&'static str> {
        match self {
            Self::Subsonic { .. } => vec!["URL", "Username", "Password", "Legacy Auth"],
            Self::Mpd { .. } => vec!["Host", "Port", "Certificate Auth", "Password"],
            Self::Dlna { .. } => vec!["Location URL", "Friendly Name"],
            Self::Peer { .. } => vec!["Host", "Port", "Fingerprint"],
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
                password,
                use_client_cert,
            } => match index {
                0 => host.clone(),
                1 => port.to_string(),
                2 => use_client_cert.to_string(),
                3 => password
                    .as_ref()
                    .map_or_else(String::new, |p| "*".repeat(p.len().min(8))),
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
                password,
                use_client_cert,
            } => match index {
                0 => *host = value.to_string(),
                1 => {
                    if let Ok(p) = value.parse() {
                        *port = p;
                    }
                }
                2 => *use_client_cert = value == "true",
                3 => {
                    *password = if value.is_empty() {
                        None
                    } else {
                        Some(value.to_string())
                    };
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
                0 => *host = value.to_string(),
                1 => {
                    if let Ok(p) = value.parse() {
                        *port = p;
                    }
                }
                2 => {
                    *accepted_fingerprint = if value.is_empty() {
                        None
                    } else {
                        Some(value.to_string())
                    };
                }
                _ => {}
            },
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
}

/// Runtime connection status (not persisted).
#[derive(Clone, Debug)]
pub enum ConnectionStatus {
    Untested,
    Testing,
    Connected { version: Option<String> },
    Error(String),
}

impl ConnectionStatus {
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Untested => "untested",
            Self::Testing => "testing...",
            Self::Connected { .. } => "connected",
            Self::Error(_) => "error",
        }
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
                password: None,
                use_client_cert: true,
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
    }
}
