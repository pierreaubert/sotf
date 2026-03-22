// ============================================================================
// StreamingService Trait
// ============================================================================
//
// Abstract interface for streaming service integrations.
// Each service (Spotify, Tidal) implements this trait.

use std::io::Read;

/// Error type for streaming service operations.
#[derive(Debug, Clone)]
pub enum ServiceError {
    /// Authentication failed (bad credentials, expired token, etc.)
    AuthError(String),
    /// Network/API request failed
    NetworkError(String),
    /// Track not found or unavailable
    NotFound(String),
    /// Service-specific error
    Other(String),
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceError::AuthError(msg) => write!(f, "Auth error: {}", msg),
            ServiceError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ServiceError::NotFound(msg) => write!(f, "Not found: {}", msg),
            ServiceError::Other(msg) => write!(f, "Service error: {}", msg),
        }
    }
}

impl std::error::Error for ServiceError {}

/// Credentials for service authentication.
#[derive(Debug, Clone)]
pub enum ServiceCredentials {
    /// Username + password (Spotify)
    UsernamePassword { username: String, password: String },
    /// OAuth2 access token (Tidal)
    AccessToken(String),
    /// OAuth2 device code flow (Tidal) — returns a URL for the user to visit
    DeviceCode,
    /// Cached session from a previous authentication
    CachedSession(Vec<u8>),
}

/// Audio quality preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioQuality {
    /// Low quality (~96 kbps)
    Low,
    /// Normal quality (~160 kbps)
    Normal,
    /// High quality (~320 kbps)
    #[default]
    High,
    /// Lossless (FLAC, ~1411 kbps for CD quality)
    Lossless,
    /// Hi-Res (>44.1kHz/16bit)
    HiRes,
}

/// Metadata for a track from a streaming service.
#[derive(Debug, Clone)]
pub struct ServiceTrack {
    /// Service-specific track ID
    pub id: String,
    /// Track title
    pub title: String,
    /// Artist name
    pub artist: String,
    /// Album name
    pub album: String,
    /// Duration in seconds
    pub duration_secs: f64,
    /// Track number within album
    pub track_number: Option<u32>,
    /// Album art URL
    pub album_art_url: Option<String>,
    /// Available quality levels
    pub available_qualities: Vec<AudioQuality>,
}

/// A stream of raw PCM audio from a service.
///
/// The service decodes the audio internally (e.g. librespot decodes Vorbis)
/// and provides interleaved f32 samples. The engine's decoder thread wraps
/// this in a pass-through AudioDecoder.
pub struct PcmStream {
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u16,
    /// Bits per sample (for metadata only — samples are always f32)
    pub bits_per_sample: u16,
    /// Total frames if known (None for live/infinite streams)
    pub total_frames: Option<u64>,
    /// Reader that produces interleaved f32 PCM samples as raw bytes.
    /// Each sample is 4 bytes (f32 little-endian).
    pub reader: Box<dyn Read + Send>,
}

/// Trait for streaming service integrations.
///
/// Each service crate (spotify, tidal) implements this trait.
/// The engine's decoder thread uses it to obtain audio data for
/// `AudioSource::ServiceStream` sources.
pub trait StreamingService: Send + 'static {
    /// Authenticate with the service.
    ///
    /// For OAuth2 device code flow, this may return `AuthError` with a
    /// URL that the user needs to visit. The caller should display this
    /// URL and retry after the user completes authentication.
    fn authenticate(&mut self, credentials: ServiceCredentials) -> Result<(), ServiceError>;

    /// Check if we have a valid authenticated session.
    fn is_authenticated(&self) -> bool;

    /// Search for tracks matching the query.
    fn search_tracks(&self, query: &str, limit: u32) -> Result<Vec<ServiceTrack>, ServiceError>;

    /// Search for albums matching the query.
    fn search_albums(&self, query: &str, limit: u32) -> Result<Vec<ServiceAlbum>, ServiceError>;

    /// Get tracks for an album.
    fn album_tracks(&self, album_id: &str) -> Result<Vec<ServiceTrack>, ServiceError>;

    /// Start streaming a track. Returns a PCM reader.
    ///
    /// For services that provide direct URLs (Tidal), this may return
    /// a URL string instead — the caller should use `AudioSource::Url`.
    fn start_stream(
        &mut self,
        track_id: &str,
        quality: AudioQuality,
    ) -> Result<ServiceStreamResult, ServiceError>;

    /// Stop the current stream and release resources.
    fn stop_stream(&mut self);

    /// Get the service name (for logging/UI).
    fn service_name(&self) -> &str;
}

/// Result of starting a stream — either raw PCM or a URL to feed to the decoder.
pub enum ServiceStreamResult {
    /// Raw PCM audio stream (e.g. from librespot which decodes internally).
    Pcm(PcmStream),
    /// A URL that can be passed to the HTTP streaming decoder.
    /// Used by Tidal which provides direct FLAC/AAC URLs.
    Url {
        url: String,
        format_hint: Option<String>,
    },
}

/// Album metadata from a streaming service.
#[derive(Debug, Clone)]
pub struct ServiceAlbum {
    /// Service-specific album ID
    pub id: String,
    /// Album title
    pub title: String,
    /// Artist name
    pub artist: String,
    /// Release year
    pub year: Option<u32>,
    /// Number of tracks
    pub track_count: u32,
    /// Album art URL
    pub album_art_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_error_display() {
        let err = ServiceError::AuthError("token expired".to_string());
        assert_eq!(err.to_string(), "Auth error: token expired");

        let err = ServiceError::NotFound("track123".to_string());
        assert_eq!(err.to_string(), "Not found: track123");
    }

    #[test]
    fn test_audio_quality_default() {
        assert_eq!(AudioQuality::default(), AudioQuality::High);
    }

    #[test]
    fn test_service_track_fields() {
        let track = ServiceTrack {
            id: "abc123".to_string(),
            title: "Comfortably Numb".to_string(),
            artist: "Pink Floyd".to_string(),
            album: "The Wall".to_string(),
            duration_secs: 382.0,
            track_number: Some(6),
            album_art_url: None,
            available_qualities: vec![AudioQuality::High, AudioQuality::Lossless],
        };
        assert_eq!(track.duration_secs, 382.0);
        assert_eq!(track.available_qualities.len(), 2);
    }
}
