use crate::database::MusicDatabase;
use sotf_federation::ProviderAlbum;
use std::sync::atomic::{AtomicBool, Ordering};

/// Result of a federation source scan.
#[derive(Debug, Clone)]
pub struct FederationScanResult {
    pub source_id: String,
    pub albums: usize,
    pub tracks: usize,
    pub error: Option<String>,
}

/// Progress callback invoked after each album is merged.
pub type ScanProgressFn = Box<dyn Fn(usize, usize) + Send>;

/// Merge fetched albums into the local database.
///
/// Opens a secondary DB connection, clears old data for this source,
/// then merges all albums and tracks. Calls `on_progress(albums_merged, tracks_merged)`
/// after each album. Checks `cancel` between albums.
pub fn merge_albums_to_db(
    source_id: &str,
    albums: &[ProviderAlbum],
    cancel: &AtomicBool,
    on_progress: Option<&ScanProgressFn>,
) -> FederationScanResult {
    let db = match MusicDatabase::default_path() {
        Some(path) => match MusicDatabase::open_secondary(&path) {
            Ok(db) => db,
            Err(e) => {
                return FederationScanResult {
                    source_id: source_id.to_string(),
                    albums: 0,
                    tracks: 0,
                    error: Some(format!("failed to open database: {e}")),
                };
            }
        },
        None => {
            return FederationScanResult {
                source_id: source_id.to_string(),
                albums: 0,
                tracks: 0,
                error: Some("no database path configured".to_string()),
            };
        }
    };

    merge_albums_into_db(&db, source_id, albums, cancel, on_progress)
}

/// Merge fetched albums into the provided database connection.
///
/// This is the core merge engine used by the sync scheduler and tests. It
/// performs a full source refresh: old rows for the source are detached first,
/// then the latest provider snapshot is smart-merged into local albums/tracks.
pub fn merge_albums_into_db(
    db: &MusicDatabase,
    source_id: &str,
    albums: &[ProviderAlbum],
    cancel: &AtomicBool,
    on_progress: Option<&ScanProgressFn>,
) -> FederationScanResult {
    if cancel.load(Ordering::Relaxed) {
        return FederationScanResult {
            source_id: source_id.to_string(),
            albums: 0,
            tracks: 0,
            error: Some("cancelled".to_string()),
        };
    }

    // Full resync starts by unmerging only this source. Local tracks and tracks
    // mirrored by other sources stay attached; synthetic tracks exclusive to the
    // source are removed before the fresh provider snapshot is merged.
    if let Err(e) = db.unmerge_federation_source(source_id) {
        log::warn!("Failed to unmerge previous federation data for {source_id}: {e}");
    }

    let mut album_count = 0;
    let mut track_count = 0;

    for album in albums {
        if cancel.load(Ordering::Relaxed) {
            return FederationScanResult {
                source_id: source_id.to_string(),
                albums: album_count,
                tracks: track_count,
                error: Some("cancelled".to_string()),
            };
        }

        match db.merge_federation_album(source_id, album) {
            Ok(album_id) => {
                album_count += 1;
                for track in &album.tracks {
                    match db.merge_federation_track(source_id, album_id, track) {
                        Ok(_) => track_count += 1,
                        Err(e) => log::warn!("Failed to merge track '{}': {e}", track.title),
                    }
                }
            }
            Err(e) => log::warn!("Failed to merge album '{}': {e}", album.title),
        }

        if let Some(cb) = on_progress {
            cb(album_count, track_count);
        }
    }

    log::info!(
        "Federation scan of '{source_id}' complete: {album_count} albums, {track_count} tracks merged"
    );

    FederationScanResult {
        source_id: source_id.to_string(),
        albums: album_count,
        tracks: track_count,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::federation_config::{FederationSourceEntry, SourceConnectionConfig};
    use sotf_audio::decoder::AudioSource;
    use sotf_federation::ProviderTrack;

    fn source() -> FederationSourceEntry {
        FederationSourceEntry {
            source_id: "peer:studio".to_string(),
            display_name: "Studio".to_string(),
            priority: 0,
            is_enabled: true,
            connection: SourceConnectionConfig::Peer {
                host: "studio.local".to_string(),
                port: 8732,
                accepted_fingerprint: None,
                auth_token: Some("secret".to_string()),
            },
            is_available: None,
        }
    }

    fn album(track_ids: &[&str]) -> ProviderAlbum {
        ProviderAlbum {
            external_id: "album-1".to_string(),
            title: "Remote Album".to_string(),
            artist: "Remote Artist".to_string(),
            year: Some(2026),
            album_art_url: None,
            tracks: track_ids
                .iter()
                .enumerate()
                .map(|(index, id)| ProviderTrack {
                    external_id: (*id).to_string(),
                    title: format!("Track {}", index + 1),
                    artist: Some("Remote Artist".to_string()),
                    album_artist: Some("Remote Artist".to_string()),
                    track_number: Some((index + 1) as u32),
                    disc_number: Some(1),
                    duration_secs: Some(60.0),
                    genre: None,
                    composer: None,
                    channels: Some(2),
                    sample_rate: Some(44_100),
                    bit_depth: Some(16),
                    audio_source: AudioSource::Url {
                        url: format!("https://studio.local/media/{id}"),
                        format_hint: Some("flac".to_string()),
                        seekable: true,
                    },
                })
                .collect(),
        }
    }

    #[test]
    fn merge_albums_into_db_full_refresh_removes_stale_synthetic_tracks() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("music.sqlite");
        let db = MusicDatabase::open_for_testing(&db_path).unwrap();
        let source = source();
        db.save_federation_source(&source).unwrap();
        let cancel = AtomicBool::new(false);

        let first =
            merge_albums_into_db(&db, &source.source_id, &[album(&["a", "b"])], &cancel, None);
        assert!(first.error.is_none(), "{first:?}");
        assert_eq!(first.tracks, 2);
        assert_eq!(db.load_library().unwrap()[0].tracks.len(), 2);

        let second = merge_albums_into_db(&db, &source.source_id, &[album(&["a"])], &cancel, None);
        assert!(second.error.is_none(), "{second:?}");
        assert_eq!(second.tracks, 1);
        let library = db.load_library().unwrap();
        assert_eq!(library.len(), 1);
        assert_eq!(library[0].tracks.len(), 1);
        assert_eq!(library[0].tracks[0].title.as_deref(), Some("Track 1"));
    }

    #[test]
    fn merge_albums_into_db_cancelled_before_refresh_preserves_existing_tracks() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("music.sqlite");
        let db = MusicDatabase::open_for_testing(&db_path).unwrap();
        let source = source();
        db.save_federation_source(&source).unwrap();
        let cancel = AtomicBool::new(false);

        let first =
            merge_albums_into_db(&db, &source.source_id, &[album(&["a", "b"])], &cancel, None);
        assert!(first.error.is_none(), "{first:?}");
        assert_eq!(db.load_library().unwrap()[0].tracks.len(), 2);

        cancel.store(true, Ordering::Relaxed);
        let cancelled =
            merge_albums_into_db(&db, &source.source_id, &[album(&["a"])], &cancel, None);

        assert_eq!(cancelled.error.as_deref(), Some("cancelled"));
        let library = db.load_library().unwrap();
        assert_eq!(library.len(), 1);
        assert_eq!(library[0].tracks.len(), 2);
    }

    #[test]
    fn merge_albums_into_db_merges_service_stream_tracks() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("music.sqlite");
        let db = MusicDatabase::open_for_testing(&db_path).unwrap();

        let source = FederationSourceEntry {
            source_id: "tidal:account".to_string(),
            display_name: "Tidal".to_string(),
            priority: 0,
            is_enabled: true,
            connection: SourceConnectionConfig::Tidal {
                access_token: "token".to_string(),
                client_id: "client".to_string(),
                refresh_token: "refresh".to_string(),
                quality: "LOSSLESS".to_string(),
                country_code: "US".to_string(),
            },
            is_available: None,
        };
        db.save_federation_source(&source).unwrap();
        let cancel = AtomicBool::new(false);

        let tidal_album = ProviderAlbum {
            external_id: "7".to_string(),
            title: "The Wall".to_string(),
            artist: "Pink Floyd".to_string(),
            year: Some(1979),
            album_art_url: Some("https://resources.tidal.com/images/ab/640x640.jpg".to_string()),
            tracks: vec![ProviderTrack {
                external_id: "101".to_string(),
                title: "In the Flesh?".to_string(),
                artist: Some("Pink Floyd".to_string()),
                album_artist: Some("Pink Floyd".to_string()),
                track_number: Some(1),
                disc_number: None,
                duration_secs: Some(200.0),
                genre: None,
                composer: None,
                channels: None,
                sample_rate: None,
                bit_depth: None,
                audio_source: AudioSource::ServiceStream {
                    service: sotf_audio::decoder::ServiceId::Tidal,
                    track_id: "101".to_string(),
                },
            }],
        };

        let result = merge_albums_into_db(&db, &source.source_id, &[tidal_album], &cancel, None);
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(result.albums, 1);
        assert_eq!(result.tracks, 1);
        let library = db.load_library().unwrap();
        assert_eq!(library.len(), 1);
        assert_eq!(library[0].title, "The Wall");
        assert_eq!(library[0].tracks.len(), 1);
    }
}
