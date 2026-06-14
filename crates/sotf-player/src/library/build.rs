use super::types::DirectoryInfo;
use std::path::PathBuf;
use std::time::SystemTime;

/// Build directory info without recursion (shallow — children loaded on demand).
pub(super) fn build_directory_info(path: PathBuf) -> DirectoryInfo {
    DirectoryInfo {
        path,
        file_count: 0,
        album_count: 0,
        last_scanned: None,
        expanded: false,
        subdirectories: Vec::new(),
        children_loaded: false,
    }
}

/// Build a shallow directory node for loading from database.
/// Does NOT recurse into subdirectories — they are loaded lazily on expand.
pub(super) fn build_directory_shallow(
    path: PathBuf,
    file_count: usize,
    album_count: usize,
    last_scanned: Option<SystemTime>,
) -> DirectoryInfo {
    DirectoryInfo {
        path,
        file_count,
        album_count,
        last_scanned,
        expanded: false,
        subdirectories: Vec::new(),
        children_loaded: false,
    }
}
