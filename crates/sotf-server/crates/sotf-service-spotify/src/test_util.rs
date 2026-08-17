//! Shared mock HTTP server used by the offline tests in this crate.
//!
//! Follows the same pattern as `sotf-streaming/tests/integration.rs`: a tiny
//! loopback `TcpListener` server, so tests stay deterministic and need no
//! extra dev-dependencies.

#![cfg(test)]

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

pub(crate) struct MockHttpServer {
    pub(crate) base_url: String,
}

/// Spawn a mock HTTP server on a loopback port. `handler` receives each
/// parsed request and returns (status code, JSON body). Responses always use
/// `Connection: close` so the client opens one connection per request.
pub(crate) fn spawn_mock_server<F>(handler: F) -> MockHttpServer
where
    F: Fn(&MockRequest) -> (u16, String) + Send + Sync + 'static,
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
                let (status, body) = handler(&request);
                let reason = match status {
                    200 => "OK",
                    400 => "Bad Request",
                    401 => "Unauthorized",
                    403 => "Forbidden",
                    404 => "Not Found",
                    500 => "Internal Server Error",
                    _ => "Status",
                };
                let response = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            });
        }
    });

    MockHttpServer {
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
