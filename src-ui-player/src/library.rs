use std::collections::HashMap;
use std::path::{Path, PathBuf};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use walkdir::WalkDir;

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
    pub directories: Vec<PathBuf>,
    pub albums: Vec<Album>,
}

impl MusicLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_directory(&mut self, path: PathBuf) {
        if !self.directories.contains(&path) {
            self.directories.push(path);
        }
    }

    pub fn remove_directory(&mut self, index: usize) -> Option<PathBuf> {
        if index < self.directories.len() {
            Some(self.directories.remove(index))
        } else {
            None
        }
    }

    pub fn scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut album_map: HashMap<(String, String), Album> = HashMap::new();

        for dir in &self.directories {
            self.scan_directory(dir, &mut album_map)?;
        }

        self.albums = album_map.into_values().collect();

        // Sort tracks within each album
        for album in &mut self.albums {
            album.sort_tracks();
        }

        // Sort albums by artist and title
        self.albums.sort_by(|a, b| {
            a.artist
                .cmp(&b.artist)
                .then(a.title.cmp(&b.title))
        });

        Ok(())
    }

    fn scan_directory(
        &self,
        dir: &Path,
        album_map: &mut HashMap<(String, String), Album>,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
                    if let Ok(metadata) = extract_metadata(path) {
                        let artist = metadata.artist.unwrap_or_else(|| "Unknown Artist".to_string());
                        let album_title = metadata.album.unwrap_or_else(|| "Unknown Album".to_string());
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

        Ok(())
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

fn extract_metadata(path: &Path) -> Result<TrackMetadata, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension() {
        hint.with_extension(&ext.to_string_lossy());
    }

    let format_opts = FormatOptions::default();
    let metadata_opts = MetadataOptions::default();

    let probed = symphonia::default::get_probe().format(&hint, mss, &format_opts, &metadata_opts)?;

    let mut metadata = TrackMetadata::default();

    // Extract duration from the format
    if let Some(track) = probed.format.default_track() {
        if let Some(time_base) = track.codec_params.time_base {
            if let Some(n_frames) = track.codec_params.n_frames {
                let duration = time_base.calc_time(n_frames);
                metadata.duration_secs = Some(duration.seconds);
            }
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
                Some(symphonia::core::meta::StandardTagKey::Date) |
                Some(symphonia::core::meta::StandardTagKey::ReleaseDate) => {
                    // Try to extract year from date string
                    let date_str = tag.value.to_string();
                    if let Some(year_str) = date_str.split('-').next() {
                        if let Ok(year) = year_str.parse() {
                            metadata.year = Some(year);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(metadata)
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
