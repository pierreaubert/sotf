use super::album::Album;
use super::consts::ALBUM_ART_FILENAMES;
use super::consts::generate_thumbnail;
use super::is::is_image_file;
use std::path::{Path, PathBuf};

/// Find album art in the given directory with smart heuristics:
/// 1. Look for common filenames (cover, folder, front, album, artwork)
/// 2. Look for files with "front" in the name (case-insensitive)
/// 3. If only one image file exists in the directory, use it
/// 4. Check subdirectories named "Artwork" or "Covers" (case-insensitive)
pub(super) fn find_album_art(dir: &Path) -> Option<PathBuf> {
    // Strategy 1: Look for common album art filenames in priority order.
    for name in ALBUM_ART_FILENAMES {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    // Strategy 2: Look for files with "front" in the name (case-insensitive)
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_image_file(&path) {
                if let Some(filename) = path.file_name() {
                    let filename_lower = filename.to_string_lossy().to_lowercase();
                    if filename_lower.contains("front") {
                        return Some(path);
                    }
                }
            }
        }
    }

    // Strategy 3: If there's only one image in the directory, use it
    if let Ok(entries) = std::fs::read_dir(dir) {
        let images: Vec<PathBuf> = entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.is_file() && is_image_file(&path) {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();

        if images.len() == 1 {
            return Some(images[0].clone());
        }
    }

    // Strategy 4: Check for "Artwork" or "Covers" subdirectories
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(dirname) = path.file_name() {
                    let dirname_lower = dirname.to_string_lossy().to_lowercase();
                    if dirname_lower == "artwork" || dirname_lower == "covers" {
                        // Recursively search in this subdirectory
                        // Try common names first
                        if let Some(art) = find_album_art_in_subdir(&path) {
                            return Some(art);
                        }
                    }
                }
            }
        }
    }

    None
}

/// Find album art in a subdirectory (used for Artwork/Covers folders)
/// Uses the same strategies but doesn't recurse further
pub(super) fn find_album_art_in_subdir(dir: &Path) -> Option<PathBuf> {
    // Strategy 1: Look for common filenames in priority order.
    for name in ALBUM_ART_FILENAMES {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    // Strategy 2: Look for "front" in filename
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_image_file(&path) {
                if let Some(filename) = path.file_name() {
                    let filename_lower = filename.to_string_lossy().to_lowercase();
                    if filename_lower.contains("front") {
                        return Some(path);
                    }
                }
            }
        }
    }

    // Strategy 3: If only one image, use it
    if let Ok(entries) = std::fs::read_dir(dir) {
        let images: Vec<PathBuf> = entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.is_file() && is_image_file(&path) {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();

        if images.len() == 1 {
            return Some(images[0].clone());
        }
    }

    None
}

/// Return the deepest directory that contains every path in `paths`.
fn common_ancestor(paths: &[&Path]) -> Option<PathBuf> {
    if paths.is_empty() {
        return None;
    }

    let first_components: Vec<_> = paths[0].components().collect();
    let mut common_len = first_components.len();

    for path in &paths[1..] {
        let components: Vec<_> = path.components().collect();
        let mut i = 0;
        while i < common_len && i < components.len() && first_components[i] == components[i] {
            i += 1;
        }
        common_len = i;
        if common_len == 0 {
            return None;
        }
    }

    Some(first_components[..common_len].iter().collect())
}

/// Find album art and generate thumbnail for an album based on its tracks.
///
/// The search starts at the deepest directory shared by all tracks, then
/// walks one level up so that cover art placed at the album root is found
/// even when tracks live in disc subdirectories (e.g. `CD1/`).
pub fn find_and_generate_album_thumbnail(album: &mut Album) {
    if album.tracks.is_empty() {
        return;
    }

    let track_dirs: Vec<&Path> = album
        .tracks
        .iter()
        .filter_map(|t| t.path.parent())
        .collect();
    if track_dirs.is_empty() {
        return;
    }

    let Some(common_dir) = common_ancestor(&track_dirs) else {
        return;
    };

    let mut search_dirs = vec![common_dir.clone()];
    if let Some(parent) = common_dir.parent() {
        search_dirs.push(parent.to_path_buf());
    }

    let mut found_art = None;
    for dir in &search_dirs {
        if let Some(art) = find_album_art(dir) {
            found_art = Some(art);
            break;
        }
    }

    if let Some(art_path) = found_art {
        let changed = album.album_art_path.as_ref() != Some(&art_path);
        if changed || album.album_art_thumbnail.is_none() {
            album.album_art_path = Some(art_path.clone());

            if let Some(thumbnail) = generate_thumbnail(&art_path) {
                log::debug!(
                    "Generated thumbnail for {} - {} ({} bytes)",
                    album.artist(),
                    album.title,
                    thumbnail.len()
                );
                album.album_art_thumbnail = Some(thumbnail);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_testkit::db::temp_files;

    fn temp_dir_with_files(files: &[&str]) -> (tempfile::TempDir, std::path::PathBuf) {
        let (dir, _paths) = temp_files(files);
        let path = dir.path().to_path_buf();
        (dir, path)
    }

    fn create_subdir(parent: &std::path::Path, name: &str, files: &[&str]) -> std::path::PathBuf {
        let sub = parent.join(name);
        std::fs::create_dir(&sub).expect("create subdir");
        for file in files {
            std::fs::write(sub.join(file), b"").expect("write subdir file");
        }
        sub
    }

    #[test]
    fn find_album_art_prefers_common_filename() {
        let (_dir, path) = temp_dir_with_files(&["cover.jpg", "other.jpg", "album.txt"]);
        let found = find_album_art(&path);
        assert_eq!(found, Some(path.join("cover.jpg")));
    }

    #[test]
    fn find_album_art_falls_back_to_front_filename() {
        let (_dir, path) = temp_dir_with_files(&["notes.txt", "album_front.jpg"]);
        let found = find_album_art(&path);
        assert_eq!(found, Some(path.join("album_front.jpg")));
    }

    #[test]
    fn find_album_art_uses_only_image_when_unique() {
        let (_dir, path) = temp_dir_with_files(&["track.flac", "photo.png"]);
        let found = find_album_art(&path);
        assert_eq!(found, Some(path.join("photo.png")));
    }

    #[test]
    fn find_album_art_refuses_multiple_ambiguous_images() {
        let (_dir, path) = temp_dir_with_files(&["a.jpg", "b.png"]);
        assert!(find_album_art(&path).is_none());
    }

    #[test]
    fn find_album_art_searches_artwork_subdirectory() {
        let (_dir, path) = temp_dir_with_files(&["track.flac"]);
        create_subdir(&path, "Artwork", &["cover.jpg"]);
        let found = find_album_art(&path);
        assert_eq!(found, Some(path.join("Artwork").join("cover.jpg")));
    }

    #[test]
    fn find_album_art_searches_covers_subdirectory() {
        let (_dir, path) = temp_dir_with_files(&["track.flac"]);
        create_subdir(&path, "Covers", &["front.png"]);
        let found = find_album_art(&path);
        assert_eq!(found, Some(path.join("Covers").join("front.png")));
    }

    #[test]
    fn find_album_art_returns_none_for_empty_directory() {
        let (_dir, path) = temp_dir_with_files(&[]);
        assert!(find_album_art(&path).is_none());
    }

    #[test]
    fn find_album_art_prefers_cover_over_front() {
        let (_dir, path) = temp_dir_with_files(&["front.png", "cover.jpg"]);
        let found = find_album_art(&path);
        assert_eq!(found, Some(path.join("cover.jpg")));
    }

    #[test]
    fn common_ancestor_of_single_path_is_that_path() {
        let a = PathBuf::from("/music/album/cd1");
        assert_eq!(common_ancestor(&[&a]), Some(a));
    }

    #[test]
    fn common_ancestor_finds_shared_parent() {
        let a = PathBuf::from("/music/album/cd1/track.wav");
        let b = PathBuf::from("/music/album/cd2/track.wav");
        assert_eq!(
            common_ancestor(&[&a, &b]),
            Some(PathBuf::from("/music/album"))
        );
    }
}
