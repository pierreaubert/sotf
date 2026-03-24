//! Audio source identification types.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

/// Identifies a streaming service that provides raw PCM audio directly
/// (bypassing Symphonia decoding).
/// PartialEq is derived to support direct comparison (e.g., gapless transition detection).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServiceId {
    Spotify,
    Tidal,
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceId::Spotify => write!(f, "Spotify"),
            ServiceId::Tidal => write!(f, "Tidal"),
        }
    }
}

/// Identifies where audio data comes from.
///
/// Replaces raw `PathBuf` at every interface boundary in the engine,
/// enabling the decoder thread to handle files, HTTP streams, and
/// service-managed audio sources uniformly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AudioSource {
    /// Local file (existing behavior).
    File(PathBuf),

    /// HTTP(S) URL — internet radio, Subsonic streams, Tidal direct URLs.
    Url {
        url: String,
        /// Hint for Symphonia format detection (e.g., "mp3", "flac").
        format_hint: Option<String>,
        /// Whether the source supports seeking (false for live radio).
        seekable: bool,
    },

    /// Service-managed stream — the service crate provides raw PCM directly.
    /// Used by librespot (Spotify) which decodes internally.
    ServiceStream {
        service: ServiceId,
        track_id: String,
    },

    /// Silent / driver source (existing HAL mode).
    Driver,
}

impl AudioSource {
    /// A short human-readable name for display in logs and UI.
    pub fn display_name(&self) -> String {
        match self {
            AudioSource::File(path) => path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string()),
            AudioSource::Url { url, .. } => url.clone(),
            AudioSource::ServiceStream { service, track_id } => {
                format!("{}:{}", service, track_id)
            }
            AudioSource::Driver => "driver".to_string(),
        }
    }

    /// Returns the local file path if this is a `File` source.
    pub fn as_path(&self) -> Option<&Path> {
        match self {
            AudioSource::File(path) => Some(path),
            _ => None,
        }
    }

    /// Returns `true` if seeking is supported for this source.
    pub fn is_seekable(&self) -> bool {
        match self {
            AudioSource::File(_) => true,
            AudioSource::Url { seekable, .. } => *seekable,
            AudioSource::ServiceStream { .. } => false,
            AudioSource::Driver => false,
        }
    }
}

impl From<PathBuf> for AudioSource {
    fn from(path: PathBuf) -> Self {
        AudioSource::File(path)
    }
}

impl From<&Path> for AudioSource {
    fn from(path: &Path) -> Self {
        AudioSource::File(path.to_path_buf())
    }
}

impl From<&PathBuf> for AudioSource {
    fn from(path: &PathBuf) -> Self {
        AudioSource::File(path.clone())
    }
}

impl fmt::Display for AudioSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioSource::File(path) => write!(f, "{}", path.display()),
            AudioSource::Url { url, .. } => write!(f, "{}", url),
            AudioSource::ServiceStream { service, track_id } => {
                write!(f, "{}:{}", service, track_id)
            }
            AudioSource::Driver => write!(f, "driver"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_pathbuf() {
        let source: AudioSource = PathBuf::from("/music/song.flac").into();
        assert!(matches!(source, AudioSource::File(_)));
        assert_eq!(source.as_path().unwrap(), Path::new("/music/song.flac"));
        assert!(source.is_seekable());
    }

    #[test]
    fn test_from_path_ref() {
        let path = Path::new("/music/song.mp3");
        let source: AudioSource = path.into();
        assert!(matches!(source, AudioSource::File(_)));
    }

    #[test]
    fn test_url_seekable() {
        let source = AudioSource::Url {
            url: "http://example.com/song.flac".to_string(),
            format_hint: Some("flac".to_string()),
            seekable: true,
        };
        assert!(source.is_seekable());
    }

    #[test]
    fn test_url_non_seekable() {
        let source = AudioSource::Url {
            url: "http://radio.example.com/stream".to_string(),
            format_hint: Some("mp3".to_string()),
            seekable: false,
        };
        assert!(!source.is_seekable());
    }

    #[test]
    fn test_display_name() {
        let file_source: AudioSource = PathBuf::from("/music/album/song.flac").into();
        assert_eq!(file_source.display_name(), "song.flac");

        let url_source = AudioSource::Url {
            url: "http://example.com/stream".to_string(),
            format_hint: None,
            seekable: false,
        };
        assert_eq!(url_source.display_name(), "http://example.com/stream");

        let service_source = AudioSource::ServiceStream {
            service: ServiceId::Spotify,
            track_id: "abc123".to_string(),
        };
        assert_eq!(service_source.display_name(), "Spotify:abc123");
    }
}
