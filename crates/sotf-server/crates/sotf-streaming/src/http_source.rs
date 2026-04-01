use crate::icy::IcyMetadata;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_TYPE, HeaderMap, HeaderValue, RANGE};
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::mpsc;
use std::time::Duration;
use symphonia_core::io::MediaSource;

/// Maximum number of reconnection attempts on network error.
const MAX_RECONNECT_ATTEMPTS: u32 = 5;

/// Initial backoff delay for reconnection (doubles each attempt).
const INITIAL_BACKOFF_MS: u64 = 200;

/// Read-ahead buffer size in bytes (128KB).
const READ_AHEAD_SIZE: usize = 128 * 1024;

/// Stream metadata updates delivered from the HTTP source.
#[derive(Debug, Clone)]
pub enum StreamMetadata {
    /// ICY metadata update (title/url change).
    Icy(IcyMetadata),
    /// Content type detected from HTTP headers.
    ContentType(String),
    /// Bitrate detected from ICY headers (kbps).
    Bitrate(u32),
}

/// HTTP media source that implements Symphonia's `MediaSource` trait.
///
/// Supports:
/// - Seekable HTTP sources via Range requests
/// - Non-seekable streams (internet radio)
/// - ICY metadata extraction for radio streams
/// - Read-ahead buffering for network jitter smoothing
/// - Auto-reconnection with exponential backoff
pub struct HttpMediaSource {
    /// HTTP client (reusable across reconnections).
    client: Client,
    /// The URL being streamed.
    url: String,
    /// Current response body reader.
    reader: Box<dyn Read + Send + Sync>,
    /// Total content length (None for live/infinite streams).
    content_length: Option<u64>,
    /// Current read position in the stream.
    position: u64,
    /// Whether the server supports Range requests.
    seekable: bool,
    /// ICY metadata interval in bytes (0 = no ICY metadata).
    icy_metaint: usize,
    /// Bytes read since last ICY metadata block.
    icy_bytes_since_meta: usize,
    /// Channel to send metadata updates.
    metadata_tx: mpsc::Sender<StreamMetadata>,
    /// Read-ahead buffer.
    buffer: Vec<u8>,
    /// Current read position within the buffer.
    buffer_pos: usize,
    /// Number of valid bytes in the buffer.
    buffer_len: usize,
    /// Content type from HTTP response.
    content_type: Option<String>,
    /// Reusable temporary buffer for fill_buffer (avoids per-call allocation).
    tmp_buf: Vec<u8>,
}

impl HttpMediaSource {
    /// Open an HTTP media source.
    ///
    /// Returns the source and a receiver for stream metadata updates (ICY title changes, etc.).
    pub fn open(url: &str) -> io::Result<(Self, mpsc::Receiver<StreamMetadata>)> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| io::Error::other(e.to_string()))?;

        Self::open_with_client(client, url)
    }

    /// Open an HTTP media source with a pre-configured client.
    pub fn open_with_client(
        client: Client,
        url: &str,
    ) -> io::Result<(Self, mpsc::Receiver<StreamMetadata>)> {
        let (metadata_tx, metadata_rx) = mpsc::channel();

        let mut source = HttpMediaSource {
            client,
            url: url.to_string(),
            reader: Box::new(io::empty()),
            content_length: None,
            position: 0,
            seekable: false,
            icy_metaint: 0,
            icy_bytes_since_meta: 0,
            metadata_tx,
            buffer: vec![0u8; READ_AHEAD_SIZE],
            buffer_pos: 0,
            buffer_len: 0,
            content_type: None,
            tmp_buf: vec![0u8; 8192],
        };

        source.connect(0)?;
        Ok((source, metadata_rx))
    }

    /// Establish (or re-establish) the HTTP connection starting at the given byte offset.
    fn connect(&mut self, offset: u64) -> io::Result<()> {
        let mut request = self
            .client
            .get(&self.url)
            // Request ICY metadata from Icecast/SHOUTcast servers
            .header("Icy-MetaData", "1")
            // Identify ourselves
            .header("User-Agent", "SOTF/1.0");

        if offset > 0 {
            request = request.header(RANGE, format!("bytes={}-", offset));
        }

        let response = request
            .send()
            .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e.to_string()))?;

        if !response.status().is_success() && response.status().as_u16() != 206 {
            return Err(io::Error::other(format!(
                "HTTP {} for {}",
                response.status(),
                self.url
            )));
        }

        let headers = response.headers();

        // Detect seek support from Accept-Ranges or 206 response
        if offset == 0 {
            self.seekable = headers
                .get(ACCEPT_RANGES)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v != "none")
                || response.status().as_u16() == 206;
        }

        // Get content length
        if offset == 0 {
            self.content_length = headers
                .get(CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok());
        }

        // Get content type
        if let Some(ct) = headers
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string())
        {
            self.content_type = Some(ct.clone());
            let _ = self.metadata_tx.send(StreamMetadata::ContentType(ct));
        }

        // ICY metadata interval
        self.icy_metaint = Self::parse_icy_metaint(headers);
        self.icy_bytes_since_meta = 0;

        // ICY bitrate
        if let Some(br) = headers
            .get("icy-br")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
        {
            let _ = self.metadata_tx.send(StreamMetadata::Bitrate(br));
        }

        self.position = offset;
        self.buffer_pos = 0;
        self.buffer_len = 0;
        self.reader = Box::new(response);

        log::info!(
            "[HttpMediaSource] Connected to {} (seekable={}, length={:?}, icy_metaint={})",
            self.url,
            self.seekable,
            self.content_length,
            self.icy_metaint,
        );

        Ok(())
    }

    /// Attempt to reconnect with exponential backoff.
    fn reconnect(&mut self) -> io::Result<()> {
        let offset = self.position;
        let mut delay_ms = INITIAL_BACKOFF_MS;

        for attempt in 1..=MAX_RECONNECT_ATTEMPTS {
            log::warn!(
                "[HttpMediaSource] Reconnection attempt {}/{} at offset {} (backoff {}ms)",
                attempt,
                MAX_RECONNECT_ATTEMPTS,
                offset,
                delay_ms,
            );
            std::thread::sleep(Duration::from_millis(delay_ms));

            match self.connect(offset) {
                Ok(()) => {
                    log::info!("[HttpMediaSource] Reconnected successfully");
                    return Ok(());
                }
                Err(e) => {
                    log::warn!(
                        "[HttpMediaSource] Reconnection attempt {} failed: {}",
                        attempt,
                        e
                    );
                    delay_ms *= 2;
                }
            }
        }

        Err(io::Error::new(
            io::ErrorKind::ConnectionAborted,
            format!(
                "Failed to reconnect after {} attempts",
                MAX_RECONNECT_ATTEMPTS
            ),
        ))
    }

    fn parse_icy_metaint(headers: &HeaderMap<HeaderValue>) -> usize {
        headers
            .get("icy-metaint")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }

    /// Read raw bytes from the HTTP response, stripping ICY metadata if present.
    fn read_raw(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.icy_metaint == 0 {
            // No ICY metadata — straight passthrough
            return self.reader.read(buf);
        }

        // ICY metadata is interleaved: every `icy_metaint` bytes of audio data
        // is followed by a metadata block. The first byte of the metadata block
        // is the length * 16. We need to strip the metadata and only return audio.

        let bytes_until_meta = self.icy_metaint - self.icy_bytes_since_meta;

        if bytes_until_meta == 0 {
            // Time to read a metadata block
            self.read_icy_metadata()?;
            self.icy_bytes_since_meta = 0;
            // Now read audio data
            let max_read = buf.len().min(self.icy_metaint);
            let n = self.reader.read(&mut buf[..max_read])?;
            self.icy_bytes_since_meta = n;
            return Ok(n);
        }

        // Read up to `bytes_until_meta` audio bytes
        let max_read = buf.len().min(bytes_until_meta);
        let n = self.reader.read(&mut buf[..max_read])?;
        self.icy_bytes_since_meta += n;
        Ok(n)
    }

    /// Read and parse an ICY metadata block from the stream.
    fn read_icy_metadata(&mut self) -> io::Result<()> {
        // First byte: length * 16
        let mut len_byte = [0u8; 1];
        self.reader.read_exact(&mut len_byte)?;
        let meta_len = len_byte[0] as usize * 16;

        if meta_len == 0 {
            return Ok(());
        }

        let mut meta_buf = vec![0u8; meta_len];
        self.reader.read_exact(&mut meta_buf)?;

        let metadata = IcyMetadata::parse(&meta_buf);
        log::debug!("[HttpMediaSource] ICY metadata: {:?}", metadata);
        let _ = self.metadata_tx.send(StreamMetadata::Icy(metadata));

        Ok(())
    }

    /// Fill the internal read-ahead buffer from the network.
    fn fill_buffer(&mut self) -> io::Result<()> {
        self.buffer_pos = 0;
        self.buffer_len = 0;

        // Fill the buffer in chunks, handling ICY metadata stripping
        while self.buffer_len < self.buffer.len() {
            let remaining = self.buffer.len() - self.buffer_len;
            // Take tmp_buf out to avoid borrow conflict with read_raw(&mut self)
            let mut tmp = std::mem::take(&mut self.tmp_buf);
            let chunk_size = remaining.min(tmp.len());
            match self.read_raw(&mut tmp[..chunk_size]) {
                Ok(0) => {
                    self.tmp_buf = tmp;
                    break;
                }
                Ok(n) => {
                    self.buffer[self.buffer_len..self.buffer_len + n].copy_from_slice(&tmp[..n]);
                    self.buffer_len += n;
                    self.tmp_buf = tmp;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                    self.tmp_buf = tmp;
                    continue;
                }
                Err(e) => {
                    self.tmp_buf = tmp;
                    return Err(e);
                }
            }
            // Don't block trying to fill the entire buffer — return what we have
            // once we've read at least one chunk
            if self.buffer_len >= chunk_size {
                break;
            }
        }

        Ok(())
    }

    /// The detected content type of the stream (e.g. "audio/mpeg", "audio/flac").
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    /// Whether the source supports seeking (server supports Range requests).
    pub fn is_seekable(&self) -> bool {
        self.seekable
    }

    /// Total content length in bytes, if known.
    pub fn content_length(&self) -> Option<u64> {
        self.content_length
    }

    /// Infer a Symphonia format hint from the URL or content type.
    pub fn format_hint(&self) -> Option<String> {
        // Try URL extension first
        if let Some(hint) = url_extension_hint(&self.url) {
            return Some(hint);
        }
        // Fall back to content type
        if let Some(ct) = &self.content_type {
            return content_type_to_hint(ct);
        }
        None
    }
}

impl Read for HttpMediaSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Serve from buffer first
        if self.buffer_pos < self.buffer_len {
            let available = self.buffer_len - self.buffer_pos;
            let to_copy = buf.len().min(available);
            buf[..to_copy]
                .copy_from_slice(&self.buffer[self.buffer_pos..self.buffer_pos + to_copy]);
            self.buffer_pos += to_copy;
            self.position += to_copy as u64;
            return Ok(to_copy);
        }

        // Buffer exhausted — refill
        match self.fill_buffer() {
            Ok(()) => {}
            Err(e) if is_retriable(&e) => {
                // Network error — try reconnecting
                self.reconnect()?;
                self.fill_buffer()?;
            }
            Err(e) => return Err(e),
        }

        if self.buffer_len == 0 {
            return Ok(0); // EOF
        }

        let to_copy = buf.len().min(self.buffer_len);
        buf[..to_copy].copy_from_slice(&self.buffer[..to_copy]);
        self.buffer_pos = to_copy;
        self.position += to_copy as u64;
        Ok(to_copy)
    }
}

impl Seek for HttpMediaSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        if !self.seekable {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Stream does not support seeking",
            ));
        }

        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::Current(offset) => {
                let current = self.position as i64;
                let new = current + offset;
                if new < 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Seek before start of stream",
                    ));
                }
                new as u64
            }
            SeekFrom::End(offset) => {
                let length = self.content_length.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::Unsupported,
                        "Cannot seek from end: content length unknown",
                    )
                })?;
                let new = length as i64 + offset;
                if new < 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Seek before start of stream",
                    ));
                }
                new as u64
            }
        };

        // Check if the target position is within our current buffer
        let buffer_start = self.position - self.buffer_pos as u64;
        let buffer_end = buffer_start + self.buffer_len as u64;
        if new_pos >= buffer_start && new_pos < buffer_end {
            self.buffer_pos = (new_pos - buffer_start) as usize;
            self.position = new_pos;
            return Ok(new_pos);
        }

        // Need to reconnect at the new position
        self.connect(new_pos)?;
        Ok(new_pos)
    }
}

impl MediaSource for HttpMediaSource {
    fn is_seekable(&self) -> bool {
        self.seekable
    }

    fn byte_len(&self) -> Option<u64> {
        self.content_length
    }
}

/// Extract a format hint from a URL's file extension.
fn url_extension_hint(url: &str) -> Option<String> {
    // Strip query string and fragment
    let path = url.split('?').next()?;
    let path = path.split('#').next()?;
    let ext = path.rsplit('.').next()?;
    let ext = ext.to_lowercase();
    match ext.as_str() {
        "flac" | "mp3" | "ogg" | "oga" | "wav" | "aiff" | "aif" | "aac" | "m4a" | "mp4" => {
            Some(ext)
        }
        _ => None,
    }
}

/// Map an HTTP Content-Type to a Symphonia format hint.
fn content_type_to_hint(content_type: &str) -> Option<String> {
    let ct = content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim();
    match ct {
        "audio/mpeg" | "audio/mp3" => Some("mp3".to_string()),
        "audio/flac" | "audio/x-flac" => Some("flac".to_string()),
        "audio/ogg" | "application/ogg" | "audio/vorbis" => Some("ogg".to_string()),
        "audio/wav" | "audio/x-wav" | "audio/wave" => Some("wav".to_string()),
        "audio/aac" | "audio/aacp" => Some("aac".to_string()),
        "audio/mp4" | "audio/x-m4a" => Some("m4a".to_string()),
        "audio/aiff" | "audio/x-aiff" => Some("aiff".to_string()),
        _ => None,
    }
}

/// Check if an I/O error is likely transient and worth retrying.
fn is_retriable(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::TimedOut
            | io::ErrorKind::UnexpectedEof
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_extension_hint() {
        assert_eq!(
            url_extension_hint("http://example.com/song.flac"),
            Some("flac".to_string())
        );
        assert_eq!(
            url_extension_hint("http://example.com/song.MP3"),
            Some("mp3".to_string())
        );
        assert_eq!(
            url_extension_hint("http://example.com/song.ogg?token=abc"),
            Some("ogg".to_string())
        );
        assert_eq!(url_extension_hint("http://example.com/stream"), None);
        assert_eq!(url_extension_hint("http://example.com/song.xyz"), None);
    }

    #[test]
    fn test_content_type_to_hint() {
        assert_eq!(content_type_to_hint("audio/mpeg"), Some("mp3".to_string()));
        assert_eq!(content_type_to_hint("audio/flac"), Some("flac".to_string()));
        assert_eq!(
            content_type_to_hint("audio/ogg; codecs=vorbis"),
            Some("ogg".to_string())
        );
        assert_eq!(content_type_to_hint("audio/wav"), Some("wav".to_string()));
        assert_eq!(content_type_to_hint("text/html"), None);
    }

    #[test]
    fn test_is_retriable() {
        assert!(is_retriable(&io::Error::new(
            io::ErrorKind::ConnectionReset,
            "reset"
        )));
        assert!(is_retriable(&io::Error::new(
            io::ErrorKind::TimedOut,
            "timeout"
        )));
        assert!(!is_retriable(&io::Error::new(
            io::ErrorKind::NotFound,
            "not found"
        )));
        assert!(!is_retriable(&io::Error::new(
            io::ErrorKind::PermissionDenied,
            "denied"
        )));
    }
}
