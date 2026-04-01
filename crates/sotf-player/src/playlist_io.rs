//! M3U/M3U8 playlist import and export.
//!
//! Supports Extended M3U format — the most universal playlist format,
//! compatible with VLC, foobar2000, iTunes, Winamp, and most other players.
//!
//! Format:
//! ```text
//! #EXTM3U
//! #EXTINF:231,Artist - Track Title
//! /absolute/path/to/track.flac
//! ```

use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::{MusicLibrary, Playlist};

/// A playlist imported from an M3U file.
#[derive(Debug, Clone)]
pub struct ImportedPlaylist {
    /// Playlist name (derived from filename if not specified).
    pub name: String,
    /// Imported entries with resolved paths and optional metadata.
    pub entries: Vec<ImportedEntry>,
}

/// A single entry from an imported M3U file.
#[derive(Debug, Clone)]
pub struct ImportedEntry {
    /// Absolute path to the audio file.
    pub path: PathBuf,
    /// Track title from EXTINF (if present).
    pub title: Option<String>,
    /// Duration in seconds from EXTINF (if present).
    pub duration_secs: Option<u64>,
}

/// Export a playlist to an M3U8 file.
///
/// Writes Extended M3U format with UTF-8 encoding. Track metadata (artist, title,
/// duration) is looked up from the library when available.
pub fn export_m3u8(
    playlist: &Playlist,
    library: &MusicLibrary,
    output_path: &Path,
) -> Result<(), String> {
    let mut content = String::new();
    writeln!(content, "#EXTM3U").unwrap();

    for entry in &playlist.entries {
        // Look up track metadata from library
        let track = library
            .albums
            .iter()
            .flat_map(|a| &a.tracks)
            .find(|t| t.path == entry.track_path);

        if let Some(track) = track {
            let duration = track.duration_secs.unwrap_or(0) as i64;
            let artist = track.artist.as_deref().unwrap_or("Unknown");
            let title = track.title.as_deref().unwrap_or("Unknown");
            writeln!(content, "#EXTINF:{},{} - {}", duration, artist, title).unwrap();
        }

        writeln!(content, "{}", entry.track_path.display()).unwrap();
    }

    fs::write(output_path, content).map_err(|e| format!("Failed to write M3U8 file: {}", e))
}

/// Import a playlist from an M3U/M3U8 file.
///
/// Tolerant parser — works with or without `#EXTM3U` header, handles both
/// `#EXTINF` metadata lines and plain file paths. Relative paths are resolved
/// against the directory containing the M3U file.
pub fn import_m3u8(input_path: &Path) -> Result<ImportedPlaylist, String> {
    let file = fs::File::open(input_path).map_err(|e| format!("Failed to open M3U file: {}", e))?;
    let reader = BufReader::new(file);

    let name = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Imported Playlist")
        .to_string();

    let base_dir = input_path.parent().unwrap_or(Path::new("."));

    let mut entries = Vec::new();
    let mut pending_title: Option<String> = None;
    let mut pending_duration: Option<u64> = None;

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Failed to read line: {}", e))?;
        let line = line.trim().to_string();

        // Skip empty lines
        if line.is_empty() {
            continue;
        }

        // Skip #EXTM3U header
        if line.eq_ignore_ascii_case("#EXTM3U") {
            continue;
        }

        // Parse EXTINF lines
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            if let Some((duration_str, title_str)) = rest.split_once(',') {
                pending_duration = duration_str
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .map(|d| d.max(0.0) as u64);
                pending_title = Some(title_str.trim().to_string());
            }
            continue;
        }

        // Skip other comment/directive lines
        if line.starts_with('#') {
            continue;
        }

        // This is a file path
        let path = PathBuf::from(&line);
        let resolved = if path.is_absolute() {
            path
        } else {
            base_dir.join(&path)
        };

        entries.push(ImportedEntry {
            path: resolved,
            title: pending_title.take(),
            duration_secs: pending_duration.take(),
        });
    }

    Ok(ImportedPlaylist { name, entries })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Album, Track};

    fn make_library() -> MusicLibrary {
        let mut lib = MusicLibrary::new();
        lib.albums.push(Album {
            title: "Test Album".to_string(),
            tracks: vec![
                Track {
                    path: PathBuf::from("/music/artist/track1.flac"),
                    title: Some("First Track".to_string()),
                    artist: Some("Test Artist".to_string()),
                    duration_secs: Some(231),
                    ..Default::default()
                },
                Track {
                    path: PathBuf::from("/music/artist/track2.flac"),
                    title: Some("Second Track".to_string()),
                    artist: Some("Test Artist".to_string()),
                    duration_secs: Some(302),
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
        lib
    }

    fn make_playlist() -> Playlist {
        use crate::PlaylistEntry;
        Playlist {
            id: Some(1),
            name: "Test Playlist".to_string(),
            description: None,
            entries: vec![
                PlaylistEntry {
                    track_path: PathBuf::from("/music/artist/track1.flac"),
                    position: 0,
                },
                PlaylistEntry {
                    track_path: PathBuf::from("/music/artist/track2.flac"),
                    position: 1,
                },
            ],
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn test_export_m3u8() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("test.m3u8");
        let library = make_library();
        let playlist = make_playlist();

        export_m3u8(&playlist, &library, &output).unwrap();

        let content = fs::read_to_string(&output).unwrap();
        assert!(content.starts_with("#EXTM3U\n"));
        assert!(content.contains("#EXTINF:231,Test Artist - First Track"));
        assert!(content.contains("/music/artist/track1.flac"));
        assert!(content.contains("#EXTINF:302,Test Artist - Second Track"));
        assert!(content.contains("/music/artist/track2.flac"));
    }

    #[test]
    fn test_import_m3u8() {
        let dir = tempfile::tempdir().unwrap();
        let m3u_path = dir.path().join("my_playlist.m3u8");

        let content = "#EXTM3U\n\
            #EXTINF:231,Test Artist - First Track\n\
            /music/artist/track1.flac\n\
            #EXTINF:302,Test Artist - Second Track\n\
            /music/artist/track2.flac\n";
        fs::write(&m3u_path, content).unwrap();

        let imported = import_m3u8(&m3u_path).unwrap();
        assert_eq!(imported.name, "my_playlist");
        assert_eq!(imported.entries.len(), 2);
        assert_eq!(
            imported.entries[0].path,
            PathBuf::from("/music/artist/track1.flac")
        );
        assert_eq!(imported.entries[0].duration_secs, Some(231));
        assert_eq!(
            imported.entries[0].title.as_deref(),
            Some("Test Artist - First Track")
        );
        assert_eq!(
            imported.entries[1].path,
            PathBuf::from("/music/artist/track2.flac")
        );
    }

    #[test]
    fn test_import_plain_m3u() {
        let dir = tempfile::tempdir().unwrap();
        let m3u_path = dir.path().join("simple.m3u");

        let content = "/music/track1.flac\n/music/track2.flac\n";
        fs::write(&m3u_path, content).unwrap();

        let imported = import_m3u8(&m3u_path).unwrap();
        assert_eq!(imported.entries.len(), 2);
        assert!(imported.entries[0].title.is_none());
        assert!(imported.entries[0].duration_secs.is_none());
    }

    #[test]
    fn test_import_relative_paths() {
        let dir = tempfile::tempdir().unwrap();
        let music_dir = dir.path().join("music");
        fs::create_dir_all(&music_dir).unwrap();
        let m3u_path = dir.path().join("playlist.m3u");

        let content = "music/track1.flac\nmusic/track2.flac\n";
        fs::write(&m3u_path, content).unwrap();

        let imported = import_m3u8(&m3u_path).unwrap();
        assert_eq!(
            imported.entries[0].path,
            dir.path().join("music/track1.flac")
        );
        assert_eq!(
            imported.entries[1].path,
            dir.path().join("music/track2.flac")
        );
    }

    #[test]
    fn test_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let m3u_path = dir.path().join("roundtrip.m3u8");
        let library = make_library();
        let playlist = make_playlist();

        export_m3u8(&playlist, &library, &m3u_path).unwrap();
        let imported = import_m3u8(&m3u_path).unwrap();

        assert_eq!(imported.entries.len(), playlist.entries.len());
        for (imported_entry, original_entry) in imported.entries.iter().zip(&playlist.entries) {
            assert_eq!(imported_entry.path, original_entry.track_path);
        }
    }

    #[test]
    fn test_import_decimal_duration() {
        // Bug: EXTINF durations can be decimal (e.g., 231.5) — VLC exports these
        let dir = tempfile::tempdir().unwrap();
        let m3u_path = dir.path().join("decimal.m3u8");

        let content = "#EXTM3U\n\
            #EXTINF:231.5,Artist - Track\n\
            /music/track.flac\n";
        fs::write(&m3u_path, content).unwrap();

        let imported = import_m3u8(&m3u_path).unwrap();
        assert_eq!(imported.entries.len(), 1);
        assert_eq!(imported.entries[0].duration_secs, Some(231));
    }
}
