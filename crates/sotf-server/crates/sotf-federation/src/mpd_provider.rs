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
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufStream};
use tokio::net::TcpStream;

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
        let stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| ProviderError::Network(format!("failed to connect to {}: {}", addr, e)))?;

        let mut reader = BufStream::new(stream);

        // Read greeting line
        let mut greeting = String::new();
        reader
            .read_line(&mut greeting)
            .await
            .map_err(|e| ProviderError::Network(format!("failed to read greeting: {}", e)))?;

        if !greeting.starts_with("OK MPD") {
            return Err(ProviderError::Network(format!(
                "unexpected MPD greeting: {}",
                greeting.trim()
            )));
        }

        // Send password if configured
        if let Some(ref password) = self.config.password {
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
        reader
            .write_all(full_cmd.as_bytes())
            .await
            .map_err(|e| ProviderError::Network(format!("failed to send command: {}", e)))?;
        reader
            .flush()
            .await
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
            let n = reader
                .read_line(&mut line)
                .await
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
            // Fetch details for each album using find command
            self.send_command(
                &mut reader,
                &format!("find album \"{}\"", escape_mpd_string(&album_name)),
            )
            .await?;
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

        // Build an mpd-stream:// URL that encodes everything the decoder needs:
        // host, control port, httpd port, file path, and optional password.
        let mut stream_url = format!(
            "mpd-stream://{}:{}:{}/{}",
            self.config.host, self.config.port, self.config.httpd_port, &file
        );
        if let Some(ref pw) = self.config.password {
            stream_url.push_str(&format!("?password={}", urlencoding_encode(pw)));
        }

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

fn escape_mpd_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Simple percent-encoding for URL query values.
fn urlencoding_encode(s: &str) -> String {
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
            // Build mpd-stream:// URL — the decoder handles the MPD control + httpd connection
            let mut url = format!(
                "mpd-stream://{}:{}:{}/{}",
                config.host, config.port, config.httpd_port, file_path
            );
            if let Some(ref pw) = config.password {
                url.push_str(&format!("?password={}", urlencoding_encode(pw)));
            }
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
            TcpStream::connect(&addr).await.is_ok()
        })
    }
}
