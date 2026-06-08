//\! Database schema and migrations
use rusqlite::{Result as SqlResult, params};
use std::collections::HashMap;

use super::{MusicDatabase, current_timestamp};

/// A database migration with description and apply function
#[derive(Debug)]
pub struct Migration {
    pub description: &'static str,
    pub apply: fn(&MusicDatabase) -> SqlResult<()>,
}

/// Define all database migrations
pub fn get_migrations(_db: &MusicDatabase) -> HashMap<i64, Migration> {
    let mut migrations = HashMap::new();
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
                    db.conn
                        .execute("ALTER TABLE albums ADD COLUMN album_art_thumbnail BLOB", [])?;
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

    // Migration 15: Add track path to FTS index for filename-based search
    migrations.insert(
            15,
            Migration {
                description: "Add track path to FTS index for filename-based search",
                apply: |db| {
                    // Drop old FTS table and triggers
                    db.conn.execute("DROP TABLE IF EXISTS library_fts", [])?;
                    db.conn.execute("DROP TRIGGER IF EXISTS tracks_ai", [])?;
                    db.conn.execute("DROP TRIGGER IF EXISTS tracks_ad", [])?;
                    db.conn.execute("DROP TRIGGER IF EXISTS tracks_au", [])?;
                    db.conn.execute("DROP TRIGGER IF EXISTS albums_au", [])?;

                    // Create new FTS table with track_path column
                    db.conn.execute(
                        "CREATE VIRTUAL TABLE IF NOT EXISTS library_fts USING fts5(
                            artist,
                            album_title,
                            track_title,
                            track_path,
                            album_id UNINDEXED
                        )",
                        [],
                    )?;

                    // Create triggers with path included
                    db.conn.execute(
                        "CREATE TRIGGER tracks_ai AFTER INSERT ON tracks BEGIN
                            INSERT INTO library_fts(artist, album_title, track_title, track_path, album_id)
                            SELECT
                                COALESCE(new.album_artist, new.artist, 'Unknown Artist'),
                                a.title,
                                new.title,
                                new.path,
                                new.album_id
                            FROM albums a WHERE a.id = new.album_id;
                        END;",
                        [],
                    )?;

                    db.conn.execute(
                        "CREATE TRIGGER tracks_ad AFTER DELETE ON tracks BEGIN
                            DELETE FROM library_fts WHERE track_path = old.path;
                        END;",
                        [],
                    )?;

                    db.conn.execute(
                        "CREATE TRIGGER tracks_au AFTER UPDATE ON tracks BEGIN
                            DELETE FROM library_fts WHERE track_path = old.path;
                            INSERT INTO library_fts(artist, album_title, track_title, track_path, album_id)
                            SELECT
                                COALESCE(new.album_artist, new.artist, 'Unknown Artist'),
                                a.title,
                                new.title,
                                new.path,
                                new.album_id
                            FROM albums a WHERE a.id = new.album_id;
                        END;",
                        [],
                    )?;

                    db.conn.execute(
                        "CREATE TRIGGER albums_au AFTER UPDATE ON albums BEGIN
                            DELETE FROM library_fts WHERE album_id = old.id;
                            INSERT INTO library_fts(artist, album_title, track_title, track_path, album_id)
                            SELECT
                                COALESCE(t.album_artist, t.artist, 'Unknown Artist'),
                                new.title,
                                t.title,
                                t.path,
                                t.album_id
                            FROM tracks t WHERE t.album_id = new.id;
                        END;",
                        [],
                    )?;

                    // Rebuild FTS index with path data
                    db.sync_fts_index()?;

                    log::info!("Added track_path to FTS index for filename-based search");
                    Ok(())
                },
            },
        );

    // Migration 16: Add favorites support for tracks and albums
    migrations.insert(
        16,
        Migration {
            description: "Add is_favorite column to tracks and albums tables",
            apply: |db| {
                // Add is_favorite to tracks
                let has_track_fav = db
                    .conn
                    .prepare("SELECT is_favorite FROM tracks LIMIT 1")
                    .is_ok();

                if !has_track_fav {
                    db.conn.execute(
                        "ALTER TABLE tracks ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0",
                        [],
                    )?;
                    log::info!("Added is_favorite column to tracks table");
                }

                // Add is_favorite to albums
                let has_album_fav = db
                    .conn
                    .prepare("SELECT is_favorite FROM albums LIMIT 1")
                    .is_ok();

                if !has_album_fav {
                    db.conn.execute(
                        "ALTER TABLE albums ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0",
                        [],
                    )?;
                    log::info!("Added is_favorite column to albums table");
                }

                // Create indexes for favorite queries
                db.conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_tracks_favorite ON tracks(is_favorite)",
                    [],
                )?;
                db.conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_albums_favorite ON albums(is_favorite)",
                    [],
                )?;

                log::info!("Added favorites support to tracks and albums");
                Ok(())
            },
        },
    );

    // Migration 17: Add scanner error columns for tracking failures
    migrations.insert(
        17,
        Migration {
            description: "Add error columns for replay_gain, waveform, and bliss scanners",
            apply: |db| {
                db.conn
                    .execute("ALTER TABLE tracks ADD COLUMN replay_gain_error TEXT", [])?;
                db.conn
                    .execute("ALTER TABLE tracks ADD COLUMN waveform_error TEXT", [])?;
                db.conn
                    .execute("ALTER TABLE tracks ADD COLUMN bliss_error TEXT", [])?;
                log::info!("Added scanner error columns to tracks table");
                Ok(())
            },
        },
    );

    // Migration 18: Add composite index for faster load_library track queries
    migrations.insert(
            18,
            Migration {
                description: "Add composite index on tracks(album_id, disc_number, track_number) for faster library loading",
                apply: |db| {
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_tracks_album_disc_track ON tracks(album_id, disc_number, track_number)",
                        [],
                    )?;
                    log::info!("Added composite index idx_tracks_album_disc_track");
                    Ok(())
                },
            },
        );

    // Migration 19: Library federation tables + stable UUIDs
    migrations.insert(
            19,
            Migration {
                description: "Add library federation tables (library_sources, track_sources, album_sources) and UUID columns",
                apply: |db| {
                    // Sources registry: one row per configured library provider
                    db.conn.execute(
                        "CREATE TABLE IF NOT EXISTS library_sources (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            source_id TEXT NOT NULL UNIQUE,
                            source_type TEXT NOT NULL,
                            display_name TEXT NOT NULL,
                            config_json TEXT,
                            last_sync_at INTEGER,
                            is_enabled INTEGER NOT NULL DEFAULT 1,
                            priority INTEGER NOT NULL DEFAULT 0,
                            created_at INTEGER NOT NULL,
                            updated_at INTEGER NOT NULL
                        )",
                        [],
                    )?;

                    // Seed the default local source with highest priority
                    let now = current_timestamp();
                    db.conn.execute(
                        "INSERT OR IGNORE INTO library_sources (source_id, source_type, display_name, priority, created_at, updated_at)
                         VALUES ('local', 'local', 'Local Files', 100, ?1, ?1)",
                        params![now],
                    )?;

                    // Track <-> source junction (a track can exist in multiple sources)
                    db.conn.execute(
                        "CREATE TABLE IF NOT EXISTS track_sources (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            track_id INTEGER NOT NULL,
                            source_id INTEGER NOT NULL,
                            external_id TEXT NOT NULL,
                            source_path TEXT,
                            audio_source_json TEXT,
                            FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE,
                            FOREIGN KEY(source_id) REFERENCES library_sources(id) ON DELETE CASCADE,
                            UNIQUE(source_id, external_id)
                        )",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_track_sources_track ON track_sources(track_id)",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_track_sources_source ON track_sources(source_id)",
                        [],
                    )?;

                    // Album <-> source junction
                    db.conn.execute(
                        "CREATE TABLE IF NOT EXISTS album_sources (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            album_id INTEGER NOT NULL,
                            source_id INTEGER NOT NULL,
                            external_id TEXT NOT NULL,
                            FOREIGN KEY(album_id) REFERENCES albums(id) ON DELETE CASCADE,
                            FOREIGN KEY(source_id) REFERENCES library_sources(id) ON DELETE CASCADE,
                            UNIQUE(source_id, external_id)
                        )",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_album_sources_album ON album_sources(album_id)",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE INDEX IF NOT EXISTS idx_album_sources_source ON album_sources(source_id)",
                        [],
                    )?;

                    // Stable UUIDs for P2P readiness
                    db.conn.execute(
                        "ALTER TABLE albums ADD COLUMN uuid TEXT",
                        [],
                    )?;
                    db.conn.execute(
                        "ALTER TABLE tracks ADD COLUMN uuid TEXT",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE UNIQUE INDEX IF NOT EXISTS idx_albums_uuid ON albums(uuid) WHERE uuid IS NOT NULL",
                        [],
                    )?;
                    db.conn.execute(
                        "CREATE UNIQUE INDEX IF NOT EXISTS idx_tracks_uuid ON tracks(uuid) WHERE uuid IS NOT NULL",
                        [],
                    )?;

                    // Backfill: link all existing tracks to the local source
                    let local_source_id: i64 = db.conn.query_row(
                        "SELECT id FROM library_sources WHERE source_id = 'local'",
                        [],
                        |row| row.get(0),
                    )?;

                    db.conn.execute(
                        "INSERT OR IGNORE INTO track_sources (track_id, source_id, external_id, source_path)
                         SELECT id, ?1, path, path FROM tracks",
                        params![local_source_id],
                    )?;

                    db.conn.execute(
                        "INSERT OR IGNORE INTO album_sources (album_id, source_id, external_id)
                         SELECT id, ?1, CAST(id AS TEXT) FROM albums",
                        params![local_source_id],
                    )?;

                    let track_count: i64 = db.conn.query_row(
                        "SELECT COUNT(*) FROM track_sources",
                        [],
                        |row| row.get(0),
                    )?;
                    let album_count: i64 = db.conn.query_row(
                        "SELECT COUNT(*) FROM album_sources",
                        [],
                        |row| row.get(0),
                    )?;
                    log::info!(
                        "Federation migration: created source tables, linked {} tracks and {} albums to local source",
                        track_count, album_count
                    );

                    Ok(())
                },
            },
        );
    migrations.insert(
            20,
            Migration {
                description: "Add is_available column to library_sources for tracking source reachability",
                apply: |db| {
                    db.conn.execute(
                        "ALTER TABLE library_sources ADD COLUMN is_available INTEGER",
                        [],
                    )?;

                    // Local source is always available
                    db.conn.execute(
                        "UPDATE library_sources SET is_available = 1 WHERE source_id = 'local'",
                        [],
                    )?;

                    Ok(())
                },
            },
        );

    // Migration 21: Persist ReplayGain extended stats for album gain computation
    migrations.insert(
        21,
        Migration {
            description: "Add ReplayGain gating stats for album gain computation",
            apply: |db| {
                let has_block_count = db
                    .conn
                    .prepare("SELECT replay_gain_block_count FROM tracks LIMIT 1")
                    .is_ok();
                if !has_block_count {
                    db.conn.execute(
                        "ALTER TABLE tracks ADD COLUMN replay_gain_block_count INTEGER",
                        [],
                    )?;
                    log::info!("Added replay_gain_block_count column to tracks table");
                }

                let has_energy = db
                    .conn
                    .prepare("SELECT replay_gain_energy FROM tracks LIMIT 1")
                    .is_ok();
                if !has_energy {
                    db.conn
                        .execute("ALTER TABLE tracks ADD COLUMN replay_gain_energy REAL", [])?;
                    log::info!("Added replay_gain_energy column to tracks table");
                }

                Ok(())
            },
        },
    );
    migrations
}
