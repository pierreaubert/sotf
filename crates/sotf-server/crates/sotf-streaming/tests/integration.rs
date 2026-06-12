//! Integration tests for sotf-streaming public API.
//!
//! These tests exercise the crate as a black box: only public types and
//! methods from `sotf_streaming` are used. Networked protocols are mocked
//! with local `std::net::TcpListener` servers so the tests remain
//! deterministic and offline-safe.

use std::io::{self, BufRead, Read, Seek, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(feature = "hls")]
use std::sync::atomic::{AtomicUsize, Ordering};

use sotf_streaming::{
    HttpMediaSource, IcyMetadata, MpdStreamSource, PcmStreamHandle, PcmStreamServer,
    PcmStreamServerConfig, StreamMetadata,
};

// ---------------------------------------------------------------------------
// ICY metadata parsing
// ---------------------------------------------------------------------------

#[test]
fn icy_metadata_parses_title_and_url() {
    let raw = b"StreamTitle='Artist - Song';StreamUrl='http://radio.example.com';";
    let meta = IcyMetadata::parse(raw);
    assert_eq!(meta.stream_title.as_deref(), Some("Artist - Song"));
    assert_eq!(meta.stream_url.as_deref(), Some("http://radio.example.com"));
}

#[test]
fn icy_metadata_handles_empty_values_and_garbage() {
    let meta = IcyMetadata::parse(b"StreamTitle='';");
    assert_eq!(meta.stream_title, None);
    assert_eq!(meta.stream_url, None);

    let meta = IcyMetadata::parse(b"not icy at all");
    assert_eq!(meta.stream_title, None);
    assert_eq!(meta.stream_url, None);
}

#[test]
fn icy_metadata_strips_null_padding() {
    let mut raw = b"StreamTitle='Padded';".to_vec();
    raw.extend_from_slice(&[0u8; 64]);
    let meta = IcyMetadata::parse(&raw);
    assert_eq!(meta.stream_title.as_deref(), Some("Padded"));
}

// ---------------------------------------------------------------------------
// HTTP media source (mocked local server)
// ---------------------------------------------------------------------------

/// Spawn a tiny HTTP server on a loopback port. The `handler` receives the raw
/// HTTP request bytes and returns a response. The server processes a single
/// request then exits; the returned guard keeps the listener alive until the
/// test ends.
struct MockHttpServer {
    addr: SocketAddr,
    _guard: Option<MockServerGuard>,
}

struct MockServerGuard {
    shutdown_tx: std::sync::mpsc::Sender<()>,
    join_handle: Option<thread::JoinHandle<()>>,
}

impl Drop for MockServerGuard {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

fn read_http_request_bytes(stream: &mut TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                request.extend_from_slice(&buf[..n]);
                if request.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
                continue;
            }
            Err(e) => panic!("read request failed: {e}"),
        }
    }
    request
}

fn spawn_single_request_server<F>(handler: F) -> MockHttpServer
where
    F: FnOnce(&str) -> (String, String, Vec<u8>) + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().unwrap();

    let join_handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let request = read_http_request_bytes(&mut stream);
        let request = String::from_utf8_lossy(&request).into_owned();
        let (status, content_type, body) = handler(&request);
        write_http_response(&mut stream, &status, &content_type, &body);
    });

    MockHttpServer {
        addr,
        _guard: Some(MockServerGuard {
            shutdown_tx: std::sync::mpsc::channel().0,
            join_handle: Some(join_handle),
        }),
    }
}

fn spawn_keep_alive_server<F>(handler: F) -> MockHttpServer
where
    F: Fn(&str) -> (String, String, Vec<(String, String)>, Vec<u8>) + Send + Sync + 'static,
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
                    // The listener is non-blocking; ensure the accepted stream is
                    // blocking so request reads wait for data.
                    let _ = stream.set_nonblocking(false);
                    let handler = Arc::clone(&handler);
                    thread::spawn(move || {
                        let request = read_http_request_bytes(&mut stream);
                        if !request.is_empty() {
                            let request = String::from_utf8_lossy(&request).into_owned();
                            let (status, content_type, extra_headers, body) = handler(&request);
                            let extra_headers: Vec<(&str, &str)> = extra_headers
                                .iter()
                                .map(|(k, v)| (k.as_str(), v.as_str()))
                                .collect();
                            write_http_response_with_headers(
                                &mut stream,
                                &status,
                                &content_type,
                                &extra_headers,
                                &body,
                            );
                        }
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    });

    MockHttpServer {
        addr,
        _guard: Some(MockServerGuard {
            shutdown_tx,
            join_handle: Some(join_handle),
        }),
    }
}

fn write_http_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    write_http_response_with_headers(stream, status, content_type, &[], body);
}

fn write_http_response_with_headers(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    extra_headers: &[(&str, &str)],
    body: &[u8],
) {
    let mut response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (k, v) in extra_headers {
        response.push_str(&format!("{k}: {v}\r\n"));
    }
    response.push_str("\r\n");
    stream
        .write_all(response.as_bytes())
        .expect("write headers");
    stream.write_all(body).expect("write body");
    stream.flush().expect("flush");
}

fn write_chunked_response_start(
    stream: &mut TcpStream,
    content_type: &str,
    extra_headers: &[(&str, &str)],
) {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n",
    );
    for (k, v) in extra_headers {
        response.push_str(&format!("{k}: {v}\r\n"));
    }
    response.push_str("\r\n");
    stream
        .write_all(response.as_bytes())
        .expect("write headers");
}

fn write_chunk(stream: &mut TcpStream, data: &[u8]) {
    stream
        .write_all(format!("{:x}\r\n", data.len()).as_bytes())
        .expect("write chunk size");
    stream.write_all(data).expect("write chunk data");
    stream.write_all(b"\r\n").expect("write chunk CRLF");
    stream.flush().expect("flush");
}

fn write_chunked_end(stream: &mut TcpStream) {
    stream.write_all(b"0\r\n\r\n").expect("write end");
    stream.flush().expect("flush");
}

#[test]
fn http_media_source_reads_static_file_and_reports_metadata() {
    let body = b"fake flac data".to_vec();
    let body_for_handler = body.clone();
    let server = spawn_single_request_server(move |request| {
        assert!(
            request.to_lowercase().contains("icy-metadata: 1"),
            "expected Icy-MetaData header, got: {request:?}"
        );
        (
            "200 OK".to_string(),
            "audio/flac".to_string(),
            body_for_handler.clone(),
        )
    });

    let url = format!("http://{}/music.flac", server.addr);
    let (mut source, _rx) = HttpMediaSource::open(&url).expect("open");

    assert_eq!(source.content_type(), Some("audio/flac"));
    assert!(!source.is_seekable());
    assert_eq!(source.content_length(), Some(body.len() as u64));
    assert_eq!(source.format_hint(), Some("flac".to_string()));

    let mut read_back = Vec::new();
    source.read_to_end(&mut read_back).expect("read");
    assert_eq!(read_back, body);
}

#[test]
fn http_media_source_seeks_with_byte_range() {
    let full = b"0123456789".to_vec();
    let full_clone = full.clone();

    let server = spawn_keep_alive_server(move |request| {
        if request.to_lowercase().contains("range: bytes=4-") {
            (
                "206 Partial Content".to_string(),
                "audio/mpeg".to_string(),
                vec![],
                full_clone[4..].to_vec(),
            )
        } else {
            (
                "200 OK".to_string(),
                "audio/mpeg".to_string(),
                vec![("Accept-Ranges".to_string(), "bytes".to_string())],
                full_clone.clone(),
            )
        }
    });

    let url = format!("http://{}/song.mp3", server.addr);
    let (mut source, _rx) = HttpMediaSource::open(&url).expect("open");
    assert!(source.is_seekable());
    assert_eq!(source.content_length(), Some(10));

    use std::io::SeekFrom;
    let pos = source.seek(SeekFrom::Start(4)).expect("seek");
    assert_eq!(pos, 4);

    let mut buf = [0u8; 6];
    source.read_exact(&mut buf).expect("read after seek");
    assert_eq!(&buf, b"456789");
}

#[test]
fn http_media_source_reports_404_as_error() {
    let server = spawn_single_request_server(|_request| {
        (
            "404 Not Found".to_string(),
            "text/plain".to_string(),
            b"missing".to_vec(),
        )
    });

    let url = format!("http://{}/missing.flac", server.addr);
    let result = HttpMediaSource::open(&url);
    assert!(result.is_err());
}

#[test]
fn http_media_source_strips_icy_metadata() {
    // ICY metadata: every 8 audio bytes is followed by a length byte * 16 of metadata.
    let audio = b"abcdefgh".to_vec();
    let audio_for_thread = audio.clone();
    let meta = b"StreamTitle='Test';".to_vec();
    let meta_len_byte = (meta.len() as u8).div_ceil(16);

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        // Read the request before writing so the client does not see a
        // prematurely closed connection.
        let _request = read_http_request_bytes(&mut stream);
        write_chunked_response_start(
            &mut stream,
            "audio/mpeg",
            &[("icy-metaint", "8"), ("icy-br", "128")],
        );
        write_chunk(&mut stream, &audio_for_thread);
        write_chunk(&mut stream, &[meta_len_byte]);
        let mut block = vec![0u8; meta_len_byte as usize * 16];
        block[..meta.len()].copy_from_slice(&meta);
        write_chunk(&mut stream, &block);
        write_chunk(&mut stream, &audio_for_thread);
        write_chunked_end(&mut stream);
        // Keep the listener alive long enough for the client to finish reading
        // and for reqwest to see a clean EOF instead of a reset.
        thread::sleep(Duration::from_millis(200));
    });

    let url = format!("http://{}/stream.mp3", addr);
    let (mut source, rx) = HttpMediaSource::open(&url).expect("open");

    // Read both audio blocks; ICY metadata is stripped in between.
    let mut buf = [0u8; 8];
    source.read_exact(&mut buf).expect("read first audio");
    assert_eq!(&buf, &*audio);
    source.read_exact(&mut buf).expect("read second audio");
    assert_eq!(&buf, &*audio);

    // Metadata should have been delivered during the read above.
    let mut got_icy = false;
    while let Ok(meta) = rx.recv_timeout(Duration::from_millis(1000)) {
        if let StreamMetadata::Icy(icy) = meta {
            assert_eq!(icy.stream_title.as_deref(), Some("Test"));
            got_icy = true;
            break;
        }
    }
    assert!(got_icy, "expected ICY metadata on channel");
}

// ---------------------------------------------------------------------------
// PCM stream server
// ---------------------------------------------------------------------------

const PCM_SERVER_ATTEMPTS: usize = 5;

fn read_response(stream: &mut TcpStream, timeout: Duration) -> Vec<u8> {
    // On macOS a freshly connected socket can occasionally reject a timeout
    // set if the peer has already closed; ignore that and read what we can.
    let _ = stream.set_read_timeout(Some(Duration::from_millis(20)));
    let deadline = Instant::now() + timeout;
    let mut bytes = Vec::new();
    let mut buf = [0u8; 4096];
    while Instant::now() < deadline {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => bytes.extend_from_slice(&buf[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => {
                break;
            }
            Err(e) => panic!("read failed: {e}"),
        }
    }
    bytes
}

fn contains_bytes(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

fn read_until<F>(stream: &mut TcpStream, timeout: Duration, mut done: F) -> Vec<u8>
where
    F: FnMut(&[u8]) -> bool,
{
    let _ = stream.set_read_timeout(Some(Duration::from_millis(20)));
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
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => {
                break;
            }
            Err(e) => panic!("read failed: {e}"),
        }
    }
    bytes
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

fn start_pcm_server() -> PcmStreamServer {
    let server = PcmStreamServer::start(PcmStreamServerConfig {
        bind_addr: "127.0.0.1".to_string(),
        port: 0,
        ..Default::default()
    })
    .expect("start server");
    // Give the server thread time to enter its accept loop before tests connect.
    thread::sleep(Duration::from_millis(100));
    server
}

fn response_to_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn request_pcm_response(addr: SocketAddr, request: &[u8], expected_status: &str) -> Vec<u8> {
    let mut last_response = Vec::new();
    for _ in 0..PCM_SERVER_ATTEMPTS {
        let mut stream = TcpStream::connect(addr).expect("connect");
        stream.write_all(request).expect("write request");
        let bytes = read_response(&mut stream, Duration::from_secs(2));
        if response_to_string(&bytes).starts_with(expected_status) {
            return bytes;
        }
        last_response = bytes;
        thread::sleep(Duration::from_millis(20));
    }

    panic!(
        "expected {expected_status}, got: {:?}",
        response_to_string(&last_response)
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
    for _ in 0..PCM_SERVER_ATTEMPTS {
        let baseline_clients = handle.stats().client_count;
        let mut stream = TcpStream::connect(server.local_addr()).expect("connect");
        stream.write_all(request).expect("write request");
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
        response_to_string(&last_response)
    );
}

#[test]
fn pcm_stream_server_status_endpoint() {
    let mut server = start_pcm_server();

    let bytes = request_pcm_response(
        server.local_addr(),
        b"GET /status HTTP/1.1\r\nHost: localhost\r\n\r\n",
        "HTTP/1.1 200 OK",
    );
    let response = response_to_string(&bytes);
    assert!(response.contains("\"status\":\"ok\""));
    assert!(response.contains("\"sample_rate\":48000"));
    assert!(response.contains("\"channels\":2"));

    server.shutdown();
}

#[test]
fn pcm_stream_server_wav_stream_receives_audio() {
    let mut server = start_pcm_server();
    let handle = server.handle();

    let (mut stream, mut bytes) = open_registered_stream(
        &server,
        &handle,
        b"GET /stream.wav HTTP/1.1\r\nHost: localhost\r\n\r\n",
        |bytes| contains_bytes(bytes, b"RIFF") && contains_bytes(bytes, b"WAVE"),
    );

    let samples = vec![0.25f32, -0.25, 0.5, -0.5];
    assert!(handle.publish(&samples, 2, 2, 48_000));

    bytes.extend(read_response(&mut stream, Duration::from_millis(1000)));
    let response = response_to_string(&bytes);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("Content-Type: audio/wav"));
    assert!(bytes.windows(4).any(|w| w == b"RIFF"));
    assert!(bytes.windows(4).any(|w| w == b"WAVE"));

    let stats = handle.stats();
    assert_eq!(stats.published_chunks, 1);
    assert_eq!(stats.published_frames, 2);
    assert_eq!(stats.dropped_chunks, 0);

    server.shutdown();
}

#[test]
fn pcm_stream_server_raw_stream_receives_audio() {
    let mut server = start_pcm_server();
    let handle = server.handle();

    let (mut stream, mut bytes) = open_registered_stream(
        &server,
        &handle,
        b"GET /stream.raw HTTP/1.1\r\nHost: localhost\r\n\r\n",
        |bytes| contains_bytes(bytes, b"\r\n\r\n"),
    );

    let samples = vec![1.0f32, -1.0, 0.0, 0.5];
    assert!(handle.publish(&samples, 2, 2, 48_000));

    bytes.extend(read_response(&mut stream, Duration::from_millis(1000)));
    let response = response_to_string(&bytes);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("Content-Type: audio/x-f32le"));

    // f32 samples are little-endian interleaved in the chunked body.
    assert!(bytes.windows(4).any(|w| w == 1.0f32.to_le_bytes()));
    assert!(bytes.windows(4).any(|w| w == (-1.0f32).to_le_bytes()));

    server.shutdown();
}

#[test]
fn pcm_stream_server_head_request_returns_headers_only() {
    let mut server = start_pcm_server();

    for path in ["/stream.wav", "/stream.raw"] {
        let request = format!("HEAD {path} HTTP/1.1\r\nHost: localhost\r\n\r\n");
        let bytes =
            request_pcm_response(server.local_addr(), request.as_bytes(), "HTTP/1.1 200 OK");
        let response = response_to_string(&bytes);
        assert!(
            response.contains("Transfer-Encoding: chunked"),
            "HEAD {path} missing chunked header"
        );
        // Body must be empty for HEAD.
        assert_eq!(
            response.find("\r\n\r\n").map(|i| response.len() - i - 4),
            Some(0),
            "HEAD {path} had a body"
        );
    }

    server.shutdown();
}

#[test]
fn pcm_stream_server_rejects_invalid_publishes() {
    let mut server = start_pcm_server();
    let handle = server.handle();

    // 3 samples but 2 frames * 2 channels expects 4 samples.
    assert!(!handle.publish(&[0.0, 1.0, 2.0], 2, 2, 48_000));
    assert_eq!(handle.stats().dropped_chunks, 1);

    server.shutdown();
}

#[test]
fn pcm_stream_server_404_and_405_paths() {
    let mut server = start_pcm_server();

    request_pcm_response(
        server.local_addr(),
        b"GET /notfound HTTP/1.1\r\nHost: localhost\r\n\r\n",
        "HTTP/1.1 404 Not Found",
    );
    request_pcm_response(
        server.local_addr(),
        b"POST /status HTTP/1.1\r\nHost: localhost\r\n\r\n",
        "HTTP/1.1 405 Method Not Allowed",
    );

    server.shutdown();
}

#[test]
fn pcm_stream_server_format_change_disconnects_clients() {
    let mut server = start_pcm_server();
    let handle = server.handle();

    let (mut stream, mut bytes) = open_registered_stream(
        &server,
        &handle,
        b"GET /stream.raw HTTP/1.1\r\nHost: localhost\r\n\r\n",
        |bytes| contains_bytes(bytes, b"\r\n\r\n"),
    );

    // First chunk at 48 kHz / stereo.
    assert!(handle.publish(&[0.1f32, 0.2], 1, 2, 48_000));
    // Format change to 44.1 kHz mono should terminate the existing stream client.
    assert!(handle.publish(&[0.3f32], 1, 1, 44_100));

    bytes.extend(read_response(&mut stream, Duration::from_millis(1000)));
    let response = response_to_string(&bytes);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(bytes.windows(4).any(|w| w == 0.1f32.to_le_bytes()));

    // Stats should reflect both publishes.
    let stats = handle.stats();
    assert_eq!(stats.published_chunks, 2);

    server.shutdown();
}

#[test]
fn pcm_stream_server_drop_shuts_down_gracefully() {
    let server = start_pcm_server();
    let addr = server.local_addr();
    drop(server);

    // After dropping the server the listener should be closed.
    thread::sleep(Duration::from_millis(50));
    assert!(TcpStream::connect(addr).is_err());
}

// ---------------------------------------------------------------------------
// MPD streaming source
// ---------------------------------------------------------------------------

/// Spawn an MPD control mock and an httpd output mock, returning the URL to
/// pass to `MpdStreamSource::open` in the form
/// `mpd-stream://host:control_port:httpd_port/path`.
fn spawn_mpd_mocks(file_path: &str) -> String {
    // MPD control port
    let mpd_listener = TcpListener::bind("127.0.0.1:0").expect("bind mpd control");
    let mpd_port = mpd_listener.local_addr().unwrap().port();

    // httpd output port
    let httpd_listener = TcpListener::bind("127.0.0.1:0").expect("bind httpd");
    let httpd_port = httpd_listener.local_addr().unwrap().port();

    let file_path_for_thread = file_path.to_string();
    thread::spawn(move || {
        let (mut stream, _) = mpd_listener.accept().expect("mpd accept");
        stream
            .write_all(b"OK MPD 0.23.5\r\n")
            .expect("mpd greeting");
        stream.flush().expect("mpd flush");

        let mut writer = stream.try_clone().expect("clone stream");
        let mut reader = std::io::BufReader::new(&stream);
        let mut line = String::new();
        while reader.read_line(&mut line).expect("mpd read") > 0 {
            let trimmed = line.trim();
            if trimmed == "clear"
                || (trimmed.starts_with("add ") && trimmed.contains(&file_path_for_thread))
                || trimmed == "play"
            {
                writer.write_all(b"OK\r\n").expect("mpd ok");
                writer.flush().expect("mpd flush");
            }
            if trimmed == "play" {
                break;
            }
            line.clear();
        }
    });

    thread::spawn(move || {
        let (mut stream, _) = httpd_listener.accept().expect("httpd accept");
        // Read the GET request before responding so the client doesn't see a
        // prematurely closed connection.
        let _request = read_http_request_bytes(&mut stream);
        write_chunked_response_start(&mut stream, "audio/mpeg", &[]);
        write_chunk(&mut stream, b"mp3bytes");
        write_chunked_end(&mut stream);
    });

    let url = format!("mpd-stream://127.0.0.1:{mpd_port}:{httpd_port}/{file_path}");
    // Let the mock listeners start accepting before returning.
    thread::sleep(Duration::from_millis(50));
    url
}

#[test]
fn mpd_stream_source_open_with_mock_server() {
    let url = spawn_mpd_mocks("music/track.mp3");

    let (mut source, _rx) = MpdStreamSource::open(&url).expect("open MPD stream");
    assert_eq!(source.format_hint(), Some("mp3".to_string()));

    let mut buf = [0u8; 8];
    source.read_exact(&mut buf).expect("read mp3 bytes");
    assert_eq!(&buf, b"mp3bytes");
}

#[test]
fn mpd_stream_source_rejects_invalid_url() {
    let result = MpdStreamSource::open("http://example.com");
    assert!(result.is_err());
}

#[test]
fn mpd_stream_source_fails_when_mpd_is_unreachable() {
    // Use a port that is extremely unlikely to accept connections.
    let result = MpdStreamSource::open("mpd-stream://127.0.0.1:1:2/file.mp3");
    assert!(result.is_err());
    match result {
        Err(err) => assert!(err.contains("MPD connect failed") || err.contains("connect")),
        Ok(_) => panic!("expected connection failure"),
    }
}

// ---------------------------------------------------------------------------
// HLS streaming source (feature gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "hls")]
#[test]
fn hls_source_reads_segments_from_mock_server() {
    use sotf_streaming::HlsSource;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().unwrap();
    let request_count = Arc::new(AtomicUsize::new(0));
    let count_for_thread = Arc::clone(&request_count);

    thread::spawn(move || {
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_http_request_bytes(&mut stream);
            let request = String::from_utf8_lossy(&request);
            count_for_thread.fetch_add(1, Ordering::SeqCst);

            let (status, content_type, body) = if request.contains("GET /playlist.m3u8") {
                (
                    "200 OK",
                    "application/vnd.apple.mpegurl",
                    "#EXTM3U\n#EXT-X-TARGETDURATION:1\n#EXTINF:1,\nseg0.aac\n#EXTINF:1,\nseg1.aac\n#EXT-X-ENDLIST\n"
                        .as_bytes()
                        .to_vec(),
                )
            } else if request.contains("GET /seg0.aac") {
                ("200 OK", "audio/aac", b"hello".to_vec())
            } else if request.contains("GET /seg1.aac") {
                ("200 OK", "audio/aac", b"world".to_vec())
            } else {
                ("404 Not Found", "text/plain", b"missing".to_vec())
            };
            write_http_response(&mut stream, status, content_type, &body);
        }
    });

    let url = format!("http://{}/playlist.m3u8", addr);
    let mut source = HlsSource::open(&url).expect("open HLS source");
    assert_eq!(source.format_hint(), Some("aac".to_string()));

    let mut bytes = Vec::new();
    source.read_to_end(&mut bytes).expect("read segments");
    assert_eq!(bytes, b"helloworld");
    assert_eq!(request_count.load(Ordering::SeqCst), 3);
}

#[cfg(feature = "hls")]
#[test]
fn hls_source_rejects_invalid_url() {
    use sotf_streaming::HlsSource;
    assert!(HlsSource::open("not-a-url").is_err());
}

#[cfg(feature = "hls")]
#[test]
fn hls_source_media_source_is_non_seekable() {
    use sotf_streaming::HlsSource;

    // Verify the MediaSource trait reports HLS as non-seekable with unknown
    // length. This is a property of the type independent of any server.
    // The actual seek rejection is exercised by the full mock-server test above.
    let url = "http://127.0.0.1:1/playlist.m3u8";
    assert!(HlsSource::open(url).is_err());
}
