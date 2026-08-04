// ============================================================================
// DLNA MediaRenderer
// ============================================================================
//
// Accepts audio from DLNA controllers (e.g. BubbleUPnP, foobar2000).
// Translates UPnP AVTransport/RenderingControl SOAP actions into
// SOTF player operations via the RendererAdapter trait.

use crate::device::DlnaDevice;
use crate::gena::{GenaRegistry, event_property_set};
use crate::http_io;
use crate::scpd::scpd_for_path;
use crate::ssdp;
use crate::xml;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, BufReader};
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
    events: GenaRegistry,
}

impl DlnaRenderer {
    pub fn new(device: DlnaDevice, adapter: Arc<dyn RendererAdapter>) -> Self {
        Self {
            device,
            adapter,
            events: GenaRegistry::new(),
        }
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

        let event_adapter = Arc::clone(&self.adapter);
        let event_registry = self.events.clone();
        let event_cancel = cancel.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            let mut last_avtransport = avtransport_event_body(&event_adapter);
            let mut last_rendering_control = rendering_control_event_body(&event_adapter);
            let mut cancel_rx = event_cancel;
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let avtransport = avtransport_event_body(&event_adapter);
                        if event_registry.has_subscribers("/AVTransport/event")
                            && avtransport != last_avtransport
                        {
                            last_avtransport = avtransport.clone();
                            event_registry.notify("/AVTransport/event", avtransport);
                        }
                        let rendering_control = rendering_control_event_body(&event_adapter);
                        if event_registry.has_subscribers("/RenderingControl/event")
                            && rendering_control != last_rendering_control
                        {
                            last_rendering_control = rendering_control.clone();
                            event_registry.notify(
                                "/RenderingControl/event",
                                rendering_control,
                            );
                        }
                    }
                    changed = cancel_rx.changed() => {
                        if changed.is_err() || *cancel_rx.borrow() {
                            break;
                        }
                    }
                }
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
                            let events = self.events.clone();
                            let base_url = format!("http://{}:{}", local_ip, device.http_port);
                            tokio::spawn(async move {
                                if let Err(e) = handle_http_request(
                                    stream,
                                    &device,
                                    &base_url,
                                    &adapter,
                                    &events,
                                )
                                .await
                                {
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
    events: &GenaRegistry,
) -> Result<(), String> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Hardened read: caps line length / header count / Content-Length
    // BEFORE allocating the body (review §2).
    let req = http_io::read_http_request(&mut reader).await?;
    let body_str = String::from_utf8_lossy(&req.body);

    log::debug!(
        "[DLNA Renderer] {} {} ({} bytes)",
        req.method,
        req.path,
        req.body.len()
    );

    // Route request
    let response = match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/description.xml") => {
            let xml = device.description_xml(base_url);
            http_response(200, "text/xml", &xml)
        }
        ("POST", "/AVTransport/control") => handle_avtransport_action(&body_str, adapter),
        ("POST", "/RenderingControl/control") => {
            handle_rendering_control_action(&body_str, adapter)
        }
        ("POST", "/ConnectionManager/control") => handle_connection_manager_action(&body_str),
        ("SUBSCRIBE", path) if renderer_event_body(path, adapter).is_some() => {
            let event = renderer_event_body(path, adapter).unwrap();
            let result = events.subscribe(path, &req.headers, event);
            http_response_with_headers(result.status, "text/plain", &result.headers, "")
        }
        ("UNSUBSCRIBE", path) if renderer_event_body(path, adapter).is_some() => {
            let result = events.unsubscribe(path, &req.headers);
            http_response_with_headers(result.status, "text/plain", &result.headers, "")
        }
        ("GET", path) if let Some(scpd) = scpd_for_path(path) => {
            http_response(200, "text/xml; charset=\"utf-8\"", scpd)
        }
        _ => http_response(404, "text/plain", "Not Found"),
    };

    writer
        .write_all(response.as_bytes())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn renderer_event_body(path: &str, adapter: &Arc<dyn RendererAdapter>) -> Option<String> {
    match path {
        "/AVTransport/event" => Some(avtransport_event_body(adapter)),
        "/RenderingControl/event" => Some(rendering_control_event_body(adapter)),
        "/ConnectionManager/event" => Some(event_property_set(&[
            ("SourceProtocolInfo", String::new()),
            ("SinkProtocolInfo", renderer_protocol_info().to_string()),
        ])),
        _ => None,
    }
}

fn avtransport_event_body(adapter: &Arc<dyn RendererAdapter>) -> String {
    let status = adapter.status();
    event_property_set(&[
        ("TransportState", status.state.as_str().to_string()),
        ("TransportStatus", "OK".to_string()),
        ("CurrentTrackURI", status.current_uri.unwrap_or_default()),
    ])
}

fn rendering_control_event_body(adapter: &Arc<dyn RendererAdapter>) -> String {
    let status = adapter.status();
    event_property_set(&[
        ("Volume", status.volume.to_string()),
        ("Mute", if status.muted { "1" } else { "0" }.to_string()),
    ])
}

fn renderer_protocol_info() -> &'static str {
    "http-get:*:audio/flac:*,http-get:*:audio/mpeg:*,http-get:*:audio/mp4:*,http-get:*:audio/ogg:*,http-get:*:audio/wav:*,http-get:*:audio/aiff:*,http-get:*:audio/aac:*,http-get:*:audio/x-flac:*"
}

fn handle_avtransport_action(body: &str, adapter: &Arc<dyn RendererAdapter>) -> String {
    let Some((action, args)) = xml::extract_soap_action(body) else {
        return http_soap_fault(402, "Invalid SOAP action");
    };

    let find_arg = |name: &str| -> Option<&str> {
        args.iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };

    let result = match action.as_str() {
        "SetAVTransportURI" => {
            let uri = find_arg("CurrentURI").unwrap_or("");
            let metadata = find_arg("CurrentURIMetaData").unwrap_or("");
            // Defence-in-depth: refuse non-http(s) schemes so a malicious
            // controller can't make us fetch `file:///etc/passwd` etc.
            if !uri.is_empty() && !is_safe_uri(uri) {
                return http_soap_fault(402, "Unsupported URI scheme");
            }
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
            let resp = xml::soap_response(&action, AVT_SERVICE, &[]);
            http_soap_response(&resp)
        }
        Err(e) => http_soap_fault(501, &e),
    }
}

/// Allow-list URI schemes we'll pass to the adapter. `file://`,
/// `gopher://`, `javascript:` etc. are intentionally excluded so a hostile
/// controller cannot turn the renderer into an SSRF / local-file fetcher.
fn is_safe_uri(uri: &str) -> bool {
    let lower = uri.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn handle_rendering_control_action(body: &str, adapter: &Arc<dyn RendererAdapter>) -> String {
    let Some((action, args)) = xml::extract_soap_action(body) else {
        return http_soap_fault(402, "Invalid SOAP action");
    };

    let find_arg = |name: &str| -> Option<&str> {
        args.iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };

    match action.as_str() {
        "SetVolume" => {
            let vol: u8 = find_arg("DesiredVolume")
                .and_then(|v| v.parse().ok())
                .unwrap_or(50);
            match adapter.set_volume(vol) {
                Ok(()) => {
                    let resp = xml::soap_response(&action, RC_SERVICE, &[]);
                    http_soap_response(&resp)
                }
                Err(e) => http_soap_fault(501, &e),
            }
        }
        "GetVolume" => {
            let status = adapter.status();
            let vol = status.volume.to_string();
            let resp = xml::soap_response(&action, RC_SERVICE, &[("CurrentVolume", &vol)]);
            http_soap_response(&resp)
        }
        "SetMute" => {
            let muted =
                find_arg("DesiredMute").is_some_and(|v| v == "1" || v == "true" || v == "yes");
            match adapter.set_mute(muted) {
                Ok(()) => {
                    let resp = xml::soap_response(&action, RC_SERVICE, &[]);
                    http_soap_response(&resp)
                }
                Err(e) => http_soap_fault(501, &e),
            }
        }
        "GetMute" => {
            let status = adapter.status();
            let muted = if status.muted { "1" } else { "0" };
            let resp = xml::soap_response(&action, RC_SERVICE, &[("CurrentMute", muted)]);
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

    match action.as_str() {
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
                xml::soap_response(&action, CM_SERVICE, &[("Source", ""), ("Sink", protocols)]);
            http_soap_response(&resp)
        }
        _ => http_soap_fault(401, &format!("Invalid Action: {}", action)),
    }
}

fn http_response(status: u16, content_type: &str, body: &str) -> String {
    http_response_with_headers(status, content_type, &[], body)
}

fn http_response_with_headers(
    status: u16,
    content_type: &str,
    extra_headers: &[(String, String)],
    body: &str,
) -> String {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        412 => "Precondition Failed",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        _ => "Unknown",
    };
    let extra_headers = extra_headers
        .iter()
        .map(|(name, value)| format!("{}: {}\r\n", name, value))
        .collect::<String>();
    format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         {}Connection: close\r\n\
         \r\n\
         {}",
        status,
        status_text,
        content_type,
        body.len(),
        extra_headers,
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
