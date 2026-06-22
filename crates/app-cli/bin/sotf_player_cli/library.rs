use sotf_audio_player::{Album, LibraryStats, MusicDatabase};
use std::path::Path;

use super::types::LibraryAction;

/// Load albums from a library database, treating a missing or empty database
/// as an empty library rather than a fatal error.
fn load_albums_or_empty(db_path: &Path) -> Result<Vec<Album>, String> {
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let db = MusicDatabase::open_secondary(db_path).map_err(|e| {
        format!(
            "Failed to open library database '{}': {}",
            db_path.display(),
            e
        )
    })?;

    // An existing but uninitialised SQLite file (e.g. a freshly-created temp
    // path) is reported as an empty library.
    db.load_library()
        .map_err(|e| format!("Failed to load library from '{}': {}", db_path.display(), e))
}

/// Run `library --db <PATH> <action>`.
pub(super) fn run_library_command(db_path: &Path, action: &LibraryAction) -> Result<(), String> {
    match action {
        LibraryAction::List => {
            let albums = load_albums_or_empty(db_path)?;
            let stats = LibraryStats::compute(&albums);
            let total_duration: u64 = albums
                .iter()
                .flat_map(|a| a.tracks.iter())
                .filter_map(|t| t.duration_secs)
                .sum();

            println!("Library summary:");
            println!("  Tracks: {}", stats.total_tracks);
            println!("  Albums: {}", albums.len());
            println!("  Artists: {}", stats.artists_count);
            println!("  Total duration: {}s", total_duration);
            Ok(())
        }
        LibraryAction::Search { query } => {
            if !db_path.exists() {
                println!("No matching albums found.");
                return Ok(());
            }

            let db = MusicDatabase::open_secondary(db_path).map_err(|e| {
                format!(
                    "Failed to open library database '{}': {}",
                    db_path.display(),
                    e
                )
            })?;

            let ids = db
                .search_library(query)
                .map_err(|e| format!("Failed to search library '{}': {}", db_path.display(), e))?;

            if ids.is_empty() {
                println!("No matching albums found.");
            } else {
                println!(
                    "Matching album IDs: {}",
                    ids.iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                );
            }
            Ok(())
        }
    }
}

/// Run `status --db <PATH>` as a compact library summary.
pub(super) fn run_status_command(db_path: &Path) -> Result<(), String> {
    let albums = load_albums_or_empty(db_path)?;
    let stats = LibraryStats::compute(&albums);
    let total_duration: u64 = albums
        .iter()
        .flat_map(|a| a.tracks.iter())
        .filter_map(|t| t.duration_secs)
        .sum();

    println!(
        "Library: {} tracks, {} albums, {} artists, {}s total",
        stats.total_tracks,
        albums.len(),
        stats.artists_count,
        total_duration
    );
    Ok(())
}
