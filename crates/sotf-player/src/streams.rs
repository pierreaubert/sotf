//! Saved HTTP/SOTF stream business logic.

use serde::{Deserialize, Serialize};
use sotf_audio::decoder::{AudioSource, ServiceId};
use std::net::IpAddr;
use std::path::PathBuf;
use url::Url;

const STREAMS_FILE_NAME: &str = "streams.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedStream {
    pub name: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format_hint: Option<String>,
    #[serde(default)]
    pub seekable: bool,
}

impl SavedStream {
    pub fn new(
        name: impl Into<String>,
        url: impl Into<String>,
        format_hint: Option<String>,
        seekable: bool,
    ) -> Result<Self, StreamValidationError> {
        let stream = Self {
            name: name.into().trim().to_string(),
            url: url.into().trim().to_string(),
            format_hint: normalize_format_hint(format_hint),
            seekable,
        };
        stream.validate()?;
        Ok(stream)
    }

    pub fn validate(&self) -> Result<(), StreamValidationError> {
        if self.name.trim().is_empty() {
            return Err(StreamValidationError::MissingName);
        }
        validate_stream_url(&self.url)
    }

    pub fn audio_source(&self) -> AudioSource {
        if let Some((service, track_id)) = parse_service_stream_reference(&self.url).ok().flatten()
        {
            return AudioSource::ServiceStream { service, track_id };
        }

        AudioSource::Url {
            url: self.url.clone(),
            format_hint: self.format_hint.clone(),
            seekable: self.seekable,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedStreamStore {
    #[serde(default)]
    pub streams: Vec<SavedStream>,
}

impl SavedStreamStore {
    pub fn load_from_path(path: impl Into<PathBuf>) -> Result<Self, StreamStoreError> {
        let path = path.into();
        if !path.exists() {
            return Ok(Self::default());
        }
        crate::security::validate_config_read_path(&path).map_err(StreamStoreError::Security)?;
        let json = std::fs::read_to_string(&path).map_err(StreamStoreError::Io)?;
        serde_json::from_str(&json).map_err(StreamStoreError::Json)
    }

    pub fn save_to_path(&self, path: impl Into<PathBuf>) -> Result<(), StreamStoreError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(StreamStoreError::Io)?;
        }
        crate::security::validate_write_path(&path).map_err(StreamStoreError::Security)?;
        let json = serde_json::to_string_pretty(self).map_err(StreamStoreError::Json)?;
        std::fs::write(path, json).map_err(StreamStoreError::Io)
    }

    pub fn upsert(&mut self, stream: SavedStream) {
        if let Some(existing) = self.streams.iter_mut().find(|s| s.url == stream.url) {
            *existing = stream;
        } else {
            self.streams.push(stream);
        }
        self.streams
            .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    }

    pub fn remove_by_url(&mut self, url: &str) -> bool {
        let before = self.streams.len();
        self.streams.retain(|stream| stream.url != url);
        self.streams.len() != before
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamValidationError {
    MissingName,
    InvalidUrl(String),
    InvalidServiceReference(String),
    UnsupportedScheme(String),
    PublicHttpNotAllowed,
}

impl std::fmt::Display for StreamValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingName => write!(f, "stream name is required"),
            Self::InvalidUrl(err) => write!(f, "invalid stream URL: {err}"),
            Self::InvalidServiceReference(err) => {
                write!(f, "invalid service stream reference: {err}")
            }
            Self::UnsupportedScheme(scheme) => write!(f, "unsupported stream scheme: {scheme}"),
            Self::PublicHttpNotAllowed => write!(f, "public HTTP streams are not allowed"),
        }
    }
}

impl std::error::Error for StreamValidationError {}

#[derive(Debug)]
pub enum StreamStoreError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Security(crate::security::SecurityError),
    MissingConfigDir,
}

impl std::fmt::Display for StreamStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Json(err) => write!(f, "{err}"),
            Self::Security(err) => write!(f, "{err}"),
            Self::MissingConfigDir => write!(f, "could not determine config directory"),
        }
    }
}

impl std::error::Error for StreamStoreError {}

pub fn get_saved_streams_path() -> Option<PathBuf> {
    crate::config::get_app_config_dir().map(|dir| dir.join(STREAMS_FILE_NAME))
}

pub fn load_saved_streams() -> Result<SavedStreamStore, StreamStoreError> {
    let path = get_saved_streams_path().ok_or(StreamStoreError::MissingConfigDir)?;
    SavedStreamStore::load_from_path(path)
}

pub fn save_saved_streams(store: &SavedStreamStore) -> Result<(), StreamStoreError> {
    let path = get_saved_streams_path().ok_or(StreamStoreError::MissingConfigDir)?;
    store.save_to_path(path)
}

pub fn validate_stream_url(url: &str) -> Result<(), StreamValidationError> {
    if parse_service_stream_reference(url)?.is_some() {
        return Ok(());
    }

    let parsed =
        Url::parse(url).map_err(|err| StreamValidationError::InvalidUrl(err.to_string()))?;
    match parsed.scheme() {
        "https" => Ok(()),
        "http" if is_local_http_url(&parsed) => Ok(()),
        "http" => Err(StreamValidationError::PublicHttpNotAllowed),
        other => Err(StreamValidationError::UnsupportedScheme(other.to_string())),
    }
}

pub fn parse_service_stream_reference(
    reference: &str,
) -> Result<Option<(ServiceId, String)>, StreamValidationError> {
    let reference = reference.trim();
    if reference.is_empty() {
        return Ok(None);
    }

    let parsed = match Url::parse(reference) {
        Ok(parsed) => parsed,
        Err(_) => {
            return Ok(None);
        }
    };

    match parsed.scheme() {
        "spotify" => Ok(Some((
            ServiceId::Spotify,
            parse_service_track_id(ServiceId::Spotify, parsed.path())?,
        ))),
        "tidal" => Ok(Some((
            ServiceId::Tidal,
            parse_service_track_id(ServiceId::Tidal, parsed.path())?,
        ))),
        "https" => parse_service_web_track_url(&parsed),
        _ => Ok(None),
    }
}

fn parse_service_web_track_url(
    url: &Url,
) -> Result<Option<(ServiceId, String)>, StreamValidationError> {
    let Some(host) = url.host_str().map(|host| host.to_ascii_lowercase()) else {
        return Ok(None);
    };

    let segments = url
        .path_segments()
        .map(|segments| segments.collect::<Vec<_>>())
        .unwrap_or_default();

    if host == "open.spotify.com" || host.ends_with(".open.spotify.com") {
        if segments.first() == Some(&"track") {
            let track_id = segments.get(1).copied().unwrap_or_default();
            return Ok(Some((
                ServiceId::Spotify,
                parse_service_track_id(ServiceId::Spotify, track_id)?,
            )));
        }
    }

    if host == "tidal.com" || host.ends_with(".tidal.com") {
        if let Some(track_pos) = segments.iter().position(|segment| *segment == "track") {
            let track_id = segments.get(track_pos + 1).copied().unwrap_or_default();
            return Ok(Some((
                ServiceId::Tidal,
                parse_service_track_id(ServiceId::Tidal, track_id)?,
            )));
        }
    }

    Ok(None)
}

fn parse_service_track_id(
    service: ServiceId,
    value: &str,
) -> Result<String, StreamValidationError> {
    let mut track_id = value.trim().trim_start_matches('/').trim();
    if let Some(rest) = track_id.strip_prefix("track:") {
        track_id = rest;
    }
    if let Some(rest) = track_id.strip_prefix("track/") {
        track_id = rest;
    }
    track_id = track_id.split(['?', '#']).next().unwrap_or_default().trim();

    if track_id.is_empty() {
        return Err(StreamValidationError::InvalidServiceReference(format!(
            "{} track id is required",
            service,
        )));
    }
    if !track_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(StreamValidationError::InvalidServiceReference(format!(
            "{} track id contains unsupported characters",
            service,
        )));
    }

    Ok(track_id.to_string())
}

pub fn is_local_http_url(url: &Url) -> bool {
    if url.scheme() != "http" {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_matches(['[', ']']).to_ascii_lowercase();
    if matches!(host.as_str(), "localhost" | "localhost.localdomain") {
        return true;
    }
    if host.ends_with(".local") || host.ends_with(".lan") {
        return true;
    }
    if let Ok(addr) = host.parse::<IpAddr>() {
        return match addr {
            IpAddr::V4(v4) => v4.is_private() || v4.is_loopback() || v4.is_link_local(),
            IpAddr::V6(v6) => {
                v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local()
            }
        };
    }
    false
}

fn normalize_format_hint(format_hint: Option<String>) -> Option<String> {
    format_hint
        .map(|hint| hint.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|hint| !hint.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_stream_builds_audio_source() {
        let stream = SavedStream::new(
            "Radio",
            "https://example.com/live.mp3",
            Some("MP3".into()),
            false,
        )
        .unwrap();

        assert_eq!(
            stream.audio_source(),
            AudioSource::Url {
                url: "https://example.com/live.mp3".into(),
                format_hint: Some("mp3".into()),
                seekable: false,
            }
        );
    }

    #[test]
    fn stream_policy_allows_https_and_local_http() {
        assert!(validate_stream_url("https://example.com/live.mp3").is_ok());
        assert!(validate_stream_url("http://127.0.0.1:8732/api/v1/media/abc").is_ok());
        assert!(validate_stream_url("http://192.168.1.42:8732/api/v1/media/abc").is_ok());
        assert!(validate_stream_url("http://desk.local:8732/api/v1/media/abc").is_ok());
    }

    #[test]
    fn stream_policy_rejects_public_http() {
        assert_eq!(
            validate_stream_url("http://example.com/live.mp3"),
            Err(StreamValidationError::PublicHttpNotAllowed)
        );
    }

    #[test]
    fn stream_store_upsert_replaces_by_url() {
        let mut store = SavedStreamStore::default();
        store.upsert(SavedStream::new("B", "https://example.com/b.mp3", None, false).unwrap());
        store.upsert(
            SavedStream::new("A", "https://example.com/b.mp3", Some("aac".into()), true).unwrap(),
        );

        assert_eq!(store.streams.len(), 1);
        assert_eq!(store.streams[0].name, "A");
        assert_eq!(store.streams[0].format_hint.as_deref(), Some("aac"));
        assert!(store.streams[0].seekable);
    }

    #[test]
    fn saved_stream_builds_spotify_service_source() {
        let stream = SavedStream::new(
            "Spotify Track",
            "spotify:track:4uLU6hMCjMI75M1A2tKUQC",
            None,
            false,
        )
        .unwrap();

        assert_eq!(
            stream.audio_source(),
            AudioSource::ServiceStream {
                service: ServiceId::Spotify,
                track_id: "4uLU6hMCjMI75M1A2tKUQC".into(),
            }
        );
    }

    #[test]
    fn saved_stream_builds_tidal_service_source() {
        let stream = SavedStream::new("Tidal Track", "tidal:track:123456789", None, true).unwrap();

        assert_eq!(
            stream.audio_source(),
            AudioSource::ServiceStream {
                service: ServiceId::Tidal,
                track_id: "123456789".into(),
            }
        );
    }

    #[test]
    fn stream_policy_allows_service_track_links() {
        assert!(validate_stream_url("spotify:track:4uLU6hMCjMI75M1A2tKUQC").is_ok());
        assert!(validate_stream_url("spotify:4uLU6hMCjMI75M1A2tKUQC").is_ok());
        assert!(validate_stream_url("tidal:track:123456789").is_ok());
        assert!(validate_stream_url("tidal:123456789").is_ok());
        assert!(
            validate_stream_url("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC?si=abc")
                .is_ok()
        );
        assert!(validate_stream_url("https://tidal.com/browse/track/123456789").is_ok());
        assert!(validate_stream_url("https://listen.tidal.com/track/123456789").is_ok());
    }

    #[test]
    fn stream_policy_rejects_empty_service_track_ids() {
        assert_eq!(
            validate_stream_url("spotify:track:"),
            Err(StreamValidationError::InvalidServiceReference(
                "Spotify track id is required".into()
            ))
        );
        assert_eq!(
            validate_stream_url("https://tidal.com/browse/track/"),
            Err(StreamValidationError::InvalidServiceReference(
                "Tidal track id is required".into()
            ))
        );
    }
}
