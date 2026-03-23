// ============================================================================
// LibraryProvider Trait
// ============================================================================
//
// Defines the interface for any source that can contribute albums and tracks
// to the unified library (local files, Subsonic, MPD, DLNA, peers).

use serde::{Deserialize, Serialize};
use sotf_audio::decoder::AudioSource;
use std::future::Future;
use std::pin::Pin;

/// Boxed future returned by async LibraryProvider methods (object-safe).
pub type ProviderFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Unique identifier for a library source instance.
///
/// Examples: `"local"`, `"subsonic:myserver"`, `"mpd:192.168.1.5:6600"`
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceId(pub String);

impl std::fmt::Display for SourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The type of a library source (for database storage and UI display).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceType {
    Local,
    Subsonic,
    Mpd,
    Dlna,
    Peer,
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceType::Local => write!(f, "local"),
            SourceType::Subsonic => write!(f, "subsonic"),
            SourceType::Mpd => write!(f, "mpd"),
            SourceType::Dlna => write!(f, "dlna"),
            SourceType::Peer => write!(f, "peer"),
        }
    }
}

/// Capabilities that vary per provider.
#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    /// Can this source be written to (e.g., edit metadata, delete tracks)?
    pub writable: bool,
    /// Does this source support byte-range seeking during playback?
    pub seekable: bool,
    /// Is content available when the source is offline (local files, cached)?
    pub offline_available: bool,
    /// Can this source push change notifications?
    pub supports_events: bool,
    /// Does this source provide album art?
    pub has_album_art: bool,
}

/// Album metadata as returned by a provider (before merge into unified library).
#[derive(Debug, Clone)]
pub struct ProviderAlbum {
    /// Provider-specific ID (e.g., file path for local, Subsonic album ID, MPD URI).
    pub external_id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<u32>,
    pub album_art_url: Option<String>,
    pub tracks: Vec<ProviderTrack>,
}

/// Track metadata as returned by a provider.
#[derive(Debug, Clone)]
pub struct ProviderTrack {
    /// Provider-specific track ID.
    pub external_id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub duration_secs: Option<f64>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub channels: Option<u32>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    /// How to play this track.
    pub audio_source: AudioSource,
}

/// A change event from a provider (for push-based sync).
#[derive(Debug, Clone)]
pub enum LibraryEvent {
    AlbumAdded(ProviderAlbum),
    AlbumRemoved { album_id: String },
    AlbumUpdated(ProviderAlbum),
    TrackAdded { album_id: String, track: ProviderTrack },
    TrackRemoved { album_id: String, track_id: String },
    /// Provider requests a full re-sync (e.g., database was rebuilt).
    FullSyncRequired,
}

/// Errors from provider operations.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("provider error: {0}")]
    Other(String),
}

/// The core library provider trait.
///
/// Each source (local files, Subsonic, MPD client, DLNA browser, peer)
/// implements this trait to contribute albums/tracks to the unified library.
pub trait LibraryProvider: Send + Sync {
    /// Unique source identifier.
    fn source_id(&self) -> &SourceId;

    /// Human-readable display name.
    fn display_name(&self) -> &str;

    /// Source type for database storage.
    fn source_type(&self) -> SourceType;

    /// Provider capabilities.
    fn capabilities(&self) -> ProviderCapabilities;

    /// Fetch all albums (full sync). Used on first connect or when
    /// incremental sync is not available.
    fn fetch_all_albums(&self) -> ProviderFuture<'_, Result<Vec<ProviderAlbum>, ProviderError>>;

    /// Fetch albums changed since a given timestamp (incremental sync).
    /// Returns `None` if the provider does not support incremental sync,
    /// signaling the caller to do a full `fetch_all_albums()` instead.
    fn fetch_changes_since(
        &self,
        since: u64, // UNIX timestamp
    ) -> ProviderFuture<'_, Result<Option<Vec<LibraryEvent>>, ProviderError>>;

    /// Subscribe to real-time change events. Returns `None` if not supported.
    fn subscribe_events(&self) -> Option<tokio::sync::broadcast::Receiver<LibraryEvent>>;

    /// Resolve a track's AudioSource for playback.
    /// Some providers may need to generate a fresh URL (e.g., signed URLs expire).
    fn resolve_source(
        &self,
        track_external_id: &str,
    ) -> ProviderFuture<'_, Result<AudioSource, ProviderError>>;

    /// Fetch album art bytes for display. Returns `None` if not available.
    fn fetch_album_art(
        &self,
        album_external_id: &str,
    ) -> ProviderFuture<'_, Result<Option<Vec<u8>>, ProviderError>>;

    /// Check if the provider is currently reachable.
    fn is_available(&self) -> ProviderFuture<'_, bool>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_id_display() {
        let id = SourceId("subsonic:myserver".to_string());
        assert_eq!(id.to_string(), "subsonic:myserver");
    }

    #[test]
    fn test_source_type_display() {
        assert_eq!(SourceType::Local.to_string(), "local");
        assert_eq!(SourceType::Subsonic.to_string(), "subsonic");
        assert_eq!(SourceType::Mpd.to_string(), "mpd");
        assert_eq!(SourceType::Dlna.to_string(), "dlna");
        assert_eq!(SourceType::Peer.to_string(), "peer");
    }

    #[test]
    fn test_provider_error_display() {
        let err = ProviderError::Auth("token expired".to_string());
        assert_eq!(err.to_string(), "authentication failed: token expired");

        let err = ProviderError::Network("connection refused".to_string());
        assert_eq!(err.to_string(), "network error: connection refused");
    }
}
