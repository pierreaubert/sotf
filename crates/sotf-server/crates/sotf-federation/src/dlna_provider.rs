// ============================================================================
// DLNA Provider
// ============================================================================
//
// A LibraryProvider backed by a DLNA MediaServer.
// Connects via HTTP, uses UPnP ContentDirectory Browse to fetch albums/tracks.
// Audio is streamed via the resource URLs that DLNA provides (standard HTTP).

use crate::provider::{
    LibraryEvent, LibraryProvider, ProviderAlbum, ProviderCapabilities, ProviderError,
    ProviderFuture, ProviderTrack, SourceId, SourceType,
};
use sotf_audio::decoder::AudioSource;
use std::io::{BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Configuration for a DLNA federation source.
#[derive(Debug, Clone)]
pub struct DlnaProviderConfig {
    /// Location URL of the device description XML (from SSDP discovery).
    pub location_url: String,
    /// User-friendly device name.
    pub friendly_name: String,
}

/// A DLNA MediaServer federation provider.
pub struct DlnaProvider {
    source_id: SourceId,
    config: DlnaProviderConfig,
}

impl DlnaProvider {
    pub fn new(source_id: SourceId, config: DlnaProviderConfig) -> Self {
        Self { source_id, config }
    }

    /// Fetch the ContentDirectory control URL from the device description XML.
    fn get_content_directory_url(&self) -> Result<(String, String), ProviderError> {
        let (host, base_url, body) = http_get(&self.config.location_url)?;

        // Parse the device description XML to find the ContentDirectory service control URL
        let control_url = extract_content_directory_control_url(&body)
            .ok_or_else(|| {
                ProviderError::Other(
                    "ContentDirectory service not found in device description".to_string(),
                )
            })?;

        // Resolve relative URL against the device base
        let full_url = if control_url.starts_with("http") {
            control_url
        } else {
            format!("{}{}", base_url, control_url.trim_start_matches('/'))
        };

        Ok((host, full_url))
    }

    /// Send a Browse SOAP request and return the DIDL-Lite XML body.
    fn browse(
        &self,
        control_url: &str,
        object_id: &str,
        start: u32,
        count: u32,
    ) -> Result<String, ProviderError> {
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
<s:Body>
<u:Browse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
<ObjectID>{object_id}</ObjectID>
<BrowseFlag>BrowseDirectChildren</BrowseFlag>
<Filter>*</Filter>
<StartingIndex>{start}</StartingIndex>
<RequestedCount>{count}</RequestedCount>
<SortCriteria></SortCriteria>
</u:Browse>
</s:Body>
</s:Envelope>"#
        );

        let response = soap_post(
            control_url,
            "urn:schemas-upnp-org:service:ContentDirectory:1#Browse",
            &body,
        )?;

        // Extract DIDL-Lite from the SOAP response (it's XML-escaped inside <Result>)
        extract_browse_result(&response).ok_or_else(|| {
            ProviderError::Other("failed to extract Browse result from SOAP response".to_string())
        })
    }

    /// Fetch all albums from the DLNA server (Browse ObjectID=0).
    async fn fetch_all_albums_internal(&self) -> Result<Vec<ProviderAlbum>, ProviderError> {
        let (_host, control_url) = self.get_content_directory_url()?;

        // Browse root to get containers (albums/folders)
        let mut all_albums = Vec::new();
        let mut start = 0u32;
        let page_size = 100u32;

        loop {
            let didl = self.browse(&control_url, "0", start, page_size)?;
            let containers = parse_didl_containers(&didl);

            if containers.is_empty() {
                break;
            }

            let batch_len = containers.len() as u32;

            for (container_id, container_title) in &containers {
                // Browse each container to get its tracks
                let track_didl = self.browse(&control_url, container_id, 0, 10000)?;
                let tracks = parse_didl_items(&track_didl);

                if tracks.is_empty() {
                    continue;
                }

                // Derive artist from tracks
                let artist = tracks
                    .iter()
                    .find_map(|t| t.artist.clone())
                    .unwrap_or_default();

                let provider_tracks: Vec<ProviderTrack> = tracks
                    .into_iter()
                    .map(|t| {
                        let format_hint = mime_to_format_hint(&t.mime_type);
                        ProviderTrack {
                            external_id: t.id.clone(),
                            title: t.title,
                            artist: t.artist.clone(),
                            album_artist: t.artist,
                            track_number: t.track_number,
                            disc_number: None,
                            duration_secs: t.duration_secs,
                            genre: t.genre,
                            composer: None,
                            channels: t.channels,
                            sample_rate: t.sample_rate,
                            bit_depth: t.bit_depth,
                            audio_source: AudioSource::Url {
                                url: t.resource_url,
                                format_hint,
                                seekable: true, // DLNA servers support HTTP Range
                            },
                        }
                    })
                    .collect();

                all_albums.push(ProviderAlbum {
                    external_id: container_id.clone(),
                    title: container_title.clone(),
                    artist,
                    year: None,
                    album_art_url: None,
                    tracks: provider_tracks,
                });
            }

            start += batch_len;
            if batch_len < page_size {
                break;
            }
        }

        Ok(all_albums)
    }
}

impl LibraryProvider for DlnaProvider {
    fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    fn display_name(&self) -> &str {
        &self.config.friendly_name
    }

    fn source_type(&self) -> SourceType {
        SourceType::Dlna
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            writable: false,
            seekable: true, // HTTP Range requests
            offline_available: false,
            supports_events: false,
            has_album_art: false,
        }
    }

    fn fetch_all_albums(&self) -> ProviderFuture<'_, Result<Vec<ProviderAlbum>, ProviderError>> {
        let config = self.config.clone();
        let source_id = self.source_id.clone();
        Box::pin(async move {
            let provider = DlnaProvider::new(source_id, config);
            provider.fetch_all_albums_internal().await
        })
    }

    fn fetch_changes_since(
        &self,
        _since: u64,
    ) -> ProviderFuture<'_, Result<Option<Vec<LibraryEvent>>, ProviderError>> {
        Box::pin(async { Ok(None) })
    }

    fn subscribe_events(&self) -> Option<tokio::sync::broadcast::Receiver<LibraryEvent>> {
        None
    }

    fn resolve_source(
        &self,
        _track_external_id: &str,
    ) -> ProviderFuture<'_, Result<AudioSource, ProviderError>> {
        // DLNA tracks already have their HTTP resource URL stored as AudioSource::Url.
        // No additional resolution needed — the URL is directly streamable.
        Box::pin(async {
            Err(ProviderError::Other(
                "DLNA tracks have direct HTTP URLs; no resolve step needed".to_string(),
            ))
        })
    }

    fn fetch_album_art(
        &self,
        _album_external_id: &str,
    ) -> ProviderFuture<'_, Result<Option<Vec<u8>>, ProviderError>> {
        Box::pin(async { Ok(None) })
    }

    fn is_available(&self) -> ProviderFuture<'_, bool> {
        let url = self.config.location_url.clone();
        Box::pin(async move { http_get(&url).is_ok() })
    }
}

// ─── HTTP helpers ────────────────────────────────────────────────────────────

/// Minimal blocking HTTP GET. Returns (host_header, base_url, body).
fn http_get(url: &str) -> Result<(String, String, String), ProviderError> {
    let (host, port, path) = parse_http_url(url)?;
    let addr = format!("{host}:{port}");

    let mut stream = TcpStream::connect_timeout(
        &addr
            .parse()
            .map_err(|e| ProviderError::Network(format!("invalid address {addr}: {e}")))?,
        Duration::from_secs(5),
    )
    .map_err(|e| ProviderError::Network(format!("connect {addr}: {e}")))?;

    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();

    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| ProviderError::Network(format!("write: {e}")))?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader
        .read_to_string(&mut response)
        .map_err(|e| ProviderError::Network(format!("read: {e}")))?;

    // Split headers and body
    let body_start = response.find("\r\n\r\n").unwrap_or(0) + 4;
    let body = response[body_start..].to_string();
    let base_url = format!("http://{host}:{port}/");
    let host_header = format!("{host}:{port}");

    Ok((host_header, base_url, body))
}

/// Minimal blocking SOAP POST. Returns the response body.
fn soap_post(url: &str, action: &str, body: &str) -> Result<String, ProviderError> {
    let (host, port, path) = parse_http_url(url)?;
    let addr = format!("{host}:{port}");

    let mut stream = TcpStream::connect_timeout(
        &addr
            .parse()
            .map_err(|e| ProviderError::Network(format!("invalid address {addr}: {e}")))?,
        Duration::from_secs(5),
    )
    .map_err(|e| ProviderError::Network(format!("connect {addr}: {e}")))?;

    stream.set_read_timeout(Some(Duration::from_secs(30))).ok();

    let request = format!(
        "POST {path} HTTP/1.1\r\n\
         Host: {host}:{port}\r\n\
         Content-Type: text/xml; charset=\"utf-8\"\r\n\
         SOAPAction: \"{action}\"\r\n\
         Content-Length: {len}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        len = body.len(),
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| ProviderError::Network(format!("write: {e}")))?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader
        .read_to_string(&mut response)
        .map_err(|e| ProviderError::Network(format!("read: {e}")))?;

    let body_start = response.find("\r\n\r\n").unwrap_or(0) + 4;
    Ok(response[body_start..].to_string())
}

fn parse_http_url(url: &str) -> Result<(String, u16, String), ProviderError> {
    let url = url
        .strip_prefix("http://")
        .ok_or_else(|| ProviderError::Network(format!("not an HTTP URL: {url}")))?;
    let (authority, path) = url.split_once('/').unwrap_or((url, ""));
    let path = format!("/{path}");
    let (host, port) = if let Some((h, p)) = authority.rsplit_once(':') {
        (
            h.to_string(),
            p.parse::<u16>()
                .map_err(|_| ProviderError::Network(format!("invalid port: {p}")))?,
        )
    } else {
        (authority.to_string(), 80)
    };
    Ok((host, port, path))
}

// ─── XML parsing helpers ─────────────────────────────────────────────────────

/// Extract the ContentDirectory control URL from a device description XML.
fn extract_content_directory_control_url(xml: &str) -> Option<String> {
    // Find the ContentDirectory service section
    let cd_marker = "ContentDirectory";
    let cd_pos = xml.find(cd_marker)?;
    let after_cd = &xml[cd_pos..];

    // Find <controlURL>...</controlURL> within this service block
    let start_tag = "<controlURL>";
    let end_tag = "</controlURL>";
    let start = after_cd.find(start_tag)? + start_tag.len();
    let end = after_cd[start..].find(end_tag)? + start;

    Some(after_cd[start..end].trim().to_string())
}

/// Extract the Browse Result (DIDL-Lite XML) from a SOAP response.
/// The DIDL is XML-escaped inside `<Result>...</Result>`.
fn extract_browse_result(soap_response: &str) -> Option<String> {
    let start_tag = "<Result>";
    let end_tag = "</Result>";
    let start = soap_response.find(start_tag)? + start_tag.len();
    let end = soap_response[start..].find(end_tag)? + start;

    let escaped = &soap_response[start..end];
    Some(xml_unescape(escaped))
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Parse DIDL-Lite XML to extract containers (albums/folders).
/// Returns Vec of (id, title).
fn parse_didl_containers(didl: &str) -> Vec<(String, String)> {
    let mut containers = Vec::new();
    let mut pos = 0;

    while let Some(start) = didl[pos..].find("<container ") {
        let start = pos + start;
        let Some(end) = didl[start..].find("</container>") else {
            break;
        };
        let end = start + end + "</container>".len();
        let block = &didl[start..end];

        let id = extract_xml_attr(block, "id").unwrap_or_default();
        let title = extract_xml_text(block, "dc:title").unwrap_or_default();

        if !id.is_empty() && !title.is_empty() {
            containers.push((id, title));
        }

        pos = end;
    }

    containers
}

/// Parsed DIDL item (track).
struct ParsedItem {
    id: String,
    title: String,
    artist: Option<String>,
    genre: Option<String>,
    track_number: Option<u32>,
    duration_secs: Option<f64>,
    resource_url: String,
    mime_type: String,
    channels: Option<u32>,
    sample_rate: Option<u32>,
    bit_depth: Option<u32>,
}

/// Parse DIDL-Lite XML to extract items (tracks).
fn parse_didl_items(didl: &str) -> Vec<ParsedItem> {
    let mut items = Vec::new();
    let mut pos = 0;

    while let Some(start) = didl[pos..].find("<item ") {
        let start = pos + start;
        let Some(end) = didl[start..].find("</item>") else {
            break;
        };
        let end = start + end + "</item>".len();
        let block = &didl[start..end];

        let id = extract_xml_attr(block, "id").unwrap_or_default();
        let title = extract_xml_text(block, "dc:title").unwrap_or_default();
        let artist = extract_xml_text(block, "dc:creator")
            .or_else(|| extract_xml_text(block, "upnp:artist"));
        let genre = extract_xml_text(block, "upnp:genre");
        let track_number = extract_xml_text(block, "upnp:originalTrackNumber")
            .and_then(|s| s.parse().ok());

        // Parse <res> element for resource URL and attributes
        let (resource_url, mime_type, duration_secs, channels, sample_rate, bit_depth) =
            parse_res_element(block);

        if !id.is_empty() && !resource_url.is_empty() {
            items.push(ParsedItem {
                id,
                title,
                artist,
                genre,
                track_number,
                duration_secs,
                resource_url,
                mime_type,
                channels,
                sample_rate,
                bit_depth,
            });
        }

        pos = end;
    }

    items
}

/// Parse a `<res protocolInfo="..." duration="..." ...>URL</res>` element.
fn parse_res_element(
    block: &str,
) -> (String, String, Option<f64>, Option<u32>, Option<u32>, Option<u32>) {
    let Some(res_start) = block.find("<res ") else {
        return (String::new(), String::new(), None, None, None, None);
    };
    let Some(res_end) = block[res_start..].find("</res>") else {
        return (String::new(), String::new(), None, None, None, None);
    };
    let res_block = &block[res_start..res_start + res_end + "</res>".len()];

    // URL is between > and </res>
    let url = if let Some(gt) = res_block.find('>') {
        let url_end = res_block.len() - "</res>".len();
        res_block[gt + 1..url_end].trim().to_string()
    } else {
        String::new()
    };

    // Extract MIME from protocolInfo="http-get:*:audio/flac:*"
    let mime = extract_xml_attr(res_block, "protocolInfo")
        .and_then(|pi| {
            let parts: Vec<&str> = pi.split(':').collect();
            if parts.len() >= 3 {
                Some(parts[2].to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "audio/mpeg".to_string());

    // Duration: "HH:MM:SS.mmm"
    let duration = extract_xml_attr(res_block, "duration").and_then(|d| parse_upnp_duration(&d));

    let channels = extract_xml_attr(res_block, "nrAudioChannels").and_then(|s| s.parse().ok());
    let sample_rate =
        extract_xml_attr(res_block, "sampleFrequency").and_then(|s| s.parse().ok());
    let bit_depth = extract_xml_attr(res_block, "bitsPerSample").and_then(|s| s.parse().ok());

    (url, mime, duration, channels, sample_rate, bit_depth)
}

/// Parse UPnP duration "HH:MM:SS.mmm" → seconds.
fn parse_upnp_duration(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: f64 = parts[0].parse().ok()?;
    let m: f64 = parts[1].parse().ok()?;
    let s: f64 = parts[2].parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + s)
}

/// Extract an XML attribute value: `attr="value"` → `value`.
fn extract_xml_attr(xml: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let start = xml.find(&needle)? + needle.len();
    let end = xml[start..].find('"')? + start;
    Some(xml_unescape(&xml[start..end]))
}

/// Extract text content of an XML element: `<tag>text</tag>` → `text`.
fn extract_xml_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    let text = xml[start..end].trim();
    if text.is_empty() {
        None
    } else {
        Some(xml_unescape(text))
    }
}

/// Map MIME type to Symphonia format hint.
fn mime_to_format_hint(mime: &str) -> Option<String> {
    match mime {
        "audio/flac" | "audio/x-flac" => Some("flac".to_string()),
        "audio/mpeg" | "audio/mp3" => Some("mp3".to_string()),
        "audio/ogg" | "audio/x-ogg" => Some("ogg".to_string()),
        "audio/wav" | "audio/x-wav" | "audio/wave" => Some("wav".to_string()),
        "audio/aac" | "audio/x-aac" => Some("aac".to_string()),
        "audio/mp4" | "audio/x-m4a" => Some("m4a".to_string()),
        "audio/aiff" | "audio/x-aiff" => Some("aiff".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_upnp_duration() {
        assert_eq!(parse_upnp_duration("00:06:22.500"), Some(382.5));
        assert_eq!(parse_upnp_duration("01:00:00.000"), Some(3600.0));
        assert_eq!(parse_upnp_duration("invalid"), None);
    }

    #[test]
    fn test_extract_xml_text() {
        let xml = r#"<dc:title>My Song</dc:title>"#;
        assert_eq!(
            extract_xml_text(xml, "dc:title"),
            Some("My Song".to_string())
        );
    }

    #[test]
    fn test_extract_xml_attr() {
        let xml = r#"<container id="album-1" parentID="0">"#;
        assert_eq!(
            extract_xml_attr(xml, "id"),
            Some("album-1".to_string())
        );
        assert_eq!(
            extract_xml_attr(xml, "parentID"),
            Some("0".to_string())
        );
    }

    #[test]
    fn test_parse_didl_containers() {
        let didl = r#"<DIDL-Lite><container id="1" parentID="0" childCount="5"><dc:title>The Wall</dc:title></container><container id="2" parentID="0" childCount="3"><dc:title>OK Computer</dc:title></container></DIDL-Lite>"#;
        let containers = parse_didl_containers(didl);
        assert_eq!(containers.len(), 2);
        assert_eq!(containers[0], ("1".to_string(), "The Wall".to_string()));
        assert_eq!(
            containers[1],
            ("2".to_string(), "OK Computer".to_string())
        );
    }

    #[test]
    fn test_parse_didl_items() {
        let didl = r#"<DIDL-Lite><item id="track-1" parentID="1"><dc:title>Comfortably Numb</dc:title><dc:creator>Pink Floyd</dc:creator><upnp:originalTrackNumber>6</upnp:originalTrackNumber><res protocolInfo="http-get:*:audio/flac:*" duration="00:06:22.500" sampleFrequency="44100" nrAudioChannels="2">http://server/track1.flac</res></item></DIDL-Lite>"#;
        let items = parse_didl_items(didl);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Comfortably Numb");
        assert_eq!(items[0].artist.as_deref(), Some("Pink Floyd"));
        assert_eq!(items[0].track_number, Some(6));
        assert_eq!(items[0].resource_url, "http://server/track1.flac");
        assert_eq!(items[0].mime_type, "audio/flac");
        assert!((items[0].duration_secs.unwrap() - 382.5).abs() < 0.01);
        assert_eq!(items[0].sample_rate, Some(44100));
        assert_eq!(items[0].channels, Some(2));
    }

    #[test]
    fn test_extract_content_directory_url() {
        let xml = r#"<service>
            <serviceType>urn:schemas-upnp-org:service:ContentDirectory:1</serviceType>
            <controlURL>/ContentDirectory/control</controlURL>
        </service>"#;
        assert_eq!(
            extract_content_directory_control_url(xml),
            Some("/ContentDirectory/control".to_string())
        );
    }

    #[test]
    fn test_extract_browse_result() {
        let soap = r#"<BrowseResponse><Result>&lt;DIDL-Lite&gt;&lt;container id="1"&gt;&lt;/container&gt;&lt;/DIDL-Lite&gt;</Result></BrowseResponse>"#;
        let result = extract_browse_result(soap).unwrap();
        assert!(result.contains("<DIDL-Lite>"));
        assert!(result.contains("<container id=\"1\">"));
    }

    #[test]
    fn test_mime_to_format_hint() {
        assert_eq!(
            mime_to_format_hint("audio/flac"),
            Some("flac".to_string())
        );
        assert_eq!(
            mime_to_format_hint("audio/mpeg"),
            Some("mp3".to_string())
        );
        assert_eq!(mime_to_format_hint("unknown/type"), None);
    }
}
