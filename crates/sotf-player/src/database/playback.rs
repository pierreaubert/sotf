//! Play statistics, play counts, and favorites.
use super::{MusicDatabase, current_timestamp};
use rusqlite::{Result as SqlResult, params};
use std::path::{Path, PathBuf};

impl MusicDatabase {
    /// Load playback recency in one query for library presentation caches.
    ///
    /// # Errors
    /// Returns an error if SQLite cannot execute the operation or convert stored values.
    pub fn get_all_track_last_played(&self) -> SqlResult<std::collections::HashMap<PathBuf, u64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT track_path, MAX(played_at) FROM play_history GROUP BY track_path")?;
        stmt.query_map([], |row| {
            let timestamp: i64 = row.get(1)?;
            Ok((
                PathBuf::from(row.get::<_, String>(0)?),
                u64::try_from(timestamp).unwrap_or_default(),
            ))
        })?
        .collect()
    }

    /// Get top tracks by play count
    ///
    /// # Errors
    /// Returns an error if SQLite cannot execute the operation or convert stored values.
    pub fn get_top_tracks_by_play_count(&self, limit: usize) -> SqlResult<Vec<PathBuf>> {
        let mut stmt = self.conn.prepare(
            "SELECT track_path, COUNT(*) as count
             FROM play_history
             GROUP BY track_path
             ORDER BY count DESC
             LIMIT ?1",
        )?;

        let paths = stmt
            .query_map(params![crate::database::misc::sql_integer(limit)?], |row| {
                let path_str: String = row.get(0)?;
                Ok(PathBuf::from(path_str))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(paths)
    }

    /// Record a play event for a track
    /// Only records if `duration_played_secs` >= 30
    ///
    /// # Errors
    /// Returns an error if SQLite cannot execute the operation or convert stored values.
    pub fn record_play(&self, track_path: &Path, duration_played_secs: u64) -> SqlResult<()> {
        self.record_play_timestamp(track_path, duration_played_secs)
            .map(|_| ())
    }

    /// Return the exact persisted timestamp so caches need no second query.
    ///
    /// # Errors
    /// Returns an error if SQLite cannot execute the operation or convert stored values.
    pub fn record_play_timestamp(
        &self,
        track_path: &Path,
        duration_played_secs: u64,
    ) -> SqlResult<Option<u64>> {
        // Only record if played for at least 30 seconds
        if duration_played_secs < 30 {
            return Ok(None);
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
                crate::database::misc::sql_integer(duration_played_secs)?
            ],
        )?;

        Ok(Some(u64::try_from(now).unwrap_or_default()))
    }

    /// Get play count for a specific track
    ///
    /// # Errors
    /// Returns an error if SQLite cannot execute the operation or convert stored values.
    pub fn get_track_play_count(&self, track_path: &Path) -> SqlResult<usize> {
        let path_str = track_path.to_string_lossy();
        let count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM play_history WHERE track_path = ?1",
            params![path_str.as_ref()],
            |row| crate::database::misc::row_unsigned::<usize>(row, 0),
        )?;

        Ok(count)
    }

    /// Get play count for all tracks in an album
    ///
    /// # Errors
    /// Returns an error if SQLite cannot execute the operation or convert stored values.
    pub fn get_album_play_count(&self, album_id: i64) -> SqlResult<usize> {
        let count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM play_history WHERE album_id = ?1",
            params![album_id],
            |row| crate::database::misc::row_unsigned::<usize>(row, 0),
        )?;

        Ok(count)
    }

    /// Get play counts for all albums
    /// Returns a `HashMap` of `album_id` -> `play_count`
    ///
    /// # Errors
    /// Returns an error if SQLite cannot execute the operation or convert stored values.
    pub fn get_all_album_play_counts(&self) -> SqlResult<std::collections::HashMap<i64, usize>> {
        let mut stmt = self.conn.prepare(
            "SELECT album_id, COUNT(*) as play_count
             FROM play_history
             WHERE album_id IS NOT NULL
             GROUP BY album_id",
        )?;

        let counts = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    crate::database::misc::row_unsigned::<usize>(row, 1)?,
                ))
            })?
            .collect::<SqlResult<std::collections::HashMap<_, _>>>()?;

        Ok(counts)
    }

    /// Get last played timestamp for a track
    ///
    /// # Errors
    /// Returns an error if SQLite cannot execute the operation or convert stored values.
    pub fn get_track_last_played(&self, track_path: &Path) -> SqlResult<Option<u64>> {
        let path_str = track_path.to_string_lossy();
        let result = self.conn.query_row(
            "SELECT MAX(played_at) FROM play_history WHERE track_path = ?1",
            params![path_str.as_ref()],
            |row| row.get::<_, Option<i64>>(0),
        )?;

        result.map(u64::try_from).transpose().map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Integer,
                Box::new(error),
            )
        })
    }

    /// Get last played timestamp for an album
    ///
    /// # Errors
    /// Returns an error if SQLite cannot execute the operation or convert stored values.
    pub fn get_album_last_played(&self, album_id: i64) -> SqlResult<Option<u64>> {
        let result = self.conn.query_row(
            "SELECT MAX(played_at) FROM play_history WHERE album_id = ?1",
            params![album_id],
            |row| row.get::<_, Option<i64>>(0),
        )?;

        result.map(u64::try_from).transpose().map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Integer,
                Box::new(error),
            )
        })
    }

    // ==================== Favorites Methods ====================

    /// Toggle favorite status for a track, returns the new favorite state
    ///
    /// # Errors
    /// Returns an error if SQLite cannot execute the operation or convert stored values.
    pub fn toggle_track_favorite(&self, track_path: &Path) -> SqlResult<bool> {
        let path_str = track_path.to_string_lossy();
        self.conn.execute(
            "UPDATE tracks SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END WHERE path = ?1",
            params![path_str.as_ref()],
        )?;
        self.is_track_favorite(track_path)
    }

    /// Toggle favorite status for an album, returns the new favorite state
    ///
    /// # Errors
    /// Returns an error if SQLite cannot execute the operation or convert stored values.
    pub fn toggle_album_favorite(&self, album_id: i64) -> SqlResult<bool> {
        self.conn.execute(
            "UPDATE albums SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END WHERE id = ?1",
            params![album_id],
        )?;
        self.is_album_favorite(album_id)
    }

    /// Check if a track is favorited
    ///
    /// # Errors
    /// Returns an error if SQLite cannot execute the operation or convert stored values.
    pub fn is_track_favorite(&self, track_path: &Path) -> SqlResult<bool> {
        let path_str = track_path.to_string_lossy();
        let fav: i64 = self.conn.query_row(
            "SELECT COALESCE(is_favorite, 0) FROM tracks WHERE path = ?1",
            params![path_str.as_ref()],
            |row| row.get(0),
        )?;
        Ok(fav != 0)
    }

    /// Check if an album is favorited
    ///
    /// # Errors
    /// Returns an error if SQLite cannot execute the operation or convert stored values.
    pub fn is_album_favorite(&self, album_id: i64) -> SqlResult<bool> {
        let fav: i64 = self.conn.query_row(
            "SELECT COALESCE(is_favorite, 0) FROM albums WHERE id = ?1",
            params![album_id],
            |row| row.get(0),
        )?;
        Ok(fav != 0)
    }

    /// Get play counts for all tracks
    /// Returns a `HashMap` of `track_path` -> `play_count`
    ///
    /// # Errors
    /// Returns an error if SQLite cannot execute the operation or convert stored values.
    pub fn get_all_track_play_counts(&self) -> SqlResult<std::collections::HashMap<String, usize>> {
        let mut stmt = self.conn.prepare(
            "SELECT track_path, COUNT(*) as play_count
             FROM play_history
             GROUP BY track_path",
        )?;

        let counts = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    crate::database::misc::row_unsigned::<usize>(row, 1)?,
                ))
            })?
            .collect::<SqlResult<std::collections::HashMap<_, _>>>()?;

        Ok(counts)
    }
}
