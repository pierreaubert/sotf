use super::misc::invalid_configured_client_fingerprints;
use crate::federation_config::{self, ServerConfig, SotfApiSettings};

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
