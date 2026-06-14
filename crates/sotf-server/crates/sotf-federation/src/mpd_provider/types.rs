use serde::{Deserialize, Serialize};

/// Configuration for an MPD federation source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MpdProviderConfig {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
    /// Port for MPD's httpd streaming output.
    /// SOTF tells MPD to play a track, then streams from http://host:httpd_port/
    pub httpd_port: u16,
}
