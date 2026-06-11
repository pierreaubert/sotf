use super::sidecar::{backup_file, backup_path_for};
use super::{
    AlbumMetadataSidecar, MetadataAffectedFile, MetadataEditPreview, MetadataError,
    MetadataImportCandidate, MetadataPatch, MetadataTarget, TagWriter,
};
use crate::library::{Album, MusicLibrary, Track};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub struct MetadataController;

impl MetadataController {
    pub fn preview_edit(
        library: &MusicLibrary,
        target: MetadataTarget,
        patch: MetadataPatch,
    ) -> Result<MetadataEditPreview, MetadataError> {
        let patch = patch.sanitized();
        if patch.is_empty() {
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

    pub fn apply_edit(
        library: &mut MusicLibrary,
        target: MetadataTarget,
        patch: MetadataPatch,
    ) -> Result<MetadataEditPreview, MetadataError> {
        let patch = patch.sanitized();
        let preview = Self::preview_edit(library, target.clone(), patch.clone())?;
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
            TagWriter::write_patch(&file.path, &patch)?;
        }

        if let Some(sidecar_path) = &preview.sidecar_path {
            if let Some(backup_path) = &preview.sidecar_backup_path {
                backup_file(sidecar_path, backup_path)?;
            }
            let mut sidecar = AlbumMetadataSidecar::read(sidecar_path)?;
            sidecar.apply_patch(&patch);
            sidecar.write(sidecar_path)?;
        }

        apply_to_memory(library, &target, &patch)?;

        let db = library
            .get_database_mut()
            .ok_or(MetadataError::DatabaseUnavailable)?;
        match target {
            MetadataTarget::AlbumId(album_id) => db.update_album_metadata(album_id, &patch)?,
            MetadataTarget::TrackPath(path) => db.update_track_metadata(&path, &patch)?,
        }
        if let Err(err) = db.sync_fts_index() {
            log::warn!("Failed to sync FTS index after metadata edit: {err}");
        }
        library.refresh_dir_stats_cache();

        Ok(preview)
    }

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
        .find_map(|track| track.path.parent().map(|path| path.to_path_buf()))
}

fn apply_to_memory(
    library: &mut MusicLibrary,
    target: &MetadataTarget,
    patch: &MetadataPatch,
) -> Result<(), MetadataError> {
    match target {
        MetadataTarget::AlbumId(album_id) => {
            let album = find_album_mut(library, *album_id)
                .ok_or(MetadataError::AlbumNotFound(*album_id))?;
            apply_album_patch(album, patch);
        }
        MetadataTarget::TrackPath(path) => {
            let mut found = false;
            for album in &mut library.albums {
                for track in &mut album.tracks {
                    if track.path == *path {
                        apply_track_patch(track, patch);
                        if let Some(title) = &patch.album_title {
                            album.title = title.clone();
                        }
                        if let Some(year) = patch.year {
                            album.year = Some(year);
                        }
                        if let Some(edition) = &patch.edition {
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

fn apply_album_patch(album: &mut Album, patch: &MetadataPatch) {
    if let Some(title) = &patch.album_title {
        album.title = title.clone();
    }
    if let Some(year) = patch.year {
        album.year = Some(year);
    }
    if let Some(edition) = &patch.edition {
        album.edition = Some(edition.clone());
    }
    for track in &mut album.tracks {
        apply_track_patch(track, patch);
    }
}

fn apply_track_patch(track: &mut Track, patch: &MetadataPatch) {
    if let Some(value) = &patch.title {
        track.title = Some(value.clone());
    }
    if let Some(value) = &patch.artist {
        track.artist = Some(value.clone());
    }
    if let Some(value) = &patch.album_artist {
        track.album_artist = Some(value.clone());
    }
    if let Some(value) = &patch.genre {
        track.genre = Some(value.clone());
    }
    if let Some(value) = &patch.composer {
        track.composer = Some(value.clone());
    }
    if let Some(value) = patch.disc_number {
        track.disc_number = Some(value);
    }
    if let Some(value) = patch.track_number {
        track.track_number = Some(value);
    }
    if let Some(value) = &patch.conductor {
        track.conductor = Some(value.clone());
    }
    if let Some(value) = &patch.performer {
        track.performer = Some(value.clone());
    }
    if let Some(value) = &patch.isrc {
        track.isrc = Some(value.clone());
    }
    if let Some(value) = &patch.ensemble {
        track.ensemble = Some(value.clone());
    }
    if let Some(value) = &patch.edition {
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
        let patch = MetadataImportCandidate {
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

        assert_eq!(patch.title.as_deref(), Some("Track"));
        assert_eq!(patch.album_title.as_deref(), Some("Album"));
        assert_eq!(patch.year, Some(2024));
        assert_eq!(patch.track_number, Some(3));
    }

    #[test]
    fn metadata_patch_sanitized_trims_text_and_drops_blank_values() {
        let patch = MetadataPatch {
            title: Some("  Track  ".to_string()),
            artist: Some("   ".to_string()),
            album_title: Some(" Album ".to_string()),
            genre: Some("\tJazz\n".to_string()),
            ..Default::default()
        }
        .sanitized();

        assert_eq!(patch.title.as_deref(), Some("Track"));
        assert_eq!(patch.artist, None);
        assert_eq!(patch.album_title.as_deref(), Some("Album"));
        assert_eq!(patch.genre.as_deref(), Some("Jazz"));
    }
}
