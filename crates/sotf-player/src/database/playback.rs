//! Play statistics, play counts, and favorites.
use super::{MusicDatabase, current_timestamp};
use rusqlite::{Result as SqlResult, params};
use std::path::{Path, PathBuf};

impl MusicDatabase {
    /// Get top tracks by play count
    pub fn get_top_tracks_by_play_count(&self, limit: usize) -> SqlResult<Vec<PathBuf>> {
        let mut stmt = self.conn.prepare(
            "SELECT track_path, COUNT(*) as count
             FROM play_history
             GROUP BY track_path
             ORDER BY count DESC
             LIMIT ?1",
        )?;

        let paths = stmt
            .query_map(params![limit as i64], |row| {
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

    // ==================== Favorites Methods ====================

    /// Toggle favorite status for a track, returns the new favorite state
    pub fn toggle_track_favorite(&self, track_path: &Path) -> SqlResult<bool> {
        let path_str = track_path.to_string_lossy();
        self.conn.execute(
            "UPDATE tracks SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END WHERE path = ?1",
            params![path_str.as_ref()],
        )?;
        self.is_track_favorite(track_path)
    }

    /// Toggle favorite status for an album, returns the new favorite state
    pub fn toggle_album_favorite(&self, album_id: i64) -> SqlResult<bool> {
        self.conn.execute(
            "UPDATE albums SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END WHERE id = ?1",
            params![album_id],
        )?;
        self.is_album_favorite(album_id)
    }

    /// Check if a track is favorited
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
    pub fn is_album_favorite(&self, album_id: i64) -> SqlResult<bool> {
        let fav: i64 = self.conn.query_row(
            "SELECT COALESCE(is_favorite, 0) FROM albums WHERE id = ?1",
            params![album_id],
            |row| row.get(0),
        )?;
        Ok(fav != 0)
    }

    /// Get play counts for all tracks
    /// Returns a HashMap of track_path -> play_count
    pub fn get_all_track_play_counts(&self) -> SqlResult<std::collections::HashMap<String, usize>> {
        let mut stmt = self.conn.prepare(
            "SELECT track_path, COUNT(*) as play_count
             FROM play_history
             GROUP BY track_path",
        )?;

        let counts = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
            })?
            .collect::<SqlResult<std::collections::HashMap<_, _>>>()?;

        Ok(counts)
    }
}
