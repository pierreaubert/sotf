//! Playlist CRUD operations.
use super::{MusicDatabase, current_timestamp};
use crate::library::{Playlist, PlaylistEntry};
use rusqlite::{Result as SqlResult, params};
use std::path::{Path, PathBuf};

impl MusicDatabase {
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
        let start_position: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
            |row| row.get(0),
        )?;

        for (offset, track_path) in track_paths.iter().enumerate() {
            self.conn.execute(
                "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_path, position, added_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    playlist_id,
                    track_path.to_str().unwrap(),
                    start_position + offset as i64,
                    now
                ],
            )?;
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
}
