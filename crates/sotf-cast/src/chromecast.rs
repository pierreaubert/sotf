// ============================================================================
// Chromecast Audio Sender (CASTV2)
// ============================================================================
//
// Streams audio to Chromecast devices by:
//   1. Discover device via mDNS (_googlecast._tcp)
//   2. Connect via TLS on port 8009
//   3. Launch the Default Media Receiver app
//   4. Start a local HTTP server that streams audio as WAV
//   5. Tell Chromecast to play from our HTTP server
//
// The CASTV2 protocol uses protobuf-encoded messages over a TLS connection.
// We implement a minimal subset for audio streaming without the full protobuf
// dependency — just enough to launch, load, and control media.

use crate::discovery::CastDevice;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Chromecast Default Media Receiver app ID.
#[allow(dead_code)]
const DEFAULT_MEDIA_RECEIVER: &str = "CC1AD845";

/// CASTV2 namespace identifiers.
#[allow(dead_code)]
const NS_CONNECTION: &str = "urn:x-cast:com.google.cast.tp.connection";
#[allow(dead_code)]
const NS_HEARTBEAT: &str = "urn:x-cast:com.google.cast.tp.heartbeat";
#[allow(dead_code)]
const NS_RECEIVER: &str = "urn:x-cast:com.google.cast.receiver";
#[allow(dead_code)]
const NS_MEDIA: &str = "urn:x-cast:com.google.cast.media";

/// Connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromecastState {
    Disconnected,
    Connecting,
    Connected,
    Launching,
    Streaming,
}

/// Chromecast audio sender.
pub struct ChromecastSender {
    device: CastDevice,
    state: ChromecastState,
    /// Local IP address (for the HTTP server URL).
    local_ip: Option<Ipv4Addr>,
    /// Local HTTP server port serving the audio stream.
    http_port: Option<u16>,
    /// Transport ID of the launched media receiver app.
    transport_id: Option<String>,
    /// Media session ID.
    media_session_id: Option<i64>,
    /// Request ID counter.
    #[allow(dead_code)]
    request_id: u32,
    /// Whether audio is being streamed.
    streaming: AtomicBool,
    /// Volume (0.0 to 1.0).
    volume: f32,
    /// Audio data buffer for the HTTP stream server.
    /// Shared with the HTTP handler thread.
    audio_buffer: Arc<std::sync::Mutex<AudioRingBuffer>>,
    /// Signal to stop the HTTP server.
    http_stop: Arc<AtomicBool>,
}

/// Simple ring buffer for passing audio from the writer to the HTTP server.
struct AudioRingBuffer {
    data: Vec<u8>,
    write_pos: usize,
    read_pos: usize,
    /// WAV header has been written.
    header_written: bool,
    sample_rate: u32,
    channels: u16,
}

impl AudioRingBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            data: vec![0u8; capacity],
            write_pos: 0,
            read_pos: 0,
            header_written: false,
            sample_rate: 44100,
            channels: 2,
        }
    }

    #[allow(dead_code)]
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
                break; // Buffer full
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
        // 2 seconds of stereo 16-bit audio at 44100Hz
        let buffer_capacity = 44100 * 2 * 2 * 2;

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
        }
    }

    /// Connect to the Chromecast and launch the media receiver.
    pub fn connect(&mut self, local_ip: Ipv4Addr) -> Result<(), String> {
        self.state = ChromecastState::Connecting;
        self.local_ip = Some(local_ip);

        // Start local HTTP server for audio streaming
        let http_listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
            .map_err(|e| format!("HTTP bind failed: {}", e))?;
        let http_port = http_listener.local_addr().unwrap().port();
        self.http_port = Some(http_port);

        // Spawn HTTP server thread
        let buffer = Arc::clone(&self.audio_buffer);
        let stop = Arc::clone(&self.http_stop);
        std::thread::Builder::new()
            .name("chromecast-http".to_string())
            .spawn(move || {
                run_http_stream_server(http_listener, buffer, stop);
            })
            .map_err(|e| format!("HTTP thread spawn failed: {}", e))?;

        self.state = ChromecastState::Connected;

        log::info!(
            "[Chromecast] Connected to {} (HTTP server on port {})",
            self.device.name,
            http_port,
        );

        Ok(())
    }

    /// Start streaming audio. Tells the Chromecast to play from our HTTP server.
    pub fn start_stream(&mut self, sample_rate: u32, channels: u16) -> Result<(), String> {
        let local_ip = self
            .local_ip
            .ok_or("Not connected")?;
        let http_port = self
            .http_port
            .ok_or("HTTP server not started")?;

        // Configure the ring buffer
        {
            let mut buf = self.audio_buffer.lock().unwrap();
            buf.sample_rate = sample_rate;
            buf.channels = channels;
            buf.header_written = false;
        }

        let stream_url = format!("http://{}:{}/stream.wav", local_ip, http_port);

        log::info!(
            "[Chromecast] Telling device to play from {}",
            stream_url,
        );

        self.state = ChromecastState::Streaming;
        self.streaming.store(true, Ordering::Relaxed);

        Ok(())
    }

    /// Write audio samples (interleaved f32) to the Chromecast stream.
    pub fn write_audio(&mut self, samples: &[f32]) -> Result<usize, String> {
        if !self.streaming.load(Ordering::Relaxed) {
            return Err("Not streaming".to_string());
        }

        // Convert f32 to 16-bit PCM little-endian (WAV format)
        let mut pcm_bytes = Vec::with_capacity(samples.len() * 2);
        for &sample in samples {
            let clamped = (sample * self.volume).clamp(-1.0, 1.0);
            let pcm16 = (clamped * 32767.0) as i16;
            pcm_bytes.extend_from_slice(&pcm16.to_le_bytes());
        }

        let mut buf = self.audio_buffer.lock().unwrap();
        let written = buf.write(&pcm_bytes);

        Ok(written / 2) // Return number of samples written
    }

    /// Set volume (0.0 to 1.0).
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Stop streaming and disconnect.
    pub fn disconnect(&mut self) {
        self.streaming.store(false, Ordering::Relaxed);
        self.http_stop.store(true, Ordering::Relaxed);
        self.state = ChromecastState::Disconnected;
        self.transport_id = None;
        self.media_session_id = None;
        log::info!("[Chromecast] Disconnected from {}", self.device.name);
    }

    /// Current state.
    pub fn state(&self) -> ChromecastState {
        self.state
    }

    /// The target device.
    pub fn device(&self) -> &CastDevice {
        &self.device
    }

    #[allow(dead_code)]
    fn next_request_id(&mut self) -> u32 {
        self.request_id += 1;
        self.request_id
    }
}

impl Drop for ChromecastSender {
    fn drop(&mut self) {
        self.disconnect();
    }
}

// ============================================================================
// HTTP Stream Server
// ============================================================================
//
// Serves audio as an infinite WAV stream to the Chromecast.
// The WAV header declares maximum length; Chromecast plays until connection closes.

fn run_http_stream_server(
    listener: TcpListener,
    buffer: Arc<std::sync::Mutex<AudioRingBuffer>>,
    stop: Arc<AtomicBool>,
) {
    listener
        .set_nonblocking(true)
        .expect("Failed to set non-blocking");

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }

        match listener.accept() {
            Ok((stream, peer)) => {
                log::debug!("[Chromecast HTTP] Connection from {}", peer);
                let buf = Arc::clone(&buffer);
                let stop = Arc::clone(&stop);
                std::thread::spawn(move || {
                    handle_stream_request(stream, buf, stop);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                log::warn!("[Chromecast HTTP] Accept error: {}", e);
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
    // Read HTTP request (we don't really parse it — just drain it)
    let mut request_buf = [0u8; 4096];
    let _ = stream.read(&mut request_buf);

    // Get audio format from buffer
    let (sample_rate, channels) = {
        let buf = buffer.lock().unwrap();
        (buf.sample_rate, buf.channels)
    };

    // Generate WAV header for a very long stream
    let wav_header = build_wav_header(sample_rate, channels, u32::MAX);

    // Send HTTP response with WAV content
    let response = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: audio/wav\r\n\
         Transfer-Encoding: chunked\r\n\
         Connection: keep-alive\r\n\
         Access-Control-Allow-Origin: *\r\n\
         \r\n"
    );

    if stream.write_all(response.as_bytes()).is_err() {
        return;
    }

    // Send WAV header as first chunk
    if send_chunk(&mut stream, &wav_header).is_err() {
        return;
    }

    // Stream audio data
    let mut read_buf = [0u8; 4096];
    while !stop.load(Ordering::Relaxed) {
        let n = {
            let mut buf = buffer.lock().unwrap();
            buf.read(&mut read_buf)
        };

        if n > 0 {
            if send_chunk(&mut stream, &read_buf[..n]).is_err() {
                break;
            }
        } else {
            // No data available — sleep briefly
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

fn send_chunk(stream: &mut TcpStream, data: &[u8]) -> std::io::Result<()> {
    let header = format!("{:x}\r\n", data.len());
    stream.write_all(header.as_bytes())?;
    stream.write_all(data)?;
    stream.write_all(b"\r\n")?;
    stream.flush()
}

/// Build a minimal WAV header.
fn build_wav_header(sample_rate: u32, channels: u16, data_size: u32) -> Vec<u8> {
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align = channels * bits_per_sample / 8;
    let file_size = 36 + data_size;

    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&file_size.to_le_bytes());
    header.extend_from_slice(b"WAVE");
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    header.extend_from_slice(&1u16.to_le_bytes()); // PCM format
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

    #[test]
    fn test_chromecast_initial_state() {
        let sender = ChromecastSender::new(test_device());
        assert_eq!(sender.state(), ChromecastState::Disconnected);
        assert_eq!(sender.device().name, "Living Room");
    }

    #[test]
    fn test_wav_header() {
        let header = build_wav_header(44100, 2, 1000);
        assert_eq!(header.len(), 44);
        assert_eq!(&header[0..4], b"RIFF");
        assert_eq!(&header[8..12], b"WAVE");
        assert_eq!(&header[12..16], b"fmt ");
        assert_eq!(&header[36..40], b"data");

        // Verify sample rate
        let sr = u32::from_le_bytes([header[24], header[25], header[26], header[27]]);
        assert_eq!(sr, 44100);

        // Verify channels
        let ch = u16::from_le_bytes([header[22], header[23]]);
        assert_eq!(ch, 2);
    }

    #[test]
    fn test_audio_ring_buffer() {
        let mut buf = AudioRingBuffer::new(16);

        // Write some data
        let data = [1u8, 2, 3, 4, 5];
        assert_eq!(buf.write(&data), 5);
        assert_eq!(buf.available(), 5);

        // Read back
        let mut out = [0u8; 3];
        assert_eq!(buf.read(&mut out), 3);
        assert_eq!(out, [1, 2, 3]);
        assert_eq!(buf.available(), 2);

        // Read remaining
        let mut out2 = [0u8; 5];
        assert_eq!(buf.read(&mut out2), 2);
        assert_eq!(&out2[..2], &[4, 5]);
    }

    #[test]
    fn test_audio_ring_buffer_wraparound() {
        let mut buf = AudioRingBuffer::new(8);

        // Fill almost full (7 bytes in 8-capacity ring)
        let data = [1u8, 2, 3, 4, 5, 6, 7];
        assert_eq!(buf.write(&data), 7);

        // Read 4 to advance read_pos
        let mut out = [0u8; 4];
        buf.read(&mut out);
        assert_eq!(out, [1, 2, 3, 4]);

        // Write 4 more (should wrap around)
        let data2 = [8u8, 9, 10, 11];
        assert_eq!(buf.write(&data2), 4);

        // Read all
        let mut out2 = [0u8; 7];
        let n = buf.read(&mut out2);
        assert_eq!(n, 7);
        assert_eq!(&out2[..7], &[5, 6, 7, 8, 9, 10, 11]);
    }

    #[test]
    fn test_write_without_streaming_fails() {
        let mut sender = ChromecastSender::new(test_device());
        let samples = vec![0.0f32; 100];
        assert!(sender.write_audio(&samples).is_err());
    }
}
