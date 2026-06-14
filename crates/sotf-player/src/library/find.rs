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
    // Strategy 1: Look for common album art filenames in the main directory
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name() {
                    let filename_lower = filename.to_string_lossy().to_lowercase();
                    if ALBUM_ART_FILENAMES.contains(&filename_lower.as_str()) {
                        return Some(path);
                    }
                }
            }
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
    // Strategy 1: Look for common filenames
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name() {
                    let filename_lower = filename.to_string_lossy().to_lowercase();
                    if ALBUM_ART_FILENAMES.contains(&filename_lower.as_str()) {
                        return Some(path);
                    }
                }
            }
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

/// Find album art and generate thumbnail for an album based on its tracks
pub fn find_and_generate_album_thumbnail(album: &mut Album) {
    // Skip if we already have a thumbnail
    if album.album_art_thumbnail.is_some() {
        return;
    }

    // Get the directory from the first track
    let track_dir = match album.tracks.first() {
        Some(track) => track.path.parent(),
        None => return,
    };

    let track_dir = match track_dir {
        Some(dir) => dir,
        None => return,
    };

    // Find album art in the directory
    if let Some(art_path) = find_album_art(track_dir) {
        // Update album art path
        album.album_art_path = Some(art_path.clone());

        // Generate thumbnail
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
}
