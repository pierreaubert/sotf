use super::sidecar::{backup_file, backup_path_for};
use super::{
    AlbumMetadataSidecar, MetadataAffectedFile, MetadataEditPreview, MetadataError,
    MetadataImportCandidate, MetadataPatch, MetadataTarget, TagWriter,
};
use crate::library::{Album, MusicLibrary, Track};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct MetadataController;

impl MetadataController {
    /// Preview edit.
    ///
    /// # Errors
    /// Returns an error for invalid metadata edits, unsupported tag writes, or failed file and database operations.
    pub fn preview_edit(
        library: &MusicLibrary,
        target: MetadataTarget,
        changes: MetadataPatch,
    ) -> Result<MetadataEditPreview, MetadataError> {
        let changes = changes.sanitized();
        if changes.is_empty() {
            return Err(MetadataError::EmptyPatch);
        }

        let (album_ids, tracks, sidecar_path) = match &target {
            MetadataTarget::AlbumId(album_id) => {
                let album = find_album(library, *album_id)
                    .ok_or(MetadataError::AlbumNotFound(*album_id))?;
                let sidecar_path =
                    album_dir(album).map(|dir| AlbumMetadataSidecar::path_for_album_dir(&dir));
                (
                    vec![*album_id],
                    album.tracks.iter().collect::<Vec<_>>(),
                    sidecar_path,
                )
            }
            MetadataTarget::TrackPath(path) => {
                let (album, track) = find_track(library, path)
                    .ok_or_else(|| MetadataError::TrackNotFound(path.clone()))?;
                let ids = album.id.into_iter().collect::<Vec<_>>();
                (ids, vec![track], None)
            }
        };

        let mut affected_files = Vec::new();
        let mut unsupported_writes = Vec::new();
        let mut seen = BTreeSet::new();
        for track in tracks {
            if !seen.insert(track.path.clone()) {
                continue;
            }
            let backup_path = backup_path_for(&track.path);
            let reason = TagWriter::unsupported_reason(&track.path);
            let file = MetadataAffectedFile {
                path: track.path.clone(),
                backup_path,
                writable: reason.is_none(),
                reason,
            };
            if !file.writable {
                unsupported_writes.push(file.clone());
            }
            affected_files.push(file);
        }

        let sidecar_backup_path = sidecar_path.as_ref().map(|path| backup_path_for(path));
        Ok(MetadataEditPreview {
            target: Some(target),
            affected_track_paths: affected_files
                .iter()
                .map(|file| file.path.clone())
                .collect(),
            affected_files,
            sidecar_path,
            sidecar_backup_path,
            affected_album_ids: album_ids,
            unsupported_writes,
        })
    }

    /// Apply edit.
    ///
    /// # Errors
    /// Returns an error for invalid metadata edits, unsupported tag writes, or failed file and database operations.
    pub fn apply_edit(
        library: &mut MusicLibrary,
        target: MetadataTarget,
        changes: MetadataPatch,
    ) -> Result<MetadataEditPreview, MetadataError> {
        let changes = changes.sanitized();
        let preview = Self::preview_edit(library, target.clone(), changes.clone())?;
        if !preview.can_apply() {
            let paths = preview
                .unsupported_writes
                .iter()
                .map(|file| file.path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(MetadataError::UnsupportedWrites(paths));
        }

        for file in &preview.affected_files {
            backup_file(&file.path, &file.backup_path)?;
            TagWriter::write_patch(&file.path, &changes)?;
        }

        if let Some(sidecar_path) = &preview.sidecar_path {
            if let Some(backup_path) = &preview.sidecar_backup_path {
                backup_file(sidecar_path, backup_path)?;
            }
            let mut sidecar = AlbumMetadataSidecar::read(sidecar_path)?;
            sidecar.apply_patch(&changes);
            sidecar.write(sidecar_path)?;
        }

        apply_to_memory(library, &target, &changes)?;

        let db = library
            .get_database_mut()
            .ok_or(MetadataError::DatabaseUnavailable)?;
        match target {
            MetadataTarget::AlbumId(album_id) => db.update_album_metadata(album_id, &changes)?,
            MetadataTarget::TrackPath(path) => db.update_track_metadata(&path, &changes)?,
        }
        if let Err(err) = db.sync_fts_index() {
            log::warn!("Failed to sync FTS index after metadata edit: {err}");
        }
        library.refresh_dir_stats_cache();

        Ok(preview)
    }

    /// Import musicbrainz candidate.
    ///
    /// # Errors
    /// Returns an error for invalid metadata edits, unsupported tag writes, or failed file and database operations.
    pub fn import_musicbrainz_candidate(
        library: &mut MusicLibrary,
        target: MetadataTarget,
        candidate: MetadataImportCandidate,
    ) -> Result<MetadataEditPreview, MetadataError> {
        Self::apply_edit(library, target, candidate.into_patch())
    }
}

fn find_album(library: &MusicLibrary, album_id: i64) -> Option<&Album> {
    library
        .albums
        .iter()
        .find(|album| album.id == Some(album_id))
}

fn find_album_mut(library: &mut MusicLibrary, album_id: i64) -> Option<&mut Album> {
    library
        .albums
        .iter_mut()
        .find(|album| album.id == Some(album_id))
}

fn find_track<'a>(library: &'a MusicLibrary, path: &Path) -> Option<(&'a Album, &'a Track)> {
    library.albums.iter().find_map(|album| {
        album
            .tracks
            .iter()
            .find(|track| track.path == path)
            .map(|track| (album, track))
    })
}

fn album_dir(album: &Album) -> Option<PathBuf> {
    album
        .tracks
        .iter()
        .find_map(|track| track.path.parent().map(std::path::Path::to_path_buf))
}

fn apply_to_memory(
    library: &mut MusicLibrary,
    target: &MetadataTarget,
    changes: &MetadataPatch,
) -> Result<(), MetadataError> {
    match target {
        MetadataTarget::AlbumId(album_id) => {
            let album = find_album_mut(library, *album_id)
                .ok_or(MetadataError::AlbumNotFound(*album_id))?;
            apply_album_patch(album, changes);
        }
        MetadataTarget::TrackPath(path) => {
            let mut found = false;
            for album in &mut library.albums {
                for track in &mut album.tracks {
                    if track.path == *path {
                        apply_track_patch(track, changes);
                        if let Some(title) = &changes.album_title {
                            album.title.clone_from(title);
                        }
                        if let Some(year) = changes.year {
                            album.year = Some(year);
                        }
                        if let Some(edition) = &changes.edition {
                            track.edition = Some(edition.clone());
                        }
                        found = true;
                    }
                }
            }
            if !found {
                return Err(MetadataError::TrackNotFound(path.clone()));
            }
        }
    }
    Ok(())
}

fn apply_album_patch(album: &mut Album, changes: &MetadataPatch) {
    if let Some(title) = &changes.album_title {
        album.title.clone_from(title);
    }
    if let Some(year) = changes.year {
        album.year = Some(year);
    }
    if let Some(edition) = &changes.edition {
        album.edition = Some(edition.clone());
    }
    for track in &mut album.tracks {
        apply_track_patch(track, changes);
    }
}

fn apply_track_patch(track: &mut Track, changes: &MetadataPatch) {
    if let Some(value) = &changes.title {
        track.title = Some(value.clone());
    }
    if let Some(value) = &changes.artist {
        track.artist = Some(value.clone());
    }
    if let Some(value) = &changes.album_artist {
        track.album_artist = Some(value.clone());
    }
    if let Some(value) = &changes.genre {
        track.genre = Some(value.clone());
    }
    if let Some(value) = &changes.composer {
        track.composer = Some(value.clone());
    }
    if let Some(value) = changes.disc_number {
        track.disc_number = Some(value);
    }
    if let Some(value) = changes.track_number {
        track.track_number = Some(value);
    }
    if let Some(value) = &changes.conductor {
        track.conductor = Some(value.clone());
    }
    if let Some(value) = &changes.performer {
        track.performer = Some(value.clone());
    }
    if let Some(value) = &changes.isrc {
        track.isrc = Some(value.clone());
    }
    if let Some(value) = &changes.ensemble {
        track.ensemble = Some(value.clone());
    }
    if let Some(value) = &changes.edition {
        track.edition = Some(value.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{Album, MusicLibrary, Track};

    fn test_library(path: PathBuf) -> MusicLibrary {
        let mut library = MusicLibrary::new();
        library.albums = vec![Album {
            id: Some(7),
            title: "Original Album".to_string(),
            year: Some(1999),
            tracks: vec![Track {
                path,
                title: Some("Original Track".to_string()),
                artist: Some("Artist".to_string()),
                album_artist: Some("Album Artist".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        }];
        library
    }

    #[test]
    fn metadata_preview_reports_album_sidecar_and_unsupported_writes() {
        let library = test_library(PathBuf::from("/tmp/sotf-metadata-preview/test.wav"));
        let preview = MetadataController::preview_edit(
            &library,
            MetadataTarget::AlbumId(7),
            MetadataPatch {
                album_title: Some("Renamed Album".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(preview.affected_album_ids, vec![7]);
        assert_eq!(preview.affected_files.len(), 1);
        assert_eq!(preview.unsupported_writes.len(), 1);
        assert!(preview.sidecar_path.is_some());
        assert!(preview.sidecar_backup_path.is_some());
    }

    #[test]
    fn metadata_candidate_converts_to_patch() {
        let changes = MetadataImportCandidate {
            provider_id: "musicbrainz".to_string(),
            provider_entity_id: "recording-id".to_string(),
            title: Some("Track".to_string()),
            artist: Some("Artist".to_string()),
            album_artist: Some("Album Artist".to_string()),
            album_title: Some("Album".to_string()),
            year: Some(2024),
            track_number: Some(3),
            disc_number: Some(1),
            isrc: Some("USRC17607839".to_string()),
            score: 98,
        }
        .into_patch();

        assert_eq!(changes.title.as_deref(), Some("Track"));
        assert_eq!(changes.album_title.as_deref(), Some("Album"));
        assert_eq!(changes.year, Some(2024));
        assert_eq!(changes.track_number, Some(3));
    }

    #[test]
    fn metadata_patch_sanitized_trims_text_and_drops_blank_values() {
        let changes = MetadataPatch {
            title: Some("  Track  ".to_string()),
            artist: Some("   ".to_string()),
            album_title: Some(" Album ".to_string()),
            genre: Some("\tJazz\n".to_string()),
            ..Default::default()
        }
        .sanitized();

        assert_eq!(changes.title.as_deref(), Some("Track"));
        assert_eq!(changes.artist, None);
        assert_eq!(changes.album_title.as_deref(), Some("Album"));
        assert_eq!(changes.genre.as_deref(), Some("Jazz"));
    }
}
