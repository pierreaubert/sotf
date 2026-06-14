use super::compute::compute_aggregate_stats_for_path;
use super::consts::TRACK_WAVEFORM_SAMPLES;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryInfo {
    pub path: PathBuf,
    pub file_count: usize,
    pub album_count: usize,
    pub last_scanned: Option<SystemTime>,
    pub expanded: bool,
    pub subdirectories: Vec<DirectoryInfo>,
    /// Whether subdirectories have been loaded from disk.
    /// When false, `subdirectories` is empty but the directory may have children on disk.
    #[serde(default)]
    pub children_loaded: bool,
}

pub type TrackWaveform = Box<[u8; TRACK_WAVEFORM_SAMPLES]>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumChannelType {
    Stereo,            // All tracks are 2 channels
    Multichannel(u32), // All tracks have same channel count > 2
    Mixed,             // Tracks have different channel counts
}

/// Library sort order options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibrarySortOrder {
    Year,
    Genre,
    Artist,
    #[default]
    Album,
    Tracks,
    Composer,
    Popularity,
}

/// Channel filter options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChannelFilter {
    #[default]
    All, // Show all albums
    Mono,          // Only 1-channel albums
    Stereo,        // Only 2-channel albums
    Surround,      // 5.0/5.1 albums (5-6 channels)
    Surround71,    // 7.1 albums (8 channels)
    SurroundPlus,  // More than 8 channels
    Mixed,         // Only albums with mixed channel counts
    Specific(u32), // Only albums with specific channel count
}

/// A playlist entry containing a track path and its position
#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistEntry {
    pub track_path: PathBuf,
    pub position: u32,
}

#[derive(Debug, Default)]
pub(super) struct TrackMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_number: Option<u32>,
    pub year: Option<u32>,
    pub duration_secs: Option<u64>,
    pub channels: Option<u32>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    // Extended metadata fields
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub disc_number: Option<u32>,
    pub conductor: Option<String>,
    pub performer: Option<String>,
    pub isrc: Option<String>,
    pub album_artist: Option<String>,
    pub ensemble: Option<String>,
    pub edition: Option<String>,
}

/// Load immediate children of a directory from disk (one level only).
/// Computes aggregate stats for each child from the stats_map.
pub fn load_children_from_disk(
    dir_info: &mut DirectoryInfo,
    stats_map: &HashMap<PathBuf, (usize, std::collections::HashSet<String>)>,
) {
    if dir_info.children_loaded {
        return;
    }
    dir_info.children_loaded = true;

    let mut subdirectories = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir_info.path) {
        for entry in entries.filter_map(|e| e.ok()) {
            // Use file_type() from DirEntry — avoids extra stat syscall on most platforms
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            if is_dir {
                let child_path = entry.path();
                let (tracks, albums) = compute_aggregate_stats_for_path(&child_path, stats_map);
                subdirectories.push(DirectoryInfo {
                    path: child_path,
                    file_count: tracks,
                    album_count: albums,
                    last_scanned: None,
                    expanded: false,
                    subdirectories: Vec::new(),
                    children_loaded: false,
                });
            }
        }
    }
    subdirectories.sort_by(|a, b| a.path.file_name().cmp(&b.path.file_name()));
    dir_info.subdirectories = subdirectories;
}
