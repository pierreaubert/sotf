use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const DEFAULT_QUEUE_CAPACITY_CHUNKS: usize = 128;
const DEFAULT_CLIENT_QUEUE_CAPACITY_CHUNKS: usize = 32;
const ACCEPT_POLL_MS: u64 = 10;
const CLIENT_READ_TIMEOUT_MS: u64 = 2_000;
const MAX_HTTP_REQUEST_BYTES: usize = 16 * 1024;
const STREAM_DATA_SIZE: u32 = u32::MAX - 36;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PcmStreamFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

impl PcmStreamFormat {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PcmStreamServerConfig {
    pub bind_addr: String,
    pub port: u16,
    pub initial_sample_rate: u32,
    pub initial_channels: u16,
    pub queue_capacity_chunks: usize,
    pub client_queue_capacity_chunks: usize,
}

impl Default for PcmStreamServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0".to_string(),
            port: 17_890,
            initial_sample_rate: 48_000,
            initial_channels: 2,
            queue_capacity_chunks: DEFAULT_QUEUE_CAPACITY_CHUNKS,
            client_queue_capacity_chunks: DEFAULT_CLIENT_QUEUE_CAPACITY_CHUNKS,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PcmStreamChunk {
    pub samples: Vec<f32>,
    pub num_frames: usize,
    pub format: PcmStreamFormat,
}

impl PcmStreamChunk {
    pub fn new(
        samples: Vec<f32>,
        num_frames: usize,
        channels: u16,
        sample_rate: u32,
    ) -> Result<Self, String> {
        let expected = num_frames
            .checked_mul(channels as usize)
            .ok_or_else(|| "PCM stream chunk frame/channel count overflowed".to_string())?;
        if samples.len() != expected {
            return Err(format!(
                "PCM stream chunk has {} samples, expected {} ({} frames * {} channels)",
                samples.len(),
                expected,
                num_frames,
                channels
            ));
        }
        Ok(Self {
            samples,
            num_frames,
            format: PcmStreamFormat::new(sample_rate, channels),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcmStreamStats {
    pub local_addr: SocketAddr,
    pub client_count: u32,
    pub published_chunks: u64,
    pub dropped_chunks: u64,
    pub published_frames: u64,
    pub published_bytes: u64,
    pub current_format: PcmStreamFormat,
}

#[derive(Debug)]
struct SharedStats {
    local_addr: SocketAddr,
    client_count: AtomicU32,
    published_chunks: AtomicU64,
    dropped_chunks: AtomicU64,
    published_frames: AtomicU64,
    published_bytes: AtomicU64,
    sample_rate: AtomicU32,
    channels: AtomicU32,
}

impl SharedStats {
    fn new(local_addr: SocketAddr, format: PcmStreamFormat) -> Self {
        Self {
            local_addr,
            client_count: AtomicU32::new(0),
            published_chunks: AtomicU64::new(0),
            dropped_chunks: AtomicU64::new(0),
            published_frames: AtomicU64::new(0),
            published_bytes: AtomicU64::new(0),
            sample_rate: AtomicU32::new(format.sample_rate),
            channels: AtomicU32::new(u32::from(format.channels)),
        }
    }

    fn snapshot(&self) -> PcmStreamStats {
        PcmStreamStats {
            local_addr: self.local_addr,
            client_count: self.client_count.load(Ordering::Relaxed),
            published_chunks: self.published_chunks.load(Ordering::Relaxed),
            dropped_chunks: self.dropped_chunks.load(Ordering::Relaxed),
            published_frames: self.published_frames.load(Ordering::Relaxed),
            published_bytes: self.published_bytes.load(Ordering::Relaxed),
            current_format: PcmStreamFormat {
                sample_rate: self.sample_rate.load(Ordering::Relaxed),
                channels: self.channels.load(Ordering::Relaxed) as u16,
            },
        }
    }

    fn set_format(&self, format: PcmStreamFormat) {
        self.sample_rate
            .store(format.sample_rate, Ordering::Relaxed);
        self.channels
            .store(u32::from(format.channels), Ordering::Relaxed);
    }
}

#[derive(Clone)]
pub struct PcmStreamHandle {
    chunk_tx: SyncSender<PcmStreamChunk>,
    stats: Arc<SharedStats>,
}

impl PcmStreamHandle {
    pub fn publish(
        &self,
        samples: &[f32],
        num_frames: usize,
        channels: usize,
        sample_rate: u32,
    ) -> bool {
        let Ok(channels) = u16::try_from(channels) else {
            self.stats.dropped_chunks.fetch_add(1, Ordering::Relaxed);
            return false;
        };
        let chunk = match PcmStreamChunk::new(samples.to_vec(), num_frames, channels, sample_rate) {
            Ok(chunk) => chunk,
            Err(e) => {
                log::warn!("[PCM Stream] Dropping invalid chunk: {}", e);
                self.stats.dropped_chunks.fetch_add(1, Ordering::Relaxed);
                return false;
            }
        };

        match self.chunk_tx.try_send(chunk) {
            Ok(()) => {
                self.stats.published_chunks.fetch_add(1, Ordering::Relaxed);
                self.stats
                    .published_frames
                    .fetch_add(num_frames as u64, Ordering::Relaxed);
                self.stats.published_bytes.fetch_add(
                    (samples.len() * std::mem::size_of::<f32>()) as u64,
                    Ordering::Relaxed,
                );
                true
            }
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                self.stats.dropped_chunks.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.stats.local_addr
    }

    pub fn stats(&self) -> PcmStreamStats {
        self.stats.snapshot()
    }
}

pub struct PcmStreamServer {
    handle: PcmStreamHandle,
    shutdown_tx: Sender<()>,
    join_handle: Option<JoinHandle<()>>,
}

impl PcmStreamServer {
    pub fn start(config: PcmStreamServerConfig) -> io::Result<Self> {
        let listener = TcpListener::bind((config.bind_addr.as_str(), config.port))?;
        listener.set_nonblocking(true)?;
        let local_addr = listener.local_addr()?;
        let initial_format =
            PcmStreamFormat::new(config.initial_sample_rate, config.initial_channels.max(1));
        let queue_capacity = config.queue_capacity_chunks.max(1);
        let client_queue_capacity = config.client_queue_capacity_chunks.max(1);
        let (chunk_tx, chunk_rx) = mpsc::sync_channel(queue_capacity);
        let (client_tx, client_rx) = mpsc::channel();
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let stats = Arc::new(SharedStats::new(local_addr, initial_format));

        let thread_stats = Arc::clone(&stats);
        let join_handle = thread::Builder::new()
            .name("pcm-stream-server".to_string())
            .spawn(move || {
                run_server(
                    listener,
                    chunk_rx,
                    client_tx,
                    client_rx,
                    shutdown_rx,
                    thread_stats,
                    client_queue_capacity,
                );
            })?;

        Ok(Self {
            handle: PcmStreamHandle { chunk_tx, stats },
            shutdown_tx,
            join_handle: Some(join_handle),
        })
    }

    pub fn handle(&self) -> PcmStreamHandle {
        self.handle.clone()
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.handle.local_addr()
    }

    pub fn stats(&self) -> PcmStreamStats {
        self.handle.stats()
    }

    pub fn shutdown(&mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(handle) = self.join_handle.take() {
            if let Err(e) = handle.join() {
                log::warn!(
                    "[PCM Stream] Server thread panicked during shutdown: {:?}",
                    e
                );
            }
        }
    }
}

impl Drop for PcmStreamServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Debug)]
enum ClientMessage {
    Chunk(Arc<PcmStreamChunk>),
    FormatChanged,
}

#[derive(Debug)]
struct ClientRegistration {
    tx: SyncSender<ClientMessage>,
}

fn run_server(
    listener: TcpListener,
    chunk_rx: Receiver<PcmStreamChunk>,
    client_tx: Sender<ClientRegistration>,
    client_rx: Receiver<ClientRegistration>,
    shutdown_rx: Receiver<()>,
    stats: Arc<SharedStats>,
    client_queue_capacity: usize,
) {
    let mut clients: Vec<SyncSender<ClientMessage>> = Vec::new();
    let mut current_format = stats.snapshot().current_format;

    loop {
        if shutdown_rx.try_recv().is_ok() {
            break;
        }

        accept_pending_clients(
            &listener,
            &client_tx,
            Arc::clone(&stats),
            client_queue_capacity,
        );

        while let Ok(registration) = client_rx.try_recv() {
            clients.push(registration.tx);
            stats
                .client_count
                .store(clients.len() as u32, Ordering::Relaxed);
        }

        match chunk_rx.recv_timeout(Duration::from_millis(ACCEPT_POLL_MS)) {
            Ok(chunk) => {
                if chunk.format != current_format {
                    current_format = chunk.format;
                    stats.set_format(current_format);
                    notify_format_change(&mut clients);
                    stats.client_count.store(0, Ordering::Relaxed);
                }
                fanout_chunk(&mut clients, Arc::new(chunk));
                stats
                    .client_count
                    .store(clients.len() as u32, Ordering::Relaxed);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    notify_format_change(&mut clients);
    stats.client_count.store(0, Ordering::Relaxed);
}

fn accept_pending_clients(
    listener: &TcpListener,
    client_tx: &Sender<ClientRegistration>,
    stats: Arc<SharedStats>,
    client_queue_capacity: usize,
) {
    loop {
        match listener.accept() {
            Ok((stream, peer)) => {
                log::debug!("[PCM Stream] Connection from {}", peer);
                let tx = client_tx.clone();
                let stats = Arc::clone(&stats);
                let _ = thread::Builder::new()
                    .name("pcm-stream-client".to_string())
                    .spawn(move || {
                        handle_client(stream, tx, stats, client_queue_capacity);
                    });
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) => {
                log::warn!("[PCM Stream] Accept failed: {}", e);
                break;
            }
        }
    }
}

fn fanout_chunk(clients: &mut Vec<SyncSender<ClientMessage>>, chunk: Arc<PcmStreamChunk>) {
    clients.retain(
        |client| match client.try_send(ClientMessage::Chunk(Arc::clone(&chunk))) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => false,
        },
    );
}

fn notify_format_change(clients: &mut Vec<SyncSender<ClientMessage>>) {
    for client in clients.drain(..) {
        let _ = client.try_send(ClientMessage::FormatChanged);
    }
}

fn handle_client(
    mut stream: TcpStream,
    client_tx: Sender<ClientRegistration>,
    stats: Arc<SharedStats>,
    client_queue_capacity: usize,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(CLIENT_READ_TIMEOUT_MS)));
    let request = match read_http_request(&mut stream) {
        Ok(request) => request,
        Err(e) => {
            log::debug!("[PCM Stream] Failed to read request: {}", e);
            return;
        }
    };

    let Some((method, path)) = parse_request_line(&request) else {
        let _ = write_response(
            &mut stream,
            "400 Bad Request",
            "text/plain; charset=utf-8",
            b"bad request",
        );
        return;
    };

    if method != "GET" && method != "HEAD" {
        let _ = write_response(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            b"method not allowed",
        );
        return;
    }

    match path {
        "/" => {
            let body = index_json(&stats.snapshot());
            let _ = write_response(&mut stream, "200 OK", "application/json", body.as_bytes());
        }
        "/health" | "/status" => {
            let body = status_json(&stats.snapshot());
            let _ = write_response(&mut stream, "200 OK", "application/json", body.as_bytes());
        }
        "/stream.wav" => {
            if method == "HEAD" {
                let _ = write_stream_headers(&mut stream, "audio/wav", &stats.snapshot());
            } else {
                serve_stream(
                    stream,
                    client_tx,
                    stats,
                    client_queue_capacity,
                    StreamKind::Wav,
                );
            }
        }
        "/stream.raw" => {
            if method == "HEAD" {
                let _ = write_stream_headers(&mut stream, "audio/x-f32le", &stats.snapshot());
            } else {
                serve_stream(
                    stream,
                    client_tx,
                    stats,
                    client_queue_capacity,
                    StreamKind::RawF32,
                );
            }
        }
        _ => {
            let _ = write_response(
                &mut stream,
                "404 Not Found",
                "text/plain; charset=utf-8",
                b"not found",
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StreamKind {
    Wav,
    RawF32,
}

fn serve_stream(
    mut stream: TcpStream,
    client_tx: Sender<ClientRegistration>,
    stats: Arc<SharedStats>,
    client_queue_capacity: usize,
    kind: StreamKind,
) {
    let snapshot = stats.snapshot();
    let content_type = match kind {
        StreamKind::Wav => "audio/wav",
        StreamKind::RawF32 => "audio/x-f32le",
    };

    if write_stream_headers(&mut stream, content_type, &snapshot).is_err() {
        return;
    }

    if kind == StreamKind::Wav {
        let header = build_wav_stream_header_f32(snapshot.current_format);
        if write_http_chunk(&mut stream, &header).is_err() {
            return;
        }
    }

    let (tx, rx) = mpsc::sync_channel(client_queue_capacity);
    if client_tx.send(ClientRegistration { tx }).is_err() {
        let _ = write_http_chunk(&mut stream, &[]);
        return;
    }

    while let Ok(message) = rx.recv() {
        match message {
            ClientMessage::Chunk(chunk) => {
                if chunk.format != snapshot.current_format {
                    break;
                }
                if write_f32_chunk(&mut stream, &chunk.samples).is_err() {
                    break;
                }
            }
            ClientMessage::FormatChanged => break,
        }
    }

    let _ = write_http_chunk(&mut stream, &[]);
}

fn read_http_request(stream: &mut TcpStream) -> io::Result<String> {
    let mut request = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            break;
        }
        request.extend_from_slice(&buf[..n]);
        if request.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if request.len() > MAX_HTTP_REQUEST_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HTTP request too large",
            ));
        }
    }

    Ok(String::from_utf8_lossy(&request).into_owned())
}

fn parse_request_line(request: &str) -> Option<(&str, &str)> {
    let line = request.lines().next()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;
    Some((method, path))
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> io::Result<()> {
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(headers.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn write_stream_headers(
    stream: &mut TcpStream,
    content_type: &str,
    stats: &PcmStreamStats,
) -> io::Result<()> {
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nTransfer-Encoding: chunked\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nX-SOTF-Sample-Rate: {}\r\nX-SOTF-Channels: {}\r\nConnection: close\r\n\r\n",
        stats.current_format.sample_rate, stats.current_format.channels
    );
    stream.write_all(headers.as_bytes())?;
    stream.flush()
}

fn write_http_chunk(stream: &mut TcpStream, data: &[u8]) -> io::Result<()> {
    let header = format!("{:x}\r\n", data.len());
    stream.write_all(header.as_bytes())?;
    stream.write_all(data)?;
    stream.write_all(b"\r\n")?;
    stream.flush()
}

fn write_f32_chunk(stream: &mut TcpStream, samples: &[f32]) -> io::Result<()> {
    let mut bytes = Vec::with_capacity(samples.len() * std::mem::size_of::<f32>());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    write_http_chunk(stream, &bytes)
}

fn build_wav_stream_header_f32(format: PcmStreamFormat) -> [u8; 44] {
    let bits_per_sample: u16 = 32;
    let block_align = format.channels * (bits_per_sample / 8);
    let byte_rate = format.sample_rate * u32::from(block_align);
    let riff_size = STREAM_DATA_SIZE.saturating_add(36);

    let mut header = [0u8; 44];
    header[0..4].copy_from_slice(b"RIFF");
    header[4..8].copy_from_slice(&riff_size.to_le_bytes());
    header[8..12].copy_from_slice(b"WAVE");
    header[12..16].copy_from_slice(b"fmt ");
    header[16..20].copy_from_slice(&16u32.to_le_bytes());
    header[20..22].copy_from_slice(&3u16.to_le_bytes()); // IEEE float
    header[22..24].copy_from_slice(&format.channels.to_le_bytes());
    header[24..28].copy_from_slice(&format.sample_rate.to_le_bytes());
    header[28..32].copy_from_slice(&byte_rate.to_le_bytes());
    header[32..34].copy_from_slice(&block_align.to_le_bytes());
    header[34..36].copy_from_slice(&bits_per_sample.to_le_bytes());
    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&STREAM_DATA_SIZE.to_le_bytes());
    header
}

fn index_json(stats: &PcmStreamStats) -> String {
    format!(
        "{{\"service\":\"sotf-pcm-stream\",\"status\":\"ok\",\"stream_wav\":\"http://{}/stream.wav\",\"stream_raw\":\"http://{}/stream.raw\",\"sample_rate\":{},\"channels\":{}}}",
        stats.local_addr,
        stats.local_addr,
        stats.current_format.sample_rate,
        stats.current_format.channels
    )
}

fn status_json(stats: &PcmStreamStats) -> String {
    format!(
        "{{\"status\":\"ok\",\"bind_addr\":\"{}\",\"clients\":{},\"sample_rate\":{},\"channels\":{},\"published_chunks\":{},\"dropped_chunks\":{},\"published_frames\":{},\"published_bytes\":{}}}",
        stats.local_addr,
        stats.client_count,
        stats.current_format.sample_rate,
        stats.current_format.channels,
        stats.published_chunks,
        stats.dropped_chunks,
        stats.published_frames,
        stats.published_bytes
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn read_response(mut stream: &TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_millis(500)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => bytes.extend_from_slice(&buf[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    break;
                }
                Err(e) => panic!("read failed: {e}"),
            }
        }
        String::from_utf8_lossy(&bytes).into_owned()
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
        let Some(mut server) = start_test_server() else {
            return;
        };

        let mut stream = TcpStream::connect(server.local_addr()).unwrap();
        stream
            .write_all(b"GET /status HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let response = read_response(&stream);
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("\"status\":\"ok\""));
        assert!(response.contains("\"sample_rate\":48000"));
        assert!(response.contains("\"channels\":2"));

        server.shutdown();
    }

    #[test]
    fn wav_stream_receives_header_and_published_audio() {
        let Some(mut server) = start_test_server() else {
            return;
        };
        let handle = server.handle();

        let mut stream = TcpStream::connect(server.local_addr()).unwrap();
        stream
            .write_all(b"GET /stream.wav HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();

        thread::sleep(Duration::from_millis(50));
        assert!(handle.publish(&[0.25, -0.25, 0.5, -0.5], 2, 2, 48_000));

        let response = read_response(&stream);
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Content-Type: audio/wav"));
        assert!(response.as_bytes().windows(4).any(|w| w == b"RIFF"));
        assert!(response.as_bytes().windows(4).any(|w| w == b"WAVE"));
        assert!(handle.stats().published_chunks >= 1);

        server.shutdown();
    }

    #[test]
    fn publish_rejects_invalid_shape() {
        let Some(mut server) = start_test_server() else {
            return;
        };
        let handle = server.handle();

        assert!(!handle.publish(&[0.0, 1.0, 2.0], 2, 2, 48_000));
        assert_eq!(handle.stats().dropped_chunks, 1);

        server.shutdown();
    }
}
