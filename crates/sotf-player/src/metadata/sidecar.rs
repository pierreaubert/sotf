use super::{MetadataError, MetadataPatch};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const ALBUM_SIDECAR_FILE: &str = ".sotf-album.json";
pub const BACKUP_DIR: &str = ".sotf-backups";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlbumMetadataSidecar {
    pub title: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub conductor: Option<String>,
    pub performer: Option<String>,
    pub ensemble: Option<String>,
    pub edition: Option<String>,
    pub musicbrainz_release_id: Option<String>,
}

impl AlbumMetadataSidecar {
    pub fn path_for_album_dir(album_dir: &Path) -> PathBuf {
        album_dir.join(ALBUM_SIDECAR_FILE)
    }

    pub fn read(path: &Path) -> Result<Self, MetadataError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    }

    pub fn apply_patch(&mut self, patch: &MetadataPatch) {
        if let Some(value) = &patch.album_title {
            self.title = Some(value.clone());
        }
        if let Some(value) = &patch.album_artist {
            self.album_artist = Some(value.clone());
        }
        if let Some(value) = patch.year {
            self.year = Some(value);
        }
        if let Some(value) = &patch.genre {
            self.genre = Some(value.clone());
        }
        if let Some(value) = &patch.composer {
            self.composer = Some(value.clone());
        }
        if let Some(value) = &patch.conductor {
            self.conductor = Some(value.clone());
        }
        if let Some(value) = &patch.performer {
            self.performer = Some(value.clone());
        }
        if let Some(value) = &patch.ensemble {
            self.ensemble = Some(value.clone());
        }
        if let Some(value) = &patch.edition {
            self.edition = Some(value.clone());
        }
    }

    pub fn write(&self, path: &Path) -> Result<(), MetadataError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

pub fn backup_path_for(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "metadata".to_string());
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    parent
        .join(BACKUP_DIR)
        .join(format!("{file_name}.{timestamp}.bak"))
}

pub fn backup_file(path: &Path, backup_path: &Path) -> Result<(), MetadataError> {
    if !path.exists() {
        return Ok(());
    }
    if let Some(parent) = backup_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(path, backup_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_sidecar_applies_patch_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = AlbumMetadataSidecar::path_for_album_dir(dir.path());
        let mut sidecar = AlbumMetadataSidecar::default();
        sidecar.apply_patch(&MetadataPatch {
            album_title: Some("New Album".to_string()),
            album_artist: Some("New Artist".to_string()),
            year: Some(2026),
            edition: Some("Deluxe".to_string()),
            ..Default::default()
        });
        sidecar.write(&path).unwrap();

        let loaded = AlbumMetadataSidecar::read(&path).unwrap();
        assert_eq!(loaded.title.as_deref(), Some("New Album"));
        assert_eq!(loaded.album_artist.as_deref(), Some("New Artist"));
        assert_eq!(loaded.year, Some(2026));
        assert_eq!(loaded.edition.as_deref(), Some("Deluxe"));
    }
}
