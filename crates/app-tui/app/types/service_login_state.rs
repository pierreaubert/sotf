use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

/// In-progress streaming-service login (Tidal device code / Spotify OAuth)
/// for the federation sources settings screen. The blocking provider calls
/// run on a background thread that reports progress through
/// [`ServiceLoginEvent`] messages, polled from the main tick loop.
#[derive(Debug, Clone)]
pub struct ServiceLoginState {
    /// Federation source the login belongs to.
    pub source_id: String,
    pub status: ServiceLoginStatus,
    /// Set by the UI to abort the flow. Tidal checks it between device-code
    /// polls; Spotify cannot interrupt its blocking loopback listener (it has
    /// its own 180 s timeout), so there it only detaches the UI.
    pub cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub enum ServiceLoginStatus {
    /// The background thread is starting the flow (requesting the device
    /// code / building the authorize URL).
    Starting,
    /// Tidal: device-code prompt to display while polling continues.
    TidalDevicePrompt {
        verification_url: String,
        user_code: String,
        expires_in_secs: u64,
        started: Instant,
    },
    /// Spotify: the authorize URL was opened in the browser; waiting for the
    /// loopback callback.
    SpotifyOAuth { url: String, started: Instant },
}

/// Events sent by the login background thread to the UI tick loop.
///
/// Tokens cross this channel exactly once (Tidal completion) and are only
/// ever written into the source config — never rendered or logged. `Debug`
/// is implemented manually so a stray `{:?}` cannot leak those tokens.
pub enum ServiceLoginEvent {
    /// Tidal: the device-code prompt is ready to display.
    TidalPrompt {
        verification_url: String,
        user_code: String,
        expires_in_secs: u64,
    },
    /// Spotify: the authorize URL (already opened in the browser on a
    /// best-effort basis; the UI shows it as a fallback).
    SpotifyUrl {
        url: String,
    },
    /// Login succeeded. Carries the Tidal tokens to persist; Spotify writes
    /// its own credential cache, so no payload is needed there.
    Complete {
        tidal_tokens: Option<(String, String)>,
    },
    Failed(String),
    /// The background thread observed the cancel flag (Tidal only).
    Cancelled,
}

impl std::fmt::Debug for ServiceLoginEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TidalPrompt {
                verification_url,
                user_code,
                expires_in_secs,
            } => f
                .debug_struct("TidalPrompt")
                .field("verification_url", verification_url)
                .field("user_code", user_code)
                .field("expires_in_secs", expires_in_secs)
                .finish(),
            Self::SpotifyUrl { url } => f.debug_struct("SpotifyUrl").field("url", url).finish(),
            Self::Complete { tidal_tokens } => f
                .debug_struct("Complete")
                .field("tidal_tokens", &tidal_tokens.as_ref().map(|_| "<redacted>"))
                .finish(),
            Self::Failed(error) => f.debug_tuple("Failed").field(error).finish(),
            Self::Cancelled => f.write_str("Cancelled"),
        }
    }
}
