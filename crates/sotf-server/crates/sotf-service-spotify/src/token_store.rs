//! Persistence for the Spotify Web API OAuth token pair.
//!
//! `login_with_oauth` receives an access token *and* a refresh token from the
//! token endpoint, but librespot only caches the session credentials it
//! negotiates — after a restart those are stored credentials, not the Web API
//! token. We therefore persist the token pair ourselves in
//! `web_api_token.json` under the librespot cache dir so
//! `login_with_cached_credentials` can restore (and, when expired, refresh)
//! Web API access without a new browser login.

use serde::{Deserialize, Serialize};
use sotf_services::{ServiceError, redact_secret};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// File (under the librespot cache dir) holding the Web API token pair.
pub(crate) const TOKEN_FILE_NAME: &str = "web_api_token.json";

/// Tokens expiring within this window are treated as already expired, so we
/// never hand out an access token that dies mid-request.
pub(crate) const EXPIRY_SKEW: Duration = Duration::from_secs(60);

/// The persisted Web API token pair. Never logged unredacted.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct WebApiToken {
    pub(crate) access_token: String,
    /// Empty when the token cannot be refreshed (e.g. it came from
    /// `authenticate(AccessToken)` rather than the OAuth flow).
    #[serde(default)]
    pub(crate) refresh_token: String,
    /// Unix epoch seconds at which the access token stops being accepted.
    pub(crate) expires_at: u64,
}

impl std::fmt::Debug for WebApiToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebApiToken")
            .field("access_token", &redact_secret(&self.access_token))
            .field("refresh_token", &redact_secret(&self.refresh_token))
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl WebApiToken {
    /// Build a token whose access token expires `expires_in` from now.
    pub(crate) fn new(access_token: String, refresh_token: String, expires_in: Duration) -> Self {
        Self {
            access_token,
            refresh_token,
            expires_at: now_unix() + expires_in.as_secs(),
        }
    }

    /// True when the access token is expired or within [`EXPIRY_SKEW`] of it.
    pub(crate) fn is_expired(&self) -> bool {
        now_unix() + EXPIRY_SKEW.as_secs() >= self.expires_at
    }

    pub(crate) fn path(cache_dir: &Path) -> PathBuf {
        cache_dir.join(TOKEN_FILE_NAME)
    }

    /// Load the persisted token, if any. A missing file is `None`; a
    /// malformed one is warned about and treated as missing.
    pub(crate) fn load(cache_dir: &Path) -> Option<Self> {
        let path = Self::path(cache_dir);
        let data = match std::fs::read(&path) {
            Ok(data) => data,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) => {
                log::warn!("[Spotify] Cannot read {}: {e}", path.display());
                return None;
            }
        };
        match serde_json::from_slice(&data) {
            Ok(token) => Some(token),
            Err(e) => {
                log::warn!("[Spotify] Ignoring malformed {}: {e}", path.display());
                None
            }
        }
    }

    /// Persist the token with owner-only permissions (0600 on unix).
    pub(crate) fn save(&self, cache_dir: &Path) -> Result<(), ServiceError> {
        let path = Self::path(cache_dir);
        let json = serde_json::to_vec(self)
            .map_err(|e| ServiceError::Other(format!("Failed to encode Web API token: {e}")))?;

        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = opts
            .open(&path)
            .map_err(|e| ServiceError::Other(format!("Failed to write {}: {e}", path.display())))?;
        file.write_all(&json)
            .map_err(|e| ServiceError::Other(format!("Failed to write {}: {e}", path.display())))?;
        // `mode` only applies at creation; tighten an existing file too.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sotf-spotify-tokenstore-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_load_roundtrip_with_restrictive_permissions() {
        let dir = temp_dir("roundtrip");
        let token = WebApiToken::new(
            "access-1".to_string(),
            "refresh-1".to_string(),
            Duration::from_secs(3600),
        );
        token.save(&dir).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(WebApiToken::path(&dir))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "token file must be owner-only");
        }

        let loaded = WebApiToken::load(&dir).unwrap();
        assert_eq!(loaded.access_token, "access-1");
        assert_eq!(loaded.refresh_token, "refresh-1");
        assert!(!loaded.is_expired(), "fresh token must not be expired");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn expired_within_skew_window() {
        let just_expiring = WebApiToken {
            access_token: "a".to_string(),
            refresh_token: "r".to_string(),
            expires_at: now_unix() + EXPIRY_SKEW.as_secs() - 1,
        };
        assert!(just_expiring.is_expired());

        let comfortable = WebApiToken {
            access_token: "a".to_string(),
            refresh_token: "r".to_string(),
            expires_at: now_unix() + EXPIRY_SKEW.as_secs() + 3600,
        };
        assert!(!comfortable.is_expired());
    }

    #[test]
    fn load_missing_or_malformed_is_none() {
        let dir = temp_dir("missing");
        assert!(WebApiToken::load(&dir).is_none());
        std::fs::write(WebApiToken::path(&dir), b"not json").unwrap();
        assert!(WebApiToken::load(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn debug_redacts_tokens() {
        let token = WebApiToken::new(
            "access-secret".to_string(),
            "refresh-secret".to_string(),
            Duration::from_secs(3600),
        );
        let debug = format!("{token:?}");
        assert!(!debug.contains("access-secret"), "got: {debug}");
        assert!(!debug.contains("refresh-secret"), "got: {debug}");
    }
}
