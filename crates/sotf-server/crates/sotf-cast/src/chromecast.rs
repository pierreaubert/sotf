// ============================================================================
// Chromecast Audio Sender (CASTV2)
// ============================================================================
//
// Streams audio to Chromecast devices by:
// 1. Discovering the device via mDNS (_googlecast._tcp)
// 2. Opening the CASTV2 TLS channel on port 8009
// 3. Launching the Default Media Receiver
// 4. Starting a local HTTP WAV stream
// 5. Sending a CASTV2 LOAD message pointing the device at that stream

use crate::discovery::CastDevice;
use rustls::pki_types::ServerName;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const DEFAULT_MEDIA_RECEIVER: &str = "CC1AD845";
const NS_CONNECTION: &str = "urn:x-cast:com.google.cast.tp.connection";
const NS_RECEIVER: &str = "urn:x-cast:com.google.cast.receiver";
const NS_MEDIA: &str = "urn:x-cast:com.google.cast.media";
const SOURCE_ID: &str = "sender-0";
const RECEIVER_ID: &str = "receiver-0";
const CAST_MIME_TYPE: &str = "audio/wav";
const CAST_STREAM_PATH: &str = "/stream.wav";
const CAST_READ_LIMIT: usize = 12;

type CastTlsStream = rustls::StreamOwned<rustls::ClientConnection, TcpStream>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromecastState {
    Disconnected,
    Connecting,
    Connected,
    Launching,
    Streaming,
}

pub struct ChromecastSender {
    device: CastDevice,
    state: ChromecastState,
    local_ip: Option<Ipv4Addr>,
    http_port: Option<u16>,
    transport_id: Option<String>,
    media_session_id: Option<u32>,
    request_id: u32,
    streaming: AtomicBool,
    volume: f32,
    audio_buffer: Arc<std::sync::Mutex<AudioRingBuffer>>,
    http_stop: Arc<AtomicBool>,
    cast_stream: Option<CastTlsStream>,
    tofu_config_dir: PathBuf,
}

struct AudioRingBuffer {
    data: Vec<u8>,
    write_pos: usize,
    read_pos: usize,
    sample_rate: u32,
    channels: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CastMessage {
    source_id: String,
    destination_id: String,
    namespace: String,
    payload_utf8: String,
}

impl AudioRingBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            data: vec![0u8; capacity],
            write_pos: 0,
            read_pos: 0,
            sample_rate: 44_100,
            channels: 2,
        }
    }

    #[cfg(test)]
    fn available(&self) -> usize {
        if self.write_pos >= self.read_pos {
            self.write_pos - self.read_pos
        } else {
            self.data.len() - self.read_pos + self.write_pos
        }
    }

    fn write(&mut self, bytes: &[u8]) -> usize {
        let cap = self.data.len();
        let mut written = 0;
        for &b in bytes {
            let next = (self.write_pos + 1) % cap;
            if next == self.read_pos {
                break;
            }
            self.data[self.write_pos] = b;
            self.write_pos = next;
            written += 1;
        }
        written
    }

    fn read(&mut self, buf: &mut [u8]) -> usize {
        let mut read = 0;
        while read < buf.len() && self.read_pos != self.write_pos {
            buf[read] = self.data[self.read_pos];
            self.read_pos = (self.read_pos + 1) % self.data.len();
            read += 1;
        }
        read
    }
}

impl ChromecastSender {
    pub fn new(device: CastDevice) -> Self {
        let buffer_capacity = 44_100 * 2 * 2 * 2;
        Self {
            device,
            state: ChromecastState::Disconnected,
            local_ip: None,
            http_port: None,
            transport_id: None,
            media_session_id: None,
            request_id: 0,
            streaming: AtomicBool::new(false),
            volume: 1.0,
            audio_buffer: Arc::new(std::sync::Mutex::new(AudioRingBuffer::new(buffer_capacity))),
            http_stop: Arc::new(AtomicBool::new(false)),
            cast_stream: None,
            tofu_config_dir: default_tofu_config_dir(),
        }
    }

    /// Connect to Chromecast over CASTV2 TLS and launch the default media receiver.
    pub fn connect(&mut self, local_ip: Ipv4Addr) -> Result<(), String> {
        self.state = ChromecastState::Connecting;
        self.local_ip = Some(local_ip);
        self.start_http_stream_server()?;

        let mut stream = match self.connect_cast_tls() {
            Ok(stream) => stream,
            Err(err) => {
                self.disconnect();
                return Err(err);
            }
        };

        send_cast_json(
            &mut stream,
            SOURCE_ID,
            RECEIVER_ID,
            NS_CONNECTION,
            json!({"type": "CONNECT"}),
        )?;

        self.state = ChromecastState::Launching;
        let launch_request_id = self.next_request_id();
        send_cast_json(
            &mut stream,
            SOURCE_ID,
            RECEIVER_ID,
            NS_RECEIVER,
            json!({
                "type": "LAUNCH",
                "appId": DEFAULT_MEDIA_RECEIVER,
                "requestId": launch_request_id,
            }),
        )?;

        let transport_id = read_transport_id(&mut stream)?;
        send_cast_json(
            &mut stream,
            SOURCE_ID,
            &transport_id,
            NS_CONNECTION,
            json!({"type": "CONNECT"}),
        )?;

        self.transport_id = Some(transport_id);
        self.cast_stream = Some(stream);
        self.state = ChromecastState::Connected;
        log::info!(
            "[Chromecast] Connected {} at {}:{}",
            self.device.name,
            self.device.address,
            self.device.port,
        );
        Ok(())
    }

    /// Start the local WAV stream and send a CASTV2 LOAD for it.
    pub fn start_stream(&mut self, sample_rate: u32, channels: u16) -> Result<(), String> {
        let local_ip = self.local_ip.ok_or("Not connected")?;
        let http_port = self.http_port.ok_or("HTTP server not started")?;
        let transport_id = self
            .transport_id
            .clone()
            .ok_or("Media receiver not launched")?;

        {
            let mut buf = self.audio_buffer.lock().unwrap();
            buf.sample_rate = sample_rate;
            buf.channels = channels;
        }

        let stream_url = format!("http://{local_ip}:{http_port}{CAST_STREAM_PATH}");
        let request_id = self.next_request_id();
        let stream = self
            .cast_stream
            .as_mut()
            .ok_or("CASTV2 channel not connected")?;
        send_cast_json(
            stream,
            SOURCE_ID,
            &transport_id,
            NS_MEDIA,
            build_load_payload(request_id, &stream_url, sample_rate, channels),
        )?;
        self.media_session_id = read_media_session_id(stream).ok();
        self.state = ChromecastState::Streaming;
        self.streaming.store(true, Ordering::Release);
        log::info!("[Chromecast] LOAD {}", stream_url);
        Ok(())
    }

    /// Write interleaved f32 audio samples to the HTTP stream buffer.
    pub fn write_audio(&mut self, samples: &[f32]) -> Result<usize, String> {
        if !self.streaming.load(Ordering::Acquire) {
            return Err("Not streaming".to_string());
        }

        let mut pcm_bytes = Vec::with_capacity(samples.len() * 2);
        for &sample in samples {
            let clamped = (sample * self.volume).clamp(-1.0, 1.0);
            let pcm16 = (clamped * 32767.0) as i16;
            pcm_bytes.extend_from_slice(&pcm16.to_le_bytes());
        }

        let mut buf = self.audio_buffer.lock().unwrap();
        let written = buf.write(&pcm_bytes);
        Ok(written / 2)
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    pub fn disconnect(&mut self) {
        self.streaming.store(false, Ordering::Release);
        self.http_stop.store(true, Ordering::Release);
        self.cast_stream = None;
        self.state = ChromecastState::Disconnected;
        self.transport_id = None;
        self.media_session_id = None;
        log::info!("[Chromecast] Disconnected from {}", self.device.name);
    }

    pub fn state(&self) -> ChromecastState {
        self.state
    }

    pub fn device(&self) -> &CastDevice {
        &self.device
    }

    fn start_http_stream_server(&mut self) -> Result<(), String> {
        self.http_stop.store(false, Ordering::Release);
        let http_listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
            .map_err(|e| format!("HTTP bind failed: {e}"))?;
        let http_port = http_listener
            .local_addr()
            .map_err(|e| format!("HTTP local addr failed: {e}"))?
            .port();
        self.http_port = Some(http_port);

        let buffer = Arc::clone(&self.audio_buffer);
        let stop = Arc::clone(&self.http_stop);
        std::thread::Builder::new()
            .name("chromecast-http".to_string())
            .spawn(move || run_http_stream_server(http_listener, buffer, stop))
            .map_err(|e| format!("HTTP thread spawn failed: {e}"))?;

        Ok(())
    }

    fn connect_cast_tls(&self) -> Result<CastTlsStream, String> {
        match connect_cast_tls_once(&self.device, &self.tofu_config_dir) {
            Ok(stream) => Ok(stream),
            Err(err) => {
                let Some(result) = parse_embedded_tofu_error(&err) else {
                    return Err(err);
                };
                let sotf_tls::TofuResult::Unknown { fingerprint } = result else {
                    return Err(err);
                };
                let mut store = sotf_tls::TofuStore::load(&self.tofu_config_dir)?;
                store.accept(
                    &format!("{}:{}", self.device.address, self.device.port),
                    &fingerprint,
                    &self.device.name,
                )?;
                connect_cast_tls_once(&self.device, &self.tofu_config_dir)
            }
        }
    }

    fn next_request_id(&mut self) -> u32 {
        self.request_id += 1;
        self.request_id
    }

    #[cfg(test)]
    fn with_tofu_config_dir(mut self, dir: PathBuf) -> Self {
        self.tofu_config_dir = dir;
        self
    }
}

fn parse_embedded_tofu_error(err: &str) -> Option<sotf_tls::TofuResult> {
    if let Some(result) = sotf_tls::client::parse_tofu_error(err) {
        return Some(result);
    }
    for prefix in [
        sotf_tls::client::TOFU_UNKNOWN_PREFIX,
        sotf_tls::client::TOFU_CHANGED_PREFIX,
    ] {
        if let Some(offset) = err.find(prefix) {
            let embedded = err[offset..]
                .split_whitespace()
                .next()
                .unwrap_or(&err[offset..])
                .trim_matches(|ch| ch == '"' || ch == '\'');
            return sotf_tls::client::parse_tofu_error(embedded);
        }
    }
    None
}

impl Drop for ChromecastSender {
    fn drop(&mut self) {
        self.disconnect();
    }
}

fn connect_cast_tls_once(
    device: &CastDevice,
    tofu_config_dir: &Path,
) -> Result<CastTlsStream, String> {
    let addr = std::net::SocketAddr::V4(SocketAddrV4::new(device.address, device.port));
    let tcp = TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(5))
        .map_err(|e| format!("CASTV2 TCP connect failed: {e}"))?;
    tcp.set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .ok();
    tcp.set_write_timeout(Some(std::time::Duration::from_secs(5)))
        .ok();

    let store = sotf_tls::TofuStore::load(tofu_config_dir)?;
    let verifier = Arc::new(sotf_tls::client::TofuVerifier::with_port(
        Arc::new(std::sync::Mutex::new(store)),
        device.port,
    ));
    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    let config = Arc::new(config);
    let server_name = ServerName::IpAddress(device.address.into());
    let conn = rustls::ClientConnection::new(config, server_name)
        .map_err(|e| format!("CASTV2 TLS client failed: {e}"))?;
    let mut stream = rustls::StreamOwned::new(conn, tcp);
    while stream.conn.is_handshaking() {
        stream
            .conn
            .complete_io(&mut stream.sock)
            .map_err(|e| format!("CASTV2 TLS handshake failed: {e}"))?;
    }
    Ok(stream)
}

fn default_tofu_config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SOTF_CONFIG_DIR") {
        return PathBuf::from(dir);
    }

    #[cfg(target_os = "macos")]
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("org.spinorama.sotf");
    }

    #[cfg(target_os = "windows")]
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(local_app_data).join("sotf");
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config").join("sotf");
    }

    std::env::temp_dir().join("sotf")
}

fn build_load_payload(request_id: u32, stream_url: &str, sample_rate: u32, channels: u16) -> Value {
    json!({
        "type": "LOAD",
        "requestId": request_id,
        "media": {
            "contentId": stream_url,
            "streamType": "LIVE",
            "contentType": CAST_MIME_TYPE,
            "metadata": {
                "metadataType": 0,
                "title": format!("SOTF {sample_rate} Hz {channels} ch"),
            },
        },
        "autoplay": true,
        "currentTime": 0,
    })
}

fn send_cast_json(
    stream: &mut CastTlsStream,
    source_id: &str,
    destination_id: &str,
    namespace: &str,
    payload: Value,
) -> Result<(), String> {
    let message = CastMessage {
        source_id: source_id.to_string(),
        destination_id: destination_id.to_string(),
        namespace: namespace.to_string(),
        payload_utf8: payload.to_string(),
    };
    let encoded = encode_cast_message(&message);
    let len = u32::try_from(encoded.len()).map_err(|_| "CASTV2 message too large")?;
    stream
        .write_all(&len.to_be_bytes())
        .and_then(|()| stream.write_all(&encoded))
        .map_err(|e| format!("CASTV2 write failed: {e}"))
}

fn read_transport_id(stream: &mut CastTlsStream) -> Result<String, String> {
    for _ in 0..CAST_READ_LIMIT {
        let message = read_cast_message(stream)?;
        if message.namespace != NS_RECEIVER {
            continue;
        }
        let Ok(payload) = serde_json::from_str::<Value>(&message.payload_utf8) else {
            continue;
        };
        if let Some(transport_id) = payload
            .pointer("/status/applications/0/transportId")
            .and_then(Value::as_str)
        {
            return Ok(transport_id.to_string());
        }
    }
    Err("CASTV2 receiver launch did not return transportId".to_string())
}

fn read_media_session_id(stream: &mut CastTlsStream) -> Result<u32, String> {
    for _ in 0..CAST_READ_LIMIT {
        let message = read_cast_message(stream)?;
        if message.namespace != NS_MEDIA {
            continue;
        }
        let payload = serde_json::from_str::<Value>(&message.payload_utf8)
            .map_err(|e| format!("CASTV2 media status JSON failed: {e}"))?;
        if let Some(id) = payload
            .pointer("/status/0/mediaSessionId")
            .and_then(Value::as_u64)
        {
            return u32::try_from(id).map_err(|_| "CASTV2 mediaSessionId overflow".to_string());
        }
        if payload.get("type").and_then(Value::as_str) == Some("LOAD_FAILED") {
            return Err(format!("CASTV2 LOAD failed: {payload}"));
        }
    }
    Err("CASTV2 LOAD did not return media status".to_string())
}

fn read_cast_message(stream: &mut CastTlsStream) -> Result<CastMessage, String> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .map_err(|e| format!("CASTV2 length read failed: {e}"))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 1024 * 1024 {
        return Err(format!("CASTV2 frame too large: {len}"));
    }
    let mut payload = vec![0u8; len];
    stream
        .read_exact(&mut payload)
        .map_err(|e| format!("CASTV2 frame read failed: {e}"))?;
    decode_cast_message(&payload)
}

fn encode_cast_message(message: &CastMessage) -> Vec<u8> {
    let mut out = Vec::new();
    encode_varint_field(&mut out, 1, 0);
    encode_string_field(&mut out, 2, &message.source_id);
    encode_string_field(&mut out, 3, &message.destination_id);
    encode_string_field(&mut out, 4, &message.namespace);
    encode_varint_field(&mut out, 5, 0);
    encode_string_field(&mut out, 6, &message.payload_utf8);
    out
}

fn decode_cast_message(input: &[u8]) -> Result<CastMessage, String> {
    let mut pos = 0;
    let mut source_id = String::new();
    let mut destination_id = String::new();
    let mut namespace = String::new();
    let mut payload_utf8 = String::new();

    while pos < input.len() {
        let key = read_varint(input, &mut pos)?;
        let field = key >> 3;
        let wire_type = key & 0x07;
        match (field, wire_type) {
            (2, 2) => source_id = read_string(input, &mut pos)?,
            (3, 2) => destination_id = read_string(input, &mut pos)?,
            (4, 2) => namespace = read_string(input, &mut pos)?,
            (6, 2) => payload_utf8 = read_string(input, &mut pos)?,
            (_, 0) => {
                let _ = read_varint(input, &mut pos)?;
            }
            (_, 2) => {
                let len = read_varint(input, &mut pos)? as usize;
                pos = pos
                    .checked_add(len)
                    .ok_or_else(|| "CASTV2 protobuf length overflow".to_string())?;
                if pos > input.len() {
                    return Err("CASTV2 protobuf length out of bounds".to_string());
                }
            }
            _ => return Err(format!("CASTV2 unsupported protobuf wire type {wire_type}")),
        }
    }

    Ok(CastMessage {
        source_id,
        destination_id,
        namespace,
        payload_utf8,
    })
}

fn encode_string_field(out: &mut Vec<u8>, field_number: u64, value: &str) {
    write_varint(out, (field_number << 3) | 2);
    write_varint(out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
}

fn encode_varint_field(out: &mut Vec<u8>, field_number: u64, value: u64) {
    write_varint(out, field_number << 3);
    write_varint(out, value);
}

fn write_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn read_varint(input: &[u8], pos: &mut usize) -> Result<u64, String> {
    let mut value = 0u64;
    let mut shift = 0u32;
    loop {
        let byte = *input
            .get(*pos)
            .ok_or_else(|| "CASTV2 protobuf truncated varint".to_string())?;
        *pos += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift >= 64 {
            return Err("CASTV2 protobuf varint overflow".to_string());
        }
    }
}

fn read_string(input: &[u8], pos: &mut usize) -> Result<String, String> {
    let len = read_varint(input, pos)? as usize;
    let end = pos
        .checked_add(len)
        .ok_or_else(|| "CASTV2 protobuf string length overflow".to_string())?;
    let bytes = input
        .get(*pos..end)
        .ok_or_else(|| "CASTV2 protobuf truncated string".to_string())?;
    *pos = end;
    String::from_utf8(bytes.to_vec()).map_err(|e| format!("CASTV2 protobuf utf8 failed: {e}"))
}

fn run_http_stream_server(
    listener: TcpListener,
    buffer: Arc<std::sync::Mutex<AudioRingBuffer>>,
    stop: Arc<AtomicBool>,
) {
    listener.set_nonblocking(true).ok();
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, peer)) => {
                log::debug!("[Chromecast HTTP] connection from {peer}");
                let buf = Arc::clone(&buffer);
                let stop = Arc::clone(&stop);
                let _ = std::thread::Builder::new()
                    .name("chromecast-http-client".to_string())
                    .spawn(move || handle_stream_request(stream, buf, stop));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                log::warn!("[Chromecast HTTP] accept failed: {e}");
                break;
            }
        }
    }
}

fn handle_stream_request(
    mut stream: TcpStream,
    buffer: Arc<std::sync::Mutex<AudioRingBuffer>>,
    stop: Arc<AtomicBool>,
) {
    let mut request_buf = [0u8; 1024];
    let _ = stream.read(&mut request_buf);
    let (sample_rate, channels) = {
        let buf = buffer.lock().unwrap();
        (buf.sample_rate, buf.channels)
    };
    let wav_header = build_wav_header(sample_rate, channels, u32::MAX - 36);
    let response = "HTTP/1.1 200 OK\r\n\
Content-Type: audio/wav\r\n\
Transfer-Encoding: chunked\r\n\
Connection: keep-alive\r\n\
Access-Control-Allow-Origin: *\r\n\
\r\n";

    if stream.write_all(response.as_bytes()).is_err() {
        return;
    }
    if send_chunk(&mut stream, &wav_header).is_err() {
        return;
    }

    let mut read_buf = [0u8; 4096];
    while !stop.load(Ordering::Acquire) {
        let n = {
            let mut buf = buffer.lock().unwrap();
            buf.read(&mut read_buf)
        };
        if n > 0 {
            if send_chunk(&mut stream, &read_buf[..n]).is_err() {
                break;
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

fn send_chunk(stream: &mut TcpStream, data: &[u8]) -> std::io::Result<()> {
    write!(stream, "{:x}\r\n", data.len())?;
    stream.write_all(data)?;
    stream.write_all(b"\r\n")?;
    stream.flush()
}

fn build_wav_header(sample_rate: u32, channels: u16, data_size: u32) -> Vec<u8> {
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample) / 8;
    let block_align = channels * bits_per_sample / 8;
    let file_size = 36 + data_size;

    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&file_size.to_le_bytes());
    header.extend_from_slice(b"WAVE");
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes());
    header.extend_from_slice(&channels.to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&byte_rate.to_le_bytes());
    header.extend_from_slice(&block_align.to_le_bytes());
    header.extend_from_slice(&bits_per_sample.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_size.to_le_bytes());
    header
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::sync::mpsc;
    use std::time::Duration;

    fn test_device() -> CastDevice {
        CastDevice {
            device_type: crate::CastDeviceType::Chromecast,
            name: "Living Room".to_string(),
            address: Ipv4Addr::new(192, 168, 1, 100),
            port: 8009,
            instance_name: String::new(),
            txt_records: HashMap::new(),
        }
    }

    fn local_test_device(port: u16) -> CastDevice {
        CastDevice {
            device_type: crate::CastDeviceType::Chromecast,
            name: "Loopback Chromecast".to_string(),
            address: Ipv4Addr::LOCALHOST,
            port,
            instance_name: String::new(),
            txt_records: HashMap::new(),
        }
    }

    fn read_cast_message_from<R: Read>(stream: &mut R) -> Result<CastMessage, String> {
        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .map_err(|e| format!("test CASTV2 length read failed: {e}"))?;
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        stream
            .read_exact(&mut payload)
            .map_err(|e| format!("test CASTV2 frame read failed: {e}"))?;
        decode_cast_message(&payload)
    }

    fn write_cast_json_to<W: Write>(
        stream: &mut W,
        source_id: &str,
        destination_id: &str,
        namespace: &str,
        payload: Value,
    ) -> Result<(), String> {
        let message = CastMessage {
            source_id: source_id.to_string(),
            destination_id: destination_id.to_string(),
            namespace: namespace.to_string(),
            payload_utf8: payload.to_string(),
        };
        let encoded = encode_cast_message(&message);
        let len = u32::try_from(encoded.len()).map_err(|_| "test CASTV2 message too large")?;
        stream
            .write_all(&len.to_be_bytes())
            .and_then(|()| stream.write_all(&encoded))
            .map_err(|e| format!("test CASTV2 write failed: {e}"))
    }

    fn server_tls_stream(
        tcp: TcpStream,
        config: Arc<rustls::ServerConfig>,
    ) -> Option<rustls::StreamOwned<rustls::ServerConnection, TcpStream>> {
        let conn = rustls::ServerConnection::new(config).ok()?;
        let mut stream = rustls::StreamOwned::new(conn, tcp);
        while stream.conn.is_handshaking() {
            if stream.conn.complete_io(&mut stream.sock).is_err() {
                return None;
            }
        }
        Some(stream)
    }

    fn spawn_fake_chromecast_receiver() -> std::io::Result<(
        u16,
        mpsc::Receiver<String>,
        std::thread::JoinHandle<Result<(), String>>,
    )> {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
            let cert_store = sotf_tls::CertStore::load_or_generate(tmp.path())?;
            let tls_config =
                sotf_tls::build_server_tls_config(cert_store.cert_clone(), cert_store.key_clone())?;

            for _ in 0..2 {
                let (tcp, _) = listener.accept().map_err(|e| e.to_string())?;
                tcp.set_read_timeout(Some(Duration::from_secs(3))).ok();
                tcp.set_write_timeout(Some(Duration::from_secs(3))).ok();
                let Some(mut stream) = server_tls_stream(tcp, Arc::clone(&tls_config)) else {
                    continue;
                };

                let _connect = read_cast_message_from(&mut stream)?;
                let launch = read_cast_message_from(&mut stream)?;
                let launch_payload: Value =
                    serde_json::from_str(&launch.payload_utf8).map_err(|e| e.to_string())?;
                assert_eq!(launch_payload["type"], "LAUNCH");
                write_cast_json_to(
                    &mut stream,
                    RECEIVER_ID,
                    SOURCE_ID,
                    NS_RECEIVER,
                    json!({
                        "type": "RECEIVER_STATUS",
                        "status": {
                            "applications": [{
                                "transportId": "transport-1",
                                "appId": DEFAULT_MEDIA_RECEIVER
                            }]
                        }
                    }),
                )?;

                let _transport_connect = read_cast_message_from(&mut stream)?;
                let load = read_cast_message_from(&mut stream)?;
                let load_payload: Value =
                    serde_json::from_str(&load.payload_utf8).map_err(|e| e.to_string())?;
                assert_eq!(load_payload["type"], "LOAD");
                let content_id = load_payload["media"]["contentId"]
                    .as_str()
                    .ok_or_else(|| "missing LOAD contentId".to_string())?
                    .to_string();
                tx.send(content_id).map_err(|e| e.to_string())?;
                write_cast_json_to(
                    &mut stream,
                    RECEIVER_ID,
                    SOURCE_ID,
                    NS_MEDIA,
                    json!({
                        "type": "MEDIA_STATUS",
                        "status": [{
                            "mediaSessionId": 42,
                            "playerState": "PLAYING"
                        }]
                    }),
                )?;
                return Ok(());
            }

            Err("fake Chromecast receiver did not complete TLS session".to_string())
        });
        Ok((port, rx, handle))
    }

    #[test]
    fn test_chromecast_sender_initial_state() {
        let sender = ChromecastSender::new(test_device());
        assert_eq!(sender.state(), ChromecastState::Disconnected);
        assert_eq!(sender.device().name, "Living Room");
    }

    #[test]
    fn test_chromecast_uses_persistent_tofu_store() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let sender = ChromecastSender::new(test_device()).with_tofu_config_dir(tmp.path().into());
        assert_eq!(sender.tofu_config_dir, tmp.path());
    }

    #[test]
    fn test_embedded_tofu_error_is_recognized() {
        let result = parse_embedded_tofu_error(
            "CASTV2 TLS handshake failed: invalid peer certificate: TOFU_UNKNOWN:AA:BB",
        );
        assert_eq!(
            result,
            Some(sotf_tls::TofuResult::Unknown {
                fingerprint: "AA:BB".to_string(),
            })
        );
    }

    #[test]
    fn test_cast_message_round_trip() {
        let message = CastMessage {
            source_id: SOURCE_ID.to_string(),
            destination_id: RECEIVER_ID.to_string(),
            namespace: NS_RECEIVER.to_string(),
            payload_utf8: json!({"type": "CONNECT"}).to_string(),
        };
        let encoded = encode_cast_message(&message);
        assert_eq!(decode_cast_message(&encoded).unwrap(), message);
    }

    #[test]
    fn test_load_payload_wires_http_wav_stream() {
        let payload = build_load_payload(7, "http://127.0.0.1:1234/stream.wav", 48_000, 2);
        assert_eq!(payload["type"], "LOAD");
        assert_eq!(payload["requestId"], 7);
        assert_eq!(payload["media"]["contentType"], CAST_MIME_TYPE);
        assert_eq!(
            payload["media"]["contentId"],
            "http://127.0.0.1:1234/stream.wav"
        );
        assert_eq!(payload["autoplay"], true);
    }

    #[test]
    fn test_wav_header() {
        let header = build_wav_header(44_100, 2, 1000);
        assert_eq!(header.len(), 44);
        assert_eq!(&header[0..4], b"RIFF");
        assert_eq!(&header[8..12], b"WAVE");
        let sr = u32::from_le_bytes([header[24], header[25], header[26], header[27]]);
        assert_eq!(sr, 44_100);
        let ch = u16::from_le_bytes([header[22], header[23]]);
        assert_eq!(ch, 2);
    }

    #[test]
    fn test_audio_ring_buffer_wraparound() {
        let mut buf = AudioRingBuffer::new(8);
        assert_eq!(buf.write(&[1, 2, 3, 4, 5, 6, 7]), 7);
        let mut out = [0u8; 4];
        assert_eq!(buf.read(&mut out), 4);
        assert_eq!(out, [1, 2, 3, 4]);
        assert_eq!(buf.write(&[8, 9, 10, 11]), 4);
        let mut out2 = [0u8; 7];
        let n = buf.read(&mut out2);
        assert_eq!(n, 7);
        assert_eq!(&out2[..7], &[5, 6, 7, 8, 9, 10, 11]);
        assert_eq!(buf.available(), 0);
    }

    #[test]
    fn test_write_without_streaming_fails() {
        let mut sender = ChromecastSender::new(test_device());
        let samples = vec![0.0f32; 100];
        assert!(sender.write_audio(&samples).is_err());
    }

    #[test]
    fn chromecast_http_wav_stream_serves_written_audio_e2e() {
        let mut sender = ChromecastSender::new(test_device());
        sender.start_http_stream_server().unwrap();
        let port = sender.http_port.unwrap();
        {
            let mut buffer = sender.audio_buffer.lock().unwrap();
            buffer.sample_rate = 48_000;
            buffer.channels = 2;
        }
        sender.streaming.store(true, Ordering::Release);
        assert_eq!(sender.write_audio(&[0.5, -0.5, 0.25, -0.25]).unwrap(), 4);

        let mut stream = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream
            .write_all(b"GET /stream.wav HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();

        let mut response = Vec::new();
        let mut chunk = [0u8; 256];
        while response.len() < 2048 && !response.windows(4).any(|window| window == b"RIFF") {
            let n = stream.read(&mut chunk).unwrap();
            if n == 0 {
                break;
            }
            response.extend_from_slice(&chunk[..n]);
        }
        let text = String::from_utf8_lossy(&response);
        assert!(text.starts_with("HTTP/1.1 200 OK"), "{text}");
        assert!(text.contains("Content-Type: audio/wav"), "{text}");
        assert!(
            response.windows(4).any(|window| window == b"RIFF"),
            "missing WAV header in response"
        );

        sender.disconnect();
    }

    #[test]
    fn chromecast_castv2_launch_and_load_e2e() {
        let tmp = tempfile::tempdir().unwrap();
        let (port, load_urls, handle) = match spawn_fake_chromecast_receiver() {
            Ok(receiver) => receiver,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(err) => panic!("failed to bind fake Chromecast receiver: {err}"),
        };
        let mut sender =
            ChromecastSender::new(local_test_device(port)).with_tofu_config_dir(tmp.path().into());

        sender.connect(Ipv4Addr::LOCALHOST).unwrap();
        assert_eq!(sender.state(), ChromecastState::Connected);
        sender.start_stream(48_000, 2).unwrap();
        assert_eq!(sender.state(), ChromecastState::Streaming);

        let load_url = load_urls.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(
            load_url,
            format!(
                "http://127.0.0.1:{}{}",
                sender.http_port.unwrap(),
                CAST_STREAM_PATH
            )
        );

        sender.disconnect();
        handle.join().unwrap().unwrap();
    }
}
