use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetadataTarget {
    AlbumId(i64),
    TrackPath(PathBuf),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataPatch {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album_title: Option<String>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub disc_number: Option<u32>,
    pub track_number: Option<u32>,
    pub conductor: Option<String>,
    pub performer: Option<String>,
    pub isrc: Option<String>,
    pub ensemble: Option<String>,
    pub edition: Option<String>,
}

impl MetadataPatch {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    pub fn sanitized(mut self) -> Self {
        fn clean(value: Option<String>) -> Option<String> {
            value.and_then(|s| {
                let trimmed = s.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            })
        }

        self.title = clean(self.title);
        self.artist = clean(self.artist);
        self.album_artist = clean(self.album_artist);
        self.album_title = clean(self.album_title);
        self.genre = clean(self.genre);
        self.composer = clean(self.composer);
        self.conductor = clean(self.conductor);
        self.performer = clean(self.performer);
        self.isrc = clean(self.isrc);
        self.ensemble = clean(self.ensemble);
        self.edition = clean(self.edition);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataAffectedFile {
    pub path: PathBuf,
    pub backup_path: PathBuf,
    pub writable: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataEditPreview {
    pub target: Option<MetadataTarget>,
    pub affected_files: Vec<MetadataAffectedFile>,
    pub sidecar_path: Option<PathBuf>,
    pub sidecar_backup_path: Option<PathBuf>,
    pub affected_album_ids: Vec<i64>,
    pub affected_track_paths: Vec<PathBuf>,
    pub unsupported_writes: Vec<MetadataAffectedFile>,
}

impl MetadataEditPreview {
    pub fn can_apply(&self) -> bool {
        self.unsupported_writes.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataImportCandidate {
    pub provider_id: String,
    pub provider_entity_id: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album_title: Option<String>,
    pub year: Option<u32>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub isrc: Option<String>,
    pub score: u8,
}

impl MetadataImportCandidate {
    pub fn preferred_album_title(&self) -> Option<&str> {
        self.album_title.as_deref().or(self.title.as_deref())
    }

    pub fn preferred_track_title(&self) -> Option<&str> {
        self.title.as_deref().or(self.album_title.as_deref())
    }

    pub fn into_patch(self) -> MetadataPatch {
        MetadataPatch {
            title: self.title,
            artist: self.artist,
            album_artist: self.album_artist,
            album_title: self.album_title,
            year: self.year,
            track_number: self.track_number,
            disc_number: self.disc_number,
            isrc: self.isrc,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataProviderConfig {
    pub provider_id: String,
    pub enabled: bool,
    pub endpoint: String,
    pub username: Option<String>,
    pub has_stored_credentials: bool,
}

impl Default for MetadataProviderConfig {
    fn default() -> Self {
        Self {
            provider_id: "musicbrainz".to_string(),
            enabled: true,
            endpoint: "https://musicbrainz.org/ws/2/".to_string(),
            username: None,
            has_stored_credentials: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataServicesConfig {
    pub providers: Vec<MetadataProviderConfig>,
    pub user_agent: String,
}

impl Default for MetadataServicesConfig {
    fn default() -> Self {
        Self {
            providers: vec![MetadataProviderConfig::default()],
            user_agent: format!("SOTF/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

#[cfg(test)]
mod schema_tests {
    use super::{MetadataProviderConfig, MetadataServicesConfig};

    #[test]
    fn metadata_services_config_empty_defaults() {
        let json = r#"{"providers": [], "user_agent": "test"}"#;
        let config: MetadataServicesConfig = serde_json::from_str(json).unwrap();
        assert!(config.providers.is_empty());
        assert_eq!(config.user_agent, "test");
    }

    #[test]
    fn metadata_services_config_ignores_unknown_fields() {
        let json = r#"{
            "providers": [
                {
                    "provider_id": "musicbrainz",
                    "enabled": true,
                    "endpoint": "https://example.com/",
                    "username": null,
                    "has_stored_credentials": false,
                    "future_field": "ignored"
                }
            ],
            "user_agent": "test",
            "unknown_nested": {"x": 1}
        }"#;

        let config: MetadataServicesConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.providers[0].provider_id, "musicbrainz");
    }

    #[test]
    fn metadata_services_config_serde_roundtrip() {
        let config = MetadataServicesConfig {
            providers: vec![MetadataProviderConfig {
                provider_id: "musicbrainz".into(),
                enabled: true,
                endpoint: "https://example.com/".into(),
                username: Some("user".into()),
                has_stored_credentials: true,
            }],
            user_agent: "SOTF/test".into(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let decoded: MetadataServicesConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.providers.len(), config.providers.len());
        assert_eq!(decoded.user_agent, config.user_agent);
        assert_eq!(decoded.providers[0].provider_id, "musicbrainz");
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    #[error("metadata patch is empty")]
    EmptyPatch,
    #[error("album not found: {0}")]
    AlbumNotFound(i64),
    #[error("track not found: {0}")]
    TrackNotFound(PathBuf),
    #[error("metadata writes are not supported for: {0}")]
    UnsupportedWrites(String),
    #[error("database is not available")]
    DatabaseUnavailable,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("tag write error: {0}")]
    TagWrite(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
