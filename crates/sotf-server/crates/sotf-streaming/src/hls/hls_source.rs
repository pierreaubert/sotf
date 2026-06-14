use super::consts::MAX_PLAYLIST_BYTES;
use super::consts::MAX_SEGMENT_BYTES;
use super::fetch::fetch_segment;
use super::fetch::fetch_text;
use super::hls_segment::HlsSegment;
use super::misc::segment_format_hint;
use super::parse::parse_media_playlist;
use super::resolve::resolve_playlist;
use reqwest::blocking::Client;
use std::collections::HashSet;
use std::io::{self, Read, Seek, SeekFrom};
use std::thread;
use std::time::Duration;
use symphonia_core::io::MediaSource;
use url::Url;

/// HLS media source that exposes playlist segments as one continuous byte stream.
///
/// The source intentionally reports itself as non-seekable. HLS seeking should be
/// implemented at segment/playlist time boundaries, not as byte seeking over an
/// evolving concatenation of media objects.
pub struct HlsSource {
    pub(super) client: Client,
    pub(super) playlist_url: Url,
    pub(super) segments: Vec<HlsSegment>,
    pub(super) seen_segments: HashSet<String>,
    pub(super) next_segment_index: usize,
    pub(super) current_segment: Vec<u8>,
    pub(super) current_segment_pos: usize,
    pub(super) end_list: bool,
    pub(super) target_duration: Duration,
    pub(super) total_bytes_read: u64,
    pub(super) format_hint: Option<String>,
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

    pub(super) fn load_next_segment(&mut self) -> io::Result<bool> {
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

    pub(super) fn refresh_playlist(&mut self) -> io::Result<()> {
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
