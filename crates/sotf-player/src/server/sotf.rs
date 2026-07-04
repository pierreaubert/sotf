use super::dlna::dlna_advertised_ipv4;
use crate::federation_config::SotfApiSettings;
use serde_json::json;

/// URL that SOTF remote clients can use for a configured API bind address.
#[must_use]
pub fn sotf_api_server_url_for_bind(bind_address: &str, port: u16) -> String {
    sotf_api_server_url_for_bind_with_tls(bind_address, port, true)
}

/// URL that SOTF remote clients can use for the configured API server.
#[must_use]
pub fn sotf_api_server_url_for_settings(settings: &SotfApiSettings) -> String {
    sotf_api_server_url_for_bind_with_tls(
        &settings.bind_address,
        settings.port,
        settings.tls_enabled,
    )
}

#[must_use]
pub fn sotf_api_server_url_for_bind_with_tls(
    bind_address: &str,
    port: u16,
    tls_enabled: bool,
) -> String {
    let host = dlna_advertised_ipv4(bind_address);
    let scheme = if tls_enabled { "https" } else { "http" };
    format!("{scheme}://{host}:{port}/api/v1")
}

pub fn sotf_api_connection_qr_payload(settings: &SotfApiSettings) -> Result<String, String> {
    let token = settings
        .auth_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| "SOTF API auth token is not configured".to_string())?;
    let url = sotf_api_server_url_for_settings(settings);
    Ok(json!({
        "kind": "sotf-api-connection",
        "version": 1,
        "name": settings.friendly_name.clone(),
        "url": url,
        "auth": "bearer",
        "token": token,
    })
    .to_string())
}
