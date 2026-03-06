use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{Limit, MetadataOptions};
use symphonia::core::probe::{Hint, Probe};
use walkdir::WalkDir;

use crate::database::MusicDatabase;

/// Normalize an artist or album name for consistent grouping
/// Converts to lowercase, trims whitespace, removes diacritics and special characters
/// Keeps ASCII letters, numbers, periods, and UTF-8 letters/numbers
/// Examples:
/// - "2Cellos", "2CELLOS", "2 Cellos " -> "2cellos"
/// - "Café" -> "cafe"
/// - "The Beatles!" -> "thebeatles"
/// - "AC/DC" -> "acdc"
fn normalize_album_key(name: &str) -> String {
    use unicode_normalization::UnicodeNormalization;

    name.trim()
        .nfd() // Normalize to NFD (decomposed form) to separate diacritics
        .filter(|c| {
            // Keep ASCII letters, numbers, and periods
            if c.is_ascii_alphanumeric() || *c == '.' {
                return true;
            }
            // Keep UTF-8 letters and numbers (non-ASCII)
            if !c.is_ascii() && (c.is_alphabetic() || c.is_numeric()) {
                return true;
            }
            // Filter out everything else (diacritics, punctuation, etc.)
            false
        })
        .collect::<String>()
        .to_lowercase()
}

/// Capitalize the first letter of each word for display
/// Examples: "2cellos" -> "2cellos", "the beatles" -> "The Beatles"
fn capitalize_words(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let rest: String = chars.collect();
                    format!("{}{}", first.to_uppercase(), rest)
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Clean album title by removing disc/volume information and catalog numbers
/// Handles formats like:
/// - "Album Title (CD 1)" or "Album Title (CD1)"
/// - "Album Title (CD-1)"
/// - "Album Title (Disc 1)" or "Album Title (Disc1)"
/// - "Album Title CD 1" or "Album Title CD1"
/// - "Album Title (3116-2)" - catalog numbers
/// - "Album Title (R2 47730)" - catalog numbers with prefix
pub fn clean_album_title(title: &str) -> String {
    let lower = title.to_lowercase();

    // First, try to remove catalog numbers in parentheses at the end
    // Pattern: title ends with "(...)" where content looks like a catalog number
    // Catalog numbers typically have patterns like: "3116-2", "R2 47730", "ABC-12345", "MFSL 1234"
    if let Some(paren_start) = lower.rfind(" (") {
        let suffix = &lower[paren_start..];
        if suffix.ends_with(')') {
            let inner = &suffix[2..suffix.len() - 1].trim(); // Content inside parentheses
            // Check if it looks like a catalog number
            // Must contain digits AND either a dash or be alphanumeric only
            // Must NOT be a disc marker or common album suffixes
            let has_digit = inner.chars().any(|c| c.is_ascii_digit());
            let is_disc_marker =
                inner.starts_with("cd") || inner.starts_with("disc") || inner.starts_with("vol");
            let is_album_suffix = inner.contains("remaster")
                || inner.contains("deluxe")
                || inner.contains("edition")
                || inner.contains("live")
                || inner.contains("bonus")
                || inner.contains("anniversary");
            // Catalog numbers usually have a dash, or are short alphanumeric codes
            // They typically don't have spaces unless it's a prefix like "R2 47730"
            // Note: inner is lowercased, so we check for lowercase letters
            let letter_count = inner.chars().filter(|c| c.is_ascii_lowercase()).count();
            let digit_count = inner.chars().filter(|c| c.is_ascii_digit()).count();
            let looks_like_catalog = has_digit
                && !is_disc_marker
                && !is_album_suffix
                && inner.len() <= 15
                && inner.len() >= 3 // Catalog numbers are at least 3 chars
                && (inner.contains('-')
                    || (letter_count >= 1 && digit_count >= 2));
            if looks_like_catalog {
                return title[..paren_start].trim().to_string();
            }
        }
    }

    // Also try to remove catalog numbers in square brackets at the end
    // Pattern: title ends with "[...]" where content looks like a catalog number
    // e.g., "Passion [RWCD 1]", "Us [PGCD 7]", "Album [ABC-123]"
    if let Some(bracket_start) = lower.rfind(" [") {
        let suffix = &lower[bracket_start..];
        if suffix.ends_with(']') {
            let inner = &suffix[2..suffix.len() - 1].trim();
            let has_digit = inner.chars().any(|c| c.is_ascii_digit());
            let is_disc_marker =
                inner.starts_with("cd") || inner.starts_with("disc") || inner.starts_with("vol");
            let is_album_suffix = inner.contains("remaster")
                || inner.contains("deluxe")
                || inner.contains("edition")
                || inner.contains("live")
                || inner.contains("bonus")
                || inner.contains("anniversary");
            let letter_count = inner.chars().filter(|c| c.is_ascii_lowercase()).count();
            let digit_count = inner.chars().filter(|c| c.is_ascii_digit()).count();
            let looks_like_catalog = has_digit
                && !is_disc_marker
                && !is_album_suffix
                && inner.len() <= 15
                && inner.len() >= 3
                && (inner.contains('-')
                    || (letter_count >= 1 && digit_count >= 1));
            if looks_like_catalog {
                return title[..bracket_start].trim().to_string();
            }
        }
    }

    // List of markers to look for at the end of the string
    // We look for the last occurrence to avoid false positives in the middle of titles
    let markers = [
        " (cd", " (disc", " cd ", " disc ", " vol.", " vol ",
        // Handle cases without space before number (e.g., "CD1", "Disc2")
        " cd", " disc", // Handle cases without space
        "(cd", "(disc", // Handle square brackets
        " [cd", " [disc", "[cd", "[disc",
    ];

    for marker in markers {
        if let Some(idx) = lower.rfind(marker) {
            let suffix = &lower[idx..];
            let after_marker = &suffix[marker.len()..];

            // Heuristics to ensure this is actually a disc number:

            // 1. Must contain a digit
            if !suffix.chars().any(|c| c.is_ascii_digit()) {
                continue;
            }

            // 2. For markers that end with a letter (like " cd", " disc"), check that what follows
            //    starts with a digit or dash (e.g., "cd1", "cd-1")
            //    This prevents matching album names like "The CD Is Dead"
            //    Skip this check for markers with parens/brackets (they have their own checks)
            //    and for markers ending with space or period (like " vol.", " cd ")
            let has_parens_or_brackets = marker.contains('(') || marker.contains('[');
            let ends_with_letter = marker
                .chars()
                .last()
                .map(|c| c.is_ascii_alphabetic())
                .unwrap_or(false);
            if ends_with_letter && !has_parens_or_brackets {
                if let Some(first_char) = after_marker.chars().next() {
                    if !first_char.is_ascii_digit() && first_char != '-' {
                        continue;
                    }
                }
            }

            // 3. If it starts with parenthesis, it must end with parenthesis
            if marker.contains('(') && !suffix.ends_with(')') {
                continue;
            }

            // 4. If it starts with bracket, it must end with bracket
            if marker.contains('[') && !suffix.ends_with(']') {
                continue;
            }

            // 5. If it doesn't have parentheses/brackets, it should be at the very end of the string
            // (already guaranteed by rfind + we are checking the suffix)

            return title[..idx].trim().to_string();
        }
    }

    title.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryInfo {
    pub path: PathBuf,
    pub file_count: usize,
    pub album_count: usize,
    pub last_scanned: Option<SystemTime>,
    pub expanded: bool,
    pub subdirectories: Vec<DirectoryInfo>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Track {
    pub path: PathBuf,
    pub title: Option<String>,
    pub artist: Option<String>, // Track artist (may differ from album artist for compilations)
    pub track_number: Option<u32>,
    pub duration_secs: Option<u64>,
    pub channels: Option<u32>,
    pub sample_rate: Option<u32>, // Sample rate in Hz (e.g., 44100, 48000, 96000)
    pub bit_depth: Option<u32>,   // Bits per sample (e.g., 16, 24, 32)
    pub replay_gain: Option<f64>, // Track gain in dB
    pub replay_peak: Option<f64>, // Track peak (0.0 - 1.0)
    pub album_gain: Option<f64>,  // Album gain in dB
    pub album_peak: Option<f64>,  // Album peak (0.0 - 1.0)
    pub waveform: Option<Vec<u8>>, // 128 amplitude samples (0-255) for waveform visualization
    // Extended metadata fields (from audio file tags)
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub disc_number: Option<u32>,
    pub conductor: Option<String>,
    pub performer: Option<String>,
    pub isrc: Option<String>,
    pub album_artist: Option<String>,
    pub ensemble: Option<String>,
    pub edition: Option<String>,
    pub is_favorite: bool,
    pub play_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Album {
    pub id: Option<i64>,
    pub title: String,
    pub year: Option<u32>,
    pub tracks: Vec<Track>,
    pub album_art_path: Option<PathBuf>,
    /// JPEG thumbnail of album art (160x160 for high-DPI displays)
    pub album_art_thumbnail: Option<Vec<u8>>,
    pub play_count: usize,
    pub edition: Option<String>,
    pub dynamic_range: Option<f64>,
    pub is_favorite: bool,
}

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

impl Album {
    /// Determine the channel configuration of this album
    pub fn channel_type(&self) -> Option<AlbumChannelType> {
        if self.tracks.is_empty() {
            return None;
        }

        // Get channel counts from all tracks that have the info
        let channel_counts: Vec<u32> = self.tracks.iter().filter_map(|t| t.channels).collect();

        if channel_counts.is_empty() {
            return None;
        }

        // Check if all tracks have the same channel count
        let first_channels = channel_counts[0];
        let all_same = channel_counts.iter().all(|&c| c == first_channels);

        if all_same {
            if first_channels == 2 {
                Some(AlbumChannelType::Stereo)
            } else {
                Some(AlbumChannelType::Multichannel(first_channels))
            }
        } else {
            Some(AlbumChannelType::Mixed)
        }
    }

    /// Get the channel count if all tracks have the same number of channels
    pub fn uniform_channel_count(&self) -> Option<u32> {
        match self.channel_type()? {
            AlbumChannelType::Stereo => Some(2),
            AlbumChannelType::Multichannel(n) => Some(n),
            AlbumChannelType::Mixed => None,
        }
    }

    /// Get the sample rate of this album (from the first track that has it)
    pub fn sample_rate(&self) -> Option<u32> {
        self.tracks.iter().find_map(|t| t.sample_rate)
    }
}

impl Album {
    /// Get the artist(s) for this album by scanning all tracks.
    /// Returns the album_artist if consistent across tracks, otherwise
    /// returns "Various Artists" for compilations.
    /// Prefers album_artist, falls back to track artist.
    pub fn artist(&self) -> String {
        use std::collections::BTreeSet;

        // First try to collect album_artists
        let album_artists: BTreeSet<String> = self
            .tracks
            .iter()
            .filter_map(|t| t.album_artist.clone())
            .collect();

        // If we have a consistent album_artist, use it
        if album_artists.len() == 1 {
            return album_artists.into_iter().next().unwrap();
        }

        // Otherwise fall back to track artists
        let track_artists: BTreeSet<String> = self
            .tracks
            .iter()
            .filter_map(|t| t.artist.clone())
            .collect();

        if track_artists.is_empty() {
            "Unknown Artist".to_string()
        } else if track_artists.len() == 1 {
            track_artists.into_iter().next().unwrap()
        } else {
            // Multiple artists - this is a compilation
            "Various Artists".to_string()
        }
    }

    pub fn display_name(&self) -> String {
        let artist = self.artist();
        if let Some(year) = self.year {
            format!("{} - {} ({})", artist, self.title, year)
        } else {
            format!("{} - {}", artist, self.title)
        }
    }

    pub fn sort_tracks(&mut self) {
        self.tracks.sort_by_key(|t| t.track_number);
    }
}

/// A playlist entry containing a track path and its position
#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistEntry {
    pub track_path: PathBuf,
    pub position: u32,
}

/// A user-created playlist
#[derive(Debug, Clone)]
pub struct Playlist {
    pub id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub entries: Vec<PlaylistEntry>,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
}

impl Playlist {
    /// Create a new empty playlist
    pub fn new(name: String) -> Self {
        Self {
            id: None,
            name,
            description: None,
            entries: Vec::new(),
            created_at: None,
            updated_at: None,
        }
    }

    /// Create a new playlist with a description
    pub fn with_description(name: String, description: String) -> Self {
        Self {
            id: None,
            name,
            description: Some(description),
            entries: Vec::new(),
            created_at: None,
            updated_at: None,
        }
    }

    /// Get the number of tracks in the playlist
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the playlist is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get track paths in order
    pub fn track_paths(&self) -> Vec<&PathBuf> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by_key(|e| e.position);
        entries.iter().map(|e| &e.track_path).collect()
    }
}

#[derive(Debug, Default)]
pub struct MusicLibrary {
    pub directories: Vec<DirectoryInfo>,
    pub albums: Vec<Album>,
    db: Option<MusicDatabase>,
}

impl MusicLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new library with database persistence
    pub fn with_database() -> Result<Self, Box<dyn std::error::Error>> {
        let db_path =
            MusicDatabase::default_path().ok_or("Could not determine config directory")?;

        // Create a timestamped backup of the existing database before opening it
        crate::database::backup_existing_database(&db_path)?;

        let db = MusicDatabase::open(&db_path)?;

        Ok(Self {
            directories: Vec::new(),
            albums: Vec::new(),
            db: Some(db),
        })
    }

    /// Create a new library for a secondary (read-only-intent) instance.
    /// Skips backup, WAL pragma, and schema migration.
    pub fn with_database_secondary() -> Result<Self, Box<dyn std::error::Error>> {
        let db_path =
            MusicDatabase::default_path().ok_or("Could not determine config directory")?;

        let db = MusicDatabase::open_secondary(&db_path)?;

        Ok(Self {
            directories: Vec::new(),
            albums: Vec::new(),
            db: Some(db),
        })
    }

    /// Create a new library with database persistence at a custom path (for testing)
    ///
    /// This method is primarily intended for testing but is available in all builds.
    /// For production use, prefer `with_database()` which uses the default database location.
    pub fn with_custom_database<P: AsRef<std::path::Path>>(
        db_path: P,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let db = MusicDatabase::open(db_path)?;

        Ok(Self {
            directories: Vec::new(),
            albums: Vec::new(),
            db: Some(db),
        })
    }

    /// Create a new library with database persistence at a custom path, bypassing security checks.
    ///
    /// This method is only available in test builds and allows creating a database
    /// in any location (e.g., temp directories) for unit testing.
    #[cfg(any(test, feature = "testing"))]
    pub fn with_custom_database_for_testing<P: AsRef<std::path::Path>>(
        db_path: P,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let db = MusicDatabase::open_for_testing(db_path)?;

        Ok(Self {
            directories: Vec::new(),
            albums: Vec::new(),
            db: Some(db),
        })
    }

    /// Get a reference to the database
    pub fn get_database(&self) -> Option<&MusicDatabase> {
        self.db.as_ref()
    }

    /// Load library from database
    pub fn load_from_database(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(db) = &self.db {
            // Load albums
            self.albums = db.load_library()?;

            // Clear existing directories before rebuilding from database
            self.directories.clear();

            // Compute directory stats from the loaded albums
            // This gives us stats for every directory that contains tracks
            let stats_map = compute_directory_stats(&self.albums);

            // Load previously scanned directories with their stats
            let mut scanned_dirs = db.get_scanned_directories()?;

            // Filter to only keep directories that exist on disk
            scanned_dirs.retain(|(path, _, _, _)| path.exists());

            // Canonicalize paths to handle case sensitivity and symlinks
            // Build a map of canonical path -> (original path, stats)
            let mut canonical_map: std::collections::HashMap<
                PathBuf,
                (PathBuf, usize, usize, u64),
            > = std::collections::HashMap::new();

            for (dir_path, track_count, album_count, last_scan) in scanned_dirs {
                if let Ok(canonical) = dir_path.canonicalize() {
                    // If we already have this canonical path, keep the one with more recent scan time
                    canonical_map
                        .entry(canonical.clone())
                        .and_modify(|e| {
                            if last_scan > e.3 {
                                *e = (dir_path.clone(), track_count, album_count, last_scan);
                            }
                        })
                        .or_insert((dir_path, track_count, album_count, last_scan));
                }
            }

            // Convert back to vec for sorting
            let mut canonical_dirs: Vec<(PathBuf, PathBuf, usize, usize, u64)> = canonical_map
                .into_iter()
                .map(|(canonical, (original, track, album, scan))| {
                    (canonical, original, track, album, scan)
                })
                .collect();

            // Sort by path depth (shorter paths first) to ensure we keep parents
            canonical_dirs.sort_by_key(|(canonical, _, _, _, _)| canonical.components().count());

            // Remove subdirectories - only keep top-level parents
            let mut filtered_dirs: Vec<(PathBuf, PathBuf, usize, usize, u64)> = Vec::new();
            for (canonical, original, track_count, album_count, last_scan) in canonical_dirs {
                // Check if this directory is a subtree of any already-added directory
                let is_subtree = filtered_dirs.iter().any(|(parent_canonical, _, _, _, _)| {
                    canonical.starts_with(parent_canonical) && canonical != *parent_canonical
                });

                if !is_subtree {
                    // This is a new top-level directory, add it
                    filtered_dirs.push((canonical, original, track_count, album_count, last_scan));
                }
            }

            // Build directory info structures for filtered directories
            // Only build tree from what exists on disk NOW (don't include dirs that were in DB but deleted)
            for (canonical_path, _original_path, _track_count, _album_count, last_scan) in
                filtered_dirs
            {
                let last_scanned =
                    Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(last_scan));

                // Build directory tree from disk
                // Stats will be computed from the albums in the database
                let mut dir_info = build_directory_tree_from_disk(
                    canonical_path,
                    0, // Will be updated from stats_map
                    0, // Will be updated from stats_map
                    last_scanned,
                );

                // Update stats from the computed map
                update_directory_stats_from_map(&mut dir_info, &stats_map);

                self.directories.push(dir_info);
            }
        }
        Ok(())
    }

    pub fn add_directory(&mut self, path: PathBuf) -> Result<bool, String> {
        // Check if this exact path already exists
        if self.directories.iter().any(|d| d.path == path) {
            return Ok(false); // Already exists, no scan needed
        }

        // Check if this path is a subtree of an existing directory
        for existing in &self.directories {
            if path.starts_with(&existing.path) {
                return Err(format!(
                    "Directory is already covered by existing directory: {}",
                    existing.path.display()
                ));
            }
        }

        // Check if any existing directories are subtrees of this new path
        // If so, we should remove them since this new path covers them
        let to_remove: Vec<PathBuf> = self
            .directories
            .iter()
            .filter(|d| d.path.starts_with(&path))
            .map(|d| d.path.clone())
            .collect();

        if !to_remove.is_empty() {
            log::info!(
                "New directory {} covers {} existing directories, removing them",
                path.display(),
                to_remove.len()
            );
            self.directories.retain(|d| !to_remove.contains(&d.path));
        }

        self.directories.push(build_directory_info(path));

        Ok(true) // New directory added, scan needed
    }

    /// Get filtered, filtered, merged, and sorted albums
    pub fn get_filtered_albums(
        &self,
        query: &str,
        sort_order: LibrarySortOrder,
        channel_filter: ChannelFilter,
    ) -> Vec<Album> {
        // 1. Search
        let mut albums: Vec<&Album> = if query.is_empty() {
            self.albums.iter().collect()
        } else {
            self.search_albums(query)
        };

        // 2. Channel Filter
        albums.retain(|album| match channel_filter {
            ChannelFilter::All => true,
            ChannelFilter::Mono => album.uniform_channel_count() == Some(1),
            ChannelFilter::Stereo => album.uniform_channel_count() == Some(2),
            ChannelFilter::Surround => {
                // 5.0 (5 channels) or 5.1 (6 channels)
                matches!(album.uniform_channel_count(), Some(5) | Some(6))
            }
            ChannelFilter::Surround71 => album.uniform_channel_count() == Some(8),
            ChannelFilter::SurroundPlus => {
                // More than 8 channels
                album.uniform_channel_count().is_some_and(|ch| ch > 8)
            }
            ChannelFilter::Mixed => matches!(album.channel_type(), Some(AlbumChannelType::Mixed)),
            ChannelFilter::Specific(n) => album.uniform_channel_count() == Some(n),
        });

        // 3. Merge (Consolidated logic as requested)
        let mut merged_albums = group_and_merge_albums(albums);

        // 4. Sort
        match sort_order {
            LibrarySortOrder::Year => {
                merged_albums.sort_by(|a, b| {
                    b.year
                        .cmp(&a.year)
                        .then_with(|| a.artist().cmp(&b.artist()))
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            LibrarySortOrder::Genre => {
                merged_albums.sort_by(|a, b| {
                    let genre_a = a
                        .tracks
                        .first()
                        .and_then(|t| t.genre.as_ref())
                        .map(|s| s.to_lowercase());
                    let genre_b = b
                        .tracks
                        .first()
                        .and_then(|t| t.genre.as_ref())
                        .map(|s| s.to_lowercase());
                    genre_a
                        .cmp(&genre_b)
                        .then_with(|| a.artist().cmp(&b.artist()))
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            LibrarySortOrder::Artist => {
                merged_albums.sort_by(|a, b| {
                    a.artist()
                        .cmp(&b.artist())
                        .then_with(|| a.year.cmp(&b.year).reverse()) // Newest first for same artist
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            LibrarySortOrder::Album => {
                merged_albums.sort_by(|a, b| a.title.cmp(&b.title));
            }
            LibrarySortOrder::Tracks => {
                merged_albums.sort_by(|a, b| {
                    b.tracks
                        .len()
                        .cmp(&a.tracks.len())
                        .then_with(|| a.artist().cmp(&b.artist()))
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            LibrarySortOrder::Composer => {
                merged_albums.sort_by(|a, b| {
                    let composer_a = a
                        .tracks
                        .first()
                        .and_then(|t| t.composer.as_ref())
                        .map(|s| s.to_lowercase());
                    let composer_b = b
                        .tracks
                        .first()
                        .and_then(|t| t.composer.as_ref())
                        .map(|s| s.to_lowercase());
                    composer_a
                        .cmp(&composer_b)
                        .then_with(|| a.artist().cmp(&b.artist()))
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            LibrarySortOrder::Popularity => {
                merged_albums.sort_by(|a, b| {
                    // Sort by play count descending
                    b.play_count
                        .cmp(&a.play_count)
                        .then_with(|| a.artist().cmp(&b.artist()))
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
        }

        merged_albums
    }

    /// Get flattened directory tree for display (Recursive)
    pub fn get_directory_tree_items(&self) -> Vec<(PathBuf, usize, bool)> {
        let mut items = Vec::new();

        fn add_recursive(
            items: &mut Vec<(PathBuf, usize, bool)>,
            dir_info: &DirectoryInfo,
            level: usize,
        ) {
            items.push((dir_info.path.clone(), level, dir_info.expanded));

            if dir_info.expanded {
                for subdir in &dir_info.subdirectories {
                    add_recursive(items, subdir, level + 1);
                }
            }
        }

        for dir_info in &self.directories {
            add_recursive(&mut items, dir_info, 0);
        }
        items
    }

    pub fn remove_directory(&mut self, index: usize) -> Option<PathBuf> {
        if index < self.directories.len() {
            let removed = self.directories.remove(index);
            let path = removed.path.clone();

            // Clean up database: remove all tracks from this directory
            if let Some(db) = &mut self.db {
                match db.remove_tracks_from_directory(&path) {
                    Ok(count) => {
                        if count > 0 {
                            log::info!(
                                "Cleaned up {} tracks from removed directory: {}",
                                count,
                                path.display()
                            );
                        }
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to clean up tracks from removed directory {}: {}",
                            path.display(),
                            e
                        );
                    }
                }
            }

            // Also remove albums that came from this directory from memory
            self.albums
                .retain(|album| !album.tracks.iter().all(|t| t.path.starts_with(&path)));

            Some(path)
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.scan_incremental(false)
    }

    /// Scan directories with progress callback
    /// The callback is called periodically with (tracks_scanned, albums_found)
    pub fn scan_with_progress<F>(
        &mut self,
        progress_callback: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnMut(usize, usize),
    {
        self.scan_incremental_with_progress(false, None, progress_callback)
    }

    /// Scan directories with optional incremental mode
    /// If incremental is true, only scan new or modified files
    #[allow(dead_code)]
    pub fn scan_incremental(
        &mut self,
        incremental: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.scan_incremental_with_progress(incremental, None, |_, _| {})
    }

    /// Scan directories with optional incremental mode and progress reporting
    ///
    /// If `incremental` is true, only scan new or modified files (based on mtime).
    /// If `incremental` is false, scan all files regardless of modification time.
    /// ReplayGain values are preserved in the database (not overwritten during scan).
    pub fn scan_incremental_with_progress<F>(
        &mut self,
        incremental: bool,
        cancellation_token: Option<Arc<AtomicBool>>,
        mut progress_callback: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnMut(usize, usize),
    {
        self.scan_incremental_with_progress_and_pause(
            incremental,
            cancellation_token,
            None,
            &mut progress_callback,
        )
    }

    /// Scan directories with optional incremental mode, progress reporting, and pause support
    pub fn scan_incremental_with_progress_and_pause<F>(
        &mut self,
        incremental: bool,
        cancellation_token: Option<Arc<AtomicBool>>,
        pause_flag: Option<Arc<AtomicBool>>,
        progress_callback: &mut F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnMut(usize, usize),
    {
        let mut album_map: HashMap<String, Album> = HashMap::new();
        let mut total_tracks = 0;
        let mut scanned_tracks = 0;
        let scan_time = SystemTime::now();
        let mut last_progress_report = SystemTime::now();

        // Create a map of directory path to (file count, album count)
        let mut dir_stats: HashMap<PathBuf, (usize, usize)> = HashMap::new();

        // Log directories being scanned
        log::info!(
            "Scanning {} directories: {:?}",
            self.directories.len(),
            self.directories
                .iter()
                .map(|d| d.path.display().to_string())
                .collect::<Vec<_>>()
        );

        for dir_info in &self.directories {
            // We need to scan recursively and aggregate stats
            // But scan_directory already does a recursive walk
            // We need scan_directory to return stats per subdirectory?
            // Or we can just scan the whole tree and then attribute files to directories?

            // Actually, scan_directory walks the whole tree.
            // We can modify scan_directory to return a map of directory -> stats?
            // Or just let it return total stats and we figure out the rest?

            // The issue is we want to show stats for each subdirectory in the tree.
            // So we need to know how many tracks/albums are in each subdirectory.

            // Let's modify scan_directory to take a mutable map of directory stats
            let (tracks, scanned) = self.scan_directory(
                &dir_info.path,
                &mut album_map,
                incremental,
                &mut dir_stats,
                cancellation_token.clone(),
                pause_flag.clone(),
            )?;

            // Check cancellation after each directory
            if let Some(token) = &cancellation_token {
                if token.load(Ordering::Relaxed) {
                    return Err("Scan cancelled".into());
                }
            }

            total_tracks += tracks;
            scanned_tracks += scanned;

            // Report progress after each directory and every 5 seconds
            let now = SystemTime::now();
            if now
                .duration_since(last_progress_report)
                .unwrap_or_default()
                .as_secs()
                >= 5
            {
                progress_callback(total_tracks, album_map.len());
                last_progress_report = now;
            }
        }

        // Final progress report
        progress_callback(total_tracks, album_map.len());

        // Calculate album counts per directory
        // We use a temporary map to store sets of album keys per directory to ensure uniqueness
        let mut dir_albums: HashMap<PathBuf, std::collections::HashSet<String>> = HashMap::new();

        for (title, album) in &album_map {
            for track in &album.tracks {
                if let Some(parent) = track.path.parent() {
                    dir_albums
                        .entry(parent.to_path_buf())
                        .or_default()
                        .insert(title.clone());
                }
            }
        }

        // Update dir_stats with album counts
        for (dir_path, albums) in dir_albums {
            let stats = dir_stats.entry(dir_path).or_insert((0, 0));
            stats.1 = albums.len();
        }

        // Helper to update directory info recursively
        fn update_dir_info(
            info: &mut DirectoryInfo,
            stats: &HashMap<PathBuf, (usize, usize)>,
            scan_time: SystemTime,
        ) {
            // Aggregate stats from this directory and all subdirectories
            // Or does stats already contain aggregated data?
            // If scan_directory walks everything, we can just look up the path in stats?
            // But scan_directory might not have entries for intermediate directories if they have no files directly?

            // Let's assume stats contains counts for each directory that has files.
            // We want the count to be recursive (files in this dir + subdirs).

            // Actually, let's make scan_directory populate stats for every directory it encounters.

            // For now, let's just try to update from the map if it exists
            // But we need to aggregate for the tree view?
            // Usually "tracks in this folder" means recursive.

            // Let's do a post-order traversal to aggregate counts
            let mut my_files = 0;
            let mut my_albums = 0;

            // First recurse
            for subdir in &mut info.subdirectories {
                update_dir_info(subdir, stats, scan_time);
                my_files += subdir.file_count;
                my_albums += subdir.album_count;
            }

            // Then add own files (from stats map)
            if let Some((files, albums)) = stats.get(&info.path) {
                my_files += files;
                my_albums += albums;
            }

            info.file_count = my_files;
            info.album_count = my_albums;
            info.last_scanned = Some(scan_time);
        }

        // Update directory info with file counts and scan time
        for dir_info in &mut self.directories {
            update_dir_info(dir_info, &dir_stats, scan_time);
        }

        // Merge with existing albums if we have a database
        if let Some(db) = &self.db
            && incremental
        {
            // Load existing albums from database
            let existing_albums = db.load_library()?;

            // Merge existing albums that weren't updated
            for existing_album in existing_albums {
                // Single-track albums might be standalone (path-keyed in scan).
                // If the path-based key already exists, this album was re-scanned → skip it.
                if existing_album.tracks.len() == 1 {
                    let path_key = format!(
                        "__standalone__|{}",
                        existing_album.tracks[0].path.to_string_lossy()
                    );
                    if album_map.contains_key(&path_key) {
                        continue;
                    }
                }
                // Use title|edition key (matches scan format for regular albums)
                let normalized = normalize_album_key(&existing_album.title);
                let edition = existing_album
                    .edition
                    .as_ref()
                    .map(|e| normalize_album_key(e))
                    .unwrap_or_default();
                let key = format!("{}|{}", normalized, edition);
                album_map.entry(key).or_insert(existing_album);
            }
        }

        self.albums = album_map.into_values().collect();

        // Sort tracks within each album and generate album art thumbnails
        for album in &mut self.albums {
            if album.tracks.is_empty() {
                log::warn!("Found empty album (no tracks): {}", album.title);
            }
            album.sort_tracks();
            // Find album art and generate thumbnail if not already present
            find_and_generate_album_thumbnail(album);

            // Calculate dynamic range (average replay gain)
            let gains: Vec<f64> = album.tracks.iter().filter_map(|t| t.replay_gain).collect();
            if !gains.is_empty() {
                let sum: f64 = gains.iter().sum();
                album.dynamic_range = Some(sum / gains.len() as f64);
            }
        }

        // Sort albums by artist (computed from tracks) and title
        self.albums
            .sort_by(|a, b| a.artist().cmp(&b.artist()).then(a.title.cmp(&b.title)));

        // Save to database if available
        if let Some(db) = &mut self.db {
            db.save_albums(&self.albums)?;

            // Sync FTS index to ensure search works correctly after scan
            // This rebuilds the FTS index from the current database state
            if let Err(e) = db.sync_fts_index() {
                log::warn!("Failed to sync FTS index after scan: {}", e);
            }

            // Record scan history for each directory
            for dir_info in &self.directories {
                db.record_scan(&dir_info.path, total_tracks, self.albums.len())?;
            }

            // Checkpoint the WAL to prevent unbounded growth during scanning.
            // Without this, the WAL grows indefinitely when other connections
            // (e.g., the TUI read connection) hold read snapshots.
            if let Err(e) = db.checkpoint_wal() {
                log::warn!("Failed to checkpoint WAL after scan: {}", e);
            }
        }

        log::info!(
            "Scan complete: {} tracks ({} scanned), {} albums",
            total_tracks,
            scanned_tracks,
            self.albums.len()
        );

        // Log album titles for debugging (at debug level to avoid spam)
        if log::log_enabled!(log::Level::Debug) {
            for album in &self.albums {
                log::debug!("  Album: {} ({} tracks)", album.title, album.tracks.len());
            }
        }

        Ok(())
    }

    /// Clean up database by removing tracks for files that no longer exist
    pub fn clean_database(&mut self) -> Result<usize, Box<dyn std::error::Error>> {
        self.clean_database_with_progress(|_, _| {})
    }

    /// Clean up database with progress callback
    /// The callback is called periodically with (checked, total)
    pub fn clean_database_with_progress<F>(
        &mut self,
        progress_callback: F,
    ) -> Result<usize, Box<dyn std::error::Error>>
    where
        F: FnMut(usize, usize),
    {
        if let Some(db) = &mut self.db {
            let removed = db.clean_missing_files_with_progress(progress_callback)?;
            // Reload library after cleanup
            self.load_from_database()?;
            Ok(removed)
        } else {
            Ok(0)
        }
    }

    fn scan_directory(
        &self,
        dir: &Path,
        album_map: &mut HashMap<String, Album>,
        incremental: bool,
        dir_stats: &mut HashMap<PathBuf, (usize, usize)>,
        cancellation_token: Option<Arc<AtomicBool>>,
        pause_flag: Option<Arc<AtomicBool>>,
    ) -> Result<(usize, usize), Box<dyn std::error::Error>> {
        let mut total_tracks = 0;
        let mut scanned_tracks = 0;

        // We need to track albums found in each directory to avoid double counting
        // But an album might be split across directories?
        // For simplicity, let's count unique albums found in this directory (non-recursive)
        // We'll use a local map for this directory's albums

        for entry in WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Periodically check cancellation (every file is too frequent? maybe every 100?)
            // Or just check here since it's inside loop
            if let Some(token) = &cancellation_token {
                // Determine if we should check (using some counter might be better but atomic load is cheap on x86)
                if token.load(Ordering::Relaxed) {
                    return Err("Scan cancelled".into());
                }
            }

            // Wait while paused (check every 200ms, also check for cancellation)
            if let Some(pf) = &pause_flag {
                while pf.load(Ordering::Relaxed) {
                    if let Some(token) = &cancellation_token {
                        if token.load(Ordering::Relaxed) {
                            return Err("Scan cancelled".into());
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }

            if !path.is_file() {
                continue;
            }

            if let Some(ext) = path.extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                if matches!(
                    ext.as_str(),
                    "flac" | "mp3" | "m4a" | "aac" | "ogg" | "opus" | "wav"
                ) {
                    total_tracks += 1;

                    // Update stats for the parent directory of this file
                    if let Some(parent) = path.parent() {
                        let stats = dir_stats.entry(parent.to_path_buf()).or_insert((0, 0));
                        stats.0 += 1;
                        // We'll update album count later or try to track it here?
                        // Tracking unique albums per directory is tricky if we process file by file.
                        // Let's just count tracks for now in the stats map, and maybe estimate albums?
                        // Or we can track unique albums per directory in a separate map?
                    }

                    // Check if we should skip this file in incremental mode
                    if incremental && let Some(db) = &self.db {
                        let file_mtime = get_file_mtime(path).unwrap_or(0);
                        if let Ok(Some(db_mtime)) = db.get_track_mtime(path) {
                            // Skip if file hasn't been modified
                            if file_mtime <= db_mtime {
                                continue;
                            }
                        }
                    }

                    scanned_tracks += 1;

                    match extract_metadata(path) {
                        Ok(metadata) => {
                            // When a track has no album tag, create a standalone
                            // single-track album keyed by file path so that loose
                            // files are not all lumped into one giant "Unknown Album".
                            let has_album_tag = metadata.album.is_some();

                            let raw_album_title = if has_album_tag {
                                metadata.album.clone().unwrap()
                            } else {
                                // Use parent folder name as album title so all untagged
                                // files in the same directory are grouped together.
                                path.parent()
                                    .and_then(|p| p.file_name())
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "Unknown Album".to_string())
                            };

                            let album_title = clean_album_title(&raw_album_title);

                            let key = if has_album_tag {
                                // Albums are keyed by title only - artist comes from tracks
                                let normalized_title = normalize_album_key(&album_title);

                                // Include edition in key to separate versions
                                let edition_key = metadata
                                    .edition
                                    .as_ref()
                                    .map(|e| normalize_album_key(e))
                                    .unwrap_or_default();
                                format!("{}|{}", normalized_title, edition_key)
                            } else {
                                // Group by folder path so all untagged files in the same
                                // directory become one album.
                                let folder = path
                                    .parent()
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                format!("__folder__|{}", folder)
                            };

                            let album = album_map.entry(key).or_insert_with(|| {
                                // Capitalize first letter of each word for nice display
                                let display_title = capitalize_words(&album_title);

                                Album {
                                    id: None,
                                    title: display_title,
                                    year: metadata.year,
                                    tracks: Vec::new(),
                                    album_art_path: None,
                                    album_art_thumbnail: None,
                                    play_count: 0,
                                    edition: metadata.edition.clone(),
                                    dynamic_range: None,
                                    is_favorite: false,
                                }
                            });

                            let track = Track {
                                path: path.to_path_buf(),
                                title: metadata.title,
                                artist: metadata.artist,
                                track_number: metadata.track_number,
                                duration_secs: metadata.duration_secs,
                                channels: metadata.channels,
                                sample_rate: metadata.sample_rate,
                                bit_depth: metadata.bit_depth,
                                replay_gain: None,
                                replay_peak: None,
                                album_gain: None,
                                album_peak: None,
                                waveform: None, // Will be computed separately
                                genre: metadata.genre,
                                composer: metadata.composer,
                                disc_number: metadata.disc_number,
                                conductor: metadata.conductor,
                                performer: metadata.performer,
                                isrc: metadata.isrc,
                                album_artist: metadata.album_artist,
                                ensemble: metadata.ensemble,
                                edition: metadata.edition.clone(),
                                is_favorite: false,
                                play_count: 0,
                            };

                            album.tracks.push(track);
                        }
                        Err(e) => {
                            log::warn!("Failed to extract metadata from {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }

        // Post-process to count unique albums per directory
        // This is expensive if we iterate everything again.
        // Maybe we can just iterate the album_map at the end?
        // For each album, look at its tracks, find their parent directories, and update the set of albums for that directory.

        // Let's do that in scan_incremental_with_progress instead of here.
        // So here we just update track counts.

        Ok((total_tracks, scanned_tracks))
    }

    /// Search albums with fuzzy matching across artist, album title, and track titles
    /// Uses FTS5 full-text search for fast prefix matching
    /// FTS index is synced after each library scan
    pub fn search_albums(&self, query: &str) -> Vec<&Album> {
        if query.is_empty() {
            return self.albums.iter().collect();
        }

        let mut results = Vec::new();

        // Use FTS5 search if database is available
        if let Some(db) = &self.db {
            if let Ok(album_ids) = db.search_library(query) {
                if !album_ids.is_empty() {
                    results = self
                        .albums
                        .iter()
                        .filter(|album| {
                            if let Some(id) = album.id {
                                album_ids.contains(&id)
                            } else {
                                false
                            }
                        })
                        .collect();
                }
            }
        }

        // Fallback to in-memory search if no results found via DB
        // This handles cases where:
        // 1. Database is not available (e.g. tests, or initialization failed)
        // 2. FTS index is out of sync or empty
        // 3. Search query matches something not indexed or FTS is too strict (e.g. substring)
        if results.is_empty() && !query.is_empty() {
            let query_lower = query.to_lowercase();
            results = self
                .albums
                .iter()
                .filter(|album| {
                    // Match album title
                    if album.title.to_lowercase().contains(&query_lower) {
                        return true;
                    }
                    // Match album artist
                    if album.artist().to_lowercase().contains(&query_lower) {
                        return true;
                    }
                    // Match track titles, artists, and filenames
                    for track in &album.tracks {
                        if let Some(title) = &track.title {
                            if title.to_lowercase().contains(&query_lower) {
                                return true;
                            }
                        }
                        // Match track artist
                        if let Some(artist) = &track.artist {
                            if artist.to_lowercase().contains(&query_lower) {
                                return true;
                            }
                        }
                        // Match track filename (for files with no metadata tags)
                        if let Some(filename) = track.path.file_stem() {
                            if filename
                                .to_string_lossy()
                                .to_lowercase()
                                .contains(&query_lower)
                            {
                                return true;
                            }
                        }
                    }
                    false
                })
                .collect();
        }

        results
    }

    /// Update directory scan times from database
    pub fn update_directory_scan_times(&mut self) {
        if let Some(db) = &self.db {
            for dir_info in &mut self.directories {
                if let Ok(Some(scan_time)) = db.get_last_scan_time(&dir_info.path) {
                    dir_info.last_scanned =
                        Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(scan_time));
                }
            }
        }
    }
}

#[derive(Debug, Default)]
struct TrackMetadata {
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

/// Create a custom probe with all supported format readers registered
fn create_probe() -> Probe {
    let mut probe = Probe::default();

    // Register metadata readers to read ID3 tags
    probe.register_all::<symphonia_metadata::id3v2::Id3v2Reader>();

    // Register all format readers to help probe find formats more efficiently
    probe.register_all::<symphonia_bundle_flac::FlacReader>();
    probe.register_all::<symphonia_bundle_mp3::MpaReader>();
    probe.register_all::<symphonia_format_riff::WavReader>();
    probe.register_all::<symphonia_format_ogg::OggReader>();
    probe.register_all::<symphonia_format_isomp4::IsoMp4Reader>();
    probe.register_all::<symphonia_codec_aac::AdtsReader>();

    probe
}

fn extract_metadata(path: &Path) -> Result<TrackMetadata, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension() {
        hint.with_extension(&ext.to_string_lossy());
    }

    let format_opts = FormatOptions::default();
    // Use larger limits to avoid crashes with files that have large metadata or embedded artwork
    let metadata_opts = MetadataOptions {
        limit_metadata_bytes: Limit::Maximum(10 * 1024 * 1024), // 10 MB for metadata
        limit_visual_bytes: Limit::Maximum(20 * 1024 * 1024),   // 20 MB for embedded artwork
    };

    // Use custom probe with registered formats for better format detection
    let probe = create_probe();
    let mut probed = probe.format(&hint, mss, &format_opts, &metadata_opts)?;

    let mut metadata = TrackMetadata::default();

    // Extract duration, channel count, sample rate, and bit depth from the format
    if let Some(track) = probed.format.default_track() {
        // Get duration
        if let Some(time_base) = track.codec_params.time_base
            && let Some(n_frames) = track.codec_params.n_frames
        {
            let duration = time_base.calc_time(n_frames);
            metadata.duration_secs = Some(duration.seconds);
        }

        // Get channel count
        if let Some(channels) = track.codec_params.channels {
            metadata.channels = Some(channels.count() as u32);
        }

        // Get sample rate
        if let Some(sample_rate) = track.codec_params.sample_rate {
            metadata.sample_rate = Some(sample_rate);
        }

        // Get bit depth (bits per sample)
        if let Some(bits_per_sample) = track.codec_params.bits_per_sample {
            metadata.bit_depth = Some(bits_per_sample);
        }
    }

    // Extract metadata tags
    use symphonia::core::meta::StandardTagKey;

    // Helper to process tags from a metadata revision
    let mut process_tags = |metadata_rev: &symphonia::core::meta::MetadataRevision| {
        for tag in metadata_rev.tags() {
            match tag.std_key {
                Some(StandardTagKey::TrackTitle) => {
                    metadata.title = Some(tag.value.to_string());
                }
                Some(StandardTagKey::Artist) => {
                    metadata.artist = Some(tag.value.to_string());
                }
                Some(StandardTagKey::Album) => {
                    metadata.album = Some(tag.value.to_string());
                }
                Some(StandardTagKey::TrackNumber) => {
                    // Handle "1/12" format (track/total)
                    let value = tag.value.to_string();
                    let track_num = value.split('/').next().unwrap_or(&value);
                    if let Ok(num) = track_num.trim().parse() {
                        metadata.track_number = Some(num);
                    }
                }
                Some(StandardTagKey::Date) | Some(StandardTagKey::ReleaseDate) => {
                    // Try to extract year from date string
                    let date_str = tag.value.to_string();
                    if let Some(year_str) = date_str.split('-').next()
                        && let Ok(year) = year_str.parse()
                    {
                        metadata.year = Some(year);
                    }
                }
                Some(StandardTagKey::Genre) => {
                    metadata.genre = Some(tag.value.to_string());
                }
                Some(StandardTagKey::Composer) => {
                    metadata.composer = Some(tag.value.to_string());
                }
                Some(StandardTagKey::DiscNumber) => {
                    // Handle "1/2" format (disc/total)
                    let value = tag.value.to_string();
                    let disc_num = value.split('/').next().unwrap_or(&value);
                    if let Ok(num) = disc_num.trim().parse() {
                        metadata.disc_number = Some(num);
                    }
                }
                Some(StandardTagKey::Conductor) => {
                    metadata.conductor = Some(tag.value.to_string());
                }
                Some(StandardTagKey::Performer) => {
                    metadata.performer = Some(tag.value.to_string());
                }
                Some(StandardTagKey::IdentIsrc) => {
                    metadata.isrc = Some(tag.value.to_string());
                }
                Some(StandardTagKey::AlbumArtist) => {
                    metadata.album_artist = Some(tag.value.to_string());
                }
                Some(StandardTagKey::Ensemble) => {
                    metadata.ensemble = Some(tag.value.to_string());
                }
                _ => {}
            }
        }
    };

    // First check metadata from the probe phase (for ID3 tags in MP3 files)
    // ID3v2 tags are read during probe and stored in probed.metadata
    if let Some(metadata_rev) = probed.metadata.get().as_ref().and_then(|m| m.current()) {
        process_tags(metadata_rev);
    }

    // Then check format metadata (for tags embedded in the container, like FLAC, OGG)
    // This may override probe metadata if both are present
    if let Some(metadata_rev) = probed.format.metadata().current() {
        process_tags(metadata_rev);
    }

    // Try to detect edition from directory name
    if let Some(parent) = path.parent() {
        if let Some(dir_name) = parent.file_name().map(|n| n.to_string_lossy()) {
            let dir_str = dir_name.as_ref();
            let mut edition = None;

            // Look for (...)
            if let Some(start) = dir_str.rfind('(') {
                if let Some(end) = dir_str.rfind(')') {
                    if end > start {
                        edition = Some(dir_str[start + 1..end].trim().to_string());
                    }
                }
            }
            // Look for [...] - prioritizing brackets if present
            if let Some(start) = dir_str.rfind('[') {
                if let Some(end) = dir_str.rfind(']') {
                    if end > start {
                        edition = Some(dir_str[start + 1..end].trim().to_string());
                    }
                }
            }

            if let Some(ed) = edition {
                // Only use if it looks like an edition info (heuristic)
                metadata.edition = Some(ed);
            }
        }
    }

    Ok(metadata)
}

/// Get file modification time as Unix timestamp
fn get_file_mtime(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
}

/// Build directory info recursively without stats (for new directories)
fn build_directory_info(path: PathBuf) -> DirectoryInfo {
    let mut subdirectories = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&path) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.path().is_dir() {
                subdirectories.push(build_directory_info(entry.path()));
            }
        }
    }
    // Sort subdirectories by name
    subdirectories.sort_by(|a, b| a.path.file_name().cmp(&b.path.file_name()));

    DirectoryInfo {
        path,
        file_count: 0,
        album_count: 0,
        last_scanned: None,
        expanded: false,
        subdirectories,
    }
}

/// Build directory tree from disk (for loading from database)
/// Computes subdirectory stats from the albums and tracks in the database
fn build_directory_tree_from_disk(
    path: PathBuf,
    file_count: usize,
    album_count: usize,
    last_scanned: Option<SystemTime>,
) -> DirectoryInfo {
    let mut subdirectories = Vec::new();

    // Read immediate subdirectories from disk
    if let Ok(entries) = std::fs::read_dir(&path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                // Recursively build subdirectory tree
                // Stats will be 0 for subdirectories (they need to be scanned)
                subdirectories.push(build_directory_tree_from_disk(entry_path, 0, 0, None));
            }
        }
    }

    // Sort subdirectories by name
    subdirectories.sort_by(|a, b| a.path.file_name().cmp(&b.path.file_name()));

    DirectoryInfo {
        path,
        file_count,
        album_count,
        last_scanned,
        expanded: false,
        subdirectories,
    }
}

/// Compute directory stats from albums in the library
/// Returns a map of directory path -> (track count, album count)
/// Uses both original and canonicalized paths as keys for robust matching
fn compute_directory_stats(albums: &[Album]) -> HashMap<PathBuf, (usize, usize)> {
    let mut dir_track_counts: HashMap<PathBuf, usize> = HashMap::new();
    let mut dir_album_sets: HashMap<PathBuf, std::collections::HashSet<String>> = HashMap::new();

    for album in albums {
        // Album is now uniquely identified by title alone
        let album_key = album.title.clone();

        for track in &album.tracks {
            // Get the parent directory of this track
            if let Some(parent) = track.path.parent() {
                let parent_buf = parent.to_path_buf();

                // Count tracks for this directory (using original path)
                *dir_track_counts.entry(parent_buf.clone()).or_insert(0) += 1;

                // Add album to set for this directory (to count unique albums)
                dir_album_sets
                    .entry(parent_buf.clone())
                    .or_default()
                    .insert(album_key.clone());

                // Also add entry for canonicalized path if different
                // This ensures matching works regardless of symlinks or case sensitivity
                if let Ok(canonical) = parent_buf.canonicalize() {
                    if canonical != parent_buf {
                        *dir_track_counts.entry(canonical.clone()).or_insert(0) += 1;
                        dir_album_sets
                            .entry(canonical)
                            .or_default()
                            .insert(album_key.clone());
                    }
                }
            }
        }
    }

    // Combine track counts and album counts
    let mut result = HashMap::new();
    for (dir, track_count) in dir_track_counts {
        let album_count = dir_album_sets.get(&dir).map(|set| set.len()).unwrap_or(0);
        result.insert(dir, (track_count, album_count));
    }

    result
}

/// Update directory info with stats computed from albums
/// Recursively aggregates stats from subdirectories
fn update_directory_stats_from_map(
    dir_info: &mut DirectoryInfo,
    stats_map: &HashMap<PathBuf, (usize, usize)>,
) {
    // First, recursively update all subdirectories
    for subdir in &mut dir_info.subdirectories {
        update_directory_stats_from_map(subdir, stats_map);
    }

    // Then compute this directory's stats
    // Start with direct files in this directory
    let (mut my_tracks, mut my_albums) = stats_map.get(&dir_info.path).cloned().unwrap_or((0, 0));

    // Add stats from all subdirectories
    for subdir in &dir_info.subdirectories {
        my_tracks += subdir.file_count;
        my_albums += subdir.album_count;
    }

    // Update this directory's stats
    dir_info.file_count = my_tracks;
    dir_info.album_count = my_albums;
}

/// Common album art file names to look for (case-insensitive)
const ALBUM_ART_FILENAMES: &[&str] = &[
    "cover.jpg",
    "cover.jpeg",
    "cover.png",
    "folder.jpg",
    "folder.jpeg",
    "folder.png",
    "front.jpg",
    "front.jpeg",
    "front.png",
    "album.jpg",
    "album.jpeg",
    "album.png",
    "artwork.jpg",
    "artwork.jpeg",
    "artwork.png",
];

/// Check if a file is an image based on extension
fn is_image_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();
        matches!(ext_lower.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp")
    } else {
        false
    }
}

/// Find album art in the given directory with smart heuristics:
/// 1. Look for common filenames (cover, folder, front, album, artwork)
/// 2. Look for files with "front" in the name (case-insensitive)
/// 3. If only one image file exists in the directory, use it
/// 4. Check subdirectories named "Artwork" or "Covers" (case-insensitive)
fn find_album_art(dir: &Path) -> Option<PathBuf> {
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
fn find_album_art_in_subdir(dir: &Path) -> Option<PathBuf> {
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

/// Thumbnail size in pixels (160x160 for crisp display on high-DPI screens)
const THUMBNAIL_SIZE: u32 = 160;

/// Generate a PNG thumbnail from an image file
///
/// PNG format is used instead of JPEG for several reasons:
/// - Lossless compression preserves quality
/// - Standardized format ensures consistent stride/pitch for rendering
/// - Better compatibility with GPUI's image rendering pipeline
/// - Supports alpha channel if needed
fn generate_thumbnail(image_path: &Path) -> Option<Vec<u8>> {
    use image::ImageReader;
    use std::io::Cursor;

    // Load the image
    let img = match ImageReader::open(image_path) {
        Ok(reader) => match reader.decode() {
            Ok(img) => img,
            Err(e) => {
                log::warn!("Failed to decode image {}: {}", image_path.display(), e);
                return None;
            }
        },
        Err(e) => {
            log::warn!("Failed to open image {}: {}", image_path.display(), e);
            return None;
        }
    };

    // Resize to thumbnail size using Lanczos3 for quality
    let thumbnail = img.thumbnail(THUMBNAIL_SIZE, THUMBNAIL_SIZE);

    // Encode as PNG (lossless, standardized format)
    let mut buffer = Cursor::new(Vec::new());
    if let Err(e) = thumbnail.write_to(&mut buffer, image::ImageFormat::Png) {
        log::warn!(
            "Failed to encode thumbnail for {}: {}",
            image_path.display(),
            e
        );
        return None;
    }

    Some(buffer.into_inner())
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

    #[test]
    fn test_library_creation() {
        let lib = MusicLibrary::new();
        assert_eq!(lib.directories.len(), 0);
        assert_eq!(lib.albums.len(), 0);
    }

    #[test]
    fn test_add_remove_directory() {
        let mut lib = MusicLibrary::new();
        let path = PathBuf::from("/tmp/music");

        let result = lib.add_directory(path.clone());
        assert!(result.is_ok());
        assert_eq!(lib.directories.len(), 1);

        lib.remove_directory(0);
        assert_eq!(lib.directories.len(), 0);
    }

    #[test]
    fn test_add_directory_subtree_detection() {
        let mut lib = MusicLibrary::new();

        // Add parent directory
        let parent = PathBuf::from("/tmp/music");
        assert!(lib.add_directory(parent.clone()).is_ok());
        assert_eq!(lib.directories.len(), 1);

        // Try to add a subdirectory - should fail
        let child = PathBuf::from("/tmp/music/jazz");
        let result = lib.add_directory(child);
        assert!(result.is_err());
        assert_eq!(lib.directories.len(), 1); // Still just the parent

        // Add a sibling directory - should succeed
        let sibling = PathBuf::from("/tmp/videos");
        assert!(lib.add_directory(sibling).is_ok());
        assert_eq!(lib.directories.len(), 2);
    }

    #[test]
    fn test_search_albums_empty_library() {
        let lib = MusicLibrary::new();
        let results = lib.search_albums("test");
        assert_eq!(results.len(), 0);
    }

    /// Helper function to create a test track with just an artist
    fn test_track_with_artist(artist: &str) -> Track {
        Track {
            path: PathBuf::from("/test/track.flac"),
            title: Some("Test Track".to_string()),
            artist: Some(artist.to_string()),
            track_number: Some(1),
            duration_secs: None,
            channels: None,
            sample_rate: None,
            bit_depth: None,
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: None,
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
            edition: None,
            is_favorite: false,
            play_count: 0,
        }
    }

    #[test]
    fn test_search_albums_in_memory() {
        let mut lib = MusicLibrary::new();

        // Add some test albums with tracks containing artist info
        lib.albums.push(Album {
            id: None,
            title: "The Dark Side of the Moon".to_string(),
            year: Some(1973),
            tracks: vec![test_track_with_artist("Pink Floyd")],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
            edition: None,
            dynamic_range: None,
            is_favorite: false,
        });

        lib.albums.push(Album {
            id: None,
            title: "Abbey Road".to_string(),
            year: Some(1969),
            tracks: vec![test_track_with_artist("The Beatles")],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
            edition: None,
            dynamic_range: None,
            is_favorite: false,
        });

        lib.albums.push(Album {
            id: None,
            title: "IV".to_string(),
            year: Some(1971),
            tracks: vec![test_track_with_artist("Led Zeppelin")],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
            edition: None,
            dynamic_range: None,
            is_favorite: false,
        });

        // Search by artist (case insensitive)
        let results = lib.search_albums("pink");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].artist(), "Pink Floyd");

        // Search by album title
        let results = lib.search_albums("abbey");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Abbey Road");

        // Search by partial match
        let results = lib.search_albums("ze");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].artist(), "Led Zeppelin");

        // Search with no results
        let results = lib.search_albums("nonexistent");
        assert_eq!(results.len(), 0);

        // Empty search should return all albums
        let results = lib.search_albums("");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_search_albums_case_insensitive() {
        let mut lib = MusicLibrary::new();

        lib.albums.push(Album {
            id: None,
            title: "Master of Puppets".to_string(),
            year: Some(1986),
            tracks: vec![test_track_with_artist("Metallica")],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
            edition: None,
            dynamic_range: None,
            is_favorite: false,
        });

        // Test various case combinations
        assert_eq!(lib.search_albums("metallica").len(), 1);
        assert_eq!(lib.search_albums("METALLICA").len(), 1);
        assert_eq!(lib.search_albums("MeTaLLiCa").len(), 1);
        assert_eq!(lib.search_albums("master").len(), 1);
        assert_eq!(lib.search_albums("MASTER").len(), 1);
    }

    #[test]
    fn test_load_directories_from_database() {
        use crate::database::MusicDatabase;
        use tempfile::TempDir;

        // Create a temporary directory for the test database
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_music.db");

        // Create and populate database (using test-only method)
        {
            let db = MusicDatabase::open_for_testing(&db_path).unwrap();

            // Simulate a scan by recording scan history
            db.record_scan(&PathBuf::from("/music/rock"), 50, 5)
                .unwrap();
            db.record_scan(&PathBuf::from("/music/jazz"), 30, 3)
                .unwrap();
        }

        // Create library with the test database
        let mut lib = MusicLibrary {
            directories: Vec::new(),
            albums: Vec::new(),
            db: Some(MusicDatabase::open_for_testing(&db_path).unwrap()),
        };

        // Load from database
        lib.load_from_database().unwrap();

        // Note: directories will only be loaded if they exist on disk
        // In this test, they don't exist, so directories should be empty
        // In a real scenario where the paths exist, they would be loaded
        assert_eq!(lib.directories.len(), 0); // Paths don't exist
    }

    #[test]
    fn test_load_directories_filters_subtrees() {
        use crate::database::MusicDatabase;
        use tempfile::TempDir;

        // Create actual temporary directories on disk
        let temp_root = TempDir::new().unwrap();
        let parent_dir = temp_root.path().join("music");
        let child_dir = parent_dir.join("rock");

        std::fs::create_dir_all(&parent_dir).unwrap();
        std::fs::create_dir_all(&child_dir).unwrap();

        // Create database (using test-only method)
        let db_path = temp_root.path().join("test_music.db");
        {
            let db = MusicDatabase::open_for_testing(&db_path).unwrap();

            // Record scans for both parent and child
            db.record_scan(&parent_dir, 100, 10).unwrap();
            db.record_scan(&child_dir, 50, 5).unwrap();
        }

        // Create library and load from database
        let mut lib = MusicLibrary {
            directories: Vec::new(),
            albums: Vec::new(),
            db: Some(MusicDatabase::open_for_testing(&db_path).unwrap()),
        };

        lib.load_from_database().unwrap();

        // Should only have 1 directory (the parent), not the child
        assert_eq!(lib.directories.len(), 1);

        // Verify it's the parent directory
        let canonical_parent = parent_dir.canonicalize().unwrap();
        let loaded_path = lib.directories[0].path.canonicalize().unwrap();
        assert_eq!(loaded_path, canonical_parent);
    }

    #[test]
    fn test_clean_album_title() {
        // Basic cases
        assert_eq!(clean_album_title("Album Title"), "Album Title");
        assert_eq!(clean_album_title("Album Title (CD 1)"), "Album Title");
        assert_eq!(clean_album_title("Album Title (CD-1)"), "Album Title");
        assert_eq!(clean_album_title("Album Title (CD - 1)"), "Album Title");
        assert_eq!(clean_album_title("Album Title (Disc 1)"), "Album Title");
        assert_eq!(clean_album_title("Album Title (Disc-1)"), "Album Title");

        // Case insensitivity
        assert_eq!(clean_album_title("Album Title (cd 1)"), "Album Title");
        assert_eq!(clean_album_title("Album Title (DISC 1)"), "Album Title");

        // Without parentheses
        assert_eq!(clean_album_title("Album Title CD 1"), "Album Title");
        assert_eq!(clean_album_title("Album Title Disc 1"), "Album Title");
        assert_eq!(clean_album_title("Album Title Vol. 1"), "Album Title");

        // User reported cases
        assert_eq!(
            clean_album_title("After The Fall (CD - 2)"),
            "After The Fall"
        );
        assert_eq!(clean_album_title("After The Fall (CD-1)"), "After The Fall");
        assert_eq!(
            clean_album_title("A Night On The Town(CD 1)"),
            "A Night On The Town"
        );
        assert_eq!(
            clean_album_title("A Night On The Town [CD 1]"),
            "A Night On The Town"
        );
        assert_eq!(
            clean_album_title("A Night On The Town[CD 1]"),
            "A Night On The Town"
        );

        // No space before number (CD1, Disc2)
        assert_eq!(clean_album_title("ALPHA & OMEGA CD1"), "ALPHA & OMEGA");
        assert_eq!(clean_album_title("Alpha & Omega CD2"), "Alpha & Omega");
        assert_eq!(clean_album_title("Album Title CD1"), "Album Title");
        assert_eq!(clean_album_title("Album Title Disc2"), "Album Title");
        assert_eq!(clean_album_title("Album Title (CD1)"), "Album Title");

        // Catalog numbers in parentheses
        assert_eq!(
            clean_album_title("A Night On The Town (3116-2)"),
            "A Night On The Town"
        );
        assert_eq!(
            clean_album_title("A Night On The Town (R2 47730)"),
            "A Night On The Town"
        );
        assert_eq!(clean_album_title("Album Title (ABC-12345)"), "Album Title");
        assert_eq!(clean_album_title("Album Title (MFSL 1234)"), "Album Title");

        // Catalog numbers in square brackets
        assert_eq!(
            clean_album_title("Passion [RWCD 1]"),
            "Passion"
        );
        assert_eq!(
            clean_album_title("Shaking The Tree [PGCD 7]"),
            "Shaking The Tree"
        );
        assert_eq!(
            clean_album_title("Us [PGCD 7] - Digipack"),
            "Us [PGCD 7] - Digipack" // not at end, so not stripped
        );
        assert_eq!(
            clean_album_title("Album Title [ABC-123]"),
            "Album Title"
        );

        // Should NOT clean
        assert_eq!(clean_album_title("AC/DC"), "AC/DC");
        assert_eq!(clean_album_title("Disco Volante"), "Disco Volante");
        assert_eq!(clean_album_title("The CD Is Dead"), "The CD Is Dead");
        assert_eq!(clean_album_title("Album (Live)"), "Album (Live)");
        assert_eq!(
            clean_album_title("Album (Remastered)"),
            "Album (Remastered)"
        );
        assert_eq!(
            clean_album_title("Album (Deluxe Edition)"),
            "Album (Deluxe Edition)"
        );
    }

    fn create_test_album(title: &str, artist: &str, disc: u32) -> Album {
        Album {
            id: Some(1),
            title: title.to_string(),
            year: Some(2024),
            tracks: vec![Track {
                path: std::path::PathBuf::from("test"),
                title: Some("Test Track".to_string()),
                artist: Some(artist.to_string()),
                album_artist: Some(artist.to_string()),
                track_number: Some(1),
                disc_number: Some(disc),
                duration_secs: Some(180),
                channels: Some(2),
                sample_rate: Some(44100),
                bit_depth: Some(16),
                genre: None,
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                composer: None,
                conductor: None,
                performer: None,
                isrc: None,
                ensemble: None,
                edition: None,
                is_favorite: false,
                play_count: 0,
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
            edition: None,
            dynamic_range: None,
            is_favorite: false,
        }
    }

    #[test]
    fn test_album_grouping_regression() {
        // Test multi-disc album scenario: different disc numbers should merge
        let albums = vec![
            create_test_album("A Night On The Town", "Rod Stewart", 1),
            create_test_album("A Night On The Town (CD 1)", "Rod Stewart", 1),
            create_test_album("A Night On The Town (CD 2)", "Rod Stewart", 2),
        ];

        let album_refs: Vec<&Album> = albums.iter().collect();
        let merged = group_and_merge_albums(album_refs);

        assert_eq!(merged.len(), 1, "Should have grouped into 1 album");
        assert_eq!(merged[0].title, "A Night On The Town");
        // Tracks with same title+disc+track_number get deduplicated
        // So disc 1 tracks from album 1 and 2 merge, plus disc 2 track = 2 unique tracks
        assert_eq!(merged[0].tracks.len(), 2);
    }

    #[test]
    fn test_album_grouping_multi_disc() {
        // Test proper multi-disc album where each disc has unique tracks
        let mut album_cd1 = create_test_album("The Wall (CD 1)", "Pink Floyd", 1);
        album_cd1.tracks[0].title = Some("In The Flesh?".to_string());
        album_cd1.tracks[0].track_number = Some(1);

        let mut album_cd2 = create_test_album("The Wall (CD 2)", "Pink Floyd", 2);
        album_cd2.tracks[0].title = Some("Hey You".to_string());
        album_cd2.tracks[0].track_number = Some(1);

        let albums = vec![album_cd1, album_cd2];
        let album_refs: Vec<&Album> = albums.iter().collect();
        let merged = group_and_merge_albums(album_refs);

        assert_eq!(merged.len(), 1, "Should have grouped into 1 album");
        assert_eq!(merged[0].title, "The Wall");
        // Both tracks should be preserved since they have different titles
        assert_eq!(merged[0].tracks.len(), 2);
    }

    #[test]
    fn test_album_grouping_uses_highest_dr() {
        // Test that highest DR album is selected for metadata
        // Both albums have the same base title but different disc markers
        let mut album_low_dr = create_test_album("Dark Side (CD 1)", "Pink Floyd", 1);
        album_low_dr.dynamic_range = Some(8.0);
        album_low_dr.year = Some(1973);

        let mut album_high_dr = create_test_album("Dark Side (CD 2)", "Pink Floyd", 2);
        album_high_dr.dynamic_range = Some(12.0);
        album_high_dr.year = Some(2011);

        let albums = vec![album_low_dr, album_high_dr];
        let album_refs: Vec<&Album> = albums.iter().collect();
        let merged = group_and_merge_albums(album_refs);

        assert_eq!(merged.len(), 1, "Should have grouped into 1 album");
        // Should use metadata from the higher DR album
        assert_eq!(merged[0].year, Some(2011));
    }

    #[test]
    fn test_normalize_album_key() {
        // Test basic normalization
        assert_eq!(
            normalize_album_key("2Cellos"),
            normalize_album_key("2CELLOS")
        );
        assert_eq!(
            normalize_album_key("2Cellos"),
            normalize_album_key("2 Cellos ")
        );

        // Test diacritics removal
        assert_eq!(normalize_album_key("Café"), "cafe");
        assert_eq!(normalize_album_key("Naïve"), "naive");
        assert_eq!(normalize_album_key("Björk"), "bjork");
        assert_eq!(normalize_album_key("Señor"), "senor");

        // Test special character removal
        assert_eq!(normalize_album_key("The Beatles!"), "thebeatles");
        assert_eq!(normalize_album_key("AC/DC"), "acdc");
        assert_eq!(normalize_album_key("Album: Title"), "albumtitle");
        assert_eq!(normalize_album_key("The Album, Vol. 2"), "thealbumvol.2");
        assert_eq!(normalize_album_key("Rock & Roll"), "rockroll");

        // Test that periods are kept
        assert_eq!(normalize_album_key("Vol. 2"), "vol.2");
        assert_eq!(normalize_album_key("U.S.A."), "u.s.a.");

        // Test numbers are kept
        assert_eq!(normalize_album_key("2Pac"), "2pac");
        assert_eq!(normalize_album_key("Album 123"), "album123");

        // Test UTF-8 letters and numbers are kept
        assert_eq!(normalize_album_key("日本語"), "日本語");
        assert_eq!(normalize_album_key("Москва"), "москва");
        assert_eq!(normalize_album_key("Αθήνα"), "αθηνα");
    }

    #[test]
    fn test_is_image_file() {
        // Valid image extensions
        assert!(is_image_file(&PathBuf::from("cover.jpg")));
        assert!(is_image_file(&PathBuf::from("cover.jpeg")));
        assert!(is_image_file(&PathBuf::from("cover.JPG")));
        assert!(is_image_file(&PathBuf::from("cover.png")));
        assert!(is_image_file(&PathBuf::from("cover.PNG")));
        assert!(is_image_file(&PathBuf::from("cover.gif")));
        assert!(is_image_file(&PathBuf::from("cover.webp")));

        // Invalid extensions
        assert!(!is_image_file(&PathBuf::from("track.flac")));
        assert!(!is_image_file(&PathBuf::from("track.mp3")));
        assert!(!is_image_file(&PathBuf::from("readme.txt")));
        assert!(!is_image_file(&PathBuf::from("no_extension")));
    }

    #[test]
    fn test_find_album_art_common_names() {
        use std::fs::File;
        use tempfile::TempDir;

        // Create temporary directory with a common album art filename
        let temp_dir = TempDir::new().unwrap();
        let cover_path = temp_dir.path().join("cover.jpg");
        File::create(&cover_path).unwrap();

        let result = find_album_art(temp_dir.path());
        assert!(result.is_some());
        assert_eq!(result.unwrap(), cover_path);
    }

    #[test]
    fn test_find_album_art_front_in_name() {
        use std::fs::File;
        use tempfile::TempDir;

        // Create temporary directory with a file containing "front" in the name
        let temp_dir = TempDir::new().unwrap();
        let front_path = temp_dir.path().join("booklet_front.jpg");
        let back_path = temp_dir.path().join("booklet_back.jpg");
        File::create(&front_path).unwrap();
        File::create(&back_path).unwrap();

        let result = find_album_art(temp_dir.path());
        assert!(result.is_some());
        assert_eq!(result.unwrap(), front_path);
    }

    #[test]
    fn test_find_album_art_single_image() {
        use std::fs::File;
        use tempfile::TempDir;

        // Create temporary directory with only one image file
        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().join("some_random_name.jpg");
        File::create(&image_path).unwrap();
        // Add a non-image file
        File::create(temp_dir.path().join("track.flac")).unwrap();

        let result = find_album_art(temp_dir.path());
        assert!(result.is_some());
        assert_eq!(result.unwrap(), image_path);
    }

    #[test]
    fn test_find_album_art_in_artwork_subdir() {
        use std::fs::{File, create_dir};
        use tempfile::TempDir;

        // Create temporary directory with an "Artwork" subdirectory
        let temp_dir = TempDir::new().unwrap();
        let artwork_dir = temp_dir.path().join("Artwork");
        create_dir(&artwork_dir).unwrap();
        let cover_path = artwork_dir.join("cover.jpg");
        File::create(&cover_path).unwrap();

        let result = find_album_art(temp_dir.path());
        assert!(result.is_some());
        assert_eq!(result.unwrap(), cover_path);
    }

    #[test]
    fn test_find_album_art_in_covers_subdir_lowercase() {
        use std::fs::{File, create_dir};
        use tempfile::TempDir;

        // Create temporary directory with a "covers" subdirectory (lowercase)
        let temp_dir = TempDir::new().unwrap();
        let covers_dir = temp_dir.path().join("covers");
        create_dir(&covers_dir).unwrap();
        let front_path = covers_dir.join("front.png");
        File::create(&front_path).unwrap();

        let result = find_album_art(temp_dir.path());
        assert!(result.is_some());
        assert_eq!(result.unwrap(), front_path);
    }

    #[test]
    fn test_find_album_art_no_images() {
        use std::fs::File;
        use tempfile::TempDir;

        // Create temporary directory with only audio files
        let temp_dir = TempDir::new().unwrap();
        File::create(temp_dir.path().join("track1.flac")).unwrap();
        File::create(temp_dir.path().join("track2.flac")).unwrap();

        let result = find_album_art(temp_dir.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_standalone_tracks_not_merged() {
        // Standalone tracks (no album tag) with different titles should stay separate
        let make_standalone = |title: &str, path: &str| -> Album {
            let mut album = create_test_album(title, "Artist X", 1);
            album.tracks[0].path = PathBuf::from(path);
            album.tracks[0].title = Some(title.to_string());
            album
        };
        let album_a = make_standalone("MyFolder - Song A", "/music/MyFolder/song_a.flac");
        let album_b = make_standalone("MyFolder - Song B", "/music/MyFolder/song_b.flac");

        let albums = vec![album_a, album_b];
        let album_refs: Vec<&Album> = albums.iter().collect();
        let merged = group_and_merge_albums(album_refs);

        assert_eq!(
            merged.len(),
            2,
            "Standalone tracks with different titles must not merge"
        );
    }

    #[test]
    fn test_different_artists_same_title_not_merged() {
        // Bug: "Greatest Hits" by Queen and "Greatest Hits" by Fleetwood Mac
        // were merged into one album because grouping key was title-only.
        let album_a = create_test_album("Greatest Hits", "Queen", 1);
        let album_b = create_test_album("Greatest Hits", "Fleetwood Mac", 1);

        let albums = vec![album_a, album_b];
        let album_refs: Vec<&Album> = albums.iter().collect();
        let merged = group_and_merge_albums(album_refs);

        assert_eq!(
            merged.len(),
            2,
            "Albums with same title but different artists must not merge"
        );
    }

    #[test]
    fn test_unknown_artist_merges_with_known_artist() {
        // "2CELLOS" with "Unknown Artist" should merge with "2Cellos" by "2Cellos"
        let album_a = create_test_album("2Cellos", "Unknown Artist", 1);
        let mut album_b = create_test_album("2Cellos", "2Cellos", 1);
        album_b.tracks[0].title = Some("Smooth Criminal".to_string());

        let albums = vec![album_a, album_b];
        let album_refs: Vec<&Album> = albums.iter().collect();
        let merged = group_and_merge_albums(album_refs);

        assert_eq!(
            merged.len(),
            1,
            "Unknown artist album should merge with known artist album of same title"
        );
        // Should pick the known artist
        assert_eq!(merged[0].artist(), "2Cellos");
    }

    #[test]
    fn test_bracket_catalog_numbers_merge() {
        // Bug: albums with catalog numbers in brackets like [RWCD 1] and [RWCD 2]
        // were not merged because clean_album_title didn't strip bracket catalogs.
        let mut album1 = create_test_album("Passion [RWCD 1]", "Unknown Artist", 1);
        album1.tracks[0].title = Some("Track A".to_string());
        let mut album2 = create_test_album("Passion [RWCD 2]", "Unknown Artist", 2);
        album2.tracks[0].title = Some("Track B".to_string());
        let mut album3 = create_test_album("Passion [RWCD 3]", "Unknown Artist", 3);
        album3.tracks[0].title = Some("Track C".to_string());

        let albums = vec![album1, album2, album3];
        let album_refs: Vec<&Album> = albums.iter().collect();
        let merged = group_and_merge_albums(album_refs);

        assert_eq!(
            merged.len(),
            1,
            "Albums with same title but different bracket catalog numbers should merge"
        );
        assert_eq!(merged[0].title, "Passion");
        assert_eq!(merged[0].tracks.len(), 3);
    }

    #[test]
    fn test_real_albums_still_merge() {
        // Tracks that DO have a real album tag should still merge normally
        let album1 = create_test_album("Abbey Road (CD 1)", "The Beatles", 1);
        let mut album2 = create_test_album("Abbey Road (CD 2)", "The Beatles", 2);
        album2.tracks[0].title = Some("Here Comes The Sun".to_string());

        let albums = vec![album1, album2];
        let album_refs: Vec<&Album> = albums.iter().collect();
        let merged = group_and_merge_albums(album_refs);

        assert_eq!(merged.len(), 1, "Real albums with same title should merge");
        assert_eq!(merged[0].tracks.len(), 2);
    }
}

/// Helper function to group and merge albums
///
/// For albums with the same title (regardless of artist):
/// - Merge all tracks from all albums (for multi-disc albums)
/// - Deduplicate tracks by title + disc + track number (for duplicate editions)
/// - Keep the album metadata (year, art) from the album with highest DR
/// - Prefer albums with known artists over "Unknown Artist" or "Various Artists"
pub fn group_and_merge_albums(albums: Vec<&Album>) -> Vec<Album> {
    // Group by (normalized_title, normalized_artist) to keep different artists separate.
    // Albums with unknown/various artists are merged into a matching known-artist group
    // if one exists for the same title.

    let is_unknown_artist = |artist: &str| -> bool {
        let lower = artist.to_lowercase();
        lower.contains("unknown") || lower.contains("various")
    };

    // First pass: group by (title, artist)
    let mut groups: std::collections::HashMap<(String, String), Vec<&Album>> =
        std::collections::HashMap::new();

    for album in &albums {
        let title = album.title.trim();
        let normalized_title = clean_album_title(title).trim().to_lowercase();
        let artist = album.artist();
        let normalized_artist = normalize_album_key(&artist);
        groups
            .entry((normalized_title, normalized_artist))
            .or_default()
            .push(album);
    }

    // Second pass: merge unknown-artist groups into known-artist groups with the same title
    let unknown_keys: Vec<(String, String)> = groups
        .keys()
        .filter(|(title, artist_key)| {
            // Check if all albums in this group have unknown artists
            groups[&(title.clone(), artist_key.clone())]
                .iter()
                .all(|a| is_unknown_artist(&a.artist()))
        })
        .cloned()
        .collect();

    for (title, unknown_artist_key) in unknown_keys {
        // Find a known-artist group with the same title
        let known_key = groups
            .keys()
            .find(|(t, ak)| {
                *t == title
                    && *ak != unknown_artist_key
                    && groups[&(t.clone(), ak.clone())]
                        .iter()
                        .any(|a| !is_unknown_artist(&a.artist()))
            })
            .cloned();

        if let Some(target_key) = known_key {
            if let Some(unknown_albums) = groups.remove(&(title, unknown_artist_key)) {
                groups.get_mut(&target_key).unwrap().extend(unknown_albums);
            }
        }
    }

    // Merge albums and deduplicate tracks
    let mut merged_albums: Vec<Album> = Vec::new();
    for group in groups.values() {
        if group.is_empty() {
            continue;
        }

        // Helper to check if an artist is a "known" artist (not Unknown/Various)
        let is_known_artist = |artist: &str| -> bool {
            let lower = artist.to_lowercase();
            !lower.contains("unknown") && !lower.contains("various")
        };

        // Select the best album for metadata:
        // 1. Prefer albums with known artists over "Unknown Artist" / "Various Artists"
        // 2. Among albums with same artist quality, prefer highest dynamic range
        let best_album = group
            .iter()
            .max_by(|a, b| {
                let a_known = is_known_artist(&a.artist());
                let b_known = is_known_artist(&b.artist());

                // First compare by artist quality (known > unknown)
                match (a_known, b_known) {
                    (true, false) => return std::cmp::Ordering::Greater,
                    (false, true) => return std::cmp::Ordering::Less,
                    _ => {}
                }

                // Then by dynamic range
                let dr_a = a.dynamic_range.unwrap_or(0.0);
                let dr_b = b.dynamic_range.unwrap_or(0.0);
                dr_a.partial_cmp(&dr_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(&group[0]);

        let mut album = (*best_album).clone();

        // Clean up the title if there are multiple editions
        if group.len() > 1 {
            album.title = clean_album_title(&album.title);

            // Collect all tracks from all albums in the group
            let mut all_tracks = Vec::new();
            for g_album in group {
                all_tracks.extend(g_album.tracks.clone());
            }
            album.tracks = all_tracks;

            // Normalize album_artist on merged tracks to the best album's artist
            // so that Album::artist() doesn't return "Various Artists" due to
            // unknown-artist tracks mixed with known-artist tracks.
            let best_artist = best_album.artist();
            if is_known_artist(&best_artist) {
                for track in &mut album.tracks {
                    if let Some(ref aa) = track.album_artist {
                        if !is_known_artist(aa) {
                            track.album_artist = Some(best_artist.clone());
                        }
                    }
                    if let Some(ref a) = track.artist {
                        if !is_known_artist(a) {
                            track.artist = Some(best_artist.clone());
                        }
                    }
                }
            }
        }

        // Deduplicate tracks by (title + disc + track number)
        // This preserves tracks from different discs even if they have the same title
        let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
        album.tracks.retain(|track| {
            let title_key = track
                .title
                .as_ref()
                .map(|t| t.to_lowercase())
                .unwrap_or_default();
            let disc = track.disc_number.unwrap_or(1);
            let track_num = track.track_number.unwrap_or(0);
            let key = format!("{}|{}|{}", title_key, disc, track_num);
            seen_keys.insert(key)
        });

        // Sort tracks by disc and track number
        album.tracks.sort_by(|a, b| {
            a.disc_number
                .unwrap_or(1)
                .cmp(&b.disc_number.unwrap_or(1))
                .then_with(|| {
                    a.track_number
                        .unwrap_or(0)
                        .cmp(&b.track_number.unwrap_or(0))
                })
        });

        merged_albums.push(album);
    }

    merged_albums
}
