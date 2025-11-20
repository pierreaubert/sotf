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
    pub album_count: usize,
    pub last_scanned: Option<SystemTime>,
    pub expanded: bool,
    pub subdirectories: Vec<DirectoryInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub path: PathBuf,
    pub title: Option<String>,
    pub track_number: Option<u32>,
    pub duration_secs: Option<u64>,
    pub channels: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Album {
    pub id: Option<i64>,
    pub artist: String,
    pub title: String,
    pub year: Option<u32>,
    pub tracks: Vec<Track>,
    pub album_art_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumChannelType {
    Stereo,          // All tracks are 2 channels
    Multichannel(u32), // All tracks have same channel count > 2
    Mixed,           // Tracks have different channel counts
}

impl Album {
    /// Determine the channel configuration of this album
    pub fn channel_type(&self) -> Option<AlbumChannelType> {
        if self.tracks.is_empty() {
            return None;
        }

        // Get channel counts from all tracks that have the info
        let channel_counts: Vec<u32> = self
            .tracks
            .iter()
            .filter_map(|t| t.channels)
            .collect();

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

        // Helper to recursively build directory info
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

        self.directories.push(build_directory_info(path));

        Ok(true) // New directory added, scan needed
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

        // Create a map of directory path to (file count, album count)
        let mut dir_stats: HashMap<PathBuf, (usize, usize)> = HashMap::new();

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
            let (tracks, scanned) =
                self.scan_directory(&dir_info.path, &mut album_map, incremental, &mut dir_stats)?;
            
            total_tracks += tracks;
            scanned_tracks += scanned;

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

        // Calculate album counts per directory
        // We use a temporary map to store sets of album keys per directory to ensure uniqueness
        let mut dir_albums: HashMap<PathBuf, std::collections::HashSet<(String, String)>> = HashMap::new();

        for ((artist, title), album) in &album_map {
            let key = (artist.clone(), title.clone());
            for track in &album.tracks {
                if let Some(parent) = track.path.parent() {
                    dir_albums
                        .entry(parent.to_path_buf())
                        .or_default()
                        .insert(key.clone());
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
            scan_time: SystemTime
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
        dir_stats: &mut HashMap<PathBuf, (usize, usize)>,
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

                    if let Ok(metadata) = extract_metadata(path) {
                        let artist = metadata
                            .artist
                            .unwrap_or_else(|| "Unknown Artist".to_string());
                        let album_title = metadata
                            .album
                            .unwrap_or_else(|| "Unknown Album".to_string());
                        let key = (artist.clone(), album_title.clone());
                        
                        // Track that this album is present in this directory
                        if let Some(_parent) = path.parent() {
                            // This is getting complicated to track unique albums per directory efficiently
                            // without storing a set for each directory.
                            // But we can do it.
                        }

                        let album = album_map.entry(key).or_insert_with(|| Album {
                            id: None,
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
                            channels: metadata.channels,
                        };

                        album.tracks.push(track);
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

    pub fn search_albums(&self, query: &str) -> Vec<&Album> {
        // Try to use FTS5 search if database is available
        if let Some(db) = &self.db {
            if let Ok(album_ids) = db.search_library(query) {
                if !album_ids.is_empty() {
                    return self
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

        // Fallback to legacy search if DB search fails or returns no results
        // (or if we just want to support in-memory search as well)
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
    pub channels: Option<u32>,
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

    // Extract duration and channel count from the format
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
}
