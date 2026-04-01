// ============================================================================
// DLNA MediaServer
// ============================================================================
//
// Exposes the SOTF music library as a UPnP ContentDirectory.
// DLNA controllers can browse albums/tracks and stream audio files.

use crate::device::DlnaDevice;
use crate::didl::{self, DidlContainer, DidlItem};
use crate::ssdp;
use crate::xml;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

const CD_SERVICE: &str = "urn:schemas-upnp-org:service:ContentDirectory:1";
const CM_SERVICE: &str = "urn:schemas-upnp-org:service:ConnectionManager:1";

/// Album info for the DLNA server.
pub struct MediaAlbum {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<u32>,
    pub track_count: u32,
}

/// Track info for the DLNA server.
pub struct MediaTrack {
    pub id: String,
    pub album_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: Option<String>,
    pub track_number: Option<u32>,
    pub duration_secs: Option<f64>,
    pub file_path: String,
    pub mime_type: String,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub bit_depth: Option<u32>,
    pub file_size: Option<u64>,
}

/// Trait for bridging ContentDirectory requests to the SOTF library.
pub trait MediaServerAdapter: Send + Sync + 'static {
    /// Browse root: return top-level albums.
    fn browse_albums(&self, start: u32, count: u32) -> (Vec<MediaAlbum>, u32);

    /// Browse an album: return its tracks.
    fn browse_album_tracks(&self, album_id: &str) -> Vec<MediaTrack>;

    /// Search for tracks matching a query.
    fn search_tracks(&self, query: &str, start: u32, count: u32) -> (Vec<MediaTrack>, u32);

    /// Total number of albums.
    fn album_count(&self) -> u32;
}

/// DLNA MediaServer — serves library content to DLNA controllers.
pub struct DlnaMediaServer {
    device: DlnaDevice,
    adapter: Arc<dyn MediaServerAdapter>,
}

impl DlnaMediaServer {
    pub fn new(device: DlnaDevice, adapter: Arc<dyn MediaServerAdapter>) -> Self {
        Self { device, adapter }
    }

    pub async fn run(
        &self,
        local_ip: Ipv4Addr,
        cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), String> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.device.http_port))
            .await
            .map_err(|e| format!("Failed to bind server HTTP: {}", e))?;

        ssdp::send_alive(&self.device, local_ip).await?;

        let ssdp_device = self.device.clone();
        let ssdp_cancel = cancel.clone();
        tokio::spawn(async move {
            if let Err(e) = ssdp::listen_and_respond(ssdp_device, local_ip, ssdp_cancel).await {
                log::warn!("[DLNA Server] SSDP error: {}", e);
            }
        });

        log::info!(
            "[DLNA Server] '{}' running on port {}",
            self.device.friendly_name,
            self.device.http_port,
        );

        loop {
            let mut cancel_rx = cancel.clone();
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            let adapter = Arc::clone(&self.adapter);
                            let device = self.device.clone();
                            let base = format!("http://{}:{}", local_ip, device.http_port);
                            tokio::spawn(async move {
                                if let Err(e) = handle_server_request(stream, &device, &base, &adapter).await {
                                    log::debug!("[DLNA Server] HTTP error: {}", e);
                                }
                            });
                        }
                        Err(e) => log::warn!("[DLNA Server] Accept error: {}", e),
                    }
                }
                _ = cancel_rx.changed() => {
                    if *cancel_rx.borrow() {
                        ssdp::send_byebye(&self.device).await.ok();
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}

async fn handle_server_request(
    stream: tokio::net::TcpStream,
    device: &DlnaDevice,
    base_url: &str,
    adapter: &Arc<dyn MediaServerAdapter>,
) -> Result<(), String> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

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
        if let Some((key, value)) = trimmed.split_once(':')
            && key.to_lowercase() == "content-length"
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }

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

    let response = match (method, path) {
        ("GET", "/description.xml") => {
            let xml_body = device.description_xml(base_url);
            http_response(200, "text/xml", &xml_body)
        }
        ("POST", "/ContentDirectory/control") => {
            handle_content_directory(&body_str, adapter, base_url)
        }
        ("POST", "/ConnectionManager/control") => handle_cm_action(&body_str),
        _ => http_response(404, "text/plain", "Not Found"),
    };

    writer
        .write_all(response.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn handle_content_directory(
    body: &str,
    adapter: &Arc<dyn MediaServerAdapter>,
    base_url: &str,
) -> String {
    let Some((action, args)) = xml::extract_soap_action(body) else {
        return http_soap_fault(402, "Invalid SOAP");
    };

    let find_arg =
        |name: &str| -> Option<&str> { args.iter().find(|(k, _)| *k == name).map(|(_, v)| *v) };

    match action {
        "Browse" => {
            let object_id = find_arg("ObjectID").unwrap_or("0");
            let flag = find_arg("BrowseFlag").unwrap_or("BrowseDirectChildren");
            let start: u32 = find_arg("StartingIndex")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let count: u32 = find_arg("RequestedCount")
                .and_then(|v| v.parse().ok())
                .unwrap_or(50);

            let (didl_xml, total, returned) = if flag == "BrowseMetadata" {
                // Return metadata for a single object
                if object_id == "0" {
                    let containers = vec![DidlContainer {
                        id: "0".to_string(),
                        parent_id: "-1".to_string(),
                        title: "Music".to_string(),
                        child_count: adapter.album_count(),
                    }];
                    (didl::didl_lite(&containers, &[]), 1u32, 1u32)
                } else {
                    // Album metadata
                    let containers = vec![DidlContainer {
                        id: object_id.to_string(),
                        parent_id: "0".to_string(),
                        title: object_id.to_string(),
                        child_count: 0,
                    }];
                    (didl::didl_lite(&containers, &[]), 1, 1)
                }
            } else if object_id == "0" {
                // Browse root → list albums
                let (albums, total) = adapter.browse_albums(start, count);
                let containers: Vec<DidlContainer> = albums
                    .iter()
                    .map(|a| DidlContainer {
                        id: a.id.clone(),
                        parent_id: "0".to_string(),
                        title: format!("{} - {}", a.artist, a.title),
                        child_count: a.track_count,
                    })
                    .collect();
                let returned = containers.len() as u32;
                (didl::didl_lite(&containers, &[]), total, returned)
            } else {
                // Browse album → list tracks
                let tracks = adapter.browse_album_tracks(object_id);
                let items: Vec<DidlItem> = tracks
                    .iter()
                    .map(|t| DidlItem {
                        id: t.id.clone(),
                        parent_id: t.album_id.clone(),
                        title: t.title.clone(),
                        artist: Some(t.artist.clone()),
                        album: Some(t.album.clone()),
                        genre: t.genre.clone(),
                        track_number: t.track_number,
                        duration: t.duration_secs,
                        resource_url: format!("{}/media/{}", base_url, t.id),
                        mime_type: t.mime_type.clone(),
                        sample_rate: t.sample_rate,
                        channels: t.channels,
                        bit_depth: t.bit_depth,
                        file_size: t.file_size,
                    })
                    .collect();
                let total = items.len() as u32;
                let returned = items.len() as u32;
                (didl::didl_lite(&[], &items), total, returned)
            };

            // Escape the DIDL-Lite XML for embedding in SOAP
            let escaped_didl = didl_xml
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");

            let resp = xml::soap_response(
                "Browse",
                CD_SERVICE,
                &[
                    ("Result", &escaped_didl),
                    ("NumberReturned", &returned.to_string()),
                    ("TotalMatches", &total.to_string()),
                    ("UpdateID", "1"),
                ],
            );
            http_soap_response(&resp)
        }
        "Search" => {
            let query = find_arg("SearchCriteria").unwrap_or("");
            let start: u32 = find_arg("StartingIndex")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let count: u32 = find_arg("RequestedCount")
                .and_then(|v| v.parse().ok())
                .unwrap_or(50);

            let (tracks, total) = adapter.search_tracks(query, start, count);
            let items: Vec<DidlItem> = tracks
                .iter()
                .map(|t| DidlItem {
                    id: t.id.clone(),
                    parent_id: t.album_id.clone(),
                    title: t.title.clone(),
                    artist: Some(t.artist.clone()),
                    album: Some(t.album.clone()),
                    genre: t.genre.clone(),
                    track_number: t.track_number,
                    duration: t.duration_secs,
                    resource_url: format!("{}/media/{}", base_url, t.id),
                    mime_type: t.mime_type.clone(),
                    sample_rate: t.sample_rate,
                    channels: t.channels,
                    bit_depth: t.bit_depth,
                    file_size: t.file_size,
                })
                .collect();

            let escaped_didl = didl::didl_lite(&[], &items)
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");

            let resp = xml::soap_response(
                "Search",
                CD_SERVICE,
                &[
                    ("Result", &escaped_didl),
                    ("NumberReturned", &items.len().to_string()),
                    ("TotalMatches", &total.to_string()),
                    ("UpdateID", "1"),
                ],
            );
            http_soap_response(&resp)
        }
        "GetSystemUpdateID" => {
            let resp = xml::soap_response(action, CD_SERVICE, &[("Id", "1")]);
            http_soap_response(&resp)
        }
        "GetSearchCapabilities" => {
            let resp = xml::soap_response(
                action,
                CD_SERVICE,
                &[(
                    "SearchCaps",
                    "dc:title,dc:creator,upnp:album,upnp:artist,upnp:genre",
                )],
            );
            http_soap_response(&resp)
        }
        "GetSortCapabilities" => {
            let resp = xml::soap_response(
                action,
                CD_SERVICE,
                &[("SortCaps", "dc:title,dc:creator,upnp:album")],
            );
            http_soap_response(&resp)
        }
        _ => http_soap_fault(401, &format!("Invalid Action: {}", action)),
    }
}

fn handle_cm_action(body: &str) -> String {
    let Some((action, _)) = xml::extract_soap_action(body) else {
        return http_soap_fault(402, "Invalid SOAP");
    };

    match action {
        "GetProtocolInfo" => {
            let protocols = "http-get:*:audio/flac:*,http-get:*:audio/mpeg:*,http-get:*:audio/wav:*,http-get:*:audio/ogg:*,http-get:*:audio/aac:*";
            let resp =
                xml::soap_response(action, CM_SERVICE, &[("Source", protocols), ("Sink", "")]);
            http_soap_response(&resp)
        }
        _ => http_soap_fault(401, &format!("Invalid Action: {}", action)),
    }
}

fn http_response(status: u16, content_type: &str, body: &str) -> String {
    let status_text = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Error",
    };
    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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
