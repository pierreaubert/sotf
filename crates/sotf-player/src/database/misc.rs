use chrono::{Duration, Local, NaiveDateTime};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Normalize a genre value:
/// - Replace dots and underscores with spaces
/// - Title case each word (first letter uppercase, rest lowercase)
///   This is specifically for genres where formats like "world.music" or "trip_hop" are common
pub(super) fn normalize_genre_name(value: &str) -> String {
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
                if let Err(e) = prune_old_backups(&dir) {
                    log::warn!("Failed to prune old database backups: {}", e);
                }
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
pub(super) fn sha256_of_file(path: &Path) -> Result<[u8; 32], std::io::Error> {
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

pub(super) fn parse_backup_timestamp(path: &Path) -> Option<NaiveDateTime> {
    let file_name = path.file_name()?.to_str()?;
    if !file_name.starts_with("music-") || !file_name.ends_with(".sqlite") {
        return None;
    }

    let ts_part = &file_name["music-".len()..file_name.len() - ".sqlite".len()];
    NaiveDateTime::parse_from_str(ts_part, "%Y%m%d-%H%M%S").ok()
}

/// Apply retention policy to backup files in the given directory:
/// - Keep the newest backup (the previous database version).
/// - Keep the newest distinct-size backup at least 7 days old.
/// - Keep the newest distinct-size backup at least 30 days old.
/// - Fill any missing slots with newer distinct-size backups, then keep at
///   least 3 files when fewer than 3 distinct sizes are available.
///
/// This keeps the backup directory bounded at three files in normal use while
/// retaining useful short-, medium-, and long-term recovery points. File size
/// is deliberately used for deduplication because the database can contain
/// equivalent snapshots with different metadata or SQLite page layout.
///
/// # Security
/// Only deletes backup files within the config directory.
pub(super) fn prune_old_backups(dir: &Path) -> std::io::Result<()> {
    // Security validation: ensure we're operating within config directory
    if crate::security::validate_write_path(dir).is_err() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Cannot prune backups outside config directory",
        ));
    }

    let mut backups: Vec<(PathBuf, NaiveDateTime, u64)> = Vec::new();

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
            let size = match entry.metadata() {
                Ok(metadata) => metadata.len(),
                Err(_) => continue,
            };
            backups.push((path, dt, size));
        }
    }

    // Newest first so that recent backups are preferred. The newest timestamp
    // is also a stable reference for tests and for pruning after a clock
    // adjustment.
    backups.sort_by_key(|(_, timestamp, _)| std::cmp::Reverse(*timestamp));

    let Some((_, newest_timestamp, newest_size)) = backups.first() else {
        return Ok(());
    };

    let mut keep_indices = HashSet::new();
    let mut keep_sizes = HashSet::new();
    keep_indices.insert(0);
    keep_sizes.insert(*newest_size);

    let weekly_cutoff = *newest_timestamp - Duration::days(7);
    if let Some(index) = backups
        .iter()
        .position(|(_, timestamp, size)| *timestamp <= weekly_cutoff && !keep_sizes.contains(size))
    {
        keep_indices.insert(index);
        keep_sizes.insert(backups[index].2);
    }

    let monthly_cutoff = *newest_timestamp - Duration::days(30);
    if let Some(index) = backups
        .iter()
        .position(|(_, timestamp, size)| *timestamp <= monthly_cutoff && !keep_sizes.contains(size))
    {
        keep_indices.insert(index);
        keep_sizes.insert(backups[index].2);
    }

    // If the requested age buckets do not exist yet, retain recent distinct
    // versions until the normal three-file floor is reached.
    for (index, (_, _, size)) in backups.iter().enumerate() {
        if keep_indices.len() >= 3 {
            break;
        }
        if keep_sizes.insert(*size) {
            keep_indices.insert(index);
        }
    }

    // A database can legitimately produce several snapshots with the same
    // byte length. Never prune below three files solely because of that.
    for index in 0..backups.len() {
        if keep_indices.len() >= 3 {
            break;
        }
        keep_indices.insert(index);
    }

    for (index, (path, _, _)) in backups.into_iter().enumerate() {
        if keep_indices.contains(&index) {
            continue;
        }
        // Security validation for each file before deletion
        if crate::security::validate_write_path(&path).is_ok() {
            let _ = std::fs::remove_file(path);
        }
    }

    Ok(())
}

/// Get current Unix timestamp
pub(super) fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Get file modification time as Unix timestamp
pub(super) fn get_file_mtime(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
}
