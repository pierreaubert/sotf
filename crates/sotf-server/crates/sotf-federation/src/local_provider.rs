// ============================================================================
// LocalFilesProvider
// ============================================================================
//
// Wraps the existing local library (MusicLibrary + LibraryScanner) as a
// LibraryProvider. This is always present and has the highest priority.
//
// The local provider does not do its own scanning — it converts already-loaded
// Album/Track data into ProviderAlbum/ProviderTrack for the federation layer.

use crate::provider::*;
use sotf_audio::decoder::AudioSource;
use std::path::PathBuf;

/// Configuration for the local files provider.
#[derive(Debug, Clone)]
pub struct LocalProviderConfig {
    pub directories: Vec<PathBuf>,
}

/// A LibraryProvider backed by local audio files on disk.
///
/// This wraps the existing `MusicLibrary` scanning + database infrastructure.
/// Albums and tracks are provided from in-memory data that was already loaded
/// from the SQLite database.
pub struct LocalFilesProvider {
    source_id: SourceId,
    config: LocalProviderConfig,
}

impl LocalFilesProvider {
    pub fn new(config: LocalProviderConfig) -> Self {
        Self {
            source_id: SourceId("local".to_string()),
            config,
        }
    }

    /// Convert a path-based album/track list into ProviderAlbums.
    /// This is used by the federation layer to ingest local library data.
    pub fn albums_from_local(
        albums: &[(String, String, Option<u32>, Vec<LocalTrackInfo>)],
    ) -> Vec<ProviderAlbum> {
        albums
            .iter()
            .map(|(title, artist, year, tracks)| ProviderAlbum {
                external_id: format!("local:{artist}:{title}"),
                title: title.clone(),
                artist: artist.clone(),
                year: *year,
                album_art_url: None,
                tracks: tracks
                    .iter()
                    .map(|t| ProviderTrack {
                        external_id: t.path.to_string_lossy().to_string(),
                        title: t.title.clone(),
                        artist: t.artist.clone(),
                        album_artist: t.album_artist.clone(),
                        track_number: t.track_number,
                        disc_number: t.disc_number,
                        duration_secs: t.duration_secs,
                        genre: t.genre.clone(),
                        composer: t.composer.clone(),
                        channels: t.channels,
                        sample_rate: t.sample_rate,
                        bit_depth: t.bit_depth,
                        audio_source: AudioSource::File(t.path.clone()),
                    })
                    .collect(),
            })
            .collect()
    }

    pub fn directories(&self) -> &[PathBuf] {
        &self.config.directories
    }
}

/// Minimal track info from local files for conversion.
#[derive(Debug, Clone)]
pub struct LocalTrackInfo {
    pub path: PathBuf,
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
}

impl LibraryProvider for LocalFilesProvider {
    fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    fn display_name(&self) -> &str {
        "Local Files"
    }

    fn source_type(&self) -> SourceType {
        SourceType::Local
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            writable: false,
            seekable: true,
            offline_available: true,
            supports_events: false,
            has_album_art: true,
        }
    }

    fn fetch_all_albums(&self) -> ProviderFuture<'_, Result<Vec<ProviderAlbum>, ProviderError>> {
        // Local library data is loaded by the caller (MusicLibrary) and converted
        // via albums_from_local(). This method is not used for the local provider
        // since scanning is handled by LibraryScanner directly.
        Box::pin(async { Ok(vec![]) })
    }

    fn fetch_changes_since(
        &self,
        _since: u64,
    ) -> ProviderFuture<'_, Result<Option<Vec<LibraryEvent>>, ProviderError>> {
        // Local changes are detected by LibraryScanner (mtime-based).
        // The federation layer does not poll the local provider.
        Box::pin(async { Ok(None) })
    }

    fn subscribe_events(&self) -> Option<tokio::sync::broadcast::Receiver<LibraryEvent>> {
        // Could wire up to LibraryScanner completion messages in the future.
        None
    }

    fn resolve_source(
        &self,
        track_external_id: &str,
    ) -> ProviderFuture<'_, Result<AudioSource, ProviderError>> {
        let path = PathBuf::from(track_external_id);
        Box::pin(async move {
            if path.exists() {
                Ok(AudioSource::File(path))
            } else {
                Err(ProviderError::NotFound(format!(
                    "file not found: {}",
                    path.display()
                )))
            }
        })
    }

    fn fetch_album_art(
        &self,
        _album_external_id: &str,
    ) -> ProviderFuture<'_, Result<Option<Vec<u8>>, ProviderError>> {
        // Album art is handled by the existing library scanner / database.
        Box::pin(async { Ok(None) })
    }

    fn is_available(&self) -> ProviderFuture<'_, bool> {
        // Local files are always available (directories might not be mounted,
        // but that's handled at scan time).
        Box::pin(async { true })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_provider_source_id() {
        let provider = LocalFilesProvider::new(LocalProviderConfig {
            directories: vec![PathBuf::from("/music")],
        });
        assert_eq!(provider.source_id().0, "local");
        assert_eq!(provider.display_name(), "Local Files");
        assert_eq!(provider.source_type(), SourceType::Local);
    }

    #[test]
    fn test_local_provider_capabilities() {
        let provider = LocalFilesProvider::new(LocalProviderConfig {
            directories: vec![],
        });
        let caps = provider.capabilities();
        assert!(caps.seekable);
        assert!(caps.offline_available);
        assert!(!caps.writable);
        assert!(!caps.supports_events);
        assert!(caps.has_album_art);
    }

    #[test]
    fn test_albums_from_local() {
        let albums = LocalFilesProvider::albums_from_local(&[(
            "The Wall".to_string(),
            "Pink Floyd".to_string(),
            Some(1979),
            vec![LocalTrackInfo {
                path: PathBuf::from("/music/the_wall/01.flac"),
                title: "In the Flesh?".to_string(),
                artist: Some("Pink Floyd".to_string()),
                album_artist: Some("Pink Floyd".to_string()),
                track_number: Some(1),
                disc_number: Some(1),
                duration_secs: Some(199.0),
                genre: Some("Progressive Rock".to_string()),
                composer: Some("Roger Waters".to_string()),
                channels: Some(2),
                sample_rate: Some(44100),
                bit_depth: Some(16),
            }],
        )]);

        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].title, "The Wall");
        assert_eq!(albums[0].artist, "Pink Floyd");
        assert_eq!(albums[0].year, Some(1979));
        assert_eq!(albums[0].tracks.len(), 1);
        assert_eq!(albums[0].tracks[0].title, "In the Flesh?");
        assert!(matches!(
            albums[0].tracks[0].audio_source,
            AudioSource::File(_)
        ));
    }

    #[tokio::test]
    async fn test_resolve_nonexistent_file() {
        let provider = LocalFilesProvider::new(LocalProviderConfig {
            directories: vec![],
        });
        let result = provider.resolve_source("/nonexistent/file.flac").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_is_always_available() {
        let provider = LocalFilesProvider::new(LocalProviderConfig {
            directories: vec![],
        });
        assert!(provider.is_available().await);
    }
}
