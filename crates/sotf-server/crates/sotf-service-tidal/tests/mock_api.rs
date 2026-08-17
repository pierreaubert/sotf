//! Integration tests for `TidalService` against a local mock HTTP server.
//!
//! The mock is a tiny keep-alive `TcpListener` server (same pattern as
//! `sotf-streaming/tests/integration.rs`) — no external dev-dependencies.
//! `with_api_base` / `with_auth_base` point the service at the mock.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use sotf_service_tidal::{DeviceAuthPoll, TidalService};
use sotf_services::{AudioQuality, ServiceCredentials, ServiceError, StreamingService};

// ---------------------------------------------------------------------------
// Mock HTTP server
// ---------------------------------------------------------------------------

struct MockServer {
    addr: SocketAddr,
    shutdown_tx: Option<std::sync::mpsc::Sender<()>>,
    join_handle: Option<thread::JoinHandle<()>>,
}

impl Drop for MockServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

struct HttpRequest {
    method: String,
    path: String,
    body: String,
}

fn read_http_request(stream: &mut TcpStream) -> HttpRequest {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut raw = Vec::new();
    let mut buf = [0u8; 4096];
    let header_end = loop {
        match stream.read(&mut buf) {
            Ok(0) => panic!("connection closed before headers complete"),
            Ok(n) => {
                raw.extend_from_slice(&buf[..n]);
                if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                    break pos + 4;
                }
            }
            Err(e) => panic!("read headers failed: {e}"),
        }
    };

    let headers = String::from_utf8_lossy(&raw[..header_end]).into_owned();
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.trim().eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);

    while raw.len() < header_end + content_length {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => raw.extend_from_slice(&buf[..n]),
            Err(e) => panic!("read body failed: {e}"),
        }
    }

    let request_line = headers.lines().next().unwrap_or_default().to_string();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let body = String::from_utf8_lossy(&raw[header_end..]).into_owned();
    HttpRequest { method, path, body }
}

fn write_http_response(stream: &mut TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).expect("write");
    stream.flush().expect("flush");
}

/// Spawn a keep-alive mock server. `handler` receives each parsed request and
/// returns `(status, body)`.
fn spawn_mock_server<F>(handler: F) -> MockServer
where
    F: Fn(&HttpRequest) -> (String, String) + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).expect("set nonblocking");

    let handler = Arc::new(handler);
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();

    let join_handle = thread::spawn(move || {
        loop {
            if shutdown_rx.try_recv().is_ok() {
                break;
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    let handler = Arc::clone(&handler);
                    thread::spawn(move || {
                        let request = read_http_request(&mut stream);
                        let (status, body) = handler(&request);
                        write_http_response(&mut stream, &status, &body);
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    });

    MockServer {
        addr,
        shutdown_tx: Some(shutdown_tx),
        join_handle: Some(join_handle),
    }
}

fn base_url(server: &MockServer) -> String {
    format!("http://{}", server.addr)
}

fn service_for(server: &MockServer) -> TidalService {
    TidalService::new()
        .with_client_id("test-client-id")
        .with_api_base(&base_url(server))
        .with_auth_base(&base_url(server))
}

// ---------------------------------------------------------------------------
// Mock response bodies
// ---------------------------------------------------------------------------

const DEVICE_AUTH_BODY: &str = r#"{
    "deviceCode": "device-code-xyz",
    "userCode": "ABCD-EFGH",
    "verificationUri": "https://link.tidal.com",
    "verificationUriComplete": "https://link.tidal.com/ABCD-EFGH",
    "expiresIn": 300
}"#;

const TOKEN_BODY: &str = r#"{
    "access_token": "access-token-1",
    "refresh_token": "refresh-token-1",
    "token_type": "Bearer",
    "expires_in": 3600
}"#;

const PENDING_BODY: &str =
    r#"{"error": "authorization_pending", "error_description": "still waiting"}"#;

const SLOW_DOWN_BODY: &str = r#"{"error": "slow_down", "error_description": "poll less often"}"#;

const ACCESS_DENIED_BODY: &str =
    r#"{"error": "access_denied", "error_description": "the user declined the request"}"#;

const SESSION_BODY: &str = r#"{"userId": 42, "countryCode": "US"}"#;

fn album_json(id: u64) -> String {
    format!(
        r#"{{"id": {id}, "title": "Album {id}", "artist": {{"name": "Artist {id}"}}, "cover": "ab12cd34-5678-90ef-1234-567890abcdef", "numberOfTracks": 10, "releaseDate": "1991-09-24"}}"#
    )
}

fn track_json(id: u64) -> String {
    format!(
        r#"{{"id": {id}, "title": "Track {id}", "duration": 200, "trackNumber": 1, "artist": {{"name": "Artist {id}"}}, "album": {{"title": "Album {id}", "cover": "ab12cd34-5678-90ef-1234-567890abcdef"}}}}"#
    )
}

/// Favorites response for `kind` ("albums"/"tracks") with `total` entries,
/// serving `page_limit`-sized slices based on the `offset` query parameter.
fn favorites_page_json(kind: &str, total: u64, page_limit: u64, offset: u64) -> String {
    let count = (total.saturating_sub(offset)).min(page_limit);
    let items: Vec<String> = (offset..offset + count)
        .map(|i| {
            let inner = if kind == "albums" {
                album_json(i)
            } else {
                track_json(i)
            };
            format!(r#"{{"created": "2026-01-01T00:00:00.000Z", "item": {inner}}}"#)
        })
        .collect();
    format!(
        r#"{{"limit": {page_limit}, "offset": {offset}, "totalNumberOfItems": {total}, "items": [{}]}}"#,
        items.join(",")
    )
}

fn query_param(path: &str, key: &str) -> Option<String> {
    let query = path.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| v.to_string())
    })
}

// ---------------------------------------------------------------------------
// Device-code flow
// ---------------------------------------------------------------------------

/// Mock auth server: device_authorization succeeds; /token answers
/// authorization_pending `pending_polls` times, then the token response.
fn spawn_device_flow_server(pending_polls: usize) -> MockServer {
    let token_polls = Arc::new(AtomicUsize::new(0));
    let token_polls_for_handler = Arc::clone(&token_polls);
    spawn_mock_server(move |req| {
        if req.method == "POST" && req.path == "/device_authorization" {
            assert!(
                req.body.contains("client_id=test-client-id"),
                "device_authorization body missing client_id: {}",
                req.body
            );
            ("200 OK".to_string(), DEVICE_AUTH_BODY.to_string())
        } else if req.method == "POST" && req.path == "/token" {
            assert!(
                req.body
                    .contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"),
                "token body missing device_code grant: {}",
                req.body
            );
            assert!(
                req.body.contains("device_code=device-code-xyz"),
                "token body missing device_code: {}",
                req.body
            );
            let n = token_polls_for_handler.fetch_add(1, Ordering::SeqCst);
            if n < pending_polls {
                ("400 Bad Request".to_string(), PENDING_BODY.to_string())
            } else {
                ("200 OK".to_string(), TOKEN_BODY.to_string())
            }
        } else {
            ("404 Not Found".to_string(), "{}".to_string())
        }
    })
}

#[test]
fn device_auth_begin_poll_pending_then_complete() {
    let server = spawn_device_flow_server(1);
    let mut service = service_for(&server);

    let prompt = service.begin_device_auth().expect("begin_device_auth");
    assert_eq!(prompt.user_code, "ABCD-EFGH");
    assert_eq!(prompt.verification_url, "https://link.tidal.com/ABCD-EFGH");
    assert_eq!(prompt.expires_in_secs, 300);
    assert!(!service.is_authenticated());

    let first = service.poll_device_auth().expect("first poll");
    assert_eq!(first, DeviceAuthPoll::Pending);
    assert!(!service.is_authenticated());

    let second = service.poll_device_auth().expect("second poll");
    assert_eq!(second, DeviceAuthPoll::Complete);
    assert!(service.is_authenticated());
    assert_eq!(service.access_token(), Some("access-token-1"));
    assert_eq!(service.refresh_token(), Some("refresh-token-1"));

    // Pending state is cleared after completion.
    let err = service.poll_device_auth().unwrap_err();
    assert!(
        matches!(err, ServiceError::AuthError(_)),
        "expected AuthError polling without pending state, got: {err}"
    );
}

#[test]
fn poll_without_begin_errors() {
    let server = spawn_device_flow_server(0);
    let mut service = service_for(&server);
    let err = service.poll_device_auth().unwrap_err();
    match err {
        ServiceError::AuthError(msg) => assert!(msg.contains("begin_device_auth")),
        other => panic!("expected AuthError, got: {other}"),
    }
}

#[test]
fn authenticate_device_code_state_machine_is_unchanged() {
    let server = spawn_device_flow_server(1);
    let mut service = service_for(&server);

    // First call starts the flow and surfaces the prompt as an AuthError.
    let err = service
        .authenticate(ServiceCredentials::DeviceCode)
        .unwrap_err();
    match err {
        ServiceError::AuthError(msg) => {
            assert_eq!(
                msg,
                "Visit https://link.tidal.com/ABCD-EFGH and enter code: ABCD-EFGH"
            );
        }
        other => panic!("expected AuthError, got: {other}"),
    }

    // Second call polls; authorization_pending is surfaced as "Waiting ...".
    let err = service
        .authenticate(ServiceCredentials::DeviceCode)
        .unwrap_err();
    match err {
        ServiceError::AuthError(msg) => {
            assert!(
                msg.contains("Waiting for Tidal device authorization"),
                "{msg}"
            );
            assert!(msg.contains("HTTP 400"), "{msg}");
        }
        other => panic!("expected AuthError, got: {other}"),
    }

    // Third call completes.
    service
        .authenticate(ServiceCredentials::DeviceCode)
        .expect("third authenticate completes");
    assert!(service.is_authenticated());
    assert_eq!(service.access_token(), Some("access-token-1"));
}

// ---------------------------------------------------------------------------
// Refresh
// ---------------------------------------------------------------------------

#[test]
fn refresh_rotates_tokens() {
    let seen_bodies = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen_for_handler = Arc::clone(&seen_bodies);
    let server = spawn_mock_server(move |req| {
        if req.method == "POST" && req.path == "/token" {
            seen_for_handler.lock().unwrap().push(req.body.clone());
            (
                "200 OK".to_string(),
                r#"{"access_token": "access-token-2", "refresh_token": "refresh-token-2", "token_type": "Bearer"}"#
                    .to_string(),
            )
        } else {
            ("404 Not Found".to_string(), "{}".to_string())
        }
    });

    let mut service = service_for(&server);
    service.set_tokens(
        "old-access-token".to_string(),
        Some("old-refresh-token".to_string()),
    );

    service.authenticate_refresh().expect("refresh");
    assert_eq!(service.access_token(), Some("access-token-2"));
    assert_eq!(service.refresh_token(), Some("refresh-token-2"));

    let bodies = seen_bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1);
    assert!(
        bodies[0].contains("grant_type=refresh_token"),
        "{}",
        bodies[0]
    );
    assert!(
        bodies[0].contains("refresh_token=old-refresh-token"),
        "{}",
        bodies[0]
    );
    assert!(
        bodies[0].contains("client_id=test-client-id"),
        "{}",
        bodies[0]
    );
}

#[test]
fn refresh_keeps_old_refresh_token_when_not_rotated() {
    let server = spawn_mock_server(|req| {
        if req.method == "POST" && req.path == "/token" {
            (
                "200 OK".to_string(),
                r#"{"access_token": "access-token-3", "token_type": "Bearer"}"#.to_string(),
            )
        } else {
            ("404 Not Found".to_string(), "{}".to_string())
        }
    });

    let mut service = service_for(&server);
    service.set_tokens(
        "old-access-token".to_string(),
        Some("old-refresh-token".to_string()),
    );
    service.authenticate_refresh().expect("refresh");
    assert_eq!(service.access_token(), Some("access-token-3"));
    assert_eq!(service.refresh_token(), Some("old-refresh-token"));
}

#[test]
fn refresh_without_refresh_token_errors() {
    let server = spawn_device_flow_server(0);
    let mut service = service_for(&server);
    let err = service.authenticate_refresh().unwrap_err();
    match err {
        ServiceError::AuthError(msg) => assert!(msg.contains("No refresh token")),
        other => panic!("expected AuthError, got: {other}"),
    }
}

// ---------------------------------------------------------------------------
// Favorites (user library)
// ---------------------------------------------------------------------------

/// Mock API server: /sessions authenticates as user 42, favorites endpoints
/// page 51 albums / 52 tracks at 50 per page.
fn spawn_favorites_server() -> MockServer {
    spawn_mock_server(|req| {
        if req.method == "GET" && req.path.starts_with("/sessions") {
            assert!(
                req.path.contains("countryCode=US"),
                "sessions request missing countryCode: {}",
                req.path
            );
            ("200 OK".to_string(), SESSION_BODY.to_string())
        } else if req.method == "GET" && req.path.starts_with("/users/42/favorites/albums") {
            let offset: u64 = query_param(&req.path, "offset")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            assert_eq!(
                query_param(&req.path, "limit").as_deref(),
                Some("50"),
                "favorites request must use page limit 50: {}",
                req.path
            );
            (
                "200 OK".to_string(),
                favorites_page_json("albums", 51, 50, offset),
            )
        } else if req.method == "GET" && req.path.starts_with("/users/42/favorites/tracks") {
            let offset: u64 = query_param(&req.path, "offset")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            (
                "200 OK".to_string(),
                favorites_page_json("tracks", 52, 50, offset),
            )
        } else {
            ("404 Not Found".to_string(), "{}".to_string())
        }
    })
}

fn authenticated_service(server: &MockServer) -> TidalService {
    let mut service = service_for(server);
    service
        .authenticate(ServiceCredentials::AccessToken(
            "access-token-1".to_string(),
        ))
        .expect("access token auth");
    service
}

#[test]
fn access_token_auth_captures_user_id() {
    let server = spawn_favorites_server();
    let service = authenticated_service(&server);
    assert_eq!(service.user_id(), Some(42));
    assert_eq!(service.access_token(), Some("access-token-1"));
}

#[test]
fn favorites_albums_pages_across_two_pages() {
    let server = spawn_favorites_server();
    let service = authenticated_service(&server);

    let albums = service.favorites_albums().expect("favorites_albums");
    assert_eq!(albums.len(), 51);
    assert_eq!(albums[0].id, "0");
    assert_eq!(albums[0].title, "Album 0");
    assert_eq!(albums[0].artist, "Artist 0");
    assert_eq!(albums[0].year, Some(1991));
    assert_eq!(albums[0].track_count, 10);
    assert_eq!(
        albums[0].album_art_url.as_deref(),
        Some("https://resources.tidal.com/images/ab12cd34/5678/90ef/1234/567890abcdef/640x640.jpg")
    );
    // Last item comes from the second page.
    assert_eq!(albums[50].id, "50");
    assert_eq!(albums[50].title, "Album 50");
}

#[test]
fn favorites_tracks_pages_across_two_pages() {
    let server = spawn_favorites_server();
    let service = authenticated_service(&server);

    let tracks = service.favorites_tracks().expect("favorites_tracks");
    assert_eq!(tracks.len(), 52);
    assert_eq!(tracks[0].id, "0");
    assert_eq!(tracks[0].title, "Track 0");
    assert_eq!(tracks[0].artist, "Artist 0");
    assert_eq!(tracks[0].album, "Album 0");
    assert_eq!(tracks[0].duration_secs, 200.0);
    assert_eq!(tracks[0].track_number, Some(1));
    assert!(tracks[0].album_art_url.is_some());
    assert_eq!(tracks[51].id, "51");
}

#[test]
fn favorites_error_when_not_authenticated() {
    let server = spawn_favorites_server();
    let service = service_for(&server);

    let err = service.favorites_albums().unwrap_err();
    assert!(matches!(err, ServiceError::AuthError(_)), "got: {err}");
    let err = service.favorites_tracks().unwrap_err();
    assert!(matches!(err, ServiceError::AuthError(_)), "got: {err}");
}

#[test]
fn favorites_error_when_user_id_unknown() {
    let server = spawn_favorites_server();
    let mut service = service_for(&server);
    // Tokens restored from a persisted session, but no user_id captured.
    service.set_tokens("access-token-1".to_string(), None);
    assert!(service.is_authenticated());

    let err = service.favorites_albums().unwrap_err();
    match err {
        ServiceError::AuthError(msg) => assert!(msg.contains("user id"), "{msg}"),
        other => panic!("expected AuthError, got: {other}"),
    }
}

// ---------------------------------------------------------------------------
// Device-code flow: error classification
// ---------------------------------------------------------------------------

/// Mock auth server: device_authorization succeeds; /token always answers
/// 400 with `error_body`.
fn spawn_device_poll_error_server(error_body: String) -> MockServer {
    spawn_mock_server(move |req| {
        if req.method == "POST" && req.path == "/device_authorization" {
            ("200 OK".to_string(), DEVICE_AUTH_BODY.to_string())
        } else if req.method == "POST" && req.path == "/token" {
            ("400 Bad Request".to_string(), error_body.clone())
        } else {
            ("404 Not Found".to_string(), "{}".to_string())
        }
    })
}

#[test]
fn device_poll_slow_down_is_pending() {
    let server = spawn_device_poll_error_server(SLOW_DOWN_BODY.to_string());
    let mut service = service_for(&server);

    service.begin_device_auth().expect("begin_device_auth");
    let poll = service.poll_device_auth().expect("poll_device_auth");
    assert_eq!(poll, DeviceAuthPoll::Pending);
    assert!(!service.is_authenticated());
}

#[test]
fn device_poll_pending_detected_beyond_truncation() {
    // A long error JSON whose `authorization_pending` code sits past the
    // 512-char log-truncation point must still be classified as Pending.
    let padding = "x".repeat(600);
    let body = format!(r#"{{"error_description": "{padding}", "error": "authorization_pending"}}"#);
    let server = spawn_device_poll_error_server(body);
    let mut service = service_for(&server);

    service.begin_device_auth().expect("begin_device_auth");
    let poll = service.poll_device_auth().expect("poll_device_auth");
    assert_eq!(poll, DeviceAuthPoll::Pending);
}

#[test]
fn device_poll_failure_surfaces_immediately() {
    let server = spawn_device_poll_error_server(ACCESS_DENIED_BODY.to_string());
    let mut service = service_for(&server);

    service.begin_device_auth().expect("begin_device_auth");
    let err = service.poll_device_auth().unwrap_err();
    match err {
        ServiceError::AuthError(msg) => {
            assert!(msg.contains("failed"), "{msg}");
            assert!(!msg.contains("Waiting"), "{msg}");
        }
        other => panic!("expected AuthError, got: {other}"),
    }

    // The terminal failure cleared the pending state: polling again requires
    // a fresh begin_device_auth.
    let err = service.poll_device_auth().unwrap_err();
    match err {
        ServiceError::AuthError(msg) => assert!(msg.contains("begin_device_auth"), "{msg}"),
        other => panic!("expected AuthError, got: {other}"),
    }
}

#[test]
fn authenticate_device_code_failure_is_not_reported_as_waiting() {
    let server = spawn_device_poll_error_server(ACCESS_DENIED_BODY.to_string());
    let mut service = service_for(&server);

    // First call starts the flow and surfaces the prompt as an AuthError.
    let err = service
        .authenticate(ServiceCredentials::DeviceCode)
        .unwrap_err();
    match err {
        ServiceError::AuthError(msg) => assert!(msg.contains("Visit"), "{msg}"),
        other => panic!("expected AuthError, got: {other}"),
    }

    // Second call polls; access_denied is terminal, not "Waiting ...".
    let err = service
        .authenticate(ServiceCredentials::DeviceCode)
        .unwrap_err();
    match err {
        ServiceError::AuthError(msg) => {
            assert!(msg.contains("failed"), "{msg}");
            assert!(!msg.contains("Waiting"), "{msg}");
        }
        other => panic!("expected AuthError, got: {other}"),
    }

    // Pending state cleared: the next call starts a fresh flow.
    let err = service
        .authenticate(ServiceCredentials::DeviceCode)
        .unwrap_err();
    match err {
        ServiceError::AuthError(msg) => assert!(msg.contains("Visit"), "{msg}"),
        other => panic!("expected AuthError, got: {other}"),
    }
}

// ---------------------------------------------------------------------------
// Stream URL resolution
// ---------------------------------------------------------------------------

/// Mock API server answering the stream-URL endpoint with the given status.
fn spawn_stream_server(status: &'static str) -> MockServer {
    spawn_mock_server(move |req| {
        if req.method == "GET" && req.path.starts_with("/tracks/123/urlpostpaywall") {
            (status.to_string(), r#"{"detail": "nope"}"#.to_string())
        } else {
            ("404 Not Found".to_string(), "{}".to_string())
        }
    })
}

#[test]
fn start_stream_unauthorized_maps_to_auth_error() {
    for status in ["401 Unauthorized", "403 Forbidden"] {
        let server = spawn_stream_server(status);
        let mut service = service_for(&server);
        // A stored (stale) token: start_stream must reach the HTTP call.
        service.set_tokens("stale-access-token".to_string(), None);

        let err = match service.start_stream("123", AudioQuality::Lossless) {
            Err(err) => err,
            Ok(_) => panic!("expected error for {status}"),
        };
        match err {
            ServiceError::AuthError(msg) => {
                assert!(msg.contains("denied"), "{msg}");
                assert!(!msg.contains("stale-access-token"), "{msg}");
            }
            other => panic!("expected AuthError for {status}, got: {other}"),
        }
    }
}

#[test]
fn start_stream_not_found_stays_not_found() {
    let server = spawn_stream_server("404 Not Found");
    let mut service = service_for(&server);
    service.set_tokens("access-token-1".to_string(), None);

    let err = match service.start_stream("123", AudioQuality::Lossless) {
        Err(err) => err,
        Ok(_) => panic!("expected NotFound"),
    };
    assert!(matches!(err, ServiceError::NotFound(_)), "got: {err}");
}

// ---------------------------------------------------------------------------
// Favorites pagination cap
// ---------------------------------------------------------------------------

#[test]
fn favorites_pagination_is_capped() {
    let page_requests = Arc::new(AtomicUsize::new(0));
    let page_requests_for_handler = Arc::clone(&page_requests);
    let server = spawn_mock_server(move |req| {
        if req.method == "GET" && req.path.starts_with("/sessions") {
            ("200 OK".to_string(), SESSION_BODY.to_string())
        } else if req.method == "GET" && req.path.starts_with("/users/42/favorites/albums") {
            page_requests_for_handler.fetch_add(1, Ordering::SeqCst);
            let offset: u64 = query_param(&req.path, "offset")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            // Pathological server: always a full page, total far ahead of
            // any reachable offset.
            (
                "200 OK".to_string(),
                favorites_page_json("albums", 1_000_000, 50, offset),
            )
        } else {
            ("404 Not Found".to_string(), "{}".to_string())
        }
    });

    let service = authenticated_service(&server);
    let albums = service.favorites_albums().expect("favorites_albums");
    // 20 pages of 50 items each, then the loop stops despite the server
    // claiming more items exist.
    assert_eq!(albums.len(), 20 * 50);
    assert_eq!(page_requests.load(Ordering::SeqCst), 20);
}
