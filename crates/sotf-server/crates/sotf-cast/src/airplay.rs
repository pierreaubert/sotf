// ============================================================================
// AirPlay Audio Sender (RAOP v1)
// ============================================================================
//
// Implements AirPlay 1 (RAOP) protocol for sending audio to AirPlay receivers
// (HomePod, Apple TV, AirPlay-enabled speakers).
//
// Protocol overview:
//   1. Discover receiver via mDNS (_raop._tcp)
//   2. RTSP session: ANNOUNCE → SETUP → RECORD
//   3. Stream audio via RTP (UDP) as signed 16-bit PCM
//   4. Control via RTSP SET_PARAMETER (volume, progress)
//   5. RTSP TEARDOWN to disconnect
//
// AirPlay 2 (HomeKit-based, encrypted) is NOT supported — only RAOP v1.

use crate::discovery::CastDevice;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};

/// AirPlay audio format: 16-bit PCM at 44100Hz stereo.
/// RAOP v1 supports 16-bit PCM (type 0x40) and ALAC (type 0x60).
const FRAMES_PER_PACKET: usize = 352;
const SAMPLE_RATE: u32 = 44100;
const CHANNELS: u16 = 2;
const BYTES_PER_SAMPLE: usize = 2; // 16-bit
const BYTES_PER_FRAME: usize = CHANNELS as usize * BYTES_PER_SAMPLE;
const PACKET_SIZE: usize = FRAMES_PER_PACKET * BYTES_PER_FRAME;

/// RTP header size (12 bytes).
const RTP_HEADER_SIZE: usize = 12;

/// Dynamic RTP payload type used by the ANNOUNCE SDP.
const RTP_PAYLOAD_TYPE: u8 = 0x60;

/// AirPlay connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirPlayState {
    Disconnected,
    Connecting,
    Connected,
    Streaming,
}

/// AirPlay sender — streams audio to an AirPlay receiver.
pub struct AirPlaySender {
    /// Target device.
    device: CastDevice,
    /// Current connection state.
    state: AirPlayState,
    /// RTSP CSeq counter.
    cseq: u32,
    /// RTSP session ID from SETUP response.
    session_id: Option<String>,
    /// RTP sequence number.
    rtp_seq: AtomicU16,
    /// RTP timestamp (in audio frames).
    rtp_timestamp: AtomicU32,
    /// UDP socket for RTP audio data.
    rtp_socket: Option<UdpSocket>,
    /// Remote RTP port (from SETUP response).
    rtp_remote_port: u16,
    /// TCP connection for RTSP control.
    rtsp_stream: Option<std::net::TcpStream>,
    /// Current volume (0.0 to 1.0 linear, mapped to -30..0 dB for RAOP).
    volume: f32,
    /// Mute state.
    muted: AtomicBool,
    /// Pre-allocated RTP packet buffer.
    #[allow(dead_code)]
    packet_buf: Vec<u8>,
    /// Accumulation buffer for incomplete frames.
    pcm_buf: Vec<u8>,
}

impl AirPlaySender {
    pub fn new(device: CastDevice) -> Self {
        Self {
            device,
            state: AirPlayState::Disconnected,
            cseq: 0,
            session_id: None,
            rtp_seq: AtomicU16::new(0),
            rtp_timestamp: AtomicU32::new(0),
            rtp_socket: None,
            rtp_remote_port: 0,
            rtsp_stream: None,
            volume: 1.0,
            muted: AtomicBool::new(false),
            packet_buf: vec![0u8; RTP_HEADER_SIZE + PACKET_SIZE],
            pcm_buf: Vec::with_capacity(PACKET_SIZE * 2),
        }
    }

    /// Connect to the AirPlay receiver.
    ///
    /// Performs the RTSP handshake: ANNOUNCE → SETUP → RECORD.
    pub fn connect(&mut self) -> Result<(), String> {
        self.state = AirPlayState::Connecting;

        let addr =
            std::net::SocketAddr::V4(SocketAddrV4::new(self.device.address, self.device.port));

        // TCP connection for RTSP control
        let stream = std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(5))
            .map_err(|e| format!("RTSP connect failed: {}", e))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .ok();
        self.rtsp_stream = Some(stream);

        // ANNOUNCE
        let announce_sdp = format!(
            "v=0\r\n\
             o=iTunes 0 0 IN IP4 {}\r\n\
             s=iTunes\r\n\
             c=IN IP4 {}\r\n\
             t=0 0\r\n\
             m=audio 0 RTP/AVP {payload_type}\r\n\
             a=rtpmap:{payload_type} L16/{sr}/{channels}\r\n\
             a=fmtp:{payload_type} {frames} 0 16 40 10 14 {channels} 255 0 0 {sr}\r\n",
            self.device.address,
            self.device.address,
            payload_type = RTP_PAYLOAD_TYPE,
            frames = FRAMES_PER_PACKET,
            sr = SAMPLE_RATE,
            channels = CHANNELS,
        );

        self.rtsp_request("ANNOUNCE", "*", Some(&announce_sdp))?;

        // SETUP — request transport
        let setup_headers = "Transport: RTP/AVP/UDP;unicast;interleaved=0-1;mode=record;control_port=0;timing_port=0\r\n".to_string();
        let setup_response = self.rtsp_request_with_headers("SETUP", "*", &setup_headers, None)?;

        // Parse server_port from Transport header
        if let Some(transport_line) = setup_response
            .lines()
            .find(|l| l.to_lowercase().starts_with("transport:"))
        {
            for part in transport_line.split(';') {
                if let Some(port_str) = part.strip_prefix("server_port=") {
                    self.rtp_remote_port = port_str.parse().unwrap_or(6000);
                }
            }
        }

        // Parse Session header
        if let Some(session_line) = setup_response
            .lines()
            .find(|l| l.to_lowercase().starts_with("session:"))
        {
            self.session_id = session_line.split(':').nth(1).map(|s| s.trim().to_string());
        }

        if self.rtp_remote_port == 0 {
            self.rtp_remote_port = 6000; // Default fallback
        }

        // Create UDP socket for RTP
        let rtp_socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
            .map_err(|e| format!("RTP socket bind failed: {}", e))?;
        rtp_socket
            .connect(SocketAddrV4::new(self.device.address, self.rtp_remote_port))
            .map_err(|e| format!("RTP socket connect failed: {}", e))?;
        self.rtp_socket = Some(rtp_socket);

        // RECORD — start streaming
        let record_headers = "Range: npt=0-\r\nRTP-Info: seq=0;rtptime=0\r\n";
        self.rtsp_request_with_headers("RECORD", "*", record_headers, None)?;

        self.state = AirPlayState::Connected;

        log::info!(
            "[AirPlay] Connected to {} at {}:{} (RTP port {})",
            self.device.name,
            self.device.address,
            self.device.port,
            self.rtp_remote_port,
        );

        Ok(())
    }

    /// Write audio samples (interleaved f32) to the AirPlay stream.
    ///
    /// Samples are converted to 16-bit PCM, packetized into RTP, and sent via UDP.
    /// Returns the number of samples consumed.
    pub fn write_audio(&mut self, samples: &[f32]) -> Result<usize, String> {
        if self.state != AirPlayState::Connected && self.state != AirPlayState::Streaming {
            return Err("Not connected".to_string());
        }

        if self.muted.load(Ordering::Relaxed) {
            // Consume samples but don't send (silence)
            return Ok(samples.len());
        }

        // Convert f32 to 16-bit PCM bytes and accumulate
        for &sample in samples {
            let clamped = sample.clamp(-1.0, 1.0);
            let pcm16 = (clamped * 32767.0) as i16;
            self.pcm_buf.extend_from_slice(&pcm16.to_be_bytes()); // Network byte order
        }

        // Send complete packets
        while self.pcm_buf.len() >= PACKET_SIZE {
            self.send_rtp_packet(&self.pcm_buf[..PACKET_SIZE])?;
            self.pcm_buf.drain(..PACKET_SIZE);
        }

        self.state = AirPlayState::Streaming;
        Ok(samples.len())
    }

    /// Set volume (0.0 to 1.0 linear scale).
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);

        // Convert linear to RAOP dB scale: 0.0 → -144 (mute), 1.0 → 0 dB
        let db = if self.volume < 0.01 {
            -144.0
        } else {
            30.0 * (self.volume as f64).log10()
        };

        // Send via RTSP SET_PARAMETER
        let body = format!("volume: {:.6}\r\n", db);
        if let Err(e) = self.rtsp_request("SET_PARAMETER", "*", Some(&body)) {
            log::debug!("[AirPlay] Volume update failed: {}", e);
        }
    }

    /// Set mute state.
    pub fn set_muted(&mut self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    /// Disconnect from the AirPlay receiver.
    pub fn disconnect(&mut self) {
        if self.state != AirPlayState::Disconnected {
            if let Err(e) = self.rtsp_request("TEARDOWN", "*", None) {
                log::debug!("[AirPlay] TEARDOWN failed: {}", e);
            }
            self.rtp_socket = None;
            self.rtsp_stream = None;
            self.session_id = None;
            self.state = AirPlayState::Disconnected;
            self.pcm_buf.clear();
            log::info!("[AirPlay] Disconnected from {}", self.device.name);
        }
    }

    /// Current connection state.
    pub fn state(&self) -> AirPlayState {
        self.state
    }

    /// The target device.
    pub fn device(&self) -> &CastDevice {
        &self.device
    }

    // --- Internal ---

    fn send_rtp_packet(&self, pcm_data: &[u8]) -> Result<(), String> {
        let socket = self.rtp_socket.as_ref().ok_or("No RTP socket")?;

        let seq = self.rtp_seq.fetch_add(1, Ordering::Relaxed);
        let ts = self
            .rtp_timestamp
            .fetch_add(FRAMES_PER_PACKET as u32, Ordering::Relaxed);

        // Build RTP header (12 bytes)
        let mut packet = Vec::with_capacity(RTP_HEADER_SIZE + pcm_data.len());

        // Byte 0: V=2, P=0, X=0, CC=0
        packet.push(0x80);
        // Byte 1: M=1 (first packet flag cleared after first), PT=96
        packet.push(RTP_PAYLOAD_TYPE | 0x80);
        // Bytes 2-3: Sequence number
        packet.extend_from_slice(&seq.to_be_bytes());
        // Bytes 4-7: Timestamp
        packet.extend_from_slice(&ts.to_be_bytes());
        // Bytes 8-11: SSRC (use 0)
        packet.extend_from_slice(&[0, 0, 0, 0]);

        // Audio payload (raw PCM, big-endian 16-bit)
        packet.extend_from_slice(pcm_data);

        socket
            .send(&packet)
            .map_err(|e| format!("RTP send failed: {}", e))?;

        Ok(())
    }

    fn rtsp_request(
        &mut self,
        method: &str,
        uri: &str,
        body: Option<&str>,
    ) -> Result<String, String> {
        self.rtsp_request_with_headers(method, uri, "", body)
    }

    fn rtsp_request_with_headers(
        &mut self,
        method: &str,
        uri: &str,
        extra_headers: &str,
        body: Option<&str>,
    ) -> Result<String, String> {
        use std::io::{BufRead, BufReader, Write};

        let stream = self.rtsp_stream.as_mut().ok_or("No RTSP connection")?;

        self.cseq += 1;

        let mut request = format!(
            "{} {} RTSP/1.0\r\nCSeq: {}\r\nUser-Agent: SOTF/1.0\r\n",
            method, uri, self.cseq,
        );

        if let Some(ref session) = self.session_id {
            request.push_str(&format!("Session: {}\r\n", session));
        }

        request.push_str(extra_headers);

        if let Some(body) = body {
            request.push_str(&format!(
                "Content-Type: application/sdp\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body,
            ));
        } else {
            request.push_str("\r\n");
        }

        stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("RTSP write failed: {}", e))?;

        log::debug!("[AirPlay] RTSP >> {} {} (CSeq {})", method, uri, self.cseq);

        // Read response
        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        let mut content_length = 0usize;

        // Read status line + headers
        loop {
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .map_err(|e| format!("RTSP read failed: {}", e))?;

            if line.trim().is_empty() {
                break;
            }

            if line.to_lowercase().starts_with("content-length:") {
                content_length = line
                    .split(':')
                    .nth(1)
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0);
            }

            response.push_str(&line);
        }

        // Read body if present
        if content_length > 0 {
            let mut body_buf = vec![0u8; content_length];
            std::io::Read::read_exact(&mut reader, &mut body_buf)
                .map_err(|e| format!("RTSP body read failed: {}", e))?;
            response.push_str(&String::from_utf8_lossy(&body_buf));
        }

        log::debug!(
            "[AirPlay] RTSP << {} ({})",
            response.lines().next().unwrap_or("?"),
            response.len(),
        );

        // Check for RTSP error
        if let Some(status_line) = response.lines().next()
            && !status_line.contains("200")
        {
            return Err(format!("RTSP error: {}", status_line));
        }

        Ok(response)
    }
}

impl Drop for AirPlaySender {
    fn drop(&mut self) {
        self.disconnect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc;
    use std::time::Duration;

    fn test_device() -> CastDevice {
        CastDevice {
            device_type: crate::CastDeviceType::AirPlay,
            name: "Test HomePod".to_string(),
            address: Ipv4Addr::new(192, 168, 1, 50),
            port: 7000,
            instance_name: String::new(),
            txt_records: HashMap::new(),
        }
    }

    fn local_test_device(port: u16) -> CastDevice {
        CastDevice {
            device_type: crate::CastDeviceType::AirPlay,
            name: "Loopback AirPlay".to_string(),
            address: Ipv4Addr::LOCALHOST,
            port,
            instance_name: String::new(),
            txt_records: HashMap::new(),
        }
    }

    fn read_rtsp_request(stream: &mut TcpStream) -> Option<(String, u32)> {
        let mut reader = BufReader::new(stream.try_clone().ok()?);
        let mut first_line = String::new();
        reader.read_line(&mut first_line).ok()?;
        if first_line.is_empty() {
            return None;
        }
        let method = first_line.split_whitespace().next()?.to_string();
        let mut cseq = 0;
        let mut content_length = 0usize;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).ok()?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some(value) = trimmed.strip_prefix("CSeq:") {
                cseq = value.trim().parse().ok()?;
            }
            if let Some(value) = trimmed.strip_prefix("Content-Length:") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
        if content_length > 0 {
            let mut body = vec![0; content_length];
            reader.read_exact(&mut body).ok()?;
        }
        Some((method, cseq))
    }

    fn write_rtsp_ok(stream: &mut TcpStream, cseq: u32, extra_headers: &str) {
        let response = format!("RTSP/1.0 200 OK\r\nCSeq: {cseq}\r\n{extra_headers}\r\n");
        stream.write_all(response.as_bytes()).unwrap();
        stream.flush().unwrap();
    }

    fn spawn_airplay_receiver(
        rtp_port: u16,
    ) -> (u16, mpsc::Receiver<String>, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(3))).ok();
            while let Some((method, cseq)) = read_rtsp_request(&mut stream) {
                tx.send(method.clone()).unwrap();
                match method.as_str() {
                    "SETUP" => write_rtsp_ok(
                        &mut stream,
                        cseq,
                        &format!(
                            "Transport: RTP/AVP/UDP;unicast;server_port={rtp_port}\r\nSession: loopback-session\r\n"
                        ),
                    ),
                    _ => write_rtsp_ok(&mut stream, cseq, ""),
                }
                if method == "TEARDOWN" {
                    break;
                }
            }
        });
        (port, rx, handle)
    }

    #[test]
    fn test_airplay_sender_initial_state() {
        let sender = AirPlaySender::new(test_device());
        assert_eq!(sender.state(), AirPlayState::Disconnected);
        assert_eq!(sender.device().name, "Test HomePod");
    }

    #[test]
    fn test_airplay_write_without_connect_fails() {
        let mut sender = AirPlaySender::new(test_device());
        let samples = vec![0.0f32; 1024];
        assert!(sender.write_audio(&samples).is_err());
    }

    #[test]
    fn test_volume_db_conversion() {
        // volume=1.0 → 0 dB, volume=0.5 → ~-9 dB, volume=0.0 → -144 dB
        let vol = 1.0f32;
        let db = 30.0 * (vol as f64).log10();
        assert!((db - 0.0).abs() < 0.01);

        let vol = 0.5f32;
        let db = 30.0 * (vol as f64).log10();
        assert!((db - (-9.03)).abs() < 0.1);
    }

    #[test]
    fn test_packet_constants() {
        assert_eq!(FRAMES_PER_PACKET, 352);
        assert_eq!(BYTES_PER_FRAME, 4); // 2 channels * 2 bytes
        assert_eq!(PACKET_SIZE, 1408); // 352 * 4
    }

    #[test]
    fn airplay_rtsp_handshake_and_rtp_packet_e2e() {
        let rtp_socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        rtp_socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let rtp_port = rtp_socket.local_addr().unwrap().port();
        let (rtsp_port, methods, handle) = spawn_airplay_receiver(rtp_port);
        let mut sender = AirPlaySender::new(local_test_device(rtsp_port));

        sender.connect().unwrap();
        assert_eq!(sender.state(), AirPlayState::Connected);
        assert_eq!(
            methods.recv_timeout(Duration::from_secs(1)).unwrap(),
            "ANNOUNCE"
        );
        assert_eq!(
            methods.recv_timeout(Duration::from_secs(1)).unwrap(),
            "SETUP"
        );
        assert_eq!(
            methods.recv_timeout(Duration::from_secs(1)).unwrap(),
            "RECORD"
        );

        let consumed = sender
            .write_audio(&vec![0.25; FRAMES_PER_PACKET * CHANNELS as usize])
            .unwrap();
        assert_eq!(consumed, FRAMES_PER_PACKET * CHANNELS as usize);

        let mut packet = vec![0u8; RTP_HEADER_SIZE + PACKET_SIZE];
        let len = rtp_socket.recv(&mut packet).unwrap();
        assert_eq!(len, RTP_HEADER_SIZE + PACKET_SIZE);
        assert_eq!(packet[0], 0x80);
        assert_eq!(packet[1], RTP_PAYLOAD_TYPE | 0x80);

        sender.disconnect();
        assert_eq!(
            methods.recv_timeout(Duration::from_secs(1)).unwrap(),
            "TEARDOWN"
        );
        handle.join().unwrap();
    }
}
