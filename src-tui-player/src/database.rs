use rusqlite::{Connection, Result as SqlResult, params};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config;
use crate::library::{Album, Track};

/// Database manager for persistent music library storage
#[derive(Debug)]
pub struct MusicDatabase {
    conn: Connection,
}

impl MusicDatabase {
    /// Get the default database path
    /// Linux: ~/.config/sotf/music.db
    /// macOS: ~/Library/Application Support/org.spinorama.sotf/music.db
    /// Windows: ~/.config/sotf/music.db
    pub fn default_path() -> Option<PathBuf> {
        config::get_music_db_path()
    }

    /// Open or create database at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.initialize_schema()?;
        Ok(db)
    }

    /// Initialize database schema
    fn initialize_schema(&self) -> SqlResult<()> {
        // Albums table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS albums (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                artist TEXT NOT NULL,
                title TEXT NOT NULL,
                year INTEGER,
                album_art_path TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(artist, title)
            )",
            [],
        )?;

        // Tracks table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS tracks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                album_id INTEGER NOT NULL,
                path TEXT NOT NULL UNIQUE,
                title TEXT,
                track_number INTEGER,
                duration_secs INTEGER,
                file_mtime INTEGER NOT NULL,
                scanned_at INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY(album_id) REFERENCES albums(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Scan history table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS scan_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                directory TEXT NOT NULL,
                scanned_at INTEGER NOT NULL,
                tracks_found INTEGER NOT NULL,
                albums_found INTEGER NOT NULL
            )",
            [],
        )?;

        // Create indexes for performance
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tracks_album_id ON tracks(album_id)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tracks_path ON tracks(path)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_scan_history_directory ON scan_history(directory)",
            [],
        )?;

        Ok(())
    }

    /// Get the file modification time for a track by path
    pub fn get_track_mtime(&self, path: &Path) -> SqlResult<Option<u64>> {
        let path_str = path.to_string_lossy();
        let mut stmt = self.conn.prepare(
            "SELECT file_mtime FROM tracks WHERE path = ?1"
        )?;

        let result = stmt.query_row(params![path_str.as_ref()], |row| {
            row.get::<_, i64>(0)
        });

        match result {
            Ok(mtime) => Ok(Some(mtime as u64)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Load all albums and tracks from database
    pub fn load_library(&self) -> SqlResult<Vec<Album>> {
        let mut albums_stmt = self.conn.prepare(
            "SELECT id, artist, title, year, album_art_path FROM albums ORDER BY artist, title"
        )?;

        let mut albums = Vec::new();
        let album_rows = albums_stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,  // id
                row.get::<_, String>(1)?,  // artist
                row.get::<_, String>(2)?,  // title
                row.get::<_, Option<i64>>(3)?,  // year
                row.get::<_, Option<String>>(4)?,  // album_art_path
            ))
        })?;

        for album_row in album_rows {
            let (album_id, artist, title, year, album_art_path) = album_row?;

            // Load tracks for this album
            let mut tracks_stmt = self.conn.prepare(
                "SELECT path, title, track_number, duration_secs
                 FROM tracks
                 WHERE album_id = ?1
                 ORDER BY track_number"
            )?;

            let tracks = tracks_stmt.query_map(params![album_id], |row| {
                Ok(Track {
                    path: PathBuf::from(row.get::<_, String>(0)?),
                    title: row.get::<_, Option<String>>(1)?,
                    track_number: row.get::<_, Option<i64>>(2)?.map(|n| n as u32),
                    duration_secs: row.get::<_, Option<i64>>(3)?.map(|n| n as u64),
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;

            albums.push(Album {
                artist,
                title,
                year: year.map(|y| y as u32),
                tracks,
                album_art_path: album_art_path.map(PathBuf::from),
            });
        }

        Ok(albums)
    }

    /// Save albums and tracks to database
    pub fn save_albums(&mut self, albums: &[Album]) -> SqlResult<()> {
        let tx = self.conn.transaction()?;
        let now = current_timestamp();

        for album in albums {
            // Insert or update album
            tx.execute(
                "INSERT INTO albums (artist, title, year, album_art_path, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(artist, title) DO UPDATE SET
                 year = excluded.year,
                 album_art_path = excluded.album_art_path,
                 updated_at = excluded.updated_at",
                params![
                    &album.artist,
                    &album.title,
                    album.year.map(|y| y as i64),
                    album.album_art_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                    now,
                    now,
                ],
            )?;

            // Get album ID
            let album_id: i64 = tx.query_row(
                "SELECT id FROM albums WHERE artist = ?1 AND title = ?2",
                params![&album.artist, &album.title],
                |row| row.get(0),
            )?;

            // Insert or update tracks
            for track in &album.tracks {
                let file_mtime = get_file_mtime(&track.path).unwrap_or(0);

                tx.execute(
                    "INSERT INTO tracks (album_id, path, title, track_number, duration_secs,
                                        file_mtime, scanned_at, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                     ON CONFLICT(path) DO UPDATE SET
                     album_id = excluded.album_id,
                     title = excluded.title,
                     track_number = excluded.track_number,
                     duration_secs = excluded.duration_secs,
                     file_mtime = excluded.file_mtime,
                     scanned_at = excluded.scanned_at,
                     updated_at = excluded.updated_at",
                    params![
                        album_id,
                        track.path.to_string_lossy().to_string(),
                        track.title,
                        track.track_number.map(|n| n as i64),
                        track.duration_secs.map(|n| n as i64),
                        file_mtime as i64,
                        now,
                        now,
                        now,
                    ],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// Record a scan in the scan history
    pub fn record_scan(&self, directory: &Path, tracks_found: usize, albums_found: usize) -> SqlResult<()> {
        let now = current_timestamp();
        self.conn.execute(
            "INSERT INTO scan_history (directory, scanned_at, tracks_found, albums_found)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                directory.to_string_lossy().to_string(),
                now,
                tracks_found as i64,
                albums_found as i64,
            ],
        )?;
        Ok(())
    }

    /// Get the last scan time for a directory (or any parent/child)
    pub fn get_last_scan_time(&self, directory: &Path) -> SqlResult<Option<u64>> {
        let dir_str = directory.to_string_lossy();

        // Check for exact match or parent directories
        let mut stmt = self.conn.prepare(
            "SELECT MAX(scanned_at) FROM scan_history
             WHERE directory = ?1 OR ?1 LIKE directory || '/%' OR directory LIKE ?1 || '/%'"
        )?;

        let result = stmt.query_row(params![dir_str.as_ref()], |row| {
            row.get::<_, Option<i64>>(0)
        })?;

        Ok(result.map(|t| t as u64))
    }

    /// Remove tracks that no longer exist on disk
    pub fn clean_missing_files(&mut self) -> SqlResult<usize> {
        // Collect all tracks first to avoid borrowing issues
        let tracks: Vec<(i64, String)> = {
            let mut stmt = self.conn.prepare("SELECT id, path FROM tracks")?;
            stmt.query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?.collect::<SqlResult<Vec<_>>>()?
        }; // stmt is dropped here, releasing the immutable borrow

        let mut to_delete = Vec::new();
        for (id, path_str) in tracks {
            let path = PathBuf::from(&path_str);
            if !path.exists() {
                to_delete.push(id);
            }
        }

        let count = to_delete.len();
        if count > 0 {
            let tx = self.conn.transaction()?;
            for id in to_delete {
                tx.execute("DELETE FROM tracks WHERE id = ?1", params![id])?;
            }
            tx.commit()?;

            // Clean up albums with no tracks
            self.conn.execute(
                "DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks)",
                [],
            )?;
        }

        Ok(count)
    }
}

/// Get current Unix timestamp
fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Get file modification time as Unix timestamp
fn get_file_mtime(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
}
