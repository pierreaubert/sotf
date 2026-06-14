//! Snapshot tests for SOTF streaming protocol responses.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};
use sotf_streaming::{PcmStreamServer, PcmStreamServerConfig};

fn start_server() -> PcmStreamServer {
    PcmStreamServer::start(PcmStreamServerConfig {
        bind_addr: "127.0.0.1".to_string(),
        port: 0,
        ..Default::default()
    })
    .expect("server starts")
}

fn read_raw_response(addr: std::net::SocketAddr, request: &[u8]) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut last_error = None;
    while Instant::now() < deadline {
        match TcpStream::connect(addr) {
            Ok(mut stream) => {
                stream.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
                stream.write_all(request).unwrap();
                let mut buf = [0u8; 4096];
                let mut bytes = Vec::new();
                let inner_deadline = Instant::now() + Duration::from_secs(2);
                while Instant::now() < inner_deadline {
                    match stream.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            bytes.extend_from_slice(&buf[..n]);
                            if bytes.len() >= 4096 {
                                break;
                            }
                        }
                        Err(e)
                            if matches!(
                                e.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                            ) =>
                        {
                            continue;
                        }
                        Err(e) => panic!("read failed: {e}"),
                    }
                }
                return bytes;
            }
            Err(e) => {
                last_error = Some(e);
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    panic!(
        "failed to connect to {}: {:?}",
        addr,
        last_error.expect("error")
    );
}

fn extract_http_body(raw: &[u8]) -> Vec<u8> {
    let header_end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| p + 4)
        .unwrap_or(0);
    raw[header_end..].to_vec()
}

#[test]
fn snapshot_status_http_response_body() {
    let server = start_server();
    let raw = read_raw_response(
        server.local_addr(),
        b"GET /status HTTP/1.1\r\nHost: localhost\r\n\r\n",
    );
    let body = extract_http_body(&raw);
    let text = String::from_utf8(body).expect("status body is utf8");
    let mut value: serde_json::Value = serde_json::from_str(&text).expect("status body is json");
    // The bind_addr contains an ephemeral port; redact it for stability.
    if let Some(addr) = value.get_mut("bind_addr").and_then(|v| v.as_str()) {
        *value.get_mut("bind_addr").unwrap() = serde_json::json!(addr.replace(addr.split(':').last().unwrap_or(""), "<port>"));
    }
    insta::assert_json_snapshot!(value);
}

#[test]
fn snapshot_wav_stream_header_hex() {
    let server = start_server();
    let raw = read_raw_response(
        server.local_addr(),
        b"GET /stream.wav HTTP/1.1\r\nHost: localhost\r\n\r\n",
    );
    let body = extract_http_body(&raw);
    // Chunked encoding wraps the WAV header; locate the RIFF marker.
    let riff_pos = body
        .windows(4)
        .position(|w| w == b"RIFF")
        .expect("RIFF marker in stream");
    let header_bytes = &body[riff_pos..body.len().min(riff_pos + 44)];
    assert_eq!(header_bytes.len(), 44, "expected full 44-byte WAV header");
    let hex = header_bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    insta::assert_snapshot!(hex);
}
