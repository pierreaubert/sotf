//! Shared mock HTTP server for streaming-provider integration tests.
//!
//! Same pattern as `sotf-service-spotify/src/test_util.rs`: a tiny loopback
//! `TcpListener` server, so tests stay deterministic with no dev-dependencies.
//! `Connection: close` is used so the client opens one connection per request.

#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub(crate) struct MockRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) host: String,
    pub(crate) headers: String,
    pub(crate) body: String,
}

pub(crate) struct MockResponse {
    pub(crate) status: u16,
    pub(crate) content_type: String,
    pub(crate) body: Vec<u8>,
}

impl MockResponse {
    pub(crate) fn json(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            content_type: "application/json".to_string(),
            body: body.into().into_bytes(),
        }
    }

    pub(crate) fn image(bytes: Vec<u8>) -> Self {
        Self {
            status: 200,
            content_type: "image/jpeg".to_string(),
            body: bytes,
        }
    }
}

pub(crate) struct MockServer {
    pub(crate) base_url: String,
}

/// Spawn a mock HTTP server on a loopback port. `handler` receives each
/// parsed request and returns the response to send.
pub(crate) fn spawn_mock_server<F>(handler: F) -> MockServer
where
    F: Fn(&MockRequest) -> MockResponse + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().unwrap();
    let handler = Arc::new(handler);

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let handler = Arc::clone(&handler);
            thread::spawn(move || {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let raw = read_request(&mut stream);
                let request = parse_request(&raw);
                let response = handler(&request);
                let reason = match response.status {
                    200 => "OK",
                    400 => "Bad Request",
                    401 => "Unauthorized",
                    403 => "Forbidden",
                    404 => "Not Found",
                    500 => "Internal Server Error",
                    _ => "Status",
                };
                let head = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    response.status,
                    reason,
                    response.content_type,
                    response.body.len(),
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&response.body);
                let _ = stream.flush();
            });
        }
    });

    MockServer {
        base_url: format!("http://{addr}"),
    }
}

fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        if find_subslice(&data, b"\r\n\r\n").is_some() {
            break;
        }
        match stream.read(&mut buf) {
            Ok(0) => return data,
            Ok(n) => data.extend_from_slice(&buf[..n]),
            Err(_) => return data,
        }
    }
    let header_text = String::from_utf8_lossy(&data).into_owned();
    let content_length: usize = header_text
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.trim().eq_ignore_ascii_case("content-length") {
                value.trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);
    let body_start = find_subslice(&data, b"\r\n\r\n").unwrap() + 4;
    while data.len() < body_start + content_length {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => data.extend_from_slice(&buf[..n]),
            Err(_) => break,
        }
    }
    data
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_request(raw: &[u8]) -> MockRequest {
    let text = String::from_utf8_lossy(raw).into_owned();
    let header_end = text.find("\r\n\r\n").unwrap_or(text.len());
    let head = &text[..header_end];
    let mut lines = head.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let headers = lines.collect::<Vec<_>>().join("\n");
    let host = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.trim().eq_ignore_ascii_case("host") {
                Some(value.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();
    let body = if header_end + 4 <= text.len() {
        text[header_end + 4..].to_string()
    } else {
        String::new()
    };
    MockRequest {
        method,
        path,
        host,
        headers,
        body,
    }
}
