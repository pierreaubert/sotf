//! Login/logout helpers shared by the player frontends' federation settings
//! screens (TUI, GPUI). The provider calls live in `sotf-service-tidal` /
//! `sotf-service-spotify`; this module holds only the pieces that touch
//! persisted state — the federation source config in the music database
//! (via [`crate::federation_config`]) and the librespot credential cache —
//! so the apps stay thin glue.
//!
//! Tokens are secrets: they are written into configs and deleted from disk
//! here, never logged or formatted into user-visible strings.

use std::path::{Path, PathBuf};

use crate::federation_config::{FederationSourceEntry, SourceConnectionConfig};

/// Write freshly-issued Tidal device-auth tokens into a federation source's
/// connection config. Returns `false` (no-op) when the source is not a Tidal
/// source. The caller persists the entry afterwards via the usual
/// `save_federation_source` path.
pub fn apply_tidal_device_tokens(
    entry: &mut FederationSourceEntry,
    access_token: &str,
    refresh_token: &str,
) -> bool {
    if let SourceConnectionConfig::Tidal {
        access_token: stored_access,
        refresh_token: stored_refresh,
        ..
    } = &mut entry.connection
    {
        *stored_access = access_token.to_string();
        *stored_refresh = refresh_token.to_string();
        true
    } else {
        false
    }
}

/// Clear both Tidal tokens from a federation source (logout). Returns `false`
/// when the source is not a Tidal source.
pub fn clear_tidal_tokens(entry: &mut FederationSourceEntry) -> bool {
    apply_tidal_device_tokens(entry, "", "")
}

/// Persist a federation source entry to the music database at the default
/// path — the save path shared by every flow that rewrites a source config
/// (settings edits, Tidal token rotation from playback or library scans).
///
/// # Errors
/// Returns an error when the database path cannot be determined, the
/// database cannot be opened, or the upsert fails.
pub fn persist_federation_source(entry: &FederationSourceEntry) -> Result<(), String> {
    let path = crate::database::MusicDatabase::default_path()
        .ok_or_else(|| "could not determine the music database path".to_string())?;
    let db = crate::database::MusicDatabase::open(&path)
        .map_err(|e| format!("failed to open the music database: {e}"))?;
    db.save_federation_source(entry)
}

/// Spotify credential cache directory — same convention as
/// `ServiceManager::connect_spotify` (`<config dir>/spotify`).
#[must_use]
pub fn spotify_cache_dir() -> Option<PathBuf> {
    crate::config::get_app_config_dir().map(|dir| dir.join("spotify"))
}

/// Path of the librespot credential file inside the cache directory
/// (`librespot_core::cache::Cache` writes `credentials.json`).
#[must_use]
pub fn spotify_credentials_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join("credentials.json")
}

/// Delete the cached Spotify credentials (logout). Returns `Ok(false)` when
/// no credentials were cached; other I/O errors are propagated.
pub fn clear_spotify_cached_credentials(cache_dir: &Path) -> std::io::Result<bool> {
    match std::fs::remove_file(spotify_credentials_path(cache_dir)) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// Open a URL in the system browser, best-effort and fire-and-forget (the
/// process is spawned, not waited on). The UIs also display the URL as a
/// fallback, so a failure here is not fatal to the login flow.
///
/// The URL comes from the provider's device-auth response, so only `https://`
/// URLs are opened; `http://` is accepted for loopback hosts only (local auth
/// mocks). Anything else (`file://`, scheme-less, ...) is rejected with
/// [`std::io::ErrorKind::InvalidInput`] instead of being handed to the OS
/// handler.
pub fn open_url_in_browser(url: &str) -> std::io::Result<()> {
    if !is_browser_openable_url(url) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("refusing to open URL with a disallowed scheme or host: {url}"),
        ));
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        // rundll32 + ShellExecute avoids cmd.exe quoting pitfalls (`start`
        // splits on `&`, which OAuth URLs are full of).
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    Ok(())
}

/// `https://` anything, or `http://` to a loopback host (127.0.0.1, ::1,
/// localhost) — everything else is refused.
fn is_browser_openable_url(url: &str) -> bool {
    if url.starts_with("https://") {
        return true;
    }
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    // Strip any userinfo, then the port; bracketed IPv6 keeps its `::1`.
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    let host = if let Some(bracketed) = host_port.strip_prefix('[') {
        bracketed.split(']').next().unwrap_or("")
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

#[cfg(test)]
mod tests {
    mod tidal {
        use crate::federation_config::{FederationSourceEntry, SourceConnectionConfig};
        use crate::service_login::{apply_tidal_device_tokens, clear_tidal_tokens};

        fn tidal_entry() -> FederationSourceEntry {
            FederationSourceEntry {
                source_id: "tidal:test".to_string(),
                display_name: "Tidal".to_string(),
                priority: 0,
                is_enabled: true,
                connection: SourceConnectionConfig::Tidal {
                    access_token: String::new(),
                    client_id: "client".to_string(),
                    refresh_token: String::new(),
                    quality: "LOSSLESS".to_string(),
                    country_code: "US".to_string(),
                },
                is_available: None,
            }
        }

        #[test]
        fn apply_tokens_writes_both_fields() {
            let mut entry = tidal_entry();
            assert!(apply_tidal_device_tokens(
                &mut entry,
                "access-1",
                "refresh-1"
            ));
            match &entry.connection {
                SourceConnectionConfig::Tidal {
                    access_token,
                    refresh_token,
                    client_id,
                    ..
                } => {
                    assert_eq!(access_token, "access-1");
                    assert_eq!(refresh_token, "refresh-1");
                    // Unrelated fields are untouched.
                    assert_eq!(client_id, "client");
                }
                other => panic!("unexpected connection: {}", other.type_name()),
            }
        }

        #[test]
        fn clear_tokens_empties_both_fields() {
            let mut entry = tidal_entry();
            assert!(apply_tidal_device_tokens(
                &mut entry,
                "access-1",
                "refresh-1"
            ));
            assert!(clear_tidal_tokens(&mut entry));
            match &entry.connection {
                SourceConnectionConfig::Tidal {
                    access_token,
                    refresh_token,
                    ..
                } => {
                    assert!(access_token.is_empty());
                    assert!(refresh_token.is_empty());
                }
                other => panic!("unexpected connection: {}", other.type_name()),
            }
        }

        #[test]
        fn non_tidal_source_is_a_no_op() {
            let mut entry = tidal_entry();
            entry.connection = SourceConnectionConfig::Spotify {
                username: "user".to_string(),
                password: "pass".to_string(),
                quality: "High".to_string(),
            };
            let before = entry.connection.clone();
            assert!(!apply_tidal_device_tokens(&mut entry, "a", "r"));
            assert!(!clear_tidal_tokens(&mut entry));
            assert_eq!(entry.connection, before);
        }
    }

    mod browser_open {
        use crate::service_login::{is_browser_openable_url, open_url_in_browser};

        #[test]
        fn scheme_and_host_whitelist() {
            // https is always fine.
            assert!(is_browser_openable_url(
                "https://auth.example.com/device?code=abc"
            ));
            // http only on loopback (local auth mocks).
            assert!(is_browser_openable_url("http://127.0.0.1:8080/callback"));
            assert!(is_browser_openable_url("http://localhost/verify"));
            assert!(is_browser_openable_url("http://[::1]:9000/"));
            assert!(!is_browser_openable_url("http://example.com/"));
            assert!(!is_browser_openable_url(
                "http://127.0.0.1.evil.example.com/"
            ));
            // Everything else is refused.
            assert!(!is_browser_openable_url("file:///etc/passwd"));
            assert!(!is_browser_openable_url("javascript:alert(1)"));
            assert!(!is_browser_openable_url("ftp://example.com/x"));
            assert!(!is_browser_openable_url("example.com/no-scheme"));
            assert!(!is_browser_openable_url(""));
        }

        #[test]
        fn rejected_urls_error_instead_of_spawning() {
            let err = open_url_in_browser("file:///etc/passwd").expect_err("must be refused");
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        }
    }

    mod spotify {
        use crate::service_login::{clear_spotify_cached_credentials, spotify_credentials_path};

        #[test]
        fn credentials_path_is_librespot_convention() {
            let dir = std::path::Path::new("/tmp/cache");
            assert_eq!(
                spotify_credentials_path(dir),
                std::path::PathBuf::from("/tmp/cache/credentials.json")
            );
        }

        #[test]
        fn clear_cached_credentials_reports_whether_file_existed() {
            let tmp = tempfile::tempdir().expect("tempdir");
            let dir = tmp.path();

            // Nothing cached yet.
            assert!(!clear_spotify_cached_credentials(dir).expect("clear"));

            std::fs::write(dir.join("credentials.json"), b"{}").expect("seed credentials");
            assert!(clear_spotify_cached_credentials(dir).expect("clear"));
            assert!(!spotify_credentials_path(dir).exists());

            // Deleting again reports "nothing to remove", not an error.
            assert!(!clear_spotify_cached_credentials(dir).expect("clear"));
        }
    }
}
