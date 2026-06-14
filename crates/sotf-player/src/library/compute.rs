use super::album::Album;
use super::album::directory_album_key;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Compute aggregate stats (track count, album count) for a directory and all its descendants.
/// Iterates the stats_map in memory — no disk I/O.
///
/// Album count is the size of the union of per-directory album-key sets, so an
/// album whose tracks span several subdirectories is counted once, not once
/// per subdirectory.
pub(super) fn compute_aggregate_stats_for_path(
    root: &Path,
    stats_map: &HashMap<PathBuf, (usize, std::collections::HashSet<String>)>,
) -> (usize, usize) {
    let mut total_tracks = 0;
    let mut album_union: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for (dir, (tracks, album_keys)) in stats_map {
        if dir.starts_with(root) {
            total_tracks += tracks;
            for key in album_keys {
                album_union.insert(key.as_str());
            }
        }
    }
    (total_tracks, album_union.len())
}

/// Compute directory stats from albums in the library.
/// Returns a map of `directory path -> (track count, album-key set)`.
/// Each entry covers tracks that live *directly* in that directory; the
/// aggregator (`compute_aggregate_stats_for_path`) walks descendants and
/// unions the album-key sets so albums spanning multiple subdirectories are
/// counted once.
///
/// Both the original parent path and (when different) its canonicalized form
/// are inserted as keys so callers can look up by either form. The track
/// count and album set are duplicated under both keys; aggregation uses
/// `starts_with(root)` against a single root form, so only one form
/// participates in any given aggregation.
pub(super) fn compute_directory_stats(
    albums: &[Album],
) -> HashMap<PathBuf, (usize, std::collections::HashSet<String>)> {
    let mut result: HashMap<PathBuf, (usize, std::collections::HashSet<String>)> = HashMap::new();
    // Cache canonicalize() results — many tracks share the same parent directory
    let mut canonical_cache: HashMap<PathBuf, Option<PathBuf>> = HashMap::new();

    for album in albums {
        let album_key = directory_album_key(album);

        for track in &album.tracks {
            if let Some(parent) = track.path.parent() {
                let parent_buf = parent.to_path_buf();

                let entry = result.entry(parent_buf.clone()).or_default();
                entry.0 += 1;
                entry.1.insert(album_key.clone());

                // Use cached canonicalize result
                let canonical = canonical_cache
                    .entry(parent_buf.clone())
                    .or_insert_with(|| parent_buf.canonicalize().ok().filter(|c| *c != parent_buf));
                if let Some(canonical) = canonical.clone() {
                    let entry = result.entry(canonical).or_default();
                    entry.0 += 1;
                    entry.1.insert(album_key.clone());
                }
            }
        }
    }

    result
}
