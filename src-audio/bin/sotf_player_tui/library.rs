use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{Limit, MetadataOptions};
use symphonia::core::probe::{Hint, Probe};
use walkdir::WalkDir;

use crate::database::MusicDatabase;

#[derive(Debug, Clone)]
pub struct DirectoryInfo {
    pub path: PathBuf,
    pub file_count: usize,
    pub last_scanned: Option<SystemTime>,
    pub expanded: bool,
    pub subdirectories: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub path: PathBuf,
    pub title: Option<String>,
    pub track_number: Option<u32>,
    pub duration_secs: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Album {
    pub artist: String,
    pub title: String,
    pub year: Option<u32>,
    pub tracks: Vec<Track>,
    pub album_art_path: Option<PathBuf>,
}

impl Album {
    pub fn display_name(&self) -> String {
        if let Some(year) = self.year {
            format!("{} - {} ({})", self.artist, self.title, year)
        } else {
            format!("{} - {}", self.artist, self.title)
        }
    }

    pub fn sort_tracks(&mut self) {
        self.tracks.sort_by_key(|t| t.track_number);
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

        let db = MusicDatabase::open(&db_path)?;

        Ok(Self {
            directories: Vec::new(),
            albums: Vec::new(),
            db: Some(db),
        })
    }

    /// Load library from database
    pub fn load_from_database(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(db) = &self.db {
            self.albums = db.load_library()?;
        }
        Ok(())
    }

    pub fn add_directory(&mut self, path: PathBuf) {
        if !self.directories.iter().any(|d| d.path == path) {
            // Get immediate subdirectories
            let subdirectories = std::fs::read_dir(&path)
                .ok()
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().is_dir())
                        .map(|e| e.path())
                        .collect()
                })
                .unwrap_or_default();

            self.directories.push(DirectoryInfo {
                path,
                file_count: 0,
                last_scanned: None,
                expanded: false,
                subdirectories,
            });
        }
    }

    pub fn remove_directory(&mut self, index: usize) -> Option<PathBuf> {
        if index < self.directories.len() {
            Some(self.directories.remove(index).path)
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
        self.scan_incremental_with_progress(false, progress_callback)
    }

    /// Scan directories with optional incremental mode
    /// If incremental is true, only scan new or modified files
    #[allow(dead_code)]
    pub fn scan_incremental(
        &mut self,
        incremental: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.scan_incremental_with_progress(incremental, |_, _| {})
    }

    /// Scan directories with optional incremental mode and progress reporting
    fn scan_incremental_with_progress<F>(
        &mut self,
        incremental: bool,
        mut progress_callback: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnMut(usize, usize),
    {
        let mut album_map: HashMap<(String, String), Album> = HashMap::new();
        let mut total_tracks = 0;
        let mut scanned_tracks = 0;
        let scan_time = SystemTime::now();
        let mut last_progress_report = SystemTime::now();

        // Create a map of directory index to file count
        let mut dir_file_counts: HashMap<usize, usize> = HashMap::new();

        for (dir_idx, dir_info) in self.directories.clone().iter().enumerate() {
            let (tracks, scanned) =
                self.scan_directory(&dir_info.path, &mut album_map, incremental)?;
            total_tracks += tracks;
            scanned_tracks += scanned;
            dir_file_counts.insert(dir_idx, tracks);

            // Report progress after each directory and every 30 seconds
            let now = SystemTime::now();
            if now
                .duration_since(last_progress_report)
                .unwrap_or_default()
                .as_secs()
                >= 30
            {
                progress_callback(total_tracks, album_map.len());
                last_progress_report = now;
            }
        }

        // Final progress report
        progress_callback(total_tracks, album_map.len());

        // Update directory info with file counts and scan time
        for (dir_idx, file_count) in dir_file_counts {
            if let Some(dir_info) = self.directories.get_mut(dir_idx) {
                dir_info.file_count = file_count;
                dir_info.last_scanned = Some(scan_time);
            }
        }

        // Merge with existing albums if we have a database
        if let Some(db) = &self.db
            && incremental
        {
            // Load existing albums from database
            let existing_albums = db.load_library()?;

            // Merge existing albums that weren't updated
            for existing_album in existing_albums {
                let key = (existing_album.artist.clone(), existing_album.title.clone());
                album_map.entry(key).or_insert(existing_album);
            }
        }

        self.albums = album_map.into_values().collect();

        // Sort tracks within each album
        for album in &mut self.albums {
            album.sort_tracks();
        }

        // Sort albums by artist and title
        self.albums
            .sort_by(|a, b| a.artist.cmp(&b.artist).then(a.title.cmp(&b.title)));

        // Save to database if available
        if let Some(db) = &mut self.db {
            db.save_albums(&self.albums)?;

            // Record scan history for each directory
            for dir_info in &self.directories {
                db.record_scan(&dir_info.path, total_tracks, self.albums.len())?;
            }
        }

        log::info!(
            "Scan complete: {} tracks ({} scanned), {} albums",
            total_tracks,
            scanned_tracks,
            self.albums.len()
        );

        Ok(())
    }

    /// Clean up database by removing tracks for files that no longer exist
    pub fn clean_database(&mut self) -> Result<usize, Box<dyn std::error::Error>> {
        if let Some(db) = &mut self.db {
            let removed = db.clean_missing_files()?;
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
        album_map: &mut HashMap<(String, String), Album>,
        incremental: bool,
    ) -> Result<(usize, usize), Box<dyn std::error::Error>> {
        let mut total_tracks = 0;
        let mut scanned_tracks = 0;

        for entry in WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
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

                    if let Ok(metadata) = extract_metadata(path) {
                        let artist = metadata
                            .artist
                            .unwrap_or_else(|| "Unknown Artist".to_string());
                        let album_title = metadata
                            .album
                            .unwrap_or_else(|| "Unknown Album".to_string());
                        let key = (artist.clone(), album_title.clone());

                        let album = album_map.entry(key).or_insert_with(|| Album {
                            artist,
                            title: album_title,
                            year: metadata.year,
                            tracks: Vec::new(),
                            album_art_path: None,
                        });

                        let track = Track {
                            path: path.to_path_buf(),
                            title: metadata.title,
                            track_number: metadata.track_number,
                            duration_secs: metadata.duration_secs,
                        };

                        album.tracks.push(track);
                    }
                }
            }
        }

        Ok((total_tracks, scanned_tracks))
    }

    pub fn search_albums(&self, query: &str) -> Vec<&Album> {
        let query = query.to_lowercase();
        self.albums
            .iter()
            .filter(|album| {
                album.artist.to_lowercase().contains(&query)
                    || album.title.to_lowercase().contains(&query)
            })
            .collect()
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
}

/// Create a custom probe with all supported format readers registered
fn create_probe() -> Probe {
    let mut probe = Probe::default();

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

    // Extract duration from the format
    if let Some(track) = probed.format.default_track()
        && let Some(time_base) = track.codec_params.time_base
        && let Some(n_frames) = track.codec_params.n_frames
    {
        let duration = time_base.calc_time(n_frames);
        metadata.duration_secs = Some(duration.seconds);
    }

    // Extract metadata tags
    if let Some(metadata_rev) = probed.format.metadata().current() {
        for tag in metadata_rev.tags() {
            match tag.std_key {
                Some(symphonia::core::meta::StandardTagKey::TrackTitle) => {
                    metadata.title = Some(tag.value.to_string());
                }
                Some(symphonia::core::meta::StandardTagKey::Artist) => {
                    metadata.artist = Some(tag.value.to_string());
                }
                Some(symphonia::core::meta::StandardTagKey::Album) => {
                    metadata.album = Some(tag.value.to_string());
                }
                Some(symphonia::core::meta::StandardTagKey::TrackNumber) => {
                    if let Ok(num) = tag.value.to_string().parse() {
                        metadata.track_number = Some(num);
                    }
                }
                Some(symphonia::core::meta::StandardTagKey::Date)
                | Some(symphonia::core::meta::StandardTagKey::ReleaseDate) => {
                    // Try to extract year from date string
                    let date_str = tag.value.to_string();
                    if let Some(year_str) = date_str.split('-').next()
                        && let Ok(year) = year_str.parse()
                    {
                        metadata.year = Some(year);
                    }
                }
                _ => {}
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

        lib.add_directory(path.clone());
        assert_eq!(lib.directories.len(), 1);

        lib.remove_directory(0);
        assert_eq!(lib.directories.len(), 0);
    }
}
