//! OAuth2 authorization-code flow with PKCE for Spotify login.
//!
//! The caller opens the browser, bounds the callback wait, and can inject a
//! mock token endpoint in tests. The OAuth exchange returns the same local
//! Web API token type used for persistence and refresh.

use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, CsrfToken, EndpointNotSet, EndpointSet,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl,
    basic::BasicClient, http,
};
use sotf_services::ServiceError;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use crate::token_store::WebApiToken;

/// Overall timeout for the blocking HTTP calls against the token endpoint.
/// A half-open token endpoint must not hold the login flow indefinitely.
const TOKEN_ENDPOINT_TIMEOUT: Duration = Duration::from_secs(30);

/// Everything the second half of the flow needs after the user has been sent
/// to the authorize URL.
pub(crate) struct PkceRequest {
    /// URL the user's browser must be sent to.
    pub(crate) url: String,
    /// CSRF state that the callback must echo back.
    pub(crate) csrf_state: CsrfToken,
    /// PKCE verifier used when exchanging the returned code.
    pub(crate) verifier: PkceCodeVerifier,
}

type SpotifyOAuthClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

/// Build an OAuth client. `auth_url`/`token_url` are parameters (rather than
/// the constants directly) so tests can point the token exchange at a local
/// mock server.
pub(crate) fn build_client(
    client_id: &str,
    redirect_uri: &str,
    auth_url: &str,
    token_url: &str,
) -> Result<SpotifyOAuthClient, ServiceError> {
    let auth_url = AuthUrl::new(auth_url.to_string())
        .map_err(|e| ServiceError::Other(format!("Invalid OAuth authorize URL: {e}")))?;
    let token_url = TokenUrl::new(token_url.to_string())
        .map_err(|e| ServiceError::Other(format!("Invalid OAuth token URL: {e}")))?;
    let redirect_url = RedirectUrl::new(redirect_uri.to_string())
        .map_err(|e| ServiceError::Other(format!("Invalid OAuth redirect URI: {e}")))?;
    Ok(BasicClient::new(ClientId::new(client_id.to_string()))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_url))
}

/// Build the authorize URL for the given scopes, along with the CSRF state
/// and PKCE verifier needed to complete the flow.
pub(crate) fn build_authorize_request(client: &SpotifyOAuthClient, scopes: &[&str]) -> PkceRequest {
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        .add_scopes(scopes.iter().map(|s| Scope::new((*s).to_string())))
        .set_pkce_challenge(pkce_challenge)
        .url();
    PkceRequest {
        url: url.to_string(),
        csrf_state,
        verifier: pkce_verifier,
    }
}

/// Wait for the OAuth callback on `listener` and return the authorization
/// code. Gives up after `timeout` and verifies the CSRF `state` parameter.
pub(crate) fn await_auth_code(
    listener: &TcpListener,
    expected_state: &CsrfToken,
    timeout: Duration,
) -> Result<AuthorizationCode, ServiceError> {
    listener
        .set_nonblocking(true)
        .map_err(|e| ServiceError::Other(format!("OAuth listener error: {e}")))?;

    let deadline = Instant::now() + timeout;
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(ServiceError::AuthError(
                        "Timed out waiting for the Spotify OAuth callback; \
                         login was not completed in the browser"
                            .to_string(),
                    ));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return Err(ServiceError::NetworkError(format!(
                    "OAuth listener accept failed: {e}"
                )));
            }
        }
    };
    // The accepted stream must be blocking for the line read below.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));

    let mut request_line = String::new();
    BufReader::new(&stream)
        .read_line(&mut request_line)
        .map_err(|e| ServiceError::NetworkError(format!("OAuth callback read failed: {e}")))?;

    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| {
            ServiceError::Other(format!(
                "Malformed OAuth callback request: {request_line:?}"
            ))
        })?
        .to_string();

    // Tell the browser the flow is done before we go on to the exchange.
    let message = "SOTF Spotify login complete — you can close this tab.";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        message.len(),
        message
    );
    let _ = stream.try_clone().and_then(|mut s| {
        s.write_all(response.as_bytes())?;
        s.flush()
    });

    let url = url::Url::parse(&format!("http://127.0.0.1{path}"))
        .map_err(|e| ServiceError::Other(format!("Malformed OAuth callback URI {path:?}: {e}")))?;
    let params: std::collections::HashMap<String, String> =
        url.query_pairs().into_owned().collect();

    if let Some(error) = params.get("error") {
        return Err(ServiceError::AuthError(format!(
            "Spotify authorization failed: {error}"
        )));
    }

    match params.get("state") {
        Some(state) if state == expected_state.secret() => {}
        _ => {
            return Err(ServiceError::AuthError(
                "OAuth state mismatch — refusing callback (possible CSRF)".to_string(),
            ));
        }
    }

    let code = params.get("code").ok_or_else(|| {
        ServiceError::AuthError("OAuth callback did not carry an authorization code".to_string())
    })?;
    Ok(AuthorizationCode::new(code.clone()))
}

/// Blocking HTTP client for the token exchange, with a timeout and no redirects.
/// OAuth2 5 uses HTTP 1 request and response types, matching the reqwest client.
fn http_client_with_timeout(
    request: oauth2::HttpRequest,
) -> Result<oauth2::HttpResponse, std::io::Error> {
    let client = reqwest::blocking::Client::builder()
        // Following redirects opens the client up to SSRF vulnerabilities.
        .redirect(reqwest::redirect::Policy::none())
        .timeout(TOKEN_ENDPOINT_TIMEOUT)
        .build()
        .map_err(std::io::Error::other)?;

    let (parts, body) = request.into_parts();
    let method = reqwest::Method::from_bytes(parts.method.as_str().as_bytes())
        .map_err(std::io::Error::other)?;
    let mut request_builder = client.request(method, parts.uri.to_string()).body(body);
    for (name, value) in &parts.headers {
        request_builder = request_builder.header(name.as_str(), value.as_bytes());
    }
    let mut response = client
        .execute(request_builder.build().map_err(std::io::Error::other)?)
        .map_err(std::io::Error::other)?;

    let mut body = Vec::new();
    response.read_to_end(&mut body)?;

    let status_code =
        http::StatusCode::from_u16(response.status().as_u16()).map_err(std::io::Error::other)?;
    let mut headers = http::HeaderMap::new();
    for (name, value) in response.headers() {
        let name = http::header::HeaderName::from_bytes(name.as_str().as_bytes())
            .map_err(std::io::Error::other)?;
        let value = http::header::HeaderValue::from_bytes(value.as_bytes())
            .map_err(std::io::Error::other)?;
        headers.append(name, value);
    }

    let mut oauth_response = http::Response::builder()
        .status(status_code)
        .body(body)
        .map_err(std::io::Error::other)?;
    *oauth_response.headers_mut() = headers;
    Ok(oauth_response)
}

/// Exchange an authorization code for an access token from the sync trait surface.
pub(crate) fn exchange_code(
    client: &SpotifyOAuthClient,
    code: AuthorizationCode,
    verifier: PkceCodeVerifier,
) -> Result<WebApiToken, ServiceError> {
    let token = client
        .exchange_code(code)
        .set_pkce_verifier(verifier)
        .request(&http_client_with_timeout)
        .map_err(|e| ServiceError::AuthError(format!("OAuth token exchange failed: {e}")))?;

    let refresh_token = token
        .refresh_token()
        .map(|t| t.secret().to_string())
        .unwrap_or_default(); // Spotify always provides a refresh token.

    Ok(WebApiToken::new(
        token.access_token().secret().to_string(),
        refresh_token,
        token
            .expires_in()
            .unwrap_or_else(|| Duration::from_secs(3600)),
    ))
}

/// Response shape of the token endpoint for a refresh grant. Spotify may
/// rotate the refresh token; when it doesn't, the caller keeps the old one.
#[derive(serde::Deserialize)]
struct RefreshResponse {
    access_token: String,
    expires_in: Option<u64>,
    refresh_token: Option<String>,
}

/// Refresh an OAuth access token against the token endpoint (async; driven
/// through the service's `AsyncRuntime` by the callers). `token_url` and
/// `client_id` are parameters so tests can point at a local mock server.
///
/// Never includes token material in errors; the response body (which only
/// carries Spotify's error code) is truncated before logging.
pub(crate) async fn refresh_access_token(
    token_url: &str,
    client_id: &str,
    refresh_token: &str,
) -> Result<WebApiToken, ServiceError> {
    let client = reqwest::Client::builder()
        .timeout(TOKEN_ENDPOINT_TIMEOUT)
        .build()
        .map_err(|e| ServiceError::Other(format!("Failed to create HTTP client: {e}")))?;

    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "refresh_token")
        .append_pair("refresh_token", refresh_token)
        .append_pair("client_id", client_id)
        .finish();
    let resp = client
        .post(token_url)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .map_err(|e| ServiceError::NetworkError(e.to_string()))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let body = crate::misc::truncate_for_log(&body, 512);
        return Err(ServiceError::AuthError(format!(
            "OAuth token refresh failed: HTTP {status} ({body})"
        )));
    }

    let parsed: RefreshResponse = crate::consts::read_bounded_json(resp).await?;
    Ok(WebApiToken::new(
        parsed.access_token,
        parsed
            .refresh_token
            .unwrap_or_else(|| refresh_token.to_string()),
        Duration::from_secs(parsed.expires_in.unwrap_or(3600)),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::{
        SPOTIFY_AUTH_URL, SPOTIFY_CLIENT_ID, SPOTIFY_REDIRECT_URI, SPOTIFY_TOKEN_URL,
    };
    use crate::test_util::{spawn_mock_server, spawn_mock_server_with_headers};
    use std::io::Read;

    #[test]
    fn authorize_url_contains_pkce_and_scopes() {
        let client = build_client(
            SPOTIFY_CLIENT_ID,
            SPOTIFY_REDIRECT_URI,
            SPOTIFY_AUTH_URL,
            SPOTIFY_TOKEN_URL,
        )
        .unwrap();
        let req = build_authorize_request(&client, &["streaming", "user-library-read"]);
        let url = url::Url::parse(&req.url).unwrap();
        assert_eq!(url.host_str(), Some("accounts.spotify.com"));
        let params: std::collections::HashMap<String, String> =
            url.query_pairs().into_owned().collect();
        assert_eq!(
            params.get("client_id").map(String::as_str),
            Some(SPOTIFY_CLIENT_ID)
        );
        assert_eq!(
            params.get("redirect_uri").map(String::as_str),
            Some(SPOTIFY_REDIRECT_URI)
        );
        assert_eq!(
            params.get("response_type").map(String::as_str),
            Some("code")
        );
        assert!(params.contains_key("code_challenge"));
        assert_eq!(
            params.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        let scope = params.get("scope").unwrap();
        assert!(scope.contains("streaming"));
        assert!(scope.contains("user-library-read"));
    }

    #[test]
    fn await_auth_code_times_out() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let state = CsrfToken::new("test-state".to_string());
        let start = Instant::now();
        let result = await_auth_code(&listener, &state, Duration::from_millis(200));
        assert!(result.is_err());
        assert!(start.elapsed() >= Duration::from_millis(200));
        match result {
            Err(ServiceError::AuthError(msg)) => assert!(msg.contains("Timed out"), "got: {msg}"),
            other => panic!("expected AuthError, got: {other:?}"),
        }
    }

    #[test]
    fn await_auth_code_accepts_valid_callback() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let state = CsrfToken::new("state-123".to_string());

        let state_for_client = state.clone();
        let client_thread = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(addr).unwrap();
            let request = format!(
                "GET /login?code=authcode-xyz&state={} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
                state_for_client.secret()
            );
            stream.write_all(request.as_bytes()).unwrap();
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf).unwrap();
            String::from_utf8_lossy(&buf).into_owned()
        });

        let code = await_auth_code(&listener, &state, Duration::from_secs(5)).unwrap();
        assert_eq!(code.secret(), "authcode-xyz");
        let response = client_thread.join().unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK"), "got: {response}");
    }

    #[test]
    fn await_auth_code_rejects_state_mismatch() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let state = CsrfToken::new("expected-state".to_string());

        std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(addr).unwrap();
            stream
                .write_all(b"GET /login?code=x&state=WRONG HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
                .unwrap();
            let mut buf = Vec::new();
            let _ = stream.read_to_end(&mut buf);
        });

        let result = await_auth_code(&listener, &state, Duration::from_secs(5));
        match result {
            Err(ServiceError::AuthError(msg)) => {
                assert!(msg.contains("state mismatch"), "got: {msg}")
            }
            other => panic!("expected AuthError, got: {other:?}"),
        }
    }

    #[test]
    fn exchange_code_against_mock_token_endpoint() {
        let server = spawn_mock_server(|req| {
            assert_eq!(req.method, "POST");
            assert_eq!(req.path, "/api/token");
            // PKCE exchange must carry the code, verifier, client id and
            // (percent-encoded) redirect URI in the form body.
            assert!(
                req.body.contains("grant_type=authorization_code"),
                "{}",
                req.body
            );
            assert!(req.body.contains("code=mock-code"), "{}", req.body);
            assert!(req.body.contains("code_verifier="), "{}", req.body);
            assert!(req.body.contains("client_id=test-client"), "{}", req.body);
            assert!(
                req.body.contains("redirect_uri=http%3A%2F%2F127.0.0.1"),
                "{}",
                req.body
            );
            (
                200,
                r#"{"access_token":"mock-access-token","token_type":"Bearer",
                    "expires_in":3600,"refresh_token":"mock-refresh",
                    "scope":"streaming user-library-read"}"#
                    .to_string(),
            )
        });

        let client = build_client(
            "test-client",
            "http://127.0.0.1:8898/login",
            &format!("{}/authorize", server.base_url),
            &format!("{}/api/token", server.base_url),
        )
        .unwrap();
        let (_challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let token = exchange_code(
            &client,
            AuthorizationCode::new("mock-code".to_string()),
            verifier,
        )
        .unwrap();
        assert_eq!(token.access_token, "mock-access-token");
        assert_eq!(token.refresh_token, "mock-refresh");
        assert!(!token.is_expired());
    }

    #[test]
    fn exchange_code_maps_server_error_to_auth_error() {
        let server = spawn_mock_server(|_req| (400, r#"{"error":"invalid_grant"}"#.to_string()));
        let client = build_client(
            "test-client",
            "http://127.0.0.1:8898/login",
            &format!("{}/authorize", server.base_url),
            &format!("{}/api/token", server.base_url),
        )
        .unwrap();
        let (_challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let result = exchange_code(
            &client,
            AuthorizationCode::new("bad-code".to_string()),
            verifier,
        );
        match result {
            Err(ServiceError::AuthError(msg)) => {
                assert!(msg.contains("exchange failed"), "got: {msg}")
            }
            other => panic!("expected AuthError, got: {other:?}"),
        }
    }

    #[test]
    fn token_http_adapter_preserves_error_status_body_and_headers() {
        let server = spawn_mock_server_with_headers(|req| {
            assert_eq!(req.method, "POST");
            assert!(
                req.headers
                    .to_ascii_lowercase()
                    .contains("x-sotf-test: sent")
            );
            assert_eq!(req.body, "code=bad");
            (
                400,
                vec![
                    ("Retry-After".to_string(), "17".to_string()),
                    ("X-Token-Note".to_string(), "first".to_string()),
                    ("X-Token-Note".to_string(), "second".to_string()),
                ],
                r#"{"error":"invalid_grant"}"#.to_string(),
            )
        });
        let mut headers = http::HeaderMap::new();
        headers.insert("x-sotf-test", http::HeaderValue::from_static("sent"));
        let mut request = http::Request::builder()
            .uri(format!("{}/api/token", server.base_url))
            .method(http::Method::POST)
            .body(b"code=bad".to_vec())
            .unwrap();
        *request.headers_mut() = headers;
        let response = http_client_with_timeout(request).unwrap();

        assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
        assert_eq!(response.headers()["retry-after"].to_str().unwrap(), "17");
        let notes: Vec<_> = response.headers().get_all("x-token-note").iter().collect();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].to_str().unwrap(), "first");
        assert_eq!(notes[1].to_str().unwrap(), "second");
        assert_eq!(response.body().as_slice(), br#"{"error":"invalid_grant"}"#);
    }

    #[test]
    fn token_http_adapter_refuses_redirect() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let followed = Arc::new(AtomicBool::new(false));
        let target_hit = Arc::clone(&followed);
        let target = spawn_mock_server(move |_req| {
            target_hit.store(true, Ordering::SeqCst);
            (200, "unexpected redirect".to_string())
        });
        let destination = format!("{}/not-a-token-endpoint", target.base_url);
        let location = destination.clone();
        let server = spawn_mock_server_with_headers(move |_req| {
            (
                302,
                vec![("Location".to_string(), location.clone())],
                "redirect refused".to_string(),
            )
        });
        let response = http_client_with_timeout(
            http::Request::builder()
                .uri(format!("{}/api/token", server.base_url))
                .method(http::Method::POST)
                .body(b"code=private".to_vec())
                .unwrap(),
        )
        .unwrap();

        assert_eq!(response.status(), http::StatusCode::FOUND);
        assert_eq!(
            response.headers()["location"].to_str().unwrap(),
            destination
        );
        assert!(!followed.load(Ordering::SeqCst));
    }

    #[test]
    fn refresh_access_token_against_mock_token_endpoint() {
        let server = spawn_mock_server(|req| {
            assert_eq!(req.method, "POST");
            assert_eq!(req.path, "/api/token");
            assert!(
                req.body.contains("grant_type=refresh_token"),
                "{}",
                req.body
            );
            assert!(
                req.body.contains("refresh_token=old-refresh"),
                "{}",
                req.body
            );
            assert!(req.body.contains("client_id=test-client"), "{}", req.body);
            (
                200,
                r#"{"access_token":"fresh-access","token_type":"Bearer",
                    "expires_in":3600,"refresh_token":"rotated-refresh"}"#
                    .to_string(),
            )
        });

        let rt = crate::async_runtime::AsyncRuntime::new().unwrap();
        let token = rt
            .block_on(refresh_access_token(
                &format!("{}/api/token", server.base_url),
                "test-client",
                "old-refresh",
            ))
            .unwrap();
        assert_eq!(token.access_token, "fresh-access");
        // Spotify rotated the refresh token; the new one must be kept.
        assert_eq!(token.refresh_token, "rotated-refresh");
        assert!(!token.is_expired());
    }

    #[test]
    fn refresh_access_token_keeps_old_refresh_token_when_not_rotated() {
        let server = spawn_mock_server(|_req| {
            (
                200,
                r#"{"access_token":"fresh-access","token_type":"Bearer",
                    "expires_in":3600}"#
                    .to_string(),
            )
        });

        let rt = crate::async_runtime::AsyncRuntime::new().unwrap();
        let token = rt
            .block_on(refresh_access_token(
                &format!("{}/api/token", server.base_url),
                "test-client",
                "old-refresh",
            ))
            .unwrap();
        assert_eq!(token.refresh_token, "old-refresh");
    }

    #[test]
    fn refresh_access_token_maps_server_error_to_auth_error() {
        let server = spawn_mock_server(|_req| (400, r#"{"error":"invalid_grant"}"#.to_string()));
        let rt = crate::async_runtime::AsyncRuntime::new().unwrap();
        let result = rt.block_on(refresh_access_token(
            &format!("{}/api/token", server.base_url),
            "test-client",
            "old-refresh",
        ));
        match result {
            Err(ServiceError::AuthError(msg)) => {
                assert!(msg.contains("refresh failed"), "got: {msg}");
                assert!(!msg.contains("old-refresh"), "token leaked: {msg}");
            }
            other => panic!("expected AuthError, got: {other:?}"),
        }
    }
}
