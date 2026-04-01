// ============================================================================
// DLNA MediaRenderer
// ============================================================================
//
// Accepts audio from DLNA controllers (e.g. BubbleUPnP, foobar2000).
// Translates UPnP AVTransport/RenderingControl SOAP actions into
// SOTF player operations via the RendererAdapter trait.

use crate::device::DlnaDevice;
use crate::ssdp;
use crate::xml;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

const AVT_SERVICE: &str = "urn:schemas-upnp-org:service:AVTransport:1";
const RC_SERVICE: &str = "urn:schemas-upnp-org:service:RenderingControl:1";
const CM_SERVICE: &str = "urn:schemas-upnp-org:service:ConnectionManager:1";

/// Transport state as reported to DLNA controllers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportState {
    Stopped,
    Playing,
    PausedPlayback,
    Transitioning,
    NoMediaPresent,
}

impl TransportState {
    pub fn as_str(&self) -> &str {
        match self {
            TransportState::Stopped => "STOPPED",
            TransportState::Playing => "PLAYING",
            TransportState::PausedPlayback => "PAUSED_PLAYBACK",
            TransportState::Transitioning => "TRANSITIONING",
            TransportState::NoMediaPresent => "NO_MEDIA_PRESENT",
        }
    }
}

/// Status snapshot for DLNA transport info.
pub struct RendererStatus {
    pub state: TransportState,
    pub current_uri: Option<String>,
    pub current_title: Option<String>,
    pub elapsed_secs: f64,
    pub duration_secs: f64,
    pub volume: u8,
    pub muted: bool,
}

/// Trait for bridging DLNA renderer actions to the SOTF player.
pub trait RendererAdapter: Send + Sync + 'static {
    fn set_uri(&self, uri: &str, metadata: &str) -> Result<(), String>;
    fn play(&self) -> Result<(), String>;
    fn pause(&self) -> Result<(), String>;
    fn stop(&self) -> Result<(), String>;
    fn seek(&self, target_secs: f64) -> Result<(), String>;
    fn set_volume(&self, volume: u8) -> Result<(), String>;
    fn set_mute(&self, muted: bool) -> Result<(), String>;
    fn status(&self) -> RendererStatus;
}

/// DLNA MediaRenderer — listens for SOAP control requests and responds.
pub struct DlnaRenderer {
    device: DlnaDevice,
    adapter: Arc<dyn RendererAdapter>,
}

impl DlnaRenderer {
    pub fn new(device: DlnaDevice, adapter: Arc<dyn RendererAdapter>) -> Self {
        Self { device, adapter }
    }

    /// Run the renderer (HTTP control server + SSDP announcements).
    pub async fn run(
        &self,
        local_ip: Ipv4Addr,
        cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), String> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.device.http_port))
            .await
            .map_err(|e| format!("Failed to bind renderer HTTP: {}", e))?;

        // Send initial SSDP alive
        ssdp::send_alive(&self.device, local_ip).await?;

        // Spawn SSDP listener
        let ssdp_device = self.device.clone();
        let ssdp_cancel = cancel.clone();
        tokio::spawn(async move {
            if let Err(e) = ssdp::listen_and_respond(ssdp_device, local_ip, ssdp_cancel).await {
                log::warn!("[DLNA Renderer] SSDP error: {}", e);
            }
        });

        // Periodic SSDP alive announcements (every 15 minutes)
        let alive_device = self.device.clone();
        let alive_cancel = cancel.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(900));
            loop {
                interval.tick().await;
                if *alive_cancel.borrow() {
                    break;
                }
                if let Err(e) = ssdp::send_alive(&alive_device, local_ip).await {
                    log::warn!("[DLNA Renderer] SSDP alive failed: {}", e);
                }
            }
        });

        log::info!(
            "[DLNA Renderer] '{}' running on port {}",
            self.device.friendly_name,
            self.device.http_port,
        );

        // HTTP control loop
        loop {
            let mut cancel_rx = cancel.clone();
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _peer)) => {
                            let adapter = Arc::clone(&self.adapter);
                            let device = self.device.clone();
                            let base_url = format!("http://{}:{}", local_ip, device.http_port);
                            tokio::spawn(async move {
                                if let Err(e) = handle_http_request(stream, &device, &base_url, &adapter).await {
                                    log::debug!("[DLNA Renderer] HTTP error: {}", e);
                                }
                            });
                        }
                        Err(e) => log::warn!("[DLNA Renderer] Accept error: {}", e),
                    }
                }
                _ = cancel_rx.changed() => {
                    if *cancel_rx.borrow() {
                        // Send byebye before shutting down
                        ssdp::send_byebye(&self.device).await.ok();
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}

async fn handle_http_request(
    stream: tokio::net::TcpStream,
    device: &DlnaDevice,
    base_url: &str,
    adapter: &Arc<dyn RendererAdapter>,
) -> Result<(), String> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Read HTTP request line
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .await
        .map_err(|e| e.to_string())?;

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Ok(());
    }
    let method = parts[0];
    let path = parts[1];

    // Read headers
    let mut headers = Vec::new();
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .await
            .map_err(|e| e.to_string())?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.to_lowercase();
            let value = value.trim().to_string();
            if key == "content-length" {
                content_length = value.parse().unwrap_or(0);
            }
            headers.push((key, value));
        }
    }

    // Read body (cap at 1MB to prevent DoS)
    const MAX_BODY: usize = 1024 * 1024;
    if content_length > MAX_BODY {
        return Err(format!("Request body too large: {} bytes", content_length));
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        tokio::io::AsyncReadExt::read_exact(&mut reader, &mut body)
            .await
            .map_err(|e| e.to_string())?;
    }
    let body_str = String::from_utf8_lossy(&body);

    log::debug!(
        "[DLNA Renderer] {} {} ({} bytes)",
        method,
        path,
        content_length
    );

    // Route request
    let response = match (method, path) {
        ("GET", "/description.xml") => {
            let xml = device.description_xml(base_url);
            http_response(200, "text/xml", &xml)
        }
        ("POST", "/AVTransport/control") => handle_avtransport_action(&body_str, adapter),
        ("POST", "/RenderingControl/control") => {
            handle_rendering_control_action(&body_str, adapter)
        }
        ("POST", "/ConnectionManager/control") => handle_connection_manager_action(&body_str),
        _ => http_response(404, "text/plain", "Not Found"),
    };

    writer
        .write_all(response.as_bytes())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn handle_avtransport_action(body: &str, adapter: &Arc<dyn RendererAdapter>) -> String {
    let Some((action, args)) = xml::extract_soap_action(body) else {
        return http_soap_fault(402, "Invalid SOAP action");
    };

    let find_arg =
        |name: &str| -> Option<&str> { args.iter().find(|(k, _)| *k == name).map(|(_, v)| *v) };

    let result = match action {
        "SetAVTransportURI" => {
            let uri = find_arg("CurrentURI").unwrap_or("");
            let metadata = find_arg("CurrentURIMetaData").unwrap_or("");
            adapter.set_uri(uri, metadata)
        }
        "Play" => adapter.play(),
        "Pause" => adapter.pause(),
        "Stop" => adapter.stop(),
        "Seek" => {
            let target = find_arg("Target").unwrap_or("0");
            let secs = parse_duration_to_secs(target);
            adapter.seek(secs)
        }
        "GetTransportInfo" => {
            let status = adapter.status();
            let resp = xml::soap_response(
                "GetTransportInfo",
                AVT_SERVICE,
                &[
                    ("CurrentTransportState", status.state.as_str()),
                    ("CurrentTransportStatus", "OK"),
                    ("CurrentSpeed", "1"),
                ],
            );
            return http_soap_response(&resp);
        }
        "GetPositionInfo" => {
            let status = adapter.status();
            let elapsed = format_duration(status.elapsed_secs);
            let duration = format_duration(status.duration_secs);
            let uri = status.current_uri.as_deref().unwrap_or("");
            let resp = xml::soap_response(
                "GetPositionInfo",
                AVT_SERVICE,
                &[
                    ("Track", "1"),
                    ("TrackDuration", &duration),
                    ("TrackMetaData", ""),
                    ("TrackURI", uri),
                    ("RelTime", &elapsed),
                    ("AbsTime", &elapsed),
                    ("RelCount", "0"),
                    ("AbsCount", "0"),
                ],
            );
            return http_soap_response(&resp);
        }
        "GetMediaInfo" => {
            let status = adapter.status();
            let uri = status.current_uri.as_deref().unwrap_or("");
            let duration = format_duration(status.duration_secs);
            let resp = xml::soap_response(
                "GetMediaInfo",
                AVT_SERVICE,
                &[
                    ("NrTracks", "1"),
                    ("MediaDuration", &duration),
                    ("CurrentURI", uri),
                    ("CurrentURIMetaData", ""),
                    ("NextURI", ""),
                    ("NextURIMetaData", ""),
                    ("PlayMedium", "NETWORK"),
                    ("RecordMedium", "NOT_IMPLEMENTED"),
                    ("WriteStatus", "NOT_IMPLEMENTED"),
                ],
            );
            return http_soap_response(&resp);
        }
        _ => {
            log::debug!("[DLNA Renderer] Unknown AVTransport action: {}", action);
            return http_soap_fault(401, &format!("Invalid Action: {}", action));
        }
    };

    match result {
        Ok(()) => {
            let resp = xml::soap_response(action, AVT_SERVICE, &[]);
            http_soap_response(&resp)
        }
        Err(e) => http_soap_fault(501, &e),
    }
}

fn handle_rendering_control_action(body: &str, adapter: &Arc<dyn RendererAdapter>) -> String {
    let Some((action, args)) = xml::extract_soap_action(body) else {
        return http_soap_fault(402, "Invalid SOAP action");
    };

    let find_arg =
        |name: &str| -> Option<&str> { args.iter().find(|(k, _)| *k == name).map(|(_, v)| *v) };

    match action {
        "SetVolume" => {
            let vol: u8 = find_arg("DesiredVolume")
                .and_then(|v| v.parse().ok())
                .unwrap_or(50);
            match adapter.set_volume(vol) {
                Ok(()) => {
                    let resp = xml::soap_response(action, RC_SERVICE, &[]);
                    http_soap_response(&resp)
                }
                Err(e) => http_soap_fault(501, &e),
            }
        }
        "GetVolume" => {
            let status = adapter.status();
            let vol = status.volume.to_string();
            let resp = xml::soap_response(action, RC_SERVICE, &[("CurrentVolume", &vol)]);
            http_soap_response(&resp)
        }
        "SetMute" => {
            let muted = find_arg("DesiredMute").is_some_and(|v| v == "1" || v == "true");
            match adapter.set_mute(muted) {
                Ok(()) => {
                    let resp = xml::soap_response(action, RC_SERVICE, &[]);
                    http_soap_response(&resp)
                }
                Err(e) => http_soap_fault(501, &e),
            }
        }
        "GetMute" => {
            let status = adapter.status();
            let muted = if status.muted { "1" } else { "0" };
            let resp = xml::soap_response(action, RC_SERVICE, &[("CurrentMute", muted)]);
            http_soap_response(&resp)
        }
        _ => {
            log::debug!(
                "[DLNA Renderer] Unknown RenderingControl action: {}",
                action
            );
            http_soap_fault(401, &format!("Invalid Action: {}", action))
        }
    }
}

fn handle_connection_manager_action(body: &str) -> String {
    let Some((action, _args)) = xml::extract_soap_action(body) else {
        return http_soap_fault(402, "Invalid SOAP action");
    };

    match action {
        "GetProtocolInfo" => {
            let protocols = "http-get:*:audio/flac:*,\
                             http-get:*:audio/mpeg:*,\
                             http-get:*:audio/mp4:*,\
                             http-get:*:audio/ogg:*,\
                             http-get:*:audio/wav:*,\
                             http-get:*:audio/aiff:*,\
                             http-get:*:audio/aac:*,\
                             http-get:*:audio/x-flac:*";
            let resp =
                xml::soap_response(action, CM_SERVICE, &[("Source", ""), ("Sink", protocols)]);
            http_soap_response(&resp)
        }
        _ => http_soap_fault(401, &format!("Invalid Action: {}", action)),
    }
}

fn http_response(status: u16, content_type: &str, body: &str) -> String {
    let status_text = match status {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };
    format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        status,
        status_text,
        content_type,
        body.len(),
        body,
    )
}

fn http_soap_response(soap_body: &str) -> String {
    http_response(200, "text/xml; charset=\"utf-8\"", soap_body)
}

fn http_soap_fault(code: u32, description: &str) -> String {
    let fault = xml::soap_fault(code, description);
    http_response(500, "text/xml; charset=\"utf-8\"", &fault)
}

/// Parse a DLNA duration string "HH:MM:SS" or "HH:MM:SS.mmm" to seconds.
fn parse_duration_to_secs(s: &str) -> f64 {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        3 => {
            let h: f64 = parts[0].parse().unwrap_or(0.0);
            let m: f64 = parts[1].parse().unwrap_or(0.0);
            let s: f64 = parts[2].parse().unwrap_or(0.0);
            h * 3600.0 + m * 60.0 + s
        }
        _ => s.parse().unwrap_or(0.0), // fallback: try as raw seconds
    }
}

/// Format seconds as "HH:MM:SS".
fn format_duration(secs: f64) -> String {
    let total = secs as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert!((parse_duration_to_secs("01:30:45") - 5445.0).abs() < 0.01);
        assert!((parse_duration_to_secs("00:06:22.500") - 382.5).abs() < 0.01);
        assert!((parse_duration_to_secs("00:00:30") - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(5445.0), "01:30:45");
        assert_eq!(format_duration(382.0), "00:06:22");
        assert_eq!(format_duration(0.0), "00:00:00");
    }

    #[test]
    fn test_transport_state() {
        assert_eq!(TransportState::Playing.as_str(), "PLAYING");
        assert_eq!(TransportState::Stopped.as_str(), "STOPPED");
        assert_eq!(TransportState::PausedPlayback.as_str(), "PAUSED_PLAYBACK");
    }
}
