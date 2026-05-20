// ============================================================================
// DLNA MediaServer
// ============================================================================
//
// Exposes the SOTF music library as a UPnP ContentDirectory.
// DLNA controllers can browse albums/tracks and stream audio files.

use crate::device::DlnaDevice;
use crate::didl::{self, DidlContainer, DidlItem};
use crate::http_io;
use crate::ssdp;
use crate::xml;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, BufReader};
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

/// Resolved metadata for a `/media/{id}` request.
pub struct MediaSource {
    /// Filesystem path to the audio file.
    pub path: PathBuf,
    /// MIME type to advertise.
    pub mime_type: String,
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

    /// Resolve a `/media/{id}` URL to a filesystem path and MIME type.
    ///
    /// Returning `None` means the adapter cannot map ids to files — the
    /// HTTP handler will respond with `501 Not Implemented` (NOT 404),
    /// per `reviews/review-sotf-dlna.md` §4.
    fn media_path(&self, _track_id: &str) -> Option<MediaSource> {
        None
    }
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

    // Hardened read: caps line length / header count / Content-Length
    // BEFORE allocating the body (review §2).
    let req = http_io::read_http_request(&mut reader).await?;
    let body_str = String::from_utf8_lossy(&req.body);

    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/description.xml") => {
            let xml_body = device.description_xml(base_url);
            let response = http_response(200, "text/xml", &xml_body);
            writer
                .write_all(response.as_bytes())
                .await
                .map_err(|e| e.to_string())?;
        }
        ("POST", "/ContentDirectory/control") => {
            let response = handle_content_directory(&body_str, adapter, base_url);
            writer
                .write_all(response.as_bytes())
                .await
                .map_err(|e| e.to_string())?;
        }
        ("POST", "/ConnectionManager/control") => {
            let response = handle_cm_action(&body_str);
            writer
                .write_all(response.as_bytes())
                .await
                .map_err(|e| e.to_string())?;
        }
        (method, path) if path.starts_with("/media/") && (method == "GET" || method == "HEAD") => {
            // Basic /media/{id} handler. Range requests + DLNA profile
            // flags are NOT in this bug-fix pass — see review §4.
            handle_media(&mut writer, method, path, adapter).await?;
        }
        // GENA event + SCPD endpoints are NOT implemented in this
        // bug-fix pass. Return 501 so controllers can distinguish
        // "device wrong" from "feature missing" (review §4).
        ("SUBSCRIBE" | "UNSUBSCRIBE", p) if p.ends_with("/event") => {
            let response = http_response(501, "text/plain", "GENA eventing not implemented");
            writer
                .write_all(response.as_bytes())
                .await
                .map_err(|e| e.to_string())?;
        }
        ("GET", p) if p.ends_with("/scpd.xml") => {
            let response = http_response(501, "text/plain", "SCPD endpoints not implemented");
            writer
                .write_all(response.as_bytes())
                .await
                .map_err(|e| e.to_string())?;
        }
        _ => {
            let response = http_response(404, "text/plain", "Not Found");
            writer
                .write_all(response.as_bytes())
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

async fn handle_media(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    method: &str,
    path: &str,
    adapter: &Arc<dyn MediaServerAdapter>,
) -> Result<(), String> {
    let id = path.trim_start_matches("/media/");
    // Defend against path-traversal in `{id}`.
    if id.is_empty() || id.contains('/') || id.contains("..") {
        let response = http_response(400, "text/plain", "Bad media id");
        return writer
            .write_all(response.as_bytes())
            .await
            .map_err(|e| e.to_string());
    }

    let Some(src) = adapter.media_path(id) else {
        // Review requirement: if no source is available, return 501 (NOT
        // 404). 404 would imply "no such track" — a lie when the same
        // id is browsable through ContentDirectory.
        let response = http_response(
            501,
            "text/plain",
            "Media streaming not implemented by this adapter",
        );
        return writer
            .write_all(response.as_bytes())
            .await
            .map_err(|e| e.to_string());
    };

    // Whole-file read. Music files are typically a few tens of MB. Range
    // support is intentionally deferred (see review §4).
    let bytes = match tokio::fs::read(&src.path).await {
        Ok(b) => b,
        Err(e) => {
            log::warn!("[DLNA Server] media_path {:?} read error: {}", src.path, e);
            let response = http_response(404, "text/plain", "Not Found");
            return writer
                .write_all(response.as_bytes())
                .await
                .map_err(|e| e.to_string());
        }
    };

    let header = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Accept-Ranges: none\r\n\
         transferMode.dlna.org: Streaming\r\n\
         Connection: close\r\n\
         \r\n",
        src.mime_type,
        bytes.len()
    );
    writer
        .write_all(header.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    if method == "GET" {
        writer.write_all(&bytes).await.map_err(|e| e.to_string())?;
    }
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

    let find_arg = |name: &str| -> Option<&str> {
        args.iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };

    match action.as_str() {
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

            let (mut tracks, total) = adapter.search_tracks(query, start, count);
            // Review requirement: defensively enforce the `count` bound
            // on the response side. Adapters are *expected* to clamp,
            // but if they don't we must not blow past `RequestedCount`.
            tracks.truncate(count as usize);
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
            let resp = xml::soap_response(&action, CD_SERVICE, &[("Id", "1")]);
            http_soap_response(&resp)
        }
        "GetSearchCapabilities" => {
            let resp = xml::soap_response(
                &action,
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
                &action,
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

    match action.as_str() {
        "GetProtocolInfo" => {
            let protocols = "http-get:*:audio/flac:*,http-get:*:audio/mpeg:*,http-get:*:audio/wav:*,http-get:*:audio/ogg:*,http-get:*:audio/aac:*";
            let resp =
                xml::soap_response(&action, CM_SERVICE, &[("Source", protocols), ("Sink", "")]);
            http_soap_response(&resp)
        }
        _ => http_soap_fault(401, &format!("Invalid Action: {}", action)),
    }
}

fn http_response(status: u16, content_type: &str, body: &str) -> String {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        501 => "Not Implemented",
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Adapter that misbehaves by ignoring the `count` argument — used to
    /// prove the response-side truncation kicks in.
    struct SearchStub {
        rows: Vec<MediaTrack>,
        total: u32,
    }

    impl MediaServerAdapter for SearchStub {
        fn browse_albums(&self, _start: u32, _count: u32) -> (Vec<MediaAlbum>, u32) {
            (Vec::new(), 0)
        }
        fn browse_album_tracks(&self, _album_id: &str) -> Vec<MediaTrack> {
            Vec::new()
        }
        fn search_tracks(&self, _query: &str, _start: u32, _count: u32) -> (Vec<MediaTrack>, u32) {
            (self.rows.iter().map(clone_track).collect(), self.total)
        }
        fn album_count(&self) -> u32 {
            0
        }
    }

    fn clone_track(t: &MediaTrack) -> MediaTrack {
        MediaTrack {
            id: t.id.clone(),
            album_id: t.album_id.clone(),
            title: t.title.clone(),
            artist: t.artist.clone(),
            album: t.album.clone(),
            genre: t.genre.clone(),
            track_number: t.track_number,
            duration_secs: t.duration_secs,
            file_path: t.file_path.clone(),
            mime_type: t.mime_type.clone(),
            sample_rate: t.sample_rate,
            channels: t.channels,
            bit_depth: t.bit_depth,
            file_size: t.file_size,
        }
    }

    fn make_track(id: &str) -> MediaTrack {
        MediaTrack {
            id: id.to_string(),
            album_id: "a".to_string(),
            title: id.to_string(),
            artist: "x".to_string(),
            album: "x".to_string(),
            genre: None,
            track_number: None,
            duration_secs: None,
            file_path: String::new(),
            mime_type: "audio/flac".to_string(),
            sample_rate: None,
            channels: None,
            bit_depth: None,
            file_size: None,
        }
    }

    /// Review requirement: `Search` must honour `RequestedCount` on the
    /// response side even when the adapter misbehaves.
    #[test]
    fn search_truncates_response_to_requested_count() {
        let stub: Arc<dyn MediaServerAdapter> = Arc::new(SearchStub {
            rows: (0..50).map(|i| make_track(&format!("t{}", i))).collect(),
            total: 1000,
        });
        let soap = r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
  <s:Body>
    <u:Search xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
      <ContainerID>0</ContainerID>
      <SearchCriteria>*</SearchCriteria>
      <Filter>*</Filter>
      <StartingIndex>0</StartingIndex>
      <RequestedCount>5</RequestedCount>
      <SortCriteria></SortCriteria>
    </u:Search>
  </s:Body>
</s:Envelope>"#;
        let resp = handle_content_directory(soap, &stub, "http://1.2.3.4:80");
        assert!(
            resp.contains("<NumberReturned>5</NumberReturned>"),
            "got: {}",
            resp
        );
        assert!(
            resp.contains("<TotalMatches>1000</TotalMatches>"),
            "got: {}",
            resp
        );
    }
}
