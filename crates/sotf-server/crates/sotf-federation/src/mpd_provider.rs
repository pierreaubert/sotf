// ============================================================================
// MPD Provider
// ============================================================================
//
// A LibraryProvider backed by an MPD (Music Player Daemon) server.
// Connects via TCP and uses the MPD protocol to fetch albums and tracks.

use crate::provider::{
    LibraryEvent, LibraryProvider, ProviderAlbum, ProviderCapabilities, ProviderError,
    ProviderFuture, ProviderTrack, SourceId, SourceType,
};
use sotf_audio::decoder::AudioSource;
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufStream};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// Default timeout for establishing a TCP connection to MPD.
pub const MPD_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Default timeout for reading a response from MPD.
pub const MPD_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Configuration for an MPD federation source.
#[derive(Debug, Clone)]
pub struct MpdProviderConfig {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
    /// Port for MPD's httpd streaming output.
    /// SOTF tells MPD to play a track, then streams from http://host:httpd_port/
    pub httpd_port: u16,
}

/// An MPD (Music Player Daemon) federation provider.
///
/// Connects to a remote MPD server via TCP and fetches the library
/// using the MPD protocol.
pub struct MpdProvider {
    source_id: SourceId,
    config: MpdProviderConfig,
}

impl MpdProvider {
    pub fn new(source_id: SourceId, config: MpdProviderConfig) -> Self {
        Self { source_id, config }
    }

    async fn connect(&self) -> Result<BufStream<TcpStream>, ProviderError> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        // Wrap the connect in tokio::time::timeout so a black-holed host
        // cannot hang this future indefinitely.
        let stream = timeout(MPD_CONNECT_TIMEOUT, TcpStream::connect(&addr))
            .await
            .map_err(|_| {
                ProviderError::Network(format!(
                    "timed out connecting to {} after {:?}",
                    addr, MPD_CONNECT_TIMEOUT
                ))
            })?
            .map_err(|e| ProviderError::Network(format!("failed to connect to {}: {}", addr, e)))?;

        let mut reader = BufStream::new(stream);

        // Read greeting line under the read timeout: a server that completes
        // the TCP handshake but never sends "OK MPD" must not hang us.
        let mut greeting = String::new();
        timeout(MPD_READ_TIMEOUT, reader.read_line(&mut greeting))
            .await
            .map_err(|_| ProviderError::Network("timed out reading greeting".to_string()))?
            .map_err(|e| ProviderError::Network(format!("failed to read greeting: {}", e)))?;

        if !greeting.starts_with("OK MPD") {
            return Err(ProviderError::Network(format!(
                "unexpected MPD greeting: {}",
                greeting.trim()
            )));
        }

        // Send password if configured. Reject embedded control chars first —
        // these could otherwise be used to smuggle in a second MPD command
        // after the closing quote of our argument.
        if let Some(ref password) = self.config.password {
            validate_mpd_token(password)
                .map_err(|e| ProviderError::Auth(format!("invalid password: {}", e)))?;
            self.send_command(
                &mut reader,
                &format!("password \"{}\"", escape_mpd_string(password)),
            )
            .await?;
            let response = self.read_response(&mut reader).await?;
            if response.starts_with("ACK") {
                return Err(ProviderError::Auth("invalid password".to_string()));
            }
        }

        Ok(reader)
    }

    async fn send_command(
        &self,
        reader: &mut BufStream<TcpStream>,
        cmd: &str,
    ) -> Result<(), ProviderError> {
        let mut full_cmd = cmd.to_string();
        full_cmd.push('\n');
        timeout(MPD_READ_TIMEOUT, reader.write_all(full_cmd.as_bytes()))
            .await
            .map_err(|_| ProviderError::Network("timed out sending command".to_string()))?
            .map_err(|e| ProviderError::Network(format!("failed to send command: {}", e)))?;
        timeout(MPD_READ_TIMEOUT, reader.flush())
            .await
            .map_err(|_| ProviderError::Network("timed out flushing command".to_string()))?
            .map_err(|e| ProviderError::Network(format!("failed to flush: {}", e)))?;
        Ok(())
    }

    async fn read_response(
        &self,
        reader: &mut BufStream<TcpStream>,
    ) -> Result<String, ProviderError> {
        let mut response = String::new();
        loop {
            let mut line = String::new();
            let n = timeout(MPD_READ_TIMEOUT, reader.read_line(&mut line))
                .await
                .map_err(|_| ProviderError::Network("timed out reading response".to_string()))?
                .map_err(|e| ProviderError::Network(format!("failed to read response: {}", e)))?;
            if n == 0 {
                return Err(ProviderError::Network(
                    "connection closed unexpectedly".to_string(),
                ));
            }
            if line.trim() == "OK" || line.starts_with("ACK") {
                response.push_str(&line);
                break;
            }
            response.push_str(&line);
        }
        Ok(response)
    }

    async fn fetch_all_albums_internal(&self) -> Result<Vec<ProviderAlbum>, ProviderError> {
        let mut reader = self.connect().await?;

        // Try to get albums with artist grouping first
        self.send_command(&mut reader, "list album group artist")
            .await?;
        let response = self.read_response(&mut reader).await?;

        // Parse album list - returns Vec of (album_name, artist) tuples
        let album_entries = self.parse_album_list_with_artist(&response);

        let mut provider_albums = Vec::new();

        for (album_name, artist) in album_entries {
            // Fetch details for each album using `find`. We constrain by both
            // album AND artist to disambiguate same-named albums by different
            // artists (e.g. multiple "Greatest Hits"). `find album "X"` alone
            // returns every track across every artist sharing that title,
            // which would all be merged into a single bloated ProviderAlbum.
            let cmd = if artist.is_empty() {
                format!("find album \"{}\"", escape_mpd_string(&album_name))
            } else {
                format!(
                    "find album \"{}\" artist \"{}\"",
                    escape_mpd_string(&album_name),
                    escape_mpd_string(&artist)
                )
            };
            self.send_command(&mut reader, &cmd).await?;
            let tracks_response = self.read_response(&mut reader).await?;

            let tracks = self.parse_track_list(&tracks_response);
            if tracks.is_empty() {
                continue;
            }

            // Extract year from first "Date:" field in the tracks response
            let year = tracks_response.lines().find_map(|line| {
                let line = line.trim();
                line.strip_prefix("Date:")
                    .or_else(|| line.strip_prefix("date:"))
                    .and_then(|v| v.trim().get(..4))
                    .and_then(|y| y.parse::<u32>().ok())
            });

            let external_id = format!("{}:{}", artist, album_name);

            provider_albums.push(ProviderAlbum {
                external_id,
                title: album_name,
                artist,
                year,
                album_art_url: None,
                tracks,
            });
        }

        Ok(provider_albums)
    }

    fn parse_album_list_with_artist(&self, response: &str) -> Vec<(String, String)> {
        let mut albums: Vec<(String, String)> = Vec::new();
        let mut current_artist = String::new();

        for line in response.lines() {
            let line = line.trim();
            if line == "OK" || line.is_empty() {
                continue;
            }
            if line.starts_with("ACK") {
                break;
            }
            // MPD "list album group artist" returns Artist: first, then Album: entries under it
            if line.starts_with("Artist:") {
                current_artist = line.trim_start_matches("Artist:").trim().to_string();
            } else if line.starts_with("Album:") {
                let album = line.trim_start_matches("Album:").trim().to_string();
                if !album.is_empty() {
                    let entry = (album, current_artist.clone());
                    if !albums.contains(&entry) {
                        albums.push(entry);
                    }
                }
            }
        }

        albums
    }

    fn parse_track_list(&self, response: &str) -> Vec<ProviderTrack> {
        let mut tracks = Vec::new();
        let mut current_track: HashMap<String, String> = HashMap::new();

        for line in response.lines() {
            let line = line.trim();
            if line == "OK" || line.is_empty() {
                continue;
            }
            if line.starts_with("ACK") {
                break;
            }

            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();

                if key == "file" {
                    // New track starting
                    if !current_track.is_empty()
                        && let Some(track) = self.map_to_provider_track(&current_track)
                    {
                        tracks.push(track);
                    }
                    current_track.clear();
                    current_track.insert("file".to_string(), value.to_string());
                } else {
                    current_track.insert(key.to_string(), value.to_string());
                }
            }
        }

        // Don't forget the last track
        if !current_track.is_empty()
            && let Some(track) = self.map_to_provider_track(&current_track)
        {
            tracks.push(track);
        }

        tracks
    }

    fn map_to_provider_track(&self, mpd_track: &HashMap<String, String>) -> Option<ProviderTrack> {
        let file = mpd_track.get("file")?.clone();

        let title = mpd_track.get("Title").cloned().unwrap_or_else(|| {
            // Derive title from file path
            std::path::Path::new(&file)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        });

        let duration_secs = mpd_track.get("Time").and_then(|t| t.parse::<f64>().ok());

        let track_number = mpd_track.get("Track").and_then(|t| t.parse::<u32>().ok());

        let disc_number = mpd_track.get("Disc").and_then(|d| d.parse::<u32>().ok());

        // Build an mpd-stream:// URL with addressing information only (host,
        // control port, httpd port, file path). The password is intentionally
        // NOT embedded — the decoder retrieves it out-of-band from the
        // MpdProviderConfig keyed by source_id, so the URL alone is always
        // safe to log or display.
        let stream_url = build_mpd_stream_url(
            &self.config.host,
            self.config.port,
            self.config.httpd_port,
            &file,
        );

        Some(ProviderTrack {
            external_id: file.clone(),
            title,
            artist: mpd_track.get("Artist").cloned(),
            album_artist: mpd_track.get("AlbumArtist").cloned(),
            track_number,
            disc_number,
            duration_secs,
            genre: mpd_track.get("Genre").cloned(),
            composer: mpd_track.get("Composer").cloned(),
            channels: None,
            sample_rate: None,
            bit_depth: None,
            audio_source: AudioSource::Url {
                url: stream_url,
                format_hint: None,
                seekable: false,
            },
        })
    }
}

/// Escape a string for use inside an MPD quoted argument.
///
/// MPD's protocol uses a quoted-string syntax inside which `\` and `"` must be
/// backslash-escaped. We *additionally* escape `\n`, `\r`, and `\t`:
///
/// - `\n` is the MPD command terminator. A raw newline inside a quoted
///   string is rejected by MPD outright, but un-escaped concatenation
///   would otherwise allow command injection.
/// - `\r` behaves the same way on many MPD implementations.
/// - `\t` is escaped for hygiene (some MPD versions reject raw tabs inside
///   quoted strings).
///
/// Use [`validate_mpd_token`] when you want to *reject* inputs that contain
/// control characters rather than escape them.
pub fn escape_mpd_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

/// Validate that an MPD token (e.g. a password) does not contain control
/// characters MPD will not accept inside a quoted argument.
///
/// We reject embedded NUL, CR, and LF — any of which could otherwise be used
/// to smuggle in a second MPD command after our closing quote.
pub fn validate_mpd_token(s: &str) -> Result<(), &'static str> {
    if s.contains('\0') {
        return Err("contains NUL byte");
    }
    if s.contains('\n') {
        return Err("contains line feed");
    }
    if s.contains('\r') {
        return Err("contains carriage return");
    }
    Ok(())
}

/// Build the credential-free `mpd-stream://` URL used by the decoder.
///
/// The password is intentionally *not* embedded; callers that need it must
/// retrieve it out of band (e.g. from the `MpdProviderConfig`).
pub fn build_mpd_stream_url(host: &str, port: u16, httpd_port: u16, file: &str) -> String {
    // Percent-encode each path segment so spaces, `#`, `?`, `&`, and non-ASCII
    // characters survive the trip through downstream URL parsers.
    let encoded_path = file
        .split('/')
        .map(percent_encode_segment)
        .collect::<Vec<_>>()
        .join("/");
    format!("mpd-stream://{host}:{port}:{httpd_port}/{encoded_path}")
}

/// Return a copy of `url` with any `?password=...` (or `&password=...`) query
/// parameter stripped out — safe for use in logs and UI.
///
/// Kept exposed even though in-tree code no longer embeds passwords in URLs,
/// to provide defence in depth against accidental future regressions and
/// externally-supplied URLs that happen to carry credentials.
pub fn redact_mpd_url(url: &str) -> String {
    let Some((base, query)) = url.split_once('?') else {
        return url.to_string();
    };
    let filtered: Vec<&str> = query
        .split('&')
        .filter(|kv| {
            let key = kv.split_once('=').map(|(k, _)| k).unwrap_or(kv);
            !key.eq_ignore_ascii_case("password")
        })
        .collect();
    if filtered.is_empty() {
        base.to_string()
    } else {
        format!("{}?{}", base, filtered.join("&"))
    }
}

/// Percent-encode an individual URL path segment (does NOT preserve `/`).
fn percent_encode_segment(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(b as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", b));
            }
        }
    }
    result
}

impl LibraryProvider for MpdProvider {
    fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    fn display_name(&self) -> &str {
        &self.config.host
    }

    fn source_type(&self) -> SourceType {
        SourceType::Mpd
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            writable: false,
            seekable: false, // httpd stream is not seekable
            offline_available: false,
            supports_events: false,
            has_album_art: false,
        }
    }

    fn fetch_all_albums(&self) -> ProviderFuture<'_, Result<Vec<ProviderAlbum>, ProviderError>> {
        let config = self.config.clone();
        let source_id = self.source_id.clone();
        Box::pin(async move {
            let provider = MpdProvider::new(source_id, config);
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
        track_external_id: &str,
    ) -> ProviderFuture<'_, Result<AudioSource, ProviderError>> {
        let config = self.config.clone();
        let file_path = track_external_id.to_string();
        Box::pin(async move {
            // Build mpd-stream:// URL without credentials. The decoder
            // retrieves the password (if any) from MpdProviderConfig via the
            // registry, so the URL alone can be logged safely.
            let url =
                build_mpd_stream_url(&config.host, config.port, config.httpd_port, &file_path);
            Ok(AudioSource::Url {
                url,
                format_hint: None,
                seekable: false,
            })
        })
    }

    fn fetch_album_art(
        &self,
        _album_external_id: &str,
    ) -> ProviderFuture<'_, Result<Option<Vec<u8>>, ProviderError>> {
        Box::pin(async { Ok(None) })
    }

    fn is_available(&self) -> ProviderFuture<'_, bool> {
        let config = self.config.clone();
        Box::pin(async move {
            let addr = format!("{}:{}", config.host, config.port);
            matches!(
                timeout(MPD_CONNECT_TIMEOUT, TcpStream::connect(&addr)).await,
                Ok(Ok(_))
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reverse the rules of `escape_mpd_string` to verify the roundtrip.
    fn unescape_mpd_string(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut iter = s.chars();
        while let Some(c) = iter.next() {
            if c == '\\' {
                match iter.next() {
                    Some('\\') => out.push('\\'),
                    Some('"') => out.push('"'),
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('t') => out.push('\t'),
                    Some(other) => out.push(other),
                    None => out.push('\\'),
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn escape_mpd_string_roundtrip_handles_dangerous_chars() {
        let cases = [
            "plain text",
            "with \"quote\"",
            "with \\backslash",
            "with\nnewline",
            "with\rcarriage",
            "with\ttab",
            "mix \"and\\ \nall\rcombined\t",
            "",
            "unicode: café — ümlaut 日本語",
        ];
        for input in cases {
            let escaped = escape_mpd_string(input);
            assert!(
                !escaped.contains('\n'),
                "escaped string still contained raw LF: {:?} -> {:?}",
                input,
                escaped,
            );
            assert!(
                !escaped.contains('\r'),
                "escaped string still contained raw CR: {:?} -> {:?}",
                input,
                escaped,
            );
            assert_eq!(
                unescape_mpd_string(&escaped),
                input,
                "roundtrip mismatch for {:?}",
                input
            );
        }
    }

    #[test]
    fn escape_mpd_string_escapes_each_special_individually() {
        assert_eq!(escape_mpd_string("\\"), "\\\\");
        assert_eq!(escape_mpd_string("\""), "\\\"");
        assert_eq!(escape_mpd_string("\n"), "\\n");
        assert_eq!(escape_mpd_string("\r"), "\\r");
        assert_eq!(escape_mpd_string("\t"), "\\t");
    }

    #[test]
    fn validate_mpd_token_rejects_control_chars() {
        assert!(validate_mpd_token("good-password_123").is_ok());
        assert!(validate_mpd_token("with space").is_ok());
        assert!(validate_mpd_token("with\nnewline").is_err());
        assert!(validate_mpd_token("with\rcr").is_err());
        assert!(validate_mpd_token("with\0nul").is_err());
    }

    #[test]
    fn build_mpd_stream_url_omits_password_and_encodes_path() {
        let url = build_mpd_stream_url("host.local", 6600, 8000, "My Songs/track?.flac");
        assert!(!url.contains("password"));
        assert_eq!(
            url,
            "mpd-stream://host.local:6600:8000/My%20Songs/track%3F.flac"
        );
    }

    #[test]
    fn redact_mpd_url_strips_password_param() {
        let raw = "mpd-stream://h:6600:8000/x.flac?password=secret&foo=bar";
        let redacted = redact_mpd_url(raw);
        assert!(!redacted.contains("secret"));
        assert!(!redacted.contains("password"));
        assert!(redacted.contains("foo=bar"));

        assert_eq!(
            redact_mpd_url("mpd-stream://h:6600:8000/x.flac?password=secret"),
            "mpd-stream://h:6600:8000/x.flac"
        );

        assert_eq!(
            redact_mpd_url("mpd-stream://h:6600:8000/x.flac"),
            "mpd-stream://h:6600:8000/x.flac"
        );
    }
}
