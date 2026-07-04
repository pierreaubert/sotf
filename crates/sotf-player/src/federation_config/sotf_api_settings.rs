use super::default::default_bind_address;
use super::default::default_sotf_api_name;
use super::default::default_sotf_api_port;
use super::default::default_true;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SotfApiSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_sotf_api_port")]
    pub port: u16,
    #[serde(default = "default_sotf_api_name")]
    pub friendly_name: String,
    /// Serve the SOTF API over TLS using the persisted SOTF server certificate.
    #[serde(default = "default_true")]
    pub tls_enabled: bool,
    /// Bearer token required for all control/status endpoints except health.
    #[serde(default)]
    pub auth_token: Option<String>,
}

impl Default for SotfApiSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            bind_address: default_bind_address(),
            port: default_sotf_api_port(),
            friendly_name: default_sotf_api_name(),
            tls_enabled: true,
            auth_token: None,
        }
    }
}
