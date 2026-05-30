use reqwest::blocking::Client;
use reqwest::header::RANGE;
use std::collections::HashSet;
use std::io::{self, Read, Seek, SeekFrom};
use std::thread;
use std::time::Duration;
use symphonia_core::io::MediaSource;
use url::Url;

const USER_AGENT: &str = "SOTF/1.0";
const DEFAULT_TARGET_DURATION: Duration = Duration::from_secs(4);
const MAX_PLAYLIST_BYTES: usize = 2 * 1024 * 1024;
const MAX_SEGMENT_BYTES: usize = 64 * 1024 * 1024;

/// HLS media source that exposes playlist segments as one continuous byte stream.
///
/// The source intentionally reports itself as non-seekable. HLS seeking should be
/// implemented at segment/playlist time boundaries, not as byte seeking over an
/// evolving concatenation of media objects.
pub struct HlsSource {
    client: Client,
    playlist_url: Url,
    segments: Vec<HlsSegment>,
    seen_segments: HashSet<String>,
    next_segment_index: usize,
    current_segment: Vec<u8>,
    current_segment_pos: usize,
    end_list: bool,
    target_duration: Duration,
    total_bytes_read: u64,
    format_hint: Option<String>,
}

impl HlsSource {
    /// Open an HLS playlist URL.
    pub fn open(url: &str) -> io::Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| io::Error::other(e.to_string()))?;
        Self::open_with_client(client, url)
    }

    /// Open with a preconfigured HTTP client.
    pub fn open_with_client(client: Client, url: &str) -> io::Result<Self> {
        let playlist_url = Url::parse(url).map_err(|e| io::Error::other(e.to_string()))?;
        let playlist_text = fetch_text(&client, &playlist_url, MAX_PLAYLIST_BYTES)?;
        let resolved = resolve_playlist(&client, &playlist_url, &playlist_text)?;
        let format_hint = resolved
            .segments
            .first()
            .and_then(|segment| segment_format_hint(&segment.url));

        Ok(Self {
            client,
            playlist_url: resolved.playlist_url,
            seen_segments: resolved.segments.iter().map(HlsSegment::key).collect(),
            segments: resolved.segments,
            next_segment_index: 0,
            current_segment: Vec::new(),
            current_segment_pos: 0,
            end_list: resolved.end_list,
            target_duration: resolved.target_duration,
            total_bytes_read: 0,
            format_hint,
        })
    }

    /// Best-effort format hint for the media segments.
    pub fn format_hint(&self) -> Option<String> {
        self.format_hint.clone()
    }

    fn load_next_segment(&mut self) -> io::Result<bool> {
        loop {
            if self.next_segment_index < self.segments.len() {
                let segment = self.segments[self.next_segment_index].clone();
                self.next_segment_index += 1;
                self.current_segment = fetch_segment(&self.client, &segment, MAX_SEGMENT_BYTES)?;
                self.current_segment_pos = 0;
                if self.format_hint.is_none() {
                    self.format_hint = segment_format_hint(&segment.url);
                }
                return Ok(true);
            }

            if self.end_list {
                return Ok(false);
            }

            thread::sleep(self.target_duration.min(Duration::from_secs(2)));
            self.refresh_playlist()?;
        }
    }

    fn refresh_playlist(&mut self) -> io::Result<()> {
        let playlist_text = fetch_text(&self.client, &self.playlist_url, MAX_PLAYLIST_BYTES)?;
        let parsed = parse_media_playlist(&self.playlist_url, &playlist_text)?;
        self.end_list = parsed.end_list;
        self.target_duration = parsed.target_duration;

        for segment in parsed.segments {
            if self.seen_segments.insert(segment.key()) {
                self.segments.push(segment);
            }
        }
        Ok(())
    }
}

impl Read for HlsSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let mut written = 0;
        while written < buf.len() {
            if self.current_segment_pos >= self.current_segment.len() {
                if written > 0 && !self.end_list && self.next_segment_index >= self.segments.len() {
                    break;
                }
                if !self.load_next_segment()? {
                    break;
                }
                if self.current_segment.is_empty() {
                    continue;
                }
            }

            let remaining_segment = self.current_segment.len() - self.current_segment_pos;
            let remaining_output = buf.len() - written;
            let n = remaining_segment.min(remaining_output);
            buf[written..written + n].copy_from_slice(
                &self.current_segment[self.current_segment_pos..self.current_segment_pos + n],
            );
            self.current_segment_pos += n;
            written += n;
        }

        self.total_bytes_read += written as u64;
        Ok(written)
    }
}

impl Seek for HlsSource {
    fn seek(&mut self, _pos: SeekFrom) -> io::Result<u64> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "HLS byte seeking is not supported",
        ))
    }
}

impl MediaSource for HlsSource {
    fn is_seekable(&self) -> bool {
        false
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HlsByteRange {
    offset: u64,
    length: u64,
}

impl HlsByteRange {
    fn end_exclusive(self) -> io::Result<u64> {
        self.offset
            .checked_add(self.length)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "HLS byte range overflow"))
    }

    fn header_value(self) -> io::Result<String> {
        let end = self
            .end_exclusive()?
            .checked_sub(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty HLS byte range"))?;
        Ok(format!("bytes={}-{}", self.offset, end))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingByteRange {
    length: u64,
    offset: Option<u64>,
}

#[derive(Clone, Debug)]
struct HlsSegment {
    url: Url,
    byte_range: Option<HlsByteRange>,
}

impl HlsSegment {
    fn new(url: Url, byte_range: Option<HlsByteRange>) -> Self {
        Self { url, byte_range }
    }

    fn key(&self) -> String {
        match self.byte_range {
            Some(range) => format!("{}#{}+{}", self.url, range.offset, range.length),
            None => self.url.as_str().to_string(),
        }
    }
}

#[derive(Debug)]
struct ResolvedPlaylist {
    playlist_url: Url,
    segments: Vec<HlsSegment>,
    end_list: bool,
    target_duration: Duration,
}

fn resolve_playlist(
    client: &Client,
    playlist_url: &Url,
    playlist_text: &str,
) -> io::Result<ResolvedPlaylist> {
    if let Some(variant) = parse_master_playlist(playlist_url, playlist_text)? {
        let variant_text = fetch_text(client, &variant, MAX_PLAYLIST_BYTES)?;
        parse_media_playlist(&variant, &variant_text)
    } else {
        parse_media_playlist(playlist_url, playlist_text)
    }
}

fn parse_master_playlist(base_url: &Url, playlist: &str) -> io::Result<Option<Url>> {
    if let Ok(m3u8_rs::Playlist::MasterPlaylist(master)) =
        m3u8_rs::parse_playlist_res(playlist.as_bytes())
    {
        return master
            .variants
            .iter()
            .filter(|variant| !variant.is_i_frame)
            .max_by_key(|variant| variant.bandwidth)
            .map(|variant| {
                base_url
                    .join(&variant.uri)
                    .map_err(|e| io::Error::other(e.to_string()))
            })
            .transpose();
    }

    let mut best: Option<(u64, Url)> = None;
    let mut pending_bandwidth: Option<u64> = None;

    for raw_line in playlist.lines() {
        let line = raw_line.trim();
        if line.starts_with("#EXT-X-STREAM-INF:") {
            pending_bandwidth = parse_attribute(line, "BANDWIDTH")
                .and_then(|value| value.parse::<u64>().ok())
                .or(Some(0));
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(bandwidth) = pending_bandwidth.take() {
            let url = base_url
                .join(line)
                .map_err(|e| io::Error::other(e.to_string()))?;
            if best
                .as_ref()
                .is_none_or(|(best_bw, _)| bandwidth > *best_bw)
            {
                best = Some((bandwidth, url));
            }
        }
    }

    Ok(best.map(|(_, url)| url))
}

fn parse_media_playlist(base_url: &Url, playlist: &str) -> io::Result<ResolvedPlaylist> {
    let mut segments = Vec::new();
    let mut end_list = false;
    let mut target_duration = DEFAULT_TARGET_DURATION;
    let mut current_map: Option<HlsSegment> = None;
    let mut emitted_maps = HashSet::new();
    let mut pending_byte_range: Option<PendingByteRange> = None;
    let mut last_byte_range_end: Option<u64> = None;
    let mut last_map_byte_range_end: Option<u64> = None;
    let mut encrypted_segments = false;

    for raw_line in playlist.lines() {
        let line = raw_line.trim();
        if let Some(value) = line.strip_prefix("#EXT-X-TARGETDURATION:") {
            if let Ok(seconds) = value.trim().parse::<u64>() {
                target_duration = Duration::from_secs(seconds.max(1));
            }
            continue;
        }
        if line == "#EXT-X-ENDLIST" {
            end_list = true;
            continue;
        }
        if line.starts_with("#EXT-X-KEY:") {
            encrypted_segments = parse_attribute(line, "METHOD")
                .is_some_and(|method| !method.eq_ignore_ascii_case("NONE"));
            continue;
        }
        if let Some(value) = line.strip_prefix("#EXT-X-BYTERANGE:") {
            pending_byte_range = Some(parse_byte_range(value)?);
            continue;
        }
        if line.starts_with("#EXT-X-MAP:") {
            let uri = parse_attribute(line, "URI").ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "HLS EXT-X-MAP missing URI")
            })?;
            let byte_range = parse_attribute(line, "BYTERANGE")
                .map(parse_byte_range)
                .transpose()?
                .map(|range| resolve_byte_range(range, &mut last_map_byte_range_end))
                .transpose()?;
            let url = base_url
                .join(uri)
                .map_err(|e| io::Error::other(e.to_string()))?;
            current_map = Some(HlsSegment::new(url, byte_range));
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if encrypted_segments {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "HLS encrypted media segments are not supported",
            ));
        }

        if let Some(map) = current_map.clone()
            && emitted_maps.insert(map.key())
        {
            segments.push(map);
        }

        let byte_range = match pending_byte_range.take() {
            Some(range) => Some(resolve_byte_range(range, &mut last_byte_range_end)?),
            None => {
                last_byte_range_end = None;
                None
            }
        };
        let url = base_url
            .join(line)
            .map_err(|e| io::Error::other(e.to_string()))?;
        segments.push(HlsSegment::new(url, byte_range));
    }

    if segments.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HLS media playlist contains no segments",
        ));
    }

    Ok(ResolvedPlaylist {
        playlist_url: base_url.clone(),
        segments,
        end_list,
        target_duration,
    })
}

fn parse_attribute<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let attrs = line.split_once(':')?.1;
    let mut start = 0;
    let mut in_quotes = false;

    for (idx, ch) in attrs.char_indices() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                if let Some(value) = parse_attribute_pair(&attrs[start..idx], name) {
                    return Some(value);
                }
                start = idx + 1;
            }
            _ => {}
        }
    }

    parse_attribute_pair(&attrs[start..], name)
}

fn parse_attribute_pair<'a>(attr: &'a str, name: &str) -> Option<&'a str> {
    let (key, value) = attr.split_once('=')?;
    if key.trim() == name {
        Some(value.trim().trim_matches('"'))
    } else {
        None
    }
}

fn parse_byte_range(value: &str) -> io::Result<PendingByteRange> {
    let value = value.trim().trim_matches('"');
    let (length, offset) = match value.split_once('@') {
        Some((length, offset)) => (length, Some(offset)),
        None => (value, None),
    };
    let length = length.trim().parse::<u64>().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid HLS byte range length: {}", e),
        )
    })?;
    if length == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HLS byte range length must be greater than zero",
        ));
    }
    let offset = offset
        .map(|offset| {
            offset.trim().parse::<u64>().map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid HLS byte range offset: {}", e),
                )
            })
        })
        .transpose()?;

    Ok(PendingByteRange { length, offset })
}

fn resolve_byte_range(
    pending: PendingByteRange,
    last_end: &mut Option<u64>,
) -> io::Result<HlsByteRange> {
    let offset = pending.offset.unwrap_or_else(|| last_end.unwrap_or(0));
    let range = HlsByteRange {
        offset,
        length: pending.length,
    };
    *last_end = Some(range.end_exclusive()?);
    Ok(range)
}

fn segment_format_hint(url: &Url) -> Option<String> {
    let path = url.path().to_ascii_lowercase();
    let ext = path.rsplit('.').next()?;
    match ext {
        "aac" => Some("aac".to_string()),
        "mp3" => Some("mp3".to_string()),
        "m4a" | "m4s" | "mp4" => Some("mp4".to_string()),
        "wav" => Some("wav".to_string()),
        "flac" => Some("flac".to_string()),
        "ogg" | "oga" => Some("ogg".to_string()),
        _ => None,
    }
}

fn fetch_text(client: &Client, url: &Url, max_bytes: usize) -> io::Result<String> {
    let bytes = fetch_bytes(client, url, None, max_bytes)?;
    String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}

fn fetch_segment(client: &Client, segment: &HlsSegment, max_bytes: usize) -> io::Result<Vec<u8>> {
    fetch_bytes(client, &segment.url, segment.byte_range, max_bytes)
}

fn fetch_bytes(
    client: &Client,
    url: &Url,
    byte_range: Option<HlsByteRange>,
    max_bytes: usize,
) -> io::Result<Vec<u8>> {
    let mut request = client.get(url.clone()).header("User-Agent", USER_AGENT);
    if let Some(range) = byte_range {
        request = request.header(RANGE, range.header_value()?);
    }

    let response = request
        .send()
        .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e.to_string()))?;

    if !response.status().is_success() {
        return Err(io::Error::other(format!(
            "HTTP {} for {}",
            response.status(),
            url
        )));
    }

    let bytes = response
        .bytes()
        .map_err(|e| io::Error::other(e.to_string()))?;
    if bytes.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("HTTP body from {} exceeded {} bytes", url, max_bytes),
        ));
    }
    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn base_url(path: &str) -> Url {
        Url::parse(&format!("http://example.test/{}", path)).unwrap()
    }

    #[test]
    fn parses_media_playlist_with_relative_segments() {
        let playlist = "#EXTM3U\n#EXT-X-TARGETDURATION:6\n#EXTINF:6,\nseg0.aac\n#EXTINF:6,\nsub/seg1.aac\n#EXT-X-ENDLIST\n";
        let parsed = parse_media_playlist(&base_url("hls/live/index.m3u8"), playlist).unwrap();

        assert!(parsed.end_list);
        assert_eq!(parsed.target_duration, Duration::from_secs(6));
        assert_eq!(parsed.segments.len(), 2);
        assert_eq!(
            parsed.segments[0].url.as_str(),
            "http://example.test/hls/live/seg0.aac"
        );
        assert_eq!(
            parsed.segments[1].url.as_str(),
            "http://example.test/hls/live/sub/seg1.aac"
        );
    }

    #[test]
    fn parses_media_playlist_with_init_map_and_byte_ranges() {
        let playlist = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MAP:URI=\"init.mp4\",BYTERANGE=\"720@0\"\n#EXTINF:4,\n#EXT-X-BYTERANGE:1000@720\ntrack.m4s\n#EXTINF:4,\n#EXT-X-BYTERANGE:900\ntrack.m4s\n#EXT-X-ENDLIST\n";
        let parsed = parse_media_playlist(&base_url("hls/fmp4/index.m3u8"), playlist).unwrap();

        assert_eq!(parsed.segments.len(), 3);
        assert_eq!(
            parsed.segments[0].url.as_str(),
            "http://example.test/hls/fmp4/init.mp4"
        );
        assert_eq!(
            parsed.segments[0].byte_range,
            Some(HlsByteRange {
                offset: 0,
                length: 720
            })
        );
        assert_eq!(
            parsed.segments[1].byte_range,
            Some(HlsByteRange {
                offset: 720,
                length: 1000
            })
        );
        assert_eq!(
            parsed.segments[2].byte_range,
            Some(HlsByteRange {
                offset: 1720,
                length: 900
            })
        );
    }

    #[test]
    fn rejects_encrypted_media_segments() {
        let playlist = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\"\n#EXTINF:4,\nseg0.aac\n";
        let err = parse_media_playlist(&base_url("hls/live/index.m3u8"), playlist).unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
    }

    #[test]
    fn parses_master_playlist_by_highest_bandwidth() {
        let playlist = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=64000\nlo/index.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=192000\nhi/index.m3u8\n";
        let selected = parse_master_playlist(&base_url("root/master.m3u8"), playlist)
            .unwrap()
            .unwrap();

        assert_eq!(selected.as_str(), "http://example.test/root/hi/index.m3u8");
    }

    #[test]
    fn hls_source_reads_playlist_segments_in_order() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        let requests_for_thread = Arc::clone(&requests);

        let server = std::thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                handle_test_request(&mut stream);
                requests_for_thread.fetch_add(1, Ordering::SeqCst);
            }
        });

        let url = format!("http://{}/playlist.m3u8", addr);
        let mut source = HlsSource::open(&url).unwrap();
        let mut bytes = Vec::new();
        source.read_to_end(&mut bytes).unwrap();

        assert_eq!(bytes, b"helloworld");
        assert_eq!(source.format_hint(), Some("aac".to_string()));
        server.join().unwrap();
        assert_eq!(requests.load(Ordering::SeqCst), 3);
    }

    fn handle_test_request(stream: &mut TcpStream) {
        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).unwrap();
        let request = String::from_utf8_lossy(&buf[..n]);
        let (status, content_type, body) = if request.contains("GET /playlist.m3u8") {
            (
                "200 OK",
                "application/vnd.apple.mpegurl",
                "#EXTM3U\n#EXT-X-TARGETDURATION:1\n#EXTINF:1,\nseg0.aac\n#EXTINF:1,\nseg1.aac\n#EXT-X-ENDLIST\n"
                    .as_bytes()
                    .to_vec(),
            )
        } else if request.contains("GET /seg0.aac") {
            ("200 OK", "audio/aac", b"hello".to_vec())
        } else if request.contains("GET /seg1.aac") {
            ("200 OK", "audio/aac", b"world".to_vec())
        } else {
            ("404 Not Found", "text/plain", b"missing".to_vec())
        };

        let header = format!(
            "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            status,
            content_type,
            body.len()
        );
        stream.write_all(header.as_bytes()).unwrap();
        stream.write_all(&body).unwrap();
    }
}
