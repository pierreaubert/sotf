use sotf_services::*;

/// Spotify Web API base URL.
pub(crate) const SPOTIFY_API_BASE: &str = "https://api.spotify.com/v1";

/// Spotify OAuth authorize endpoint.
pub(crate) const SPOTIFY_AUTH_URL: &str = "https://accounts.spotify.com/authorize";

/// Spotify OAuth token endpoint.
pub(crate) const SPOTIFY_TOKEN_URL: &str = "https://accounts.spotify.com/api/token";

/// OAuth client ID registered for librespot, with the loopback redirect
/// below. Same convention as the `librespot-oauth` example: custom
/// deployments can register their own client ID, but the loopback redirect
/// must match exactly what is registered.
pub(crate) const SPOTIFY_CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";

/// Loopback redirect URI registered against `SPOTIFY_CLIENT_ID`. Spotify
/// requires an exact match, so the port is fixed rather than ephemeral.
pub(crate) const SPOTIFY_REDIRECT_URI: &str = "http://127.0.0.1:8898/login";

/// Scopes requested during OAuth login: `streaming` is required for the
/// librespot session, the read scopes back the Web API library/search calls.
pub(crate) const SPOTIFY_SCOPES: [&str; 5] = [
    "streaming",
    "user-read-private",
    "user-read-email",
    "user-library-read",
    "playlist-read-private",
];

/// How long `login_with_oauth` waits for the browser callback before giving up.
pub(crate) const OAUTH_CALLBACK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);

/// Maximum JSON response body size we are willing to parse (16 MiB).
/// Anything larger is treated as a network error to prevent a malicious or
/// misbehaving peer from exhausting memory.
pub(crate) const MAX_JSON_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Read a response body and decode it as JSON, refusing to parse anything
/// larger than `MAX_JSON_BODY_BYTES`.
///
/// We bail out early when the server advertises a `Content-Length` greater
/// than the limit, and again after buffering in case the server omitted
/// `Content-Length` and over-sent. Mirrors the Tidal crate's hygiene.
pub(crate) async fn read_bounded_json<T: serde::de::DeserializeOwned>(
    resp: reqwest::Response,
) -> Result<T, ServiceError> {
    if let Some(len) = resp.content_length()
        && (len as usize) > MAX_JSON_BODY_BYTES
    {
        return Err(ServiceError::NetworkError(format!(
            "Response body too large: {} bytes (max {})",
            len, MAX_JSON_BODY_BYTES
        )));
    }

    let body = resp
        .bytes()
        .await
        .map_err(|e| ServiceError::NetworkError(e.to_string()))?;

    if body.len() > MAX_JSON_BODY_BYTES {
        return Err(ServiceError::NetworkError(format!(
            "Response body exceeded {} bytes",
            MAX_JSON_BODY_BYTES
        )));
    }

    serde_json::from_slice(&body).map_err(|e| ServiceError::Other(e.to_string()))
}
