use super::pcm_stream_format::PcmStreamFormat;
use super::pcm_stream_format::build_wav_stream_header_f32;
use super::pcm_stream_handle::PcmStreamHandle;
use super::pcm_stream_server::PcmStreamServer;
use super::pcm_stream_server_config::PcmStreamServerConfig;
use std::collections::HashSet;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::thread::{self};
use std::time::{Duration, Instant};

const SERVER_ATTEMPTS: usize = 10;

fn start_test_server() -> Option<PcmStreamServer> {
    match PcmStreamServer::start(PcmStreamServerConfig {
        bind_addr: "127.0.0.1".to_string(),
        port: 0,
        ..Default::default()
    }) {
        Ok(server) => Some(server),
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            eprintln!("Skipping streaming server test: loopback bind unavailable ({err})");
            None
        }
        Err(err) => panic!("failed to start test streaming server: {err}"),
    }
}

fn contains_bytes(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

fn wait_until<F>(timeout: Duration, mut condition: F) -> bool
where
    F: FnMut() -> bool,
{
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(5));
    }
    condition()
}

fn read_until<F>(stream: &mut TcpStream, timeout: Duration, mut done: F) -> Vec<u8>
where
    F: FnMut(&[u8]) -> bool,
{
    stream
        .set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();
    let deadline = Instant::now() + timeout;
    let mut bytes = Vec::new();
    let mut buf = [0u8; 4096];
    while Instant::now() < deadline {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                bytes.extend_from_slice(&buf[..n]);
                if done(&bytes) {
                    break;
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(e) => panic!("read failed: {e}"),
        }
    }
    bytes
}

fn parse_content_length(headers: &[u8]) -> Option<usize> {
    let lower = headers.to_ascii_lowercase();
    let key = b"content-length:";
    let pos = lower.windows(key.len()).position(|w| w == key)?;
    let rest = &headers[pos + key.len()..];
    let line_end = rest.iter().position(|&b| b == b'\r')?;
    std::str::from_utf8(&rest[..line_end])
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn read_response(stream: &mut TcpStream, timeout: Duration) -> Vec<u8> {
    let mut bytes = read_until(stream, timeout, |b| {
        b.windows(4).any(|w| w == b"\r\n\r\n")
    });
    let Some(header_end) = bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| p + 4)
    else {
        return bytes;
    };
    let Some(content_length) = parse_content_length(&bytes[..header_end]) else {
        return bytes;
    };
    let total = header_end + content_length;
    if bytes.len() < total {
        let missing = total - bytes.len();
        let body = read_until(stream, timeout, |b| b.len() >= missing);
        bytes.extend(body.iter().take(missing));
    }
    bytes
}

fn request_response(addr: std::net::SocketAddr, request: &[u8], expected_status: &str) -> Vec<u8> {
    let mut last_response = Vec::new();
    for attempt in 0..SERVER_ATTEMPTS {
        if attempt > 0 {
            thread::sleep(Duration::from_millis(20));
        }
        let mut stream = match TcpStream::connect(addr) {
            Ok(s) => s,
            Err(e) => {
                last_response = format!("connect failed: {e}").into_bytes();
                continue;
            }
        };
        if let Err(e) = stream.write_all(request) {
            last_response = format!("write failed: {e}").into_bytes();
            continue;
        }
        let bytes = read_response(&mut stream, Duration::from_secs(2));
        if String::from_utf8_lossy(&bytes).starts_with(expected_status) {
            return bytes;
        }
        last_response = bytes;
    }

    panic!(
        "expected {expected_status}, got: {:?}",
        String::from_utf8_lossy(&last_response)
    );
}

fn open_registered_stream<F>(
    server: &PcmStreamServer,
    handle: &PcmStreamHandle,
    request: &[u8],
    mut header_ready: F,
) -> (TcpStream, Vec<u8>)
where
    F: FnMut(&[u8]) -> bool,
{
    let mut last_response = Vec::new();
    for _ in 0..SERVER_ATTEMPTS {
        let baseline_clients = handle.stats().client_count;
        let mut stream = TcpStream::connect(server.local_addr()).unwrap();
        stream.write_all(request).unwrap();
        let bytes = read_until(&mut stream, Duration::from_secs(2), |bytes| {
            header_ready(bytes)
        });
        let header_seen = header_ready(&bytes);
        if header_seen
            && wait_until(Duration::from_secs(2), || {
                handle.stats().client_count > baseline_clients
            })
        {
            return (stream, bytes);
        }
        last_response = bytes;
        thread::sleep(Duration::from_millis(20));
    }

    panic!(
        "streaming client was not registered before publishing; last response: {:?}",
        String::from_utf8_lossy(&last_response)
    );
}

#[test]
fn wav_stream_header_is_float_pcm() {
    let header = build_wav_stream_header_f32(PcmStreamFormat::new(48_000, 2));
    assert_eq!(&header[0..4], b"RIFF");
    assert_eq!(&header[8..12], b"WAVE");
    assert_eq!(&header[12..16], b"fmt ");
    assert_eq!(u16::from_le_bytes([header[20], header[21]]), 3);
    assert_eq!(u16::from_le_bytes([header[22], header[23]]), 2);
    assert_eq!(
        u32::from_le_bytes([header[24], header[25], header[26], header[27]]),
        48_000
    );
    assert_eq!(u16::from_le_bytes([header[34], header[35]]), 32);
    assert_eq!(&header[36..40], b"data");
}

#[test]
fn status_endpoint_reports_server_state() {
    let Some(server) = start_test_server() else {
        return;
    };

    let bytes = request_response(
        server.local_addr(),
        b"GET /status HTTP/1.1\r\nHost: localhost\r\n\r\n",
        "HTTP/1.1 200 OK",
    );
    let response = String::from_utf8_lossy(&bytes);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("\"status\":\"ok\""));
    assert!(response.contains("\"sample_rate\":48000"));
    assert!(response.contains("\"channels\":2"));

    // `PcmStreamServer` implements `Drop`, so the server is shut down even if
    // an assertion above panics.
}

#[test]
fn wav_stream_receives_header_and_published_audio() {
    let Some(server) = start_test_server() else {
        return;
    };
    let handle = server.handle();

    let (mut stream, mut bytes) = open_registered_stream(
        &server,
        &handle,
        b"GET /stream.wav HTTP/1.1\r\nHost: localhost\r\n\r\n",
        |bytes| contains_bytes(bytes, b"RIFF") && contains_bytes(bytes, b"WAVE"),
    );

    let samples = [0.25, -0.25, 0.5, -0.5];
    assert!(handle.publish(&samples, 2, 2, 48_000));
    let sample_bytes: Vec<u8> = samples
        .iter()
        .flat_map(|sample| f32::to_le_bytes(*sample))
        .collect();
    bytes.extend(read_until(&mut stream, Duration::from_secs(2), |bytes| {
        contains_bytes(bytes, &sample_bytes)
    }));
    let response = String::from_utf8_lossy(&bytes);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("Content-Type: audio/wav"));
    assert!(contains_bytes(&bytes, b"RIFF"));
    assert!(contains_bytes(&bytes, b"WAVE"));
    assert!(contains_bytes(&bytes, &sample_bytes));
    assert!(handle.stats().published_chunks >= 1);

    // `PcmStreamServer` implements `Drop`, so the server is shut down even if
    // an assertion above panics.
}

#[test]
fn publish_rejects_invalid_shape() {
    let Some(server) = start_test_server() else {
        return;
    };
    let handle = server.handle();

    assert!(!handle.publish(&[0.0, 1.0, 2.0], 2, 2, 48_000));
    assert_eq!(handle.stats().dropped_chunks, 1);

    // `PcmStreamServer` implements `Drop`, so the server is shut down even if
    // an assertion above panics.
}

#[test]
fn parallel_server_starts_use_distinct_ports() {
    const N: usize = 16;
    let addrs = Mutex::new(Vec::with_capacity(N));

    thread::scope(|s| {
        for _ in 0..N {
            s.spawn(|| {
                let Some(server) = start_test_server() else {
                    return;
                };
                addrs.lock().unwrap().push(server.local_addr());
                // The server is dropped here when the scope ends, releasing its
                // port. Each `local_addr()` was captured before the drop.
            });
        }
    });

    let addrs = addrs.into_inner().unwrap();
    assert_eq!(
        addrs.len(),
        N,
        "expected {N} servers to start, got {}: {addrs:?}",
        addrs.len()
    );
    let unique: HashSet<_> = addrs.iter().copied().collect();
    assert_eq!(
        unique.len(),
        N,
        "servers reused a port: {addrs:?}",
    );
}
