use crate::database::MusicDatabase;
use sotf_federation::ProviderAlbum;
use std::sync::atomic::{AtomicBool, Ordering};

/// Result of a federation source scan.
#[derive(Debug)]
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
    // Open a secondary DB connection on this background thread
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
