use rusqlite::{Connection, Result as SqlResult, params};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Local, NaiveDateTime};

use crate::config;

// Sub-modules for database functionality
mod analysis;
mod federation;
mod library;
mod metadata;
mod playback;
mod playlists;
mod schema;
mod search;
pub use schema::Migration;

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

/// Database manager for persistent music library storage
#[derive(Debug)]
pub struct MusicDatabase {
    pub(super) conn: Connection,
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

    /// Open an existing database for a secondary (read-only-intent) instance.
    /// Uses normal open (not SQLITE_OPEN_READ_ONLY) because read-only connections
    /// can't access uncommitted WAL data from the writer's shared memory map.
    /// Skips WAL pragma (already set by the primary writer) and schema migration.
    pub fn open_secondary<P: AsRef<Path>>(path: P) -> SqlResult<Self> {
        let path = path.as_ref();
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(Self { conn })
    }

    /// Internal database open that skips security validation.
    fn open_internal(path: &Path) -> SqlResult<Self> {
        let t0 = std::time::Instant::now();

        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrency
        // This allows multiple readers and one writer simultaneously
        conn.pragma_update(None, "journal_mode", "WAL")?;

        let mode: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        log::debug!("SQLite journal mode: {}", mode);

        // Set a busy timeout to avoid "database is locked" errors during concurrent access
        // 5 seconds is a reasonable default for desktop applications
        conn.busy_timeout(std::time::Duration::from_secs(5))?;

        let db = Self { conn };
        db.initialize_schema()?;

        log::info!(
            "[startup] Database open + schema init: {:.1}ms",
            t0.elapsed().as_secs_f64() * 1000.0
        );
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
        const LATEST_VERSION: i64 = 21;
        let migrations = schema::get_migrations(self);

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

    // Library methods (load_library, save_albums, scan tracking, file cleanup)
    // are in library.rs
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

    // Compute SHA-256 of the current database
    let current_hash = sha256_of_file(db_path)?;

    // Find the most recent backup and compare hashes
    if let Some(latest_backup) = find_latest_backup(&dir) {
        crate::security::validate_config_read_path(&latest_backup)?;
        if let Ok(backup_hash) = sha256_of_file(&latest_backup) {
            if current_hash == backup_hash {
                log::debug!("Database unchanged since last backup, skipping");
                return Ok(());
            }
        }
    }

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

/// Compute the SHA-256 hash of a file, reading in 8 KiB chunks.
fn sha256_of_file(path: &Path) -> Result<[u8; 32], std::io::Error> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().into())
}

/// Find the most recent backup file in the given directory.
fn find_latest_backup(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            parse_backup_timestamp(&path).map(|ts| (path, ts))
        })
        .max_by_key(|(_, ts)| *ts)
        .map(|(path, _)| path)
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

    fn fresh_config_test_dir(name: &str) -> std::path::PathBuf {
        let test_dir = crate::config::test_config_dir().join(name);
        std::fs::remove_dir_all(&test_dir).ok();
        std::fs::create_dir_all(&test_dir).unwrap();
        test_dir
    }

    #[test]
    fn test_backup_existing_database_creates_backup_file() {
        let test_dir = fresh_config_test_dir("test_backup");

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
        let test_dir = fresh_config_test_dir("test_prune");

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

    #[test]
    fn test_sha256_of_file() {
        let dir = std::env::temp_dir().join("sotf_test_sha256");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.bin");
        std::fs::write(&path, b"hello world").unwrap();

        let hash = sha256_of_file(&path).unwrap();
        // SHA-256 of "hello world" is
        // b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        assert_eq!(
            hash,
            [
                0xb9, 0x4d, 0x27, 0xb9, 0x93, 0x4d, 0x3e, 0x08, 0xa5, 0x2e, 0x52, 0xd7, 0xda, 0x7d,
                0xab, 0xfa, 0xc4, 0x84, 0xef, 0xe3, 0x7a, 0x53, 0x80, 0xee, 0x90, 0x88, 0xf7, 0xac,
                0xe2, 0xef, 0xcd, 0xe9
            ]
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_backup_skips_duplicate() {
        let test_dir = fresh_config_test_dir("test_backup_dedup");

        let db_path = test_dir.join("music.db");
        std::fs::write(&db_path, b"unchanged content").unwrap();

        // First backup should create a file
        backup_existing_database(&db_path).unwrap();
        let count_after_first = count_backups(&test_dir);
        assert_eq!(count_after_first, 1);

        // Second backup with same content should NOT create another file
        // Sleep 1 second so timestamp would differ
        std::thread::sleep(std::time::Duration::from_secs(1));
        backup_existing_database(&db_path).unwrap();
        let count_after_second = count_backups(&test_dir);
        assert_eq!(count_after_second, 1, "duplicate backup should be skipped");

        // Modify the database — next backup SHOULD create a new file
        std::fs::write(&db_path, b"modified content").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        backup_existing_database(&db_path).unwrap();
        let count_after_third = count_backups(&test_dir);
        assert_eq!(
            count_after_third, 2,
            "changed database should produce a new backup"
        );

        // Cleanup
        std::fs::remove_dir_all(&test_dir).ok();
    }

    #[test]
    fn test_remove_directory_cleans_up_albums() {
        let dir = std::env::temp_dir().join("sotf_test_remove_dir_cleanup");
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.sqlite");

        // Clean up any previous run
        let _ = std::fs::remove_file(&db_path);

        let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

        // Create an album with one track in /music/dir1
        let albums = vec![crate::library::Album {
            title: "Test Album".to_string(),
            tracks: vec![crate::library::Track {
                path: PathBuf::from("/music/dir1/track1.flac"),
                title: Some("Track 1".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        }];

        db.save_albums(&albums).unwrap();

        // Verify album exists
        let loaded = db.load_library().unwrap();
        assert_eq!(loaded.len(), 1, "should have 1 album after save");
        assert_eq!(loaded[0].tracks.len(), 1);

        // Remove the directory
        let removed = db
            .remove_tracks_from_directory(Path::new("/music/dir1"))
            .unwrap();
        assert_eq!(removed, 1, "should have removed 1 track");

        // Verify database is empty
        let loaded = db.load_library().unwrap();
        assert_eq!(
            loaded.len(),
            0,
            "should have 0 albums after removing the only directory"
        );

        // Cleanup
        std::fs::remove_dir_all(&dir).ok();
    }

    fn count_backups(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().into_string().unwrap_or_default();
                name.starts_with("music-") && name.ends_with(".sqlite")
            })
            .count()
    }
}
