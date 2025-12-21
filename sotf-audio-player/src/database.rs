use rusqlite::{Connection, Result as SqlResult, params};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Local, NaiveDateTime};

use crate::config;
use crate::library::{Album, Playlist, PlaylistEntry, Track};

/// Normalize a genre value:
/// - Replace dots and underscores with spaces
/// - Title case each word (first letter uppercase, rest lowercase)
///   This is specifically for genres where formats like "world.music" or "trip_hop" are common
fn normalize_genre_name(value: &str) -> String {
    value
        .replace(['.', '_'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let rest: String = chars.collect::<String>().to_lowercase();
                    format!("{}{}", first.to_uppercase(), rest)
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Split a metadata value by common delimiters (`,`, `/`, `;`)
/// Returns a vector of trimmed, non-empty values (preserves original capitalization)
fn split_metadata_value(value: &str) -> Vec<String> {
    value
        .split([',', '/', ';'])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Split a genre value by common delimiters and normalize each
/// (dots/underscores to spaces, title case)
fn split_and_normalize_genres(value: &str) -> Vec<String> {
    value
        .split([',', '/', ';'])
        .map(|s| normalize_genre_name(s.trim()))
        .filter(|s| !s.is_empty())
        .collect()
}

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
    ///
    /// # Security
    /// The database path must be within the application's config directory.
    pub fn open<P: AsRef<Path>>(path: P) -> SqlResult<Self> {
        let path = path.as_ref();

        // Security validation: ensure we're opening the database within config directory
        crate::security::validate_write_path(path).map_err(|e| {
            rusqlite::Error::InvalidPath(std::path::PathBuf::from(format!("{}", e)))
        })?;

        Self::open_internal(path)
    }

    /// Open or create database at the given path without security validation.
    /// This is only available in test builds for unit testing.
    #[cfg(any(test, feature = "testing"))]
    pub fn open_for_testing<P: AsRef<Path>>(path: P) -> SqlResult<Self> {
        Self::open_internal(path.as_ref())
    }

    /// Internal database open that skips security validation.
    fn open_internal(path: &Path) -> SqlResult<Self> {
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
        const LATEST_VERSION: i64 = 14;
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

        // Migration 7: Add waveform column to tracks table
        migrations.insert(
            7,
            Migration {
                description: "Add waveform column to tracks table for amplitude visualization",
                apply: |db| {
                    // Check if column already exists
                    let has_waveform = db
                        .conn
                        .prepare("SELECT waveform FROM tracks LIMIT 1")
                        .is_ok();

                    if !has_waveform {
                        db.conn
                            .execute("ALTER TABLE tracks ADD COLUMN waveform BLOB", [])?;
                        log::info!("Added waveform column to tracks table");
                    } else {
                        log::info!("waveform column already exists, skipping");
                    }

                    Ok(())
                },
            },
        );

        // Migration 8: Add playlists and playlist_tracks tables
        migrations.insert(
            8,
            Migration {
                description: "Add playlists and playlist_tracks tables for user playlists",
                apply: |db| {
                    // Create playlists table
                    db.conn.execute(
                        "CREATE TABLE IF NOT EXISTS playlists (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            name TEXT NOT NULL UNIQUE,
                            description TEXT,
                            created_at INTEGER NOT NULL,
                            updated_at INTEGER NOT NULL
                        )",
                        [],
                    )?;

                    // Create playlist_tracks table for track ordering
                    db.conn.execute(
                        "CREATE TABLE IF NOT EXISTS playlist_tracks (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            playlist_id INTEGER NOT NULL,
                            track_path TEXT NOT NULL,
                            position INTEGER NOT NULL,
                            added_at INTEGER NOT NULL,
                            FOREIGN KEY(playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
                            UNIQUE(playlist_id, track_path)
                        )",
                        [],
                    )?;

                    // Create indexes for efficient queries
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_playlist_tracks_playlist_id ON playlist_tracks(playlist_id)",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_playlist_tracks_position ON playlist_tracks(playlist_id, position)",
                        [],
                    )?;

                    log::info!("Created playlists and playlist_tracks tables");
                    Ok(())
                },
            },
        );

        // Migration 9: Add extended metadata columns to tracks table
        migrations.insert(
            9,
            Migration {
                description: "Add extended metadata columns to tracks table",
                apply: |db| {
                    // Add all metadata columns - SQLite handles NULL efficiently
                    // so sparse columns don't waste space
                    let columns = [
                        ("genre", "TEXT"),
                        ("composer", "TEXT"),
                        ("disc_number", "INTEGER"),
                        ("conductor", "TEXT"),
                        ("performer", "TEXT"),
                        ("isrc", "TEXT"),
                        ("album_artist", "TEXT"),
                        ("ensemble", "TEXT"),
                    ];

                    for (col_name, col_type) in columns {
                        // Check if column already exists
                        let has_column = db
                            .conn
                            .prepare(&format!("SELECT {} FROM tracks LIMIT 1", col_name))
                            .is_ok();

                        if !has_column {
                            db.conn.execute(
                                &format!("ALTER TABLE tracks ADD COLUMN {} {}", col_name, col_type),
                                [],
                            )?;
                            log::info!("Added {} column to tracks table", col_name);
                        }
                    }

                    // Create indexes for commonly queried fields
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_tracks_genre ON tracks(genre) WHERE genre IS NOT NULL",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_tracks_composer ON tracks(composer) WHERE composer IS NOT NULL",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_tracks_album_artist ON tracks(album_artist) WHERE album_artist IS NOT NULL",
                        [],
                    )?;

                    log::info!("Added extended metadata columns to tracks table");
                    Ok(())
                },
            },
        );

        // Migration 10: Add album art thumbnail column to albums table
        migrations.insert(
            10,
            Migration {
                description: "Add album_art_thumbnail BLOB column to albums table",
                apply: |db| {
                    // Check if column already exists
                    let has_column = db
                        .conn
                        .prepare("SELECT album_art_thumbnail FROM albums LIMIT 1")
                        .is_ok();

                    if !has_column {
                        db.conn.execute(
                            "ALTER TABLE albums ADD COLUMN album_art_thumbnail BLOB",
                            [],
                        )?;
                        log::info!("Added album_art_thumbnail column to albums table");
                    }

                    Ok(())
                },
            },
        );

        // Migration 11: Normalize metadata tables (genres, composers, conductors, performers, ensembles)
        migrations.insert(
            11,
            Migration {
                description: "Add normalized lookup tables for genres, composers, conductors, performers, ensembles",
                apply: |db| {
                    // Create lookup tables
                    db.conn.execute(
                        "CREATE TABLE IF NOT EXISTS genres (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            name TEXT NOT NULL UNIQUE COLLATE NOCASE
                        )",
                        [],
                    )?;

                    db.conn.execute(
                        "CREATE TABLE IF NOT EXISTS composers (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            name TEXT NOT NULL UNIQUE COLLATE NOCASE
                        )",
                        [],
                    )?;

                    db.conn.execute(
                        "CREATE TABLE IF NOT EXISTS conductors (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            name TEXT NOT NULL UNIQUE COLLATE NOCASE
                        )",
                        [],
                    )?;

                    db.conn.execute(
                        "CREATE TABLE IF NOT EXISTS performers (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            name TEXT NOT NULL UNIQUE COLLATE NOCASE
                        )",
                        [],
                    )?;

                    db.conn.execute(
                        "CREATE TABLE IF NOT EXISTS ensembles (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            name TEXT NOT NULL UNIQUE COLLATE NOCASE
                        )",
                        [],
                    )?;

                    // Create junction tables for many-to-many relationships
                    db.conn.execute(
                        "CREATE TABLE IF NOT EXISTS track_genres (
                            track_id INTEGER NOT NULL,
                            genre_id INTEGER NOT NULL,
                            PRIMARY KEY (track_id, genre_id),
                            FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE,
                            FOREIGN KEY(genre_id) REFERENCES genres(id) ON DELETE CASCADE
                        )",
                        [],
                    )?;

                    db.conn.execute(
                        "CREATE TABLE IF NOT EXISTS track_composers (
                            track_id INTEGER NOT NULL,
                            composer_id INTEGER NOT NULL,
                            PRIMARY KEY (track_id, composer_id),
                            FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE,
                            FOREIGN KEY(composer_id) REFERENCES composers(id) ON DELETE CASCADE
                        )",
                        [],
                    )?;

                    db.conn.execute(
                        "CREATE TABLE IF NOT EXISTS track_conductors (
                            track_id INTEGER NOT NULL,
                            conductor_id INTEGER NOT NULL,
                            PRIMARY KEY (track_id, conductor_id),
                            FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE,
                            FOREIGN KEY(conductor_id) REFERENCES conductors(id) ON DELETE CASCADE
                        )",
                        [],
                    )?;

                    db.conn.execute(
                        "CREATE TABLE IF NOT EXISTS track_performers (
                            track_id INTEGER NOT NULL,
                            performer_id INTEGER NOT NULL,
                            PRIMARY KEY (track_id, performer_id),
                            FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE,
                            FOREIGN KEY(performer_id) REFERENCES performers(id) ON DELETE CASCADE
                        )",
                        [],
                    )?;

                    db.conn.execute(
                        "CREATE TABLE IF NOT EXISTS track_ensembles (
                            track_id INTEGER NOT NULL,
                            ensemble_id INTEGER NOT NULL,
                            PRIMARY KEY (track_id, ensemble_id),
                            FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE,
                            FOREIGN KEY(ensemble_id) REFERENCES ensembles(id) ON DELETE CASCADE
                        )",
                        [],
                    )?;

                    // Create indexes for efficient lookups
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_track_genres_track ON track_genres(track_id)",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_track_genres_genre ON track_genres(genre_id)",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_track_composers_track ON track_composers(track_id)",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_track_composers_composer ON track_composers(composer_id)",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_track_conductors_track ON track_conductors(track_id)",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_track_performers_track ON track_performers(track_id)",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_track_ensembles_track ON track_ensembles(track_id)",
                        [],
                    )?;

                    // Migrate existing data from tracks table to normalized tables
                    // First, populate lookup tables from existing data
                    db.conn.execute(
                        "INSERT OR IGNORE INTO genres (name)
                         SELECT DISTINCT genre FROM tracks WHERE genre IS NOT NULL AND genre != ''",
                        [],
                    )?;
                    db.conn.execute(
                        "INSERT OR IGNORE INTO composers (name)
                         SELECT DISTINCT composer FROM tracks WHERE composer IS NOT NULL AND composer != ''",
                        [],
                    )?;
                    db.conn.execute(
                        "INSERT OR IGNORE INTO conductors (name)
                         SELECT DISTINCT conductor FROM tracks WHERE conductor IS NOT NULL AND conductor != ''",
                        [],
                    )?;
                    db.conn.execute(
                        "INSERT OR IGNORE INTO performers (name)
                         SELECT DISTINCT performer FROM tracks WHERE performer IS NOT NULL AND performer != ''",
                        [],
                    )?;
                    db.conn.execute(
                        "INSERT OR IGNORE INTO ensembles (name)
                         SELECT DISTINCT ensemble FROM tracks WHERE ensemble IS NOT NULL AND ensemble != ''",
                        [],
                    )?;

                    // Populate junction tables
                    db.conn.execute(
                        "INSERT OR IGNORE INTO track_genres (track_id, genre_id)
                         SELECT t.id, g.id FROM tracks t
                         JOIN genres g ON t.genre = g.name
                         WHERE t.genre IS NOT NULL AND t.genre != ''",
                        [],
                    )?;
                    db.conn.execute(
                        "INSERT OR IGNORE INTO track_composers (track_id, composer_id)
                         SELECT t.id, c.id FROM tracks t
                         JOIN composers c ON t.composer = c.name
                         WHERE t.composer IS NOT NULL AND t.composer != ''",
                        [],
                    )?;
                    db.conn.execute(
                        "INSERT OR IGNORE INTO track_conductors (track_id, conductor_id)
                         SELECT t.id, c.id FROM tracks t
                         JOIN conductors c ON t.conductor = c.name
                         WHERE t.conductor IS NOT NULL AND t.conductor != ''",
                        [],
                    )?;
                    db.conn.execute(
                        "INSERT OR IGNORE INTO track_performers (track_id, performer_id)
                         SELECT t.id, p.id FROM tracks t
                         JOIN performers p ON t.performer = p.name
                         WHERE t.performer IS NOT NULL AND t.performer != ''",
                        [],
                    )?;
                    db.conn.execute(
                        "INSERT OR IGNORE INTO track_ensembles (track_id, ensemble_id)
                         SELECT t.id, e.id FROM tracks t
                         JOIN ensembles e ON t.ensemble = e.name
                         WHERE t.ensemble IS NOT NULL AND t.ensemble != ''",
                        [],
                    )?;

                    log::info!("Created normalized metadata tables and migrated existing data");
                    Ok(())
                },
            },
        );

        // Migration 12: Remove artist from albums table, add artist to tracks table
        // Albums are now uniquely identified by title only; artist is derived from tracks
        migrations.insert(
            12,
            Migration {
                description: "Remove artist from albums, add artist to tracks - albums identified by title only",
                apply: |db| {
                    // Add artist column to tracks table
                    let has_artist = db
                        .conn
                        .prepare("SELECT artist FROM tracks LIMIT 1")
                        .is_ok();

                    if !has_artist {
                        db.conn
                            .execute("ALTER TABLE tracks ADD COLUMN artist TEXT", [])?;
                        log::info!("Added artist column to tracks table");
                    }

                    // Migrate artist data from albums to tracks
                    // Each track inherits the artist from its album
                    db.conn.execute(
                        "UPDATE tracks SET artist = (
                            SELECT a.artist FROM albums a WHERE a.id = tracks.album_id
                        ) WHERE artist IS NULL",
                        [],
                    )?;
                    log::info!("Migrated artist data from albums to tracks");

                    // SQLite doesn't support DROP COLUMN directly in older versions,
                    // and we need to keep the column for now to avoid breaking existing code
                    // that might still reference it. The column will be ignored in new code.
                    // In a future migration, we could recreate the table without the artist column.

                    // Update the UNIQUE constraint: albums should now be unique by title only
                    // SQLite doesn't allow modifying constraints directly, so we need to recreate the table
                    // For now, we'll leave the constraint as-is since removing it requires table recreation
                    // The application logic will handle uniqueness by title only

                    // Update FTS5 table to include track artist
                    // First, rebuild the FTS index with the new data
                    db.conn.execute("DELETE FROM library_fts", [])?;
                    db.conn.execute(
                        "INSERT INTO library_fts(artist, album_title, track_title, album_id)
                         SELECT
                            COALESCE(t.album_artist, t.artist, 'Unknown Artist'),
                            a.title,
                            t.title,
                            t.album_id
                         FROM tracks t
                         JOIN albums a ON t.album_id = a.id",
                        [],
                    )?;
                    log::info!("Rebuilt FTS index with track artist data");

                    // Update triggers to use track artist instead of album artist
                    // Drop old triggers
                    db.conn.execute("DROP TRIGGER IF EXISTS tracks_ai", [])?;
                    db.conn.execute("DROP TRIGGER IF EXISTS tracks_ad", [])?;
                    db.conn.execute("DROP TRIGGER IF EXISTS tracks_au", [])?;
                    db.conn.execute("DROP TRIGGER IF EXISTS albums_au", [])?;

                    // Create new triggers using track artist
                    db.conn.execute(
                        "CREATE TRIGGER tracks_ai AFTER INSERT ON tracks BEGIN
                            INSERT INTO library_fts(artist, album_title, track_title, album_id)
                            SELECT
                                COALESCE(new.album_artist, new.artist, 'Unknown Artist'),
                                a.title,
                                new.title,
                                new.album_id
                            FROM albums a WHERE a.id = new.album_id;
                        END;",
                        [],
                    )?;

                    db.conn.execute(
                        "CREATE TRIGGER tracks_ad AFTER DELETE ON tracks BEGIN
                            DELETE FROM library_fts WHERE album_id = old.album_id AND track_title = old.title;
                        END;",
                        [],
                    )?;

                    db.conn.execute(
                        "CREATE TRIGGER tracks_au AFTER UPDATE ON tracks BEGIN
                            DELETE FROM library_fts WHERE album_id = old.album_id AND track_title = old.title;
                            INSERT INTO library_fts(artist, album_title, track_title, album_id)
                            SELECT
                                COALESCE(new.album_artist, new.artist, 'Unknown Artist'),
                                a.title,
                                new.title,
                                new.album_id
                            FROM albums a WHERE a.id = new.album_id;
                        END;",
                        [],
                    )?;

                    db.conn.execute(
                        "CREATE TRIGGER albums_au AFTER UPDATE ON albums BEGIN
                            DELETE FROM library_fts WHERE album_id = old.id;
                            INSERT INTO library_fts(artist, album_title, track_title, album_id)
                            SELECT
                                COALESCE(t.album_artist, t.artist, 'Unknown Artist'),
                                new.title,
                                t.title,
                                t.album_id
                            FROM tracks t WHERE t.album_id = new.id;
                        END;",
                        [],
                    )?;

                    log::info!("Updated FTS triggers to use track artist");
                    Ok(())
                },
            },
        );

        // Migration 13: Add sample_rate and bit_depth columns to tracks table
        migrations.insert(
            13,
            Migration {
                description: "Add sample_rate and bit_depth columns to tracks table",
                apply: |db| {
                    // Add sample_rate column
                    let has_sample_rate = db
                        .conn
                        .prepare("SELECT sample_rate FROM tracks LIMIT 1")
                        .is_ok();

                    if !has_sample_rate {
                        db.conn
                            .execute("ALTER TABLE tracks ADD COLUMN sample_rate INTEGER", [])?;
                        log::info!("Added sample_rate column to tracks table");
                    }

                    // Add bit_depth column
                    let has_bit_depth = db
                        .conn
                        .prepare("SELECT bit_depth FROM tracks LIMIT 1")
                        .is_ok();

                    if !has_bit_depth {
                        db.conn
                            .execute("ALTER TABLE tracks ADD COLUMN bit_depth INTEGER", [])?;
                        log::info!("Added bit_depth column to tracks table");
                    }

                    Ok(())
                },
            },
        );

        // Migration 14: Add bliss audio analysis columns to tracks table
        migrations.insert(
            14,
            Migration {
                description: "Add bliss audio analysis columns for music similarity",
                apply: |db| {
                    // Add bliss_tempo column (BPM)
                    let has_bliss_tempo = db
                        .conn
                        .prepare("SELECT bliss_tempo FROM tracks LIMIT 1")
                        .is_ok();

                    if !has_bliss_tempo {
                        db.conn
                            .execute("ALTER TABLE tracks ADD COLUMN bliss_tempo REAL", [])?;
                        log::info!("Added bliss_tempo column to tracks table");
                    }

                    // Add bliss_zcr column (zero-crossing rate)
                    let has_bliss_zcr = db
                        .conn
                        .prepare("SELECT bliss_zcr FROM tracks LIMIT 1")
                        .is_ok();

                    if !has_bliss_zcr {
                        db.conn
                            .execute("ALTER TABLE tracks ADD COLUMN bliss_zcr REAL", [])?;
                        log::info!("Added bliss_zcr column to tracks table");
                    }

                    // Add bliss_loudness column (mean loudness)
                    let has_bliss_loudness = db
                        .conn
                        .prepare("SELECT bliss_loudness FROM tracks LIMIT 1")
                        .is_ok();

                    if !has_bliss_loudness {
                        db.conn
                            .execute("ALTER TABLE tracks ADD COLUMN bliss_loudness REAL", [])?;
                        log::info!("Added bliss_loudness column to tracks table");
                    }

                    // Add bliss_features column (BLOB storing full feature vector)
                    let has_bliss_features = db
                        .conn
                        .prepare("SELECT bliss_features FROM tracks LIMIT 1")
                        .is_ok();

                    if !has_bliss_features {
                        db.conn
                            .execute("ALTER TABLE tracks ADD COLUMN bliss_features BLOB", [])?;
                        log::info!("Added bliss_features column to tracks table");
                    }

                    // Add bliss_analyzed_at column (timestamp of analysis)
                    let has_bliss_analyzed_at = db
                        .conn
                        .prepare("SELECT bliss_analyzed_at FROM tracks LIMIT 1")
                        .is_ok();

                    if !has_bliss_analyzed_at {
                        db.conn.execute(
                            "ALTER TABLE tracks ADD COLUMN bliss_analyzed_at INTEGER",
                            [],
                        )?;
                        log::info!("Added bliss_analyzed_at column to tracks table");
                    }

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

    /// Rebuild FTS index from current database state
    /// This ensures FTS is in sync after bulk operations like scanning
    pub fn sync_fts_index(&self) -> SqlResult<()> {
        // Clear existing FTS data
        self.conn.execute("DELETE FROM library_fts", [])?;

        // Rebuild from tracks and albums tables
        self.conn.execute(
            "INSERT INTO library_fts(artist, album_title, track_title, album_id)
             SELECT
                COALESCE(t.album_artist, t.artist, 'Unknown Artist'),
                a.title,
                t.title,
                t.album_id
             FROM tracks t
             JOIN albums a ON t.album_id = a.id",
            [],
        )?;

        log::debug!("FTS index synchronized with database");
        Ok(())
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
        // Note: We still select artist from albums for backwards compatibility with old databases,
        // but we don't use it - artist is now derived from tracks
        let mut albums_stmt = self.conn.prepare(
            "SELECT id, title, year, album_art_path, album_art_thumbnail FROM albums ORDER BY title",
        )?;

        let mut albums = Vec::new();
        let album_rows = albums_stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,             // id
                row.get::<_, String>(1)?,          // title
                row.get::<_, Option<i64>>(2)?,     // year
                row.get::<_, Option<String>>(3)?,  // album_art_path
                row.get::<_, Option<Vec<u8>>>(4)?, // album_art_thumbnail
            ))
        })?;

        // Get all play counts at once for efficiency
        let play_counts = self.get_all_album_play_counts()?;

        for album_row in album_rows {
            let (album_id, title, year, album_art_path, album_art_thumbnail) = album_row?;

            // Load tracks for this album (now including artist)
            let mut tracks_stmt = self.conn.prepare(
                "SELECT path, title, artist, track_number, duration_secs, channels,
                        sample_rate, bit_depth,
                        replay_gain, replay_peak, album_gain, album_peak, waveform,
                        genre, composer, disc_number, conductor, performer,
                        isrc, album_artist, ensemble
                 FROM tracks
                 WHERE album_id = ?1
                 ORDER BY disc_number, track_number",
            )?;

            let tracks = tracks_stmt
                .query_map(params![album_id], |row| {
                    Ok(Track {
                        path: PathBuf::from(row.get::<_, String>(0)?),
                        title: row.get::<_, Option<String>>(1)?,
                        artist: row.get::<_, Option<String>>(2)?,
                        track_number: row.get::<_, Option<i64>>(3)?.map(|n| n as u32),
                        duration_secs: row.get::<_, Option<i64>>(4)?.map(|n| n as u64),
                        channels: row.get::<_, Option<i64>>(5)?.map(|n| n as u32),
                        sample_rate: row.get::<_, Option<i64>>(6)?.map(|n| n as u32),
                        bit_depth: row.get::<_, Option<i64>>(7)?.map(|n| n as u32),
                        replay_gain: row.get::<_, Option<f64>>(8)?,
                        replay_peak: row.get::<_, Option<f64>>(9)?,
                        album_gain: row.get::<_, Option<f64>>(10)?,
                        album_peak: row.get::<_, Option<f64>>(11)?,
                        waveform: row.get::<_, Option<Vec<u8>>>(12)?,
                        genre: row.get::<_, Option<String>>(13)?,
                        composer: row.get::<_, Option<String>>(14)?,
                        disc_number: row.get::<_, Option<i64>>(15)?.map(|n| n as u32),
                        conductor: row.get::<_, Option<String>>(16)?,
                        performer: row.get::<_, Option<String>>(17)?,
                        isrc: row.get::<_, Option<String>>(18)?,
                        album_artist: row.get::<_, Option<String>>(19)?,
                        ensemble: row.get::<_, Option<String>>(20)?,
                        edition: None,
                    })
                })?
                .collect::<SqlResult<Vec<_>>>()?;

            let play_count = *play_counts.get(&album_id).unwrap_or(&0);

            albums.push(Album {
                id: Some(album_id),
                title,
                year: year.map(|y| y as u32),
                tracks,
                album_art_path: album_art_path.map(PathBuf::from),
                album_art_thumbnail,
                play_count,
                edition: None,
                dynamic_range: None,
            });
        }

        Ok(albums)
    }

    /// Save albums and tracks to database
    pub fn save_albums(&mut self, albums: &[Album]) -> SqlResult<()> {
        let tx = self.conn.transaction()?;
        let now = current_timestamp();

        for album in albums {
            // Compute artist from tracks for backwards compatibility with old schema
            // (old schema has artist column with UNIQUE(artist, title) constraint)
            let album_artist = album.artist();

            // Insert or update album
            // Note: We still insert artist for backwards compatibility, but it's derived from tracks
            tx.execute(
                "INSERT INTO albums (artist, title, year, album_art_path, album_art_thumbnail, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(artist, title) DO UPDATE SET
                 year = excluded.year,
                 album_art_path = excluded.album_art_path,
                 album_art_thumbnail = COALESCE(excluded.album_art_thumbnail, album_art_thumbnail),
                 updated_at = excluded.updated_at",
                params![
                    &album_artist,
                    &album.title,
                    album.year.map(|y| y as i64),
                    album
                        .album_art_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string()),
                    album.album_art_thumbnail.as_ref(),
                    now,
                    now,
                ],
            )?;

            // Get album ID by title (artist may vary for compilations)
            // We use the computed artist for lookup to match the UNIQUE constraint
            let album_id: i64 = tx.query_row(
                "SELECT id FROM albums WHERE artist = ?1 AND title = ?2",
                params![&album_artist, &album.title],
                |row| row.get(0),
            )?;

            // Insert or update tracks (now including artist)
            for track in &album.tracks {
                let file_mtime = get_file_mtime(&track.path).unwrap_or(0);
                let path_str = track.path.to_string_lossy().to_string();

                tx.execute(
                    "INSERT INTO tracks (album_id, path, title, artist, track_number, duration_secs, channels,
                                        sample_rate, bit_depth,
                                        file_mtime, scanned_at, created_at, updated_at, waveform,
                                        genre, composer, disc_number, conductor, performer,
                                        isrc, album_artist, ensemble)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)
                     ON CONFLICT(path) DO UPDATE SET
                     album_id = excluded.album_id,
                     title = excluded.title,
                     artist = excluded.artist,
                     track_number = excluded.track_number,
                     duration_secs = excluded.duration_secs,
                     channels = excluded.channels,
                     sample_rate = excluded.sample_rate,
                     bit_depth = excluded.bit_depth,
                     file_mtime = excluded.file_mtime,
                     scanned_at = excluded.scanned_at,
                     updated_at = excluded.updated_at,
                     waveform = COALESCE(excluded.waveform, waveform),
                     genre = excluded.genre,
                     composer = excluded.composer,
                     disc_number = excluded.disc_number,
                     conductor = excluded.conductor,
                     performer = excluded.performer,
                     isrc = excluded.isrc,
                     album_artist = excluded.album_artist,
                     ensemble = excluded.ensemble",
                    params![
                        album_id,
                        &path_str,
                        track.title,
                        track.artist,
                        track.track_number.map(|n| n as i64),
                        track.duration_secs.map(|n| n as i64),
                        track.channels.map(|n| n as i64),
                        track.sample_rate.map(|n| n as i64),
                        track.bit_depth.map(|n| n as i64),
                        file_mtime as i64,
                        now,
                        now,
                        now,
                        track.waveform.as_ref(),
                        track.genre,
                        track.composer,
                        track.disc_number.map(|n| n as i64),
                        track.conductor,
                        track.performer,
                        track.isrc,
                        track.album_artist,
                        track.ensemble,
                    ],
                )?;

                // Get track ID for junction table updates
                let track_id: i64 = tx.query_row(
                    "SELECT id FROM tracks WHERE path = ?1",
                    params![&path_str],
                    |row| row.get(0),
                )?;

                // Clear existing junction table entries for this track
                tx.execute(
                    "DELETE FROM track_genres WHERE track_id = ?1",
                    params![track_id],
                )?;
                tx.execute(
                    "DELETE FROM track_composers WHERE track_id = ?1",
                    params![track_id],
                )?;
                tx.execute(
                    "DELETE FROM track_conductors WHERE track_id = ?1",
                    params![track_id],
                )?;
                tx.execute(
                    "DELETE FROM track_performers WHERE track_id = ?1",
                    params![track_id],
                )?;
                tx.execute(
                    "DELETE FROM track_ensembles WHERE track_id = ?1",
                    params![track_id],
                )?;

                // Insert into normalized tables and junction tables
                // Genre, composer, and performer can contain multiple values separated by , / ;
                // Genre uses special normalization (dots/underscores to spaces, title case)
                if let Some(ref genre) = track.genre {
                    for g in split_and_normalize_genres(genre) {
                        tx.execute(
                            "INSERT OR IGNORE INTO genres (name) VALUES (?1)",
                            params![&g],
                        )?;
                        tx.execute(
                            "INSERT OR IGNORE INTO track_genres (track_id, genre_id)
                             SELECT ?1, id FROM genres WHERE name = ?2",
                            params![track_id, &g],
                        )?;
                    }
                }

                if let Some(ref composer) = track.composer {
                    for c in split_metadata_value(composer) {
                        tx.execute(
                            "INSERT OR IGNORE INTO composers (name) VALUES (?1)",
                            params![&c],
                        )?;
                        tx.execute(
                            "INSERT OR IGNORE INTO track_composers (track_id, composer_id)
                             SELECT ?1, id FROM composers WHERE name = ?2",
                            params![track_id, &c],
                        )?;
                    }
                }

                // Conductor is typically a single value
                if let Some(ref conductor) = track.conductor {
                    if !conductor.is_empty() {
                        tx.execute(
                            "INSERT OR IGNORE INTO conductors (name) VALUES (?1)",
                            params![conductor],
                        )?;
                        tx.execute(
                            "INSERT OR IGNORE INTO track_conductors (track_id, conductor_id)
                             SELECT ?1, id FROM conductors WHERE name = ?2",
                            params![track_id, conductor],
                        )?;
                    }
                }

                if let Some(ref performer) = track.performer {
                    for p in split_metadata_value(performer) {
                        tx.execute(
                            "INSERT OR IGNORE INTO performers (name) VALUES (?1)",
                            params![&p],
                        )?;
                        tx.execute(
                            "INSERT OR IGNORE INTO track_performers (track_id, performer_id)
                             SELECT ?1, id FROM performers WHERE name = ?2",
                            params![track_id, &p],
                        )?;
                    }
                }

                // Ensemble is typically a single value
                if let Some(ref ensemble) = track.ensemble {
                    if !ensemble.is_empty() {
                        tx.execute(
                            "INSERT OR IGNORE INTO ensembles (name) VALUES (?1)",
                            params![ensemble],
                        )?;
                        tx.execute(
                            "INSERT OR IGNORE INTO track_ensembles (track_id, ensemble_id)
                             SELECT ?1, id FROM ensembles WHERE name = ?2",
                            params![track_id, ensemble],
                        )?;
                    }
                }
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

    /// Update waveform data for a track
    pub fn update_waveform(&self, path: &Path, waveform: &[u8]) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE tracks SET waveform = ?1 WHERE path = ?2",
            params![waveform, path.to_str().unwrap()],
        )?;
        Ok(())
    }

    /// Get tracks that don't have waveform data yet
    pub fn get_tracks_without_waveform(&self) -> SqlResult<Vec<PathBuf>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path FROM tracks WHERE waveform IS NULL")?;

        let paths = stmt
            .query_map([], |row| {
                let path_str: String = row.get(0)?;
                Ok(PathBuf::from(path_str))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(paths)
    }

    /// Update bliss audio analysis values for a track
    pub fn update_bliss(
        &self,
        path: &Path,
        analysis: &crate::bliss::BlissAnalysis,
    ) -> SqlResult<()> {
        let now = current_timestamp();
        let features_blob = analysis.to_bytes();

        self.conn.execute(
            "UPDATE tracks SET
                bliss_tempo = ?1,
                bliss_zcr = ?2,
                bliss_loudness = ?3,
                bliss_features = ?4,
                bliss_analyzed_at = ?5
             WHERE path = ?6",
            params![
                analysis.tempo as f64,
                analysis.zcr as f64,
                analysis.loudness_mean as f64,
                features_blob,
                now,
                path.to_str().unwrap()
            ],
        )?;
        Ok(())
    }

    /// Get tracks that don't have bliss analysis yet
    pub fn get_tracks_without_bliss(&self) -> SqlResult<Vec<PathBuf>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path FROM tracks WHERE bliss_analyzed_at IS NULL")?;

        let paths = stmt
            .query_map([], |row| {
                let path_str: String = row.get(0)?;
                Ok(PathBuf::from(path_str))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(paths)
    }

    /// Get bliss analysis for a track by path
    pub fn get_bliss_analysis(
        &self,
        path: &Path,
    ) -> SqlResult<Option<crate::bliss::BlissAnalysis>> {
        let path_str = path.to_string_lossy();
        let mut stmt = self.conn.prepare(
            "SELECT bliss_features FROM tracks WHERE path = ?1 AND bliss_features IS NOT NULL",
        )?;

        let result = stmt.query_row(params![path_str.as_ref()], |row| {
            let features_blob: Vec<u8> = row.get(0)?;
            Ok(features_blob)
        });

        match result {
            Ok(blob) => Ok(crate::bliss::BlissAnalysis::from_bytes(&blob)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
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

    // ==================== Playlist Methods ====================

    /// Create a new playlist
    /// Returns the ID of the newly created playlist
    pub fn create_playlist(&self, name: &str, description: Option<&str>) -> SqlResult<i64> {
        let now = current_timestamp();
        self.conn.execute(
            "INSERT INTO playlists (name, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![name, description, now, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Update a playlist's name and/or description
    pub fn update_playlist(
        &self,
        playlist_id: i64,
        name: &str,
        description: Option<&str>,
    ) -> SqlResult<()> {
        let now = current_timestamp();
        self.conn.execute(
            "UPDATE playlists SET name = ?1, description = ?2, updated_at = ?3 WHERE id = ?4",
            params![name, description, now, playlist_id],
        )?;
        Ok(())
    }

    /// Delete a playlist and all its tracks
    pub fn delete_playlist(&self, playlist_id: i64) -> SqlResult<()> {
        // Foreign key cascade will delete playlist_tracks entries
        self.conn
            .execute("DELETE FROM playlists WHERE id = ?1", params![playlist_id])?;
        Ok(())
    }

    /// Get all playlists (without their tracks)
    pub fn get_playlists(&self) -> SqlResult<Vec<Playlist>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, created_at, updated_at FROM playlists ORDER BY name",
        )?;

        let playlists = stmt
            .query_map([], |row| {
                Ok(Playlist {
                    id: Some(row.get::<_, i64>(0)?),
                    name: row.get::<_, String>(1)?,
                    description: row.get::<_, Option<String>>(2)?,
                    entries: Vec::new(), // Will be loaded separately if needed
                    created_at: Some(row.get::<_, i64>(3)? as u64),
                    updated_at: Some(row.get::<_, i64>(4)? as u64),
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(playlists)
    }

    /// Get a single playlist by ID (without tracks)
    pub fn get_playlist(&self, playlist_id: i64) -> SqlResult<Option<Playlist>> {
        let result = self.conn.query_row(
            "SELECT id, name, description, created_at, updated_at FROM playlists WHERE id = ?1",
            params![playlist_id],
            |row| {
                Ok(Playlist {
                    id: Some(row.get::<_, i64>(0)?),
                    name: row.get::<_, String>(1)?,
                    description: row.get::<_, Option<String>>(2)?,
                    entries: Vec::new(),
                    created_at: Some(row.get::<_, i64>(3)? as u64),
                    updated_at: Some(row.get::<_, i64>(4)? as u64),
                })
            },
        );

        match result {
            Ok(playlist) => Ok(Some(playlist)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a playlist by name
    pub fn get_playlist_by_name(&self, name: &str) -> SqlResult<Option<Playlist>> {
        let result = self.conn.query_row(
            "SELECT id, name, description, created_at, updated_at FROM playlists WHERE name = ?1",
            params![name],
            |row| {
                Ok(Playlist {
                    id: Some(row.get::<_, i64>(0)?),
                    name: row.get::<_, String>(1)?,
                    description: row.get::<_, Option<String>>(2)?,
                    entries: Vec::new(),
                    created_at: Some(row.get::<_, i64>(3)? as u64),
                    updated_at: Some(row.get::<_, i64>(4)? as u64),
                })
            },
        );

        match result {
            Ok(playlist) => Ok(Some(playlist)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Load tracks for a playlist
    pub fn get_playlist_tracks(&self, playlist_id: i64) -> SqlResult<Vec<PlaylistEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT track_path, position FROM playlist_tracks
             WHERE playlist_id = ?1 ORDER BY position",
        )?;

        let entries = stmt
            .query_map(params![playlist_id], |row| {
                Ok(PlaylistEntry {
                    track_path: PathBuf::from(row.get::<_, String>(0)?),
                    position: row.get::<_, i64>(1)? as u32,
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(entries)
    }

    /// Load a playlist with all its tracks
    pub fn get_playlist_with_tracks(&self, playlist_id: i64) -> SqlResult<Option<Playlist>> {
        let mut playlist = match self.get_playlist(playlist_id)? {
            Some(p) => p,
            None => return Ok(None),
        };

        playlist.entries = self.get_playlist_tracks(playlist_id)?;
        Ok(Some(playlist))
    }

    /// Add a track to a playlist at the end
    pub fn add_track_to_playlist(&self, playlist_id: i64, track_path: &Path) -> SqlResult<()> {
        let now = current_timestamp();

        // Get the next position (max + 1)
        let next_position: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
            |row| row.get(0),
        )?;

        self.conn.execute(
            "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_path, position, added_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                playlist_id,
                track_path.to_str().unwrap(),
                next_position,
                now
            ],
        )?;

        // Update playlist's updated_at
        self.conn.execute(
            "UPDATE playlists SET updated_at = ?1 WHERE id = ?2",
            params![now, playlist_id],
        )?;

        Ok(())
    }

    /// Add multiple tracks to a playlist at the end
    pub fn add_tracks_to_playlist(
        &self,
        playlist_id: i64,
        track_paths: &[PathBuf],
    ) -> SqlResult<()> {
        let now = current_timestamp();

        // Get the next position (max + 1)
        let mut next_position: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
            |row| row.get(0),
        )?;

        for track_path in track_paths {
            self.conn.execute(
                "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_path, position, added_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    playlist_id,
                    track_path.to_str().unwrap(),
                    next_position,
                    now
                ],
            )?;
            next_position += 1;
        }

        // Update playlist's updated_at
        self.conn.execute(
            "UPDATE playlists SET updated_at = ?1 WHERE id = ?2",
            params![now, playlist_id],
        )?;

        Ok(())
    }

    /// Remove a track from a playlist
    pub fn remove_track_from_playlist(&self, playlist_id: i64, track_path: &Path) -> SqlResult<()> {
        let now = current_timestamp();

        // Get the position of the track to be removed
        let position: Option<i64> = self
            .conn
            .query_row(
                "SELECT position FROM playlist_tracks WHERE playlist_id = ?1 AND track_path = ?2",
                params![playlist_id, track_path.to_str().unwrap()],
                |row| row.get(0),
            )
            .ok();

        // Delete the track
        self.conn.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_path = ?2",
            params![playlist_id, track_path.to_str().unwrap()],
        )?;

        // Reorder remaining tracks to fill the gap
        if let Some(removed_position) = position {
            self.conn.execute(
                "UPDATE playlist_tracks SET position = position - 1
                 WHERE playlist_id = ?1 AND position > ?2",
                params![playlist_id, removed_position],
            )?;
        }

        // Update playlist's updated_at
        self.conn.execute(
            "UPDATE playlists SET updated_at = ?1 WHERE id = ?2",
            params![now, playlist_id],
        )?;

        Ok(())
    }

    /// Move a track to a new position in the playlist
    pub fn move_track_in_playlist(
        &self,
        playlist_id: i64,
        track_path: &Path,
        new_position: u32,
    ) -> SqlResult<()> {
        let now = current_timestamp();

        // Get the current position
        let current_position: i64 = self.conn.query_row(
            "SELECT position FROM playlist_tracks WHERE playlist_id = ?1 AND track_path = ?2",
            params![playlist_id, track_path.to_str().unwrap()],
            |row| row.get(0),
        )?;

        let new_pos = new_position as i64;

        if current_position == new_pos {
            return Ok(()); // No change needed
        }

        if current_position < new_pos {
            // Moving down: shift tracks between current+1 and new_pos up by 1
            self.conn.execute(
                "UPDATE playlist_tracks SET position = position - 1
                 WHERE playlist_id = ?1 AND position > ?2 AND position <= ?3",
                params![playlist_id, current_position, new_pos],
            )?;
        } else {
            // Moving up: shift tracks between new_pos and current-1 down by 1
            self.conn.execute(
                "UPDATE playlist_tracks SET position = position + 1
                 WHERE playlist_id = ?1 AND position >= ?2 AND position < ?3",
                params![playlist_id, new_pos, current_position],
            )?;
        }

        // Update the track's position
        self.conn.execute(
            "UPDATE playlist_tracks SET position = ?1
             WHERE playlist_id = ?2 AND track_path = ?3",
            params![new_pos, playlist_id, track_path.to_str().unwrap()],
        )?;

        // Update playlist's updated_at
        self.conn.execute(
            "UPDATE playlists SET updated_at = ?1 WHERE id = ?2",
            params![now, playlist_id],
        )?;

        Ok(())
    }

    /// Clear all tracks from a playlist
    pub fn clear_playlist(&self, playlist_id: i64) -> SqlResult<()> {
        let now = current_timestamp();

        self.conn.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
        )?;

        // Update playlist's updated_at
        self.conn.execute(
            "UPDATE playlists SET updated_at = ?1 WHERE id = ?2",
            params![now, playlist_id],
        )?;

        Ok(())
    }

    /// Get the number of tracks in a playlist
    pub fn get_playlist_track_count(&self, playlist_id: i64) -> SqlResult<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    // ==================== Normalized Metadata Methods ====================

    /// Get all genres in the library
    pub fn get_all_genres(&self) -> SqlResult<Vec<(i64, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM genres ORDER BY name COLLATE NOCASE")?;

        let genres = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(genres)
    }

    /// Get all composers in the library
    pub fn get_all_composers(&self) -> SqlResult<Vec<(i64, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM composers ORDER BY name COLLATE NOCASE")?;

        let composers = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(composers)
    }

    /// Get all conductors in the library
    pub fn get_all_conductors(&self) -> SqlResult<Vec<(i64, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM conductors ORDER BY name COLLATE NOCASE")?;

        let conductors = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(conductors)
    }

    /// Get all performers in the library
    pub fn get_all_performers(&self) -> SqlResult<Vec<(i64, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM performers ORDER BY name COLLATE NOCASE")?;

        let performers = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(performers)
    }

    /// Get all ensembles in the library
    pub fn get_all_ensembles(&self) -> SqlResult<Vec<(i64, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM ensembles ORDER BY name COLLATE NOCASE")?;

        let ensembles = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(ensembles)
    }

    /// Get tracks by genre
    pub fn get_tracks_by_genre(&self, genre_id: i64) -> SqlResult<Vec<PathBuf>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.path FROM tracks t
             JOIN track_genres tg ON t.id = tg.track_id
             WHERE tg.genre_id = ?1
             ORDER BY t.path",
        )?;

        let paths = stmt
            .query_map(params![genre_id], |row| {
                Ok(PathBuf::from(row.get::<_, String>(0)?))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(paths)
    }

    /// Get tracks by composer
    pub fn get_tracks_by_composer(&self, composer_id: i64) -> SqlResult<Vec<PathBuf>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.path FROM tracks t
             JOIN track_composers tc ON t.id = tc.track_id
             WHERE tc.composer_id = ?1
             ORDER BY t.path",
        )?;

        let paths = stmt
            .query_map(params![composer_id], |row| {
                Ok(PathBuf::from(row.get::<_, String>(0)?))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(paths)
    }

    /// Get tracks by conductor
    pub fn get_tracks_by_conductor(&self, conductor_id: i64) -> SqlResult<Vec<PathBuf>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.path FROM tracks t
             JOIN track_conductors tc ON t.id = tc.track_id
             WHERE tc.conductor_id = ?1
             ORDER BY t.path",
        )?;

        let paths = stmt
            .query_map(params![conductor_id], |row| {
                Ok(PathBuf::from(row.get::<_, String>(0)?))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(paths)
    }

    /// Get tracks by performer
    pub fn get_tracks_by_performer(&self, performer_id: i64) -> SqlResult<Vec<PathBuf>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.path FROM tracks t
             JOIN track_performers tp ON t.id = tp.track_id
             WHERE tp.performer_id = ?1
             ORDER BY t.path",
        )?;

        let paths = stmt
            .query_map(params![performer_id], |row| {
                Ok(PathBuf::from(row.get::<_, String>(0)?))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(paths)
    }

    /// Get tracks by ensemble
    pub fn get_tracks_by_ensemble(&self, ensemble_id: i64) -> SqlResult<Vec<PathBuf>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.path FROM tracks t
             JOIN track_ensembles te ON t.id = te.track_id
             WHERE te.ensemble_id = ?1
             ORDER BY t.path",
        )?;

        let paths = stmt
            .query_map(params![ensemble_id], |row| {
                Ok(PathBuf::from(row.get::<_, String>(0)?))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(paths)
    }

    /// Get genre for a track (returns first if multiple)
    pub fn get_track_genre(&self, track_path: &Path) -> SqlResult<Option<String>> {
        let path_str = track_path.to_string_lossy();
        let result = self.conn.query_row(
            "SELECT g.name FROM genres g
             JOIN track_genres tg ON g.id = tg.genre_id
             JOIN tracks t ON t.id = tg.track_id
             WHERE t.path = ?1
             LIMIT 1",
            params![path_str.as_ref()],
            |row| row.get::<_, String>(0),
        );

        match result {
            Ok(name) => Ok(Some(name)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get all genres for a track
    pub fn get_track_genres(&self, track_path: &Path) -> SqlResult<Vec<String>> {
        let path_str = track_path.to_string_lossy();
        let mut stmt = self.conn.prepare(
            "SELECT g.name FROM genres g
             JOIN track_genres tg ON g.id = tg.genre_id
             JOIN tracks t ON t.id = tg.track_id
             WHERE t.path = ?1
             ORDER BY g.name COLLATE NOCASE",
        )?;

        let genres = stmt
            .query_map(params![path_str.as_ref()], |row| row.get::<_, String>(0))?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(genres)
    }

    /// Get albums by genre (returns album IDs)
    pub fn get_albums_by_genre(&self, genre_id: i64) -> SqlResult<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT t.album_id FROM tracks t
             JOIN track_genres tg ON t.id = tg.track_id
             WHERE tg.genre_id = ?1
             ORDER BY t.album_id",
        )?;

        let album_ids = stmt
            .query_map(params![genre_id], |row| row.get::<_, i64>(0))?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(album_ids)
    }

    /// Get albums by composer (returns album IDs)
    pub fn get_albums_by_composer(&self, composer_id: i64) -> SqlResult<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT t.album_id FROM tracks t
             JOIN track_composers tc ON t.id = tc.track_id
             WHERE tc.composer_id = ?1
             ORDER BY t.album_id",
        )?;

        let album_ids = stmt
            .query_map(params![composer_id], |row| row.get::<_, i64>(0))?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(album_ids)
    }
}

/// Create a timestamped backup of an existing database file and prune old backups
/// Backup files are named music-YYYYMMDD-HHMMSS.sqlite in the same directory
///
/// # Security
/// Both the source database and backup destination must be within the config directory.
pub fn backup_existing_database<P: AsRef<Path>>(
    db_path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = db_path.as_ref();

    // Nothing to do if the database does not exist yet
    if !db_path.exists() {
        return Ok(());
    }

    // Security validation: ensure we're operating within config directory
    crate::security::validate_config_read_path(db_path)?;

    let dir = match db_path.parent() {
        Some(d) => d.to_path_buf(),
        None => return Ok(()),
    };

    let now = Local::now().naive_local();
    let filename = format!("music-{}.sqlite", now.format("%Y%m%d-%H%M%S"));
    let backup_path = dir.join(filename);

    // Security validation: ensure we're writing within config directory
    crate::security::validate_write_path(&backup_path)?;

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
///
/// # Security
/// Only deletes backup files within the config directory.
fn prune_old_backups(dir: &Path) -> std::io::Result<()> {
    use std::collections::HashMap;

    // Security validation: ensure we're operating within config directory
    if crate::security::validate_write_path(dir).is_err() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Cannot prune backups outside config directory",
        ));
    }

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
        // Security validation for each file before deletion
        if crate::security::validate_write_path(&path).is_ok() {
            let _ = std::fs::remove_file(path);
        }
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

    #[test]
    fn test_backup_existing_database_creates_backup_file() {
        // Use the real config directory for testing to satisfy security validation
        let config_dir = match crate::config::get_app_config_dir() {
            Some(dir) => dir,
            None => {
                eprintln!("Skipping test: could not get config directory");
                return;
            }
        };

        let test_dir = config_dir.join("test_backup");
        std::fs::create_dir_all(&test_dir).unwrap();

        let db_path = test_dir.join("music.db");
        std::fs::write(&db_path, b"test").unwrap();

        backup_existing_database(&db_path).unwrap();

        let mut backups = Vec::new();
        for entry in std::fs::read_dir(&test_dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            if name.starts_with("music-") && name.ends_with(".sqlite") {
                backups.push(name);
            }
        }

        assert_eq!(backups.len(), 1);

        // Cleanup
        std::fs::remove_dir_all(&test_dir).ok();
    }

    #[test]
    fn test_prune_old_backups_limits_to_three_per_day() {
        // Use the real config directory for testing to satisfy security validation
        let config_dir = match crate::config::get_app_config_dir() {
            Some(dir) => dir,
            None => {
                eprintln!("Skipping test: could not get config directory");
                return;
            }
        };

        let test_dir = config_dir.join("test_prune");
        std::fs::create_dir_all(&test_dir).unwrap();

        // Create 5 backups for the same day
        let ts = [
            "20250101-010000",
            "20250101-020000",
            "20250101-030000",
            "20250101-040000",
            "20250101-050000",
        ];

        for t in &ts {
            let path = test_dir.join(format!("music-{}.sqlite", t));
            std::fs::write(path, b"test").unwrap();
        }

        prune_old_backups(&test_dir).unwrap();

        let mut remaining = Vec::new();
        for entry in std::fs::read_dir(&test_dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            if name.starts_with("music-") && name.ends_with(".sqlite") {
                remaining.push(name);
            }
        }

        // Only 3 backups for that day should remain
        assert_eq!(remaining.len(), 3);

        // Cleanup
        std::fs::remove_dir_all(&test_dir).ok();
    }
}
