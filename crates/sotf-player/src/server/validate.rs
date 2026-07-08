use super::misc::invalid_configured_client_fingerprints;
use crate::federation_config::{self, ServerConfig, SotfApiSettings};
use std::net::IpAddr;

/// Environment variable name that opts in to plaintext SOTF API on non-loopback
/// interfaces. This is intentionally off by default so users cannot accidentally
/// expose the bearer-token API to the LAN without TLS.
pub(super) const PLAINTEXT_LAN_OPT_IN_VAR: &str = "SOTF_ALLOW_PLAINTEXT_LAN";

/// Returns true if the configured bind address is loopback-only.
pub(super) fn is_loopback_bind_address(bind_address: &str) -> bool {
    let bind_address = bind_address.trim();
    bind_address.eq_ignore_ascii_case("localhost")
        || bind_address
            .parse::<IpAddr>()
            .is_ok_and(|addr| addr.is_loopback())
}

pub(super) fn sotf_api_plaintext_warning(settings: &SotfApiSettings) -> Option<String> {
    if settings.tls_enabled {
        return None;
    }

    if is_loopback_bind_address(&settings.bind_address) {
        None
    } else {
        Some(format!(
            "SOTF API is serving plaintext HTTP on {}:{}; restrict the bind address to loopback or use only trusted networks until TLS is enabled",
            settings.bind_address, settings.port
        ))
    }
}

/// Hard guard: refuse to start a plaintext SOTF API listener on a non-loopback
/// address unless the caller explicitly opted in via the environment variable.
///
/// Loopback-only plaintext is still allowed (useful for local testing and
/// reverse-proxy setups). This is enforced in server mode in addition to the
/// warning emitted by `sotf_api_plaintext_warning`.
pub(super) fn sotf_api_plaintext_guard(settings: &SotfApiSettings) -> Result<(), String> {
    if settings.tls_enabled || is_loopback_bind_address(&settings.bind_address) {
        return Ok(());
    }

    if std::env::var(PLAINTEXT_LAN_OPT_IN_VAR).is_ok_and(|v| !v.trim().is_empty()) {
        return Ok(());
    }

    Err(format!(
        "Refusing to start plaintext SOTF API on {}:{}. \
         Plaintext LAN exposure is disabled by default. \
         Enable TLS, bind to loopback, or set {PLAINTEXT_LAN_OPT_IN_VAR}=1 to opt in.",
        settings.bind_address, settings.port
    ))
}

pub(super) fn validate_sotf_api_token(settings: &SotfApiSettings) -> Result<String, String> {
    let token = settings.auth_token.as_deref().unwrap_or_default().trim();
    if token.is_empty() {
        return Err("SOTF API requires a non-empty auth_token when enabled".to_string());
    }
    Ok(token.to_string())
}

pub(super) fn validate_server_mode_config(
    config: &ServerConfig,
    _trusted_clients: &sotf_tls::TrustedClientStore,
) -> Result<(), Box<dyn std::error::Error>> {
    if !config.mpd.enabled && !config.dlna.enabled && !config.api.enabled {
        return Err(
            "No servers are enabled in the configuration. Enable MPD, DLNA, or the SOTF API in Configure > Servers or ~/.config/sotf/servers.json, then re-run with --server."
                .into(),
        );
    }

    if config.api.enabled {
        validate_sotf_api_token(&config.api)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    }

    if config.mpd.enabled && config.mpd.tls_enabled {
        match config.mpd.auth_mode {
            federation_config::MpdAuthMode::Certificate => {
                let invalid_fingerprints =
                    invalid_configured_client_fingerprints(&config.mpd.trusted_client_fingerprints);
                if !invalid_fingerprints.is_empty() {
                    let shown = invalid_fingerprints
                        .iter()
                        .take(3)
                        .map(|fp| format!("'{fp}'"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let suffix = if invalid_fingerprints.len() > 3 {
                        format!(" and {} more", invalid_fingerprints.len() - 3)
                    } else {
                        String::new()
                    };
                    return Err(format!(
                        "MPD trusted client fingerprint configuration contains invalid fingerprint(s): {shown}{suffix}. Expected client certificate SHA-256 fingerprints as 64 hex characters, with optional ':' separators."
                    )
                    .into());
                }
            }
            federation_config::MpdAuthMode::Password => {
                if config
                    .mpd
                    .password
                    .as_deref()
                    .unwrap_or_default()
                    .is_empty()
                {
                    return Err(
                        "MPD is enabled with password authentication, but no password is configured. Set an MPD password in Configure > Servers or disable MPD."
                            .into(),
                    );
                }
            }
        }
    }

    Ok(())
}
