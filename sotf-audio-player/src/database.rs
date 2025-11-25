use rusqlite::{Connection, Result as SqlResult, params};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Local, NaiveDateTime};

use crate::config;
use crate::library::{Album, Track};

/// A database migration with description and apply function
struct Migration {
    description: &'static str,
    apply: fn(&MusicDatabase) -> SqlResult<()>,
}

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

    /// Initialize database schema and apply migrations
    fn initialize_schema(&self) -> SqlResult<()> {
        // Create schema_version table if it doesn't exist
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL,
                description TEXT NOT NULL
            )",
            [],
        )?;

        // Get current schema version
        let current_version = self.get_schema_version()?;
        log::info!("Current database schema version: {}", current_version);

        // Define all migrations
        const LATEST_VERSION: i64 = 6;
        let migrations = self.get_migrations();

        // Apply migrations sequentially from current version to latest
        for version in (current_version + 1)..=LATEST_VERSION {
            log::info!("Applying migration to version {}...", version);
            if let Some(migration) = migrations.get(&version) {
                (migration.apply)(self)?;
                self.update_schema_version(version, migration.description)?;
                log::info!("Successfully applied migration to version {}", version);
            } else {
                log::error!("Migration for version {} not found", version);
            }
        }

        if current_version < LATEST_VERSION {
            log::info!(
                "Database schema updated from version {} to {}",
                current_version,
                LATEST_VERSION
            );
        }

        Ok(())
    }

    /// Get the current schema version
    fn get_schema_version(&self) -> SqlResult<i64> {
        let result = self
            .conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |row| {
                row.get::<_, Option<i64>>(0)
            });

        match result {
            Ok(Some(version)) => Ok(version),
            Ok(None) => Ok(0), // Fresh database, no version yet
            Err(rusqlite::Error::SqliteFailure(_, _)) => Ok(0), // Table doesn't exist yet
            Err(e) => Err(e),
        }
    }

    /// Update schema version after successful migration
    fn update_schema_version(&self, version: i64, description: &str) -> SqlResult<()> {
        let now = current_timestamp();
        self.conn.execute(
            "INSERT INTO schema_version (version, applied_at, description) VALUES (?1, ?2, ?3)",
            params![version, now, description],
        )?;
        Ok(())
    }

    /// Get migration history for debugging/inspection
    #[allow(dead_code)]
    pub fn get_migration_history(&self) -> SqlResult<Vec<(i64, u64, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT version, applied_at, description FROM schema_version ORDER BY version",
        )?;

        let history = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)? as u64,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(history)
    }

    /// Define all database migrations
    fn get_migrations(&self) -> std::collections::HashMap<i64, Migration> {
        let mut migrations = std::collections::HashMap::new();

        // Migration 1: Initial schema
        migrations.insert(
            1,
            Migration {
                description: "Initial schema with albums, tracks, and scan_history tables",
                apply: |db| {
                    // Albums table
                    db.conn.execute(
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

                    // Tracks table (without channels column initially)
                    db.conn.execute(
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
                    db.conn.execute(
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
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_tracks_album_id ON tracks(album_id)",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_tracks_path ON tracks(path)",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_scan_history_directory ON scan_history(directory)",
                        [],
                    )?;

                    Ok(())
                },
            },
        );

        // Migration 2: Add channels column to tracks table
        migrations.insert(
            2,
            Migration {
                description: "Add channels column to tracks table for channel count filtering",
                apply: |db| {
                    // Check if column already exists (for databases created with channels)
                    let has_channels = db
                        .conn
                        .prepare("SELECT channels FROM tracks LIMIT 1")
                        .is_ok();

                    if !has_channels {
                        db.conn
                            .execute("ALTER TABLE tracks ADD COLUMN channels INTEGER", [])?;
                        log::info!("Added channels column to tracks table");
                    } else {
                        log::info!("Channels column already exists, skipping");
                    }

                    Ok(())
                },
            },
        );

        // Migration 3: Add FTS5 virtual table for search
        migrations.insert(
            3,
            Migration {
                description: "Add FTS5 virtual table for full-text search",
                apply: |db| {
                    // Create FTS5 virtual table
                    // We include album_id as UNINDEXED so we can retrieve it but it's not part of the full-text index
                    db.conn.execute(
                        "CREATE VIRTUAL TABLE IF NOT EXISTS library_fts USING fts5(
                            artist,
                            album_title,
                            track_title,
                            album_id UNINDEXED
                        )",
                        [],
                    )?;

                    // Create triggers to keep FTS index in sync with albums and tracks
                    // Trigger for inserting new albums (initially no tracks, so just artist/album)
                    // Note: We primarily index tracks, but we want to find albums even if we search for artist/album name
                    // The strategy here is:
                    // 1. When a track is inserted, we insert a row into library_fts
                    // 2. When an album is inserted, we don't necessarily need to insert into FTS immediately
                    //    because the tracks will be inserted shortly after.
                    //    However, to ensure we can find albums by artist/title even without tracks (edge case),
                    //    or to simplify, we can just index based on tracks.

                    // Actually, a better approach for "search albums" is to index each album once?
                    // Or index each track?
                    // The requirement is "search albums".
                    // If we search for a track title, we want to find the album containing it.
                    // So we should index each track, and store the album_id.

                    // Trigger: After Insert Track
                    db.conn.execute(
                        "CREATE TRIGGER IF NOT EXISTS tracks_ai AFTER INSERT ON tracks BEGIN
                            INSERT INTO library_fts(artist, album_title, track_title, album_id)
                            SELECT
                                a.artist,
                                a.title,
                                new.title,
                                new.album_id
                            FROM albums a WHERE a.id = new.album_id;
                        END;",
                        [],
                    )?;

                    // Trigger: After Delete Track
                    // We need to delete entries from FTS where album_id matches and track_title matches?
                    // FTS5 doesn't support simple deletes easily without rowid if we don't manage it.
                    // But we can delete by album_id and track_title.
                    // Wait, FTS5 delete is usually done by inserting into the 'delete' table or just DELETE FROM table.
                    db.conn.execute(
                        "CREATE TRIGGER IF NOT EXISTS tracks_ad AFTER DELETE ON tracks BEGIN
                            DELETE FROM library_fts WHERE album_id = old.album_id AND track_title = old.title;
                        END;",
                        [],
                    )?;

                    // Trigger: After Update Track (title changed)
                    db.conn.execute(
                        "CREATE TRIGGER IF NOT EXISTS tracks_au AFTER UPDATE ON tracks BEGIN
                            DELETE FROM library_fts WHERE album_id = old.album_id AND track_title = old.title;
                            INSERT INTO library_fts(artist, album_title, track_title, album_id)
                            SELECT
                                a.artist,
                                a.title,
                                new.title,
                                new.album_id
                            FROM albums a WHERE a.id = new.album_id;
                        END;",
                        [],
                    )?;

                    // Trigger: After Update Album (artist or title changed)
                    // This is tricky because changing album details affects all tracks in FTS.
                    db.conn.execute(
                        "CREATE TRIGGER IF NOT EXISTS albums_au AFTER UPDATE ON albums BEGIN
                            DELETE FROM library_fts WHERE album_id = old.id;
                            INSERT INTO library_fts(artist, album_title, track_title, album_id)
                            SELECT
                                new.artist,
                                new.title,
                                t.title,
                                t.album_id
                            FROM tracks t WHERE t.album_id = new.id;
                        END;",
                        [],
                    )?;

                    // Populate FTS table with existing data
                    db.conn.execute(
                        "INSERT INTO library_fts(artist, album_title, track_title, album_id)
                         SELECT
                            a.artist,
                            a.title,
                            t.title,
                            t.album_id
                         FROM tracks t
                         JOIN albums a ON t.album_id = a.id",
                        [],
                    )?;

                    log::info!("Created FTS5 index and populated with existing data");
                    Ok(())
                },
            },
        );

        // Migration 4: Add ReplayGain columns to tracks table
        migrations.insert(
            4,
            Migration {
                description: "Add ReplayGain gain and peak columns to tracks table",
                apply: |db| {
                    // Check if columns already exist
                    let has_replay_gain = db
                        .conn
                        .prepare("SELECT replay_gain FROM tracks LIMIT 1")
                        .is_ok();

                    if !has_replay_gain {
                        db.conn
                            .execute("ALTER TABLE tracks ADD COLUMN replay_gain REAL", [])?;
                        log::info!("Added replay_gain column to tracks table");
                    } else {
                        log::info!("replay_gain column already exists, skipping");
                    }

                    let has_replay_peak = db
                        .conn
                        .prepare("SELECT replay_peak FROM tracks LIMIT 1")
                        .is_ok();

                    if !has_replay_peak {
                        db.conn
                            .execute("ALTER TABLE tracks ADD COLUMN replay_peak REAL", [])?;
                        log::info!("Added replay_peak column to tracks table");
                    } else {
                        log::info!("replay_peak column already exists, skipping");
                    }

                    Ok(())
                },
            },
        );

        // Migration 5: Add album ReplayGain columns to tracks table
        migrations.insert(
            5,
            Migration {
                description: "Add album ReplayGain columns to tracks table",
                apply: |db| {
                    // Check if columns already exist
                    let has_album_gain = db
                        .conn
                        .prepare("SELECT album_gain FROM tracks LIMIT 1")
                        .is_ok();

                    if !has_album_gain {
                        db.conn
                            .execute("ALTER TABLE tracks ADD COLUMN album_gain REAL", [])?;
                        log::info!("Added album_gain column to tracks table");
                    } else {
                        log::info!("album_gain column already exists, skipping");
                    }

                    let has_album_peak = db
                        .conn
                        .prepare("SELECT album_peak FROM tracks LIMIT 1")
                        .is_ok();

                    if !has_album_peak {
                        db.conn
                            .execute("ALTER TABLE tracks ADD COLUMN album_peak REAL", [])?;
                        log::info!("Added album_peak column to tracks table");
                    } else {
                        log::info!("album_peak column already exists, skipping");
                    }

                    Ok(())
                },
            },
        );

        // Migration 6: Add play_history table for listening statistics
        migrations.insert(
            6,
            Migration {
                description: "Add play_history table for tracking listening statistics",
                apply: |db| {
                    // Create play_history table
                    db.conn.execute(
                        "CREATE TABLE IF NOT EXISTS play_history (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            track_path TEXT NOT NULL,
                            album_id INTEGER,
                            played_at INTEGER NOT NULL,
                            duration_played_secs INTEGER NOT NULL,
                            FOREIGN KEY(album_id) REFERENCES albums(id) ON DELETE CASCADE
                        )",
                        [],
                    )?;

                    // Create indexes for efficient queries
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_play_history_track_path ON play_history(track_path)",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_play_history_album_id ON play_history(album_id)",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_play_history_played_at ON play_history(played_at)",
                        [],
                    )?;

                    log::info!("Created play_history table and indexes");
                    Ok(())
                },
            },
        );

        migrations
    }

    /// Search for albums using FTS5 with fuzzy matching
    /// Searches across artist, album_title, and track_title fields
    /// Works regardless of how the view is sorted - returns matching album IDs
    pub fn search_library(&self, query: &str) -> SqlResult<Vec<i64>> {
        // If query is empty, return empty
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Build FTS5 query with prefix matching on each term
        // For multi-word queries like "pink floyd", we use AND to require all terms
        // e.g. "pink floyd" -> "pink* AND floyd*"
        // This provides fuzzy matching while being more precise than OR
        let terms: Vec<String> = query
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|s| {
                // Escape double quotes and add wildcard for prefix matching (fuzzy search)
                let escaped = s.replace("\"", "\"\"");
                format!("{}*", escaped)
            })
            .collect();

        if terms.is_empty() {
            return Ok(Vec::new());
        }

        // Join with AND so all terms must match somewhere in artist/album/track fields
        // This makes "pink floyd" find albums where both "pink*" and "floyd*" appear
        // Since FTS5 searches across all indexed columns, this works great for fuzzy search
        let fts_query = terms.join(" AND ");

        // Use rank for relevance-based ordering
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT album_id FROM library_fts WHERE library_fts MATCH ?1 ORDER BY rank",
        )?;

        let album_ids = stmt
            .query_map(params![fts_query], |row| row.get::<_, i64>(0))?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(album_ids)
    }

    /// Get the file modification time for a track by path
    pub fn get_track_mtime(&self, path: &Path) -> SqlResult<Option<u64>> {
        let path_str = path.to_string_lossy();
        let mut stmt = self
            .conn
            .prepare("SELECT file_mtime FROM tracks WHERE path = ?1")?;

        let result = stmt.query_row(params![path_str.as_ref()], |row| row.get::<_, i64>(0));

        match result {
            Ok(mtime) => Ok(Some(mtime as u64)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Load all albums and tracks from database
    pub fn load_library(&self) -> SqlResult<Vec<Album>> {
        let mut albums_stmt = self.conn.prepare(
            "SELECT id, artist, title, year, album_art_path FROM albums ORDER BY artist, title",
        )?;

        let mut albums = Vec::new();
        let album_rows = albums_stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,            // id
                row.get::<_, String>(1)?,         // artist
                row.get::<_, String>(2)?,         // title
                row.get::<_, Option<i64>>(3)?,    // year
                row.get::<_, Option<String>>(4)?, // album_art_path
            ))
        })?;

        // Get all play counts at once for efficiency
        let play_counts = self.get_all_album_play_counts()?;

        for album_row in album_rows {
            let (album_id, artist, title, year, album_art_path) = album_row?;

            // Load tracks for this album
            let mut tracks_stmt = self.conn.prepare(
                "SELECT path, title, track_number, duration_secs, channels,
                        replay_gain, replay_peak, album_gain, album_peak
                 FROM tracks
                 WHERE album_id = ?1
                 ORDER BY track_number",
            )?;

            let tracks = tracks_stmt
                .query_map(params![album_id], |row| {
                    Ok(Track {
                        path: PathBuf::from(row.get::<_, String>(0)?),
                        title: row.get::<_, Option<String>>(1)?,
                        track_number: row.get::<_, Option<i64>>(2)?.map(|n| n as u32),
                        duration_secs: row.get::<_, Option<i64>>(3)?.map(|n| n as u64),
                        channels: row.get::<_, Option<i64>>(4)?.map(|n| n as u32),
                        replay_gain: row.get::<_, Option<f64>>(5)?,
                        replay_peak: row.get::<_, Option<f64>>(6)?,
                        album_gain: row.get::<_, Option<f64>>(7)?,
                        album_peak: row.get::<_, Option<f64>>(8)?,
                    })
                })?
                .collect::<SqlResult<Vec<_>>>()?;

            let play_count = *play_counts.get(&album_id).unwrap_or(&0);

            albums.push(Album {
                id: Some(album_id),
                artist,
                title,
                year: year.map(|y| y as u32),
                tracks,
                album_art_path: album_art_path.map(PathBuf::from),
                play_count,
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
                    album
                        .album_art_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string()),
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
                    "INSERT INTO tracks (album_id, path, title, track_number, duration_secs, channels,
                                        file_mtime, scanned_at, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                     ON CONFLICT(path) DO UPDATE SET
                     album_id = excluded.album_id,
                     title = excluded.title,
                     track_number = excluded.track_number,
                     duration_secs = excluded.duration_secs,
                     channels = excluded.channels,
                     file_mtime = excluded.file_mtime,
                     scanned_at = excluded.scanned_at,
                     updated_at = excluded.updated_at",
                    params![
                        album_id,
                        track.path.to_string_lossy().to_string(),
                        track.title,
                        track.track_number.map(|n| n as i64),
                        track.duration_secs.map(|n| n as i64),
                        track.channels.map(|n| n as i64),
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
    pub fn record_scan(
        &self,
        directory: &Path,
        tracks_found: usize,
        albums_found: usize,
    ) -> SqlResult<()> {
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
             WHERE directory = ?1 OR ?1 LIKE directory || '/%' OR directory LIKE ?1 || '/%'",
        )?;

        let result = stmt.query_row(params![dir_str.as_ref()], |row| {
            row.get::<_, Option<i64>>(0)
        })?;

        Ok(result.map(|t| t as u64))
    }

    /// Get all scanned directories with their latest scan statistics
    /// Returns (directory_path, tracks_found, albums_found, last_scanned_at)
    pub fn get_scanned_directories(&self) -> SqlResult<Vec<(PathBuf, usize, usize, u64)>> {
        // Get the most recent scan for each unique directory
        let mut stmt = self.conn.prepare(
            "SELECT directory, tracks_found, albums_found, scanned_at
             FROM scan_history
             WHERE (directory, scanned_at) IN (
                 SELECT directory, MAX(scanned_at)
                 FROM scan_history
                 GROUP BY directory
             )
             ORDER BY directory",
        )?;

        let directories = stmt
            .query_map([], |row| {
                Ok((
                    PathBuf::from(row.get::<_, String>(0)?),
                    row.get::<_, i64>(1)? as usize,
                    row.get::<_, i64>(2)? as usize,
                    row.get::<_, i64>(3)? as u64,
                ))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(directories)
    }

    /// Remove tracks that no longer exist on disk
    pub fn clean_missing_files(&mut self) -> SqlResult<usize> {
        self.clean_missing_files_with_progress(|_, _| {})
    }

    pub fn clean_missing_files_with_progress<F>(
        &mut self,
        mut progress_callback: F,
    ) -> SqlResult<usize>
    where
        F: FnMut(usize, usize),
    {
        // Collect all tracks first to avoid borrowing issues
        let tracks: Vec<(i64, String)> = {
            let mut stmt = self.conn.prepare("SELECT id, path FROM tracks")?;
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<SqlResult<Vec<_>>>()?
        }; // stmt is dropped here, releasing the immutable borrow

        let total = tracks.len();
        let mut to_delete = Vec::new();

        for (checked, (id, path_str)) in tracks.into_iter().enumerate() {
            let path = PathBuf::from(&path_str);
            if !path.exists() {
                to_delete.push(id);
            }

            // Report progress every 100 tracks or at the end
            if checked % 100 == 0 || checked == total - 1 {
                progress_callback(checked + 1, total);
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

        // Final progress report
        progress_callback(total, total);

        Ok(count)
    }

    /// Update ReplayGain values for a track
    pub fn update_replay_gain(&self, path: &Path, gain: f64, peak: f64) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE tracks SET replay_gain = ?1, replay_peak = ?2 WHERE path = ?3",
            params![gain, peak, path.to_str().unwrap()],
        )?;
        Ok(())
    }

    /// Get tracks that don't have ReplayGain values yet
    pub fn get_tracks_without_replay_gain(&self) -> SqlResult<Vec<PathBuf>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path FROM tracks WHERE replay_gain IS NULL OR replay_peak IS NULL")?;

        let paths = stmt
            .query_map([], |row| {
                let path_str: String = row.get(0)?;
                Ok(PathBuf::from(path_str))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(paths)
    }

    /// Get all track paths from the database
    pub fn get_all_track_paths(&self) -> SqlResult<Vec<PathBuf>> {
        let mut stmt = self.conn.prepare("SELECT path FROM tracks")?;

        let paths = stmt
            .query_map([], |row| {
                let path_str: String = row.get(0)?;
                Ok(PathBuf::from(path_str))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(paths)
    }

    /// Record a play event for a track
    /// Only records if duration_played_secs >= 30
    pub fn record_play(&self, track_path: &Path, duration_played_secs: u64) -> SqlResult<()> {
        // Only record if played for at least 30 seconds
        if duration_played_secs < 30 {
            return Ok(());
        }

        let now = current_timestamp();
        let path_str = track_path.to_string_lossy();

        // Get album_id for this track
        let album_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT album_id FROM tracks WHERE path = ?1",
                params![path_str.as_ref()],
                |row| row.get(0),
            )
            .ok();

        self.conn.execute(
            "INSERT INTO play_history (track_path, album_id, played_at, duration_played_secs)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                path_str.as_ref(),
                album_id,
                now,
                duration_played_secs as i64
            ],
        )?;

        Ok(())
    }

    /// Get play count for a specific track
    pub fn get_track_play_count(&self, track_path: &Path) -> SqlResult<usize> {
        let path_str = track_path.to_string_lossy();
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM play_history WHERE track_path = ?1",
            params![path_str.as_ref()],
            |row| row.get(0),
        )?;

        Ok(count as usize)
    }

    /// Get play count for all tracks in an album
    pub fn get_album_play_count(&self, album_id: i64) -> SqlResult<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM play_history WHERE album_id = ?1",
            params![album_id],
            |row| row.get(0),
        )?;

        Ok(count as usize)
    }

    /// Get play counts for all albums
    /// Returns a HashMap of album_id -> play_count
    pub fn get_all_album_play_counts(&self) -> SqlResult<std::collections::HashMap<i64, usize>> {
        let mut stmt = self.conn.prepare(
            "SELECT album_id, COUNT(*) as play_count
             FROM play_history
             WHERE album_id IS NOT NULL
             GROUP BY album_id",
        )?;

        let counts = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)? as usize))
            })?
            .collect::<SqlResult<std::collections::HashMap<_, _>>>()?;

        Ok(counts)
    }

    /// Get last played timestamp for a track
    pub fn get_track_last_played(&self, track_path: &Path) -> SqlResult<Option<u64>> {
        let path_str = track_path.to_string_lossy();
        let result = self.conn.query_row(
            "SELECT MAX(played_at) FROM play_history WHERE track_path = ?1",
            params![path_str.as_ref()],
            |row| row.get::<_, Option<i64>>(0),
        )?;

        Ok(result.map(|t| t as u64))
    }

    /// Get last played timestamp for an album
    pub fn get_album_last_played(&self, album_id: i64) -> SqlResult<Option<u64>> {
        let result = self.conn.query_row(
            "SELECT MAX(played_at) FROM play_history WHERE album_id = ?1",
            params![album_id],
            |row| row.get::<_, Option<i64>>(0),
        )?;

        Ok(result.map(|t| t as u64))
    }
}

/// Create a timestamped backup of an existing database file and prune old backups
/// Backup files are named music-YYYYMMDD-HHMMSS.sqlite in the same directory
pub fn backup_existing_database<P: AsRef<Path>>(
    db_path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = db_path.as_ref();

    // Nothing to do if the database does not exist yet
    if !db_path.exists() {
        return Ok(());
    }

    let dir = match db_path.parent() {
        Some(d) => d.to_path_buf(),
        None => return Ok(()),
    };

    let now = Local::now().naive_local();
    let filename = format!("music-{}.sqlite", now.format("%Y%m%d-%H%M%S"));
    let backup_path = dir.join(filename);

    std::fs::copy(db_path, &backup_path)?;

    // Best-effort pruning; log on error but keep the freshly created backup
    if let Err(e) = prune_old_backups(&dir) {
        log::warn!("Failed to prune old database backups: {}", e);
    }

    Ok(())
}

fn parse_backup_timestamp(path: &Path) -> Option<NaiveDateTime> {
    let file_name = path.file_name()?.to_str()?;
    if !file_name.starts_with("music-") || !file_name.ends_with(".sqlite") {
        return None;
    }

    let ts_part = &file_name["music-".len()..file_name.len() - ".sqlite".len()];
    NaiveDateTime::parse_from_str(ts_part, "%Y%m%d-%H%M%S").ok()
}

/// Apply retention policy to backup files in the given directory:
/// - Keep up to 3 backups per day
/// - Then keep up to 1 per ISO week
/// - Then keep up to 1 per month
fn prune_old_backups(dir: &Path) -> std::io::Result<()> {
    use std::collections::HashMap;

    let mut backups: Vec<(PathBuf, NaiveDateTime)> = Vec::new();

    for entry in std::fs::read_dir(dir)? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(dt) = parse_backup_timestamp(&path) {
            backups.push((path, dt));
        }
    }

    // Newest first so that recent backups are preferred
    backups.sort_by_key(|(_, dt)| *dt);
    backups.reverse();

    let mut per_day: HashMap<(i32, u32, u32), usize> = HashMap::new();
    let mut per_week: HashMap<(i32, u32), usize> = HashMap::new();
    let mut per_month: HashMap<(i32, u32), usize> = HashMap::new();
    let mut to_delete: Vec<PathBuf> = Vec::new();

    for (path, dt) in backups {
        let date = dt.date();
        let day_key = (date.year(), date.month(), date.day());
        let week = date.iso_week();
        let week_key = (week.year(), week.week());
        let month_key = (date.year(), date.month());

        if per_day.get(&day_key).copied().unwrap_or(0) < 3 {
            *per_day.entry(day_key).or_insert(0) += 1;
            // Also count this backup towards week and month quotas
            *per_week.entry(week_key).or_insert(0) += 1;
            *per_month.entry(month_key).or_insert(0) += 1;
            continue;
        }

        if per_week.get(&week_key).copied().unwrap_or(0) < 1 {
            *per_week.entry(week_key).or_insert(0) += 1;
            *per_month.entry(month_key).or_insert(0) += 1;
            continue;
        }

        if per_month.get(&month_key).copied().unwrap_or(0) < 1 {
            *per_month.entry(month_key).or_insert(0) += 1;
            continue;
        }

        to_delete.push(path);
    }

    for path in to_delete {
        let _ = std::fs::remove_file(path);
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_backup_existing_database_creates_backup_file() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("music.db");
        std::fs::write(&db_path, b"test").unwrap();

        backup_existing_database(&db_path).unwrap();

        let mut backups = Vec::new();
        for entry in std::fs::read_dir(temp.path()).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            if name.starts_with("music-") && name.ends_with(".sqlite") {
                backups.push(name);
            }
        }

        assert_eq!(backups.len(), 1);
    }

    #[test]
    fn test_prune_old_backups_limits_to_three_per_day() {
        let temp = TempDir::new().unwrap();

        // Create 5 backups for the same day
        let ts = [
            "20250101-010000",
            "20250101-020000",
            "20250101-030000",
            "20250101-040000",
            "20250101-050000",
        ];

        for t in &ts {
            let path = temp.path().join(format!("music-{}.sqlite", t));
            std::fs::write(path, b"test").unwrap();
        }

        prune_old_backups(temp.path()).unwrap();

        let mut remaining = Vec::new();
        for entry in std::fs::read_dir(temp.path()).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            if name.starts_with("music-") && name.ends_with(".sqlite") {
                remaining.push(name);
            }
        }

        // Only 3 backups for that day should remain
        assert_eq!(remaining.len(), 3);
    }
}
