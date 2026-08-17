use super::types::FederationScanMessage;
use super::types::FederationScanProgress;
use super::types::TrustedClientInfo;
use std::collections::HashMap;
use std::sync::Arc;

/// Federation & server configuration and background scan state
#[derive(Debug)]
pub struct FederationState {
    pub sources: Vec<sotf_audio_player::federation_config::FederationSourceEntry>,
    pub source_statuses: HashMap<String, sotf_audio_player::federation_config::ConnectionStatus>,
    pub server_config: sotf_audio_player::federation_config::ServerConfig,
    pub scan_receiver: Option<std::sync::mpsc::Receiver<FederationScanMessage>>,
    pub scan_cancel: Arc<std::sync::atomic::AtomicBool>,
    pub scan_progress: Option<FederationScanProgress>,
    pub cast_discovery_receiver:
        Option<std::sync::mpsc::Receiver<Vec<crate::app::state::audio_device::CastDeviceInfo>>>,
    /// Whether the local SOTF API server is in pairing mode.
    pub pairing_enabled: bool,
    /// Current pairing nonce (valid only when pairing_enabled is true).
    pub pairing_nonce: Option<String>,
    /// Server TLS fingerprint for QR code display.
    pub server_fingerprint: Option<String>,
    /// List of trusted clients paired with this server.
    pub trusted_clients: Vec<TrustedClientInfo>,
    /// Last pairing operation error message.
    pub pairing_error: Option<String>,
    /// Active Tidal device-code login flow, if any.
    #[cfg(feature = "tidal")]
    pub tidal_login: Option<TidalLoginState>,
    /// Active Spotify OAuth login flow, if any.
    #[cfg(feature = "spotify")]
    pub spotify_login: Option<SpotifyLoginState>,
}

impl Default for FederationState {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            source_statuses: HashMap::new(),
            server_config: sotf_audio_player::federation_config::ServerConfig::default(),
            scan_receiver: None,
            scan_cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            scan_progress: None,
            cast_discovery_receiver: None,
            pairing_enabled: false,
            pairing_nonce: None,
            server_fingerprint: None,
            trusted_clients: Vec::new(),
            pairing_error: None,
            #[cfg(feature = "tidal")]
            tidal_login: None,
            #[cfg(feature = "spotify")]
            spotify_login: None,
        }
    }
}

/// Messages sent by the background Tidal device-code login worker.
///
/// `Debug` is implemented manually so a stray `{:?}` cannot leak the tokens
/// carried by `Completed` into logs.
#[cfg(feature = "tidal")]
pub enum TidalLoginMessage {
    /// The device-code prompt to display (URL + user code).
    Prompt {
        verification_url: String,
        user_code: String,
        expires_in_secs: u64,
    },
    /// Authorization completed; carries the freshly-issued tokens.
    Completed {
        access_token: String,
        refresh_token: String,
    },
    /// The device code expired before the user authorized.
    Expired,
    /// The flow failed (network/API error); carries a redacted message.
    Failed(String),
}

#[cfg(feature = "tidal")]
impl std::fmt::Debug for TidalLoginMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Prompt {
                verification_url,
                user_code,
                expires_in_secs,
            } => f
                .debug_struct("Prompt")
                .field("verification_url", verification_url)
                .field("user_code", user_code)
                .field("expires_in_secs", expires_in_secs)
                .finish(),
            Self::Completed { .. } => f
                .debug_struct("Completed")
                .field("access_token", &"<redacted>")
                .field("refresh_token", &"<redacted>")
                .finish(),
            Self::Expired => f.write_str("Expired"),
            Self::Failed(error) => f.debug_tuple("Failed").field(error).finish(),
        }
    }
}

/// Display state of a running Tidal device-code login flow.
#[cfg(feature = "tidal")]
#[derive(Debug, Clone)]
pub struct TidalLoginPrompt {
    pub verification_url: String,
    pub user_code: String,
    pub expires_in_secs: u64,
}

/// State of an in-flight Tidal device-code login for one federation source.
#[cfg(feature = "tidal")]
#[derive(Debug)]
pub struct TidalLoginState {
    /// Federation source being logged into.
    pub source_id: String,
    /// Messages from the background worker thread.
    pub receiver: std::sync::mpsc::Receiver<TidalLoginMessage>,
    /// The prompt once the worker has obtained a device code.
    pub prompt: Option<TidalLoginPrompt>,
}

/// Messages sent by the background Spotify OAuth login worker.
#[cfg(feature = "spotify")]
#[derive(Debug)]
pub enum SpotifyLoginMessage {
    /// The authorize URL (shown as a fallback next to the auto-opened browser).
    AuthorizeUrl(String),
    /// Login succeeded; credentials are persisted in the librespot cache.
    Completed,
    /// The flow failed; carries a redacted message.
    Failed(String),
}

/// State of an in-flight Spotify OAuth login for one federation source.
#[cfg(feature = "spotify")]
#[derive(Debug)]
pub struct SpotifyLoginState {
    /// Federation source being logged into.
    pub source_id: String,
    /// Messages from the background worker thread.
    pub receiver: std::sync::mpsc::Receiver<SpotifyLoginMessage>,
    /// Authorize URL shown as a fallback, once known.
    pub authorize_url: Option<String>,
}
