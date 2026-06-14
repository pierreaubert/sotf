use super::http::http_response;
use super::http::http_soap_fault;
use super::http::http_soap_response;
use super::media_server_adapter::MediaServerAdapter;
use super::misc::CD_SERVICE;
use super::misc::CM_SERVICE;
use super::misc::parse_range_header;
use super::misc::stream_file_range;
use crate::device::DlnaDevice;
use crate::didl::{self, DidlContainer, DidlItem};
use crate::http_io;
use crate::xml;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, BufReader};

pub(super) async fn handle_server_request(
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
            let range = req
                .headers
                .iter()
                .find(|(name, _)| name == "range")
                .map(|(_, value)| value.as_str());
            handle_media(&mut writer, method, path, range, adapter).await?;
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

pub(super) fn compute_body_len(file_len: u64, start: u64, end: u64) -> u64 {
    if file_len == 0 {
        0
    } else {
        end.saturating_sub(start).saturating_add(1)
    }
}

pub(super) async fn handle_media(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    method: &str,
    path: &str,
    range_header: Option<&str>,
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

    let metadata = match tokio::fs::metadata(&src.path).await {
        Ok(m) if m.is_file() => m,
        Err(e) => {
            log::warn!(
                "[DLNA Server] media_path {:?} metadata error: {}",
                src.path,
                e
            );
            let response = http_response(404, "text/plain", "Not Found");
            return writer
                .write_all(response.as_bytes())
                .await
                .map_err(|e| e.to_string());
        }
        Ok(_) => {
            let response = http_response(404, "text/plain", "Not Found");
            return writer
                .write_all(response.as_bytes())
                .await
                .map_err(|e| e.to_string());
        }
    };
    let file_len = metadata.len();

    let range = match parse_range_header(range_header, file_len) {
        Ok(range) => range,
        Err(()) => {
            let header = format!(
                "HTTP/1.1 416 Range Not Satisfiable\r\n\
                 Content-Range: bytes */{}\r\n\
                 Content-Length: 0\r\n\
                 Accept-Ranges: bytes\r\n\
                 Connection: close\r\n\
                 \r\n",
                file_len
            );
            return writer
                .write_all(header.as_bytes())
                .await
                .map_err(|e| e.to_string());
        }
    };

    let (status, status_text, start, end) = match range {
        Some((start, end)) => (206, "Partial Content", start, end),
        None => {
            if file_len == 0 {
                (200, "OK", 0, 0)
            } else {
                (200, "OK", 0, file_len - 1)
            }
        }
    };
    let body_len = compute_body_len(file_len, start, end);

    let content_range = if status == 206 {
        format!("Content-Range: bytes {}-{}/{}\r\n", start, end, file_len)
    } else {
        String::new()
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Accept-Ranges: bytes\r\n\
         {}\
         transferMode.dlna.org: Streaming\r\n\
         Connection: close\r\n\
         \r\n",
        status, status_text, src.mime_type, body_len, content_range
    );
    writer
        .write_all(header.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    if method == "GET" && body_len > 0 {
        stream_file_range(writer, &src.path, start, body_len).await?;
    }
    Ok(())
}

pub(super) fn handle_content_directory(
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

pub(super) fn handle_cm_action(body: &str) -> String {
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
