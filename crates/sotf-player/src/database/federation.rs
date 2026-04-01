use crate::federation_config::{FederationSourceEntry, SourceConnectionConfig};
use rusqlite::params;
use serde_json;
use sotf_federation::{ProviderAlbum, ProviderTrack};

use super::MusicDatabase;

impl MusicDatabase {
    /// Load all federation sources from the database.
    ///
    /// # Errors
    /// Returns an error if the query fails.
    pub fn load_federation_sources(&self) -> Result<Vec<FederationSourceEntry>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT source_id, display_name, priority, is_enabled, config_json, source_type, is_available
                 FROM library_sources
                 ORDER BY priority DESC",
            )
            .map_err(|e| format!("prepare load_federation_sources: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                let source_id: String = row.get(0)?;
                let display_name: String = row.get(1)?;
                let priority: i32 = row.get(2)?;
                let is_enabled: bool = row.get(3)?;
                let config_json: Option<String> = row.get(4)?;
                let source_type: String = row.get(5)?;
                let is_available: Option<bool> = row.get(6)?;
                Ok((
                    source_id,
                    display_name,
                    priority,
                    is_enabled,
                    config_json,
                    source_type,
                    is_available,
                ))
            })
            .map_err(|e| format!("query load_federation_sources: {e}"))?;

        let mut sources = Vec::new();
        for row in rows {
            let (
                source_id,
                display_name,
                priority,
                is_enabled,
                config_json,
                source_type,
                is_available,
            ) = row.map_err(|e| format!("row: {e}"))?;

            let connection = if let Some(json) = &config_json {
                serde_json::from_str(json)
                    .unwrap_or_else(|_| SourceConnectionConfig::default_for_type(&source_type))
            } else {
                SourceConnectionConfig::default_for_type(&source_type)
            };

            sources.push(FederationSourceEntry {
                source_id,
                display_name,
                priority,
                is_enabled,
                connection,
                is_available,
            });
        }

        Ok(sources)
    }

    /// Insert or update a federation source.
    ///
    /// # Errors
    /// Returns an error if the upsert fails.
    pub fn save_federation_source(&self, source: &FederationSourceEntry) -> Result<(), String> {
        let config_json =
            serde_json::to_string(&source.connection).map_err(|e| format!("serialize: {e}"))?;

        let source_type = source.connection.source_type_key();

        let now = super::current_timestamp();

        self.conn
            .execute(
                "INSERT INTO library_sources (source_id, source_type, display_name, config_json, is_enabled, priority, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                 ON CONFLICT(source_id) DO UPDATE SET
                     display_name = excluded.display_name,
                     config_json = excluded.config_json,
                     is_enabled = excluded.is_enabled,
                     priority = excluded.priority,
                     updated_at = excluded.updated_at",
                params![
                    source.source_id,
                    source_type,
                    source.display_name,
                    config_json,
                    source.is_enabled,
                    source.priority,
                    now,
                ],
            )
            .map_err(|e| format!("save_federation_source: {e}"))?;

        Ok(())
    }

    /// Delete a federation source by `source_id`.
    ///
    /// # Errors
    /// Returns an error if the delete fails.
    pub fn delete_federation_source(&self, source_id: &str) -> Result<bool, String> {
        let affected = self
            .conn
            .execute(
                "DELETE FROM library_sources WHERE source_id = ?1",
                params![source_id],
            )
            .map_err(|e| format!("delete_federation_source: {e}"))?;

        Ok(affected > 0)
    }

    /// Toggle the enabled state of a federation source.
    ///
    /// # Errors
    /// Returns an error if the update fails.
    pub fn toggle_federation_source(&self, source_id: &str) -> Result<bool, String> {
        self.conn
            .execute(
                "UPDATE library_sources SET is_enabled = NOT is_enabled, updated_at = ?2 WHERE source_id = ?1",
                params![
                    source_id,
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs() as i64),
                ],
            )
            .map_err(|e| format!("toggle_federation_source: {e}"))?;

        // Return the new state
        let enabled: bool = self
            .conn
            .query_row(
                "SELECT is_enabled FROM library_sources WHERE source_id = ?1",
                params![source_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("query enabled state: {e}"))?;

        Ok(enabled)
    }

    /// Update the last sync timestamp for a federation source.
    pub fn update_federation_source_sync_time(&self, source_id: &str) -> Result<(), String> {
        let now = super::current_timestamp();

        self.conn
            .execute(
                "UPDATE library_sources SET last_sync_at = ?1 WHERE source_id = ?2",
                params![now, source_id],
            )
            .map_err(|e| format!("update_federation_source_sync_time: {e}"))?;

        Ok(())
    }

    /// Get the internal source ID (database primary key) from the source_id string.
    pub fn get_federation_source_db_id(&self, source_id: &str) -> Result<Option<i64>, String> {
        let id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM library_sources WHERE source_id = ?1",
                params![source_id],
                |row| row.get(0),
            )
            .ok();
        Ok(id)
    }

    /// Merge an album from a federation source into the database.
    ///
    /// This creates or updates the album and links it to the source via album_sources.
    /// Returns the album's internal database ID.
    pub fn merge_federation_album(
        &self,
        source_id: &str,
        album: &ProviderAlbum,
    ) -> Result<i64, String> {
        let db_source_id = self
            .get_federation_source_db_id(source_id)?
            .ok_or_else(|| format!("source not found: {}", source_id))?;

        let now = super::current_timestamp();

        // Compute artist from tracks (for compilations, artist varies per track)
        let artist = if album.tracks.is_empty() {
            album.artist.clone()
        } else {
            album
                .tracks
                .iter()
                .find_map(|t| t.album_artist.clone())
                .unwrap_or_else(|| album.artist.clone())
        };

        // Insert or update album
        self.conn
            .execute(
                "INSERT INTO albums (artist, title, year, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(artist, title) DO UPDATE SET
                 year = excluded.year,
                 updated_at = excluded.updated_at",
                params![
                    &artist,
                    &album.title,
                    album.year.map(|y| y as i64),
                    now,
                    now,
                ],
            )
            .map_err(|e| format!("merge_federation_album: {e}"))?;

        // Get album ID
        let album_id: i64 = self
            .conn
            .query_row(
                "SELECT id FROM albums WHERE artist = ?1 AND title = ?2",
                params![&artist, &album.title],
                |row| row.get(0),
            )
            .map_err(|e| format!("get album id: {e}"))?;

        // Insert album source junction
        self.conn
            .execute(
                "INSERT INTO album_sources (album_id, source_id, external_id)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(source_id, external_id) DO UPDATE SET
                 album_id = excluded.album_id",
                params![album_id, db_source_id, &album.external_id],
            )
            .map_err(|e| format!("merge_federation_album source: {e}"))?;

        Ok(album_id)
    }

    /// Merge a track from a federation source into the database.
    ///
    /// This creates or updates the track and links it to the source via track_sources.
    pub fn merge_federation_track(
        &self,
        source_id: &str,
        album_id: i64,
        track: &ProviderTrack,
    ) -> Result<i64, String> {
        let db_source_id = self
            .get_federation_source_db_id(source_id)?
            .ok_or_else(|| format!("source not found: {}", source_id))?;

        let now = super::current_timestamp();

        // Serialize AudioSource to JSON for storage
        let audio_source_json = serde_json::to_string(&track.audio_source)
            .map_err(|e| format!("serialize audio source: {e}"))?;

        // Use external_id as path for federation tracks (they don't have local paths)
        let path_str = format!("federation:{}:{}", source_id, track.external_id);

        // Insert or update track.
        // file_mtime=0 and scanned_at=now are required by NOT NULL constraints
        // but are meaningless for federation tracks.
        self.conn
            .execute(
                "INSERT INTO tracks (album_id, path, title, artist, album_artist,
                                track_number, disc_number, duration_secs,
                                channels, sample_rate, bit_depth,
                                genre, composer,
                                file_mtime, scanned_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0, ?14, ?14, ?14)
             ON CONFLICT(path) DO UPDATE SET
                 album_id = excluded.album_id,
                 title = excluded.title,
                 artist = excluded.artist,
                 album_artist = excluded.album_artist,
                 track_number = excluded.track_number,
                 disc_number = excluded.disc_number,
                 duration_secs = excluded.duration_secs,
                 channels = excluded.channels,
                 sample_rate = excluded.sample_rate,
                 bit_depth = excluded.bit_depth,
                 genre = excluded.genre,
                 composer = excluded.composer,
                 updated_at = excluded.updated_at",
                params![
                    album_id,
                    &path_str,
                    &track.title,
                    &track.artist,
                    &track.album_artist,
                    track.track_number.map(|n| n as i64),
                    track.disc_number.map(|n| n as i64),
                    track.duration_secs.map(|d| d as i64),
                    track.channels.map(|c| c as i64),
                    track.sample_rate.map(|r| r as i64),
                    track.bit_depth.map(|b| b as i64),
                    &track.genre,
                    &track.composer,
                    now,
                ],
            )
            .map_err(|e| format!("merge_federation_track: {e}"))?;

        // Get track ID
        let track_id: i64 = self
            .conn
            .query_row(
                "SELECT id FROM tracks WHERE path = ?1",
                params![&path_str],
                |row| row.get(0),
            )
            .map_err(|e| format!("get track id: {e}"))?;

        // Insert track source junction
        self.conn
            .execute(
                "INSERT INTO track_sources (track_id, source_id, external_id, audio_source_json)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(source_id, external_id) DO UPDATE SET
                 track_id = excluded.track_id,
                 audio_source_json = excluded.audio_source_json",
                params![
                    track_id,
                    db_source_id,
                    &track.external_id,
                    &audio_source_json
                ],
            )
            .map_err(|e| format!("merge_federation_track source: {e}"))?;

        Ok(track_id)
    }

    /// Update the availability state of a federation source.
    pub fn set_source_availability(&self, source_id: &str, available: bool) -> Result<(), String> {
        let now = super::current_timestamp();

        self.conn
            .execute(
                "UPDATE library_sources SET is_available = ?1, updated_at = ?2 WHERE source_id = ?3",
                params![available, now, source_id],
            )
            .map_err(|e| format!("set_source_availability: {e}"))?;

        Ok(())
    }

    /// Remove albums that have no tracks left after a source data clear.
    /// This prevents empty album shells from lingering in the library.
    pub fn remove_orphaned_albums(&self) -> Result<usize, String> {
        let removed = self
            .conn
            .execute(
                "DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks)",
                [],
            )
            .map_err(|e| format!("remove_orphaned_albums: {e}"))?;

        Ok(removed)
    }

    /// Remove tracks that have no track_sources entries (orphaned by a source clear).
    pub fn remove_orphaned_tracks(&self) -> Result<usize, String> {
        let removed = self
            .conn
            .execute(
                "DELETE FROM tracks WHERE path LIKE 'federation:%'
                 AND id NOT IN (SELECT DISTINCT track_id FROM track_sources)",
                [],
            )
            .map_err(|e| format!("remove_orphaned_tracks: {e}"))?;

        Ok(removed)
    }

    /// Remove federation tracks whose only source was the given source.
    /// Tracks that also belong to other sources are kept (only the junction row is removed).
    pub fn remove_exclusive_federation_tracks(&self, source_id: &str) -> Result<usize, String> {
        let db_source_id = match self.get_federation_source_db_id(source_id)? {
            Some(id) => id,
            None => return Ok(0),
        };

        // Delete tracks that ONLY belong to this source (not shared with others)
        let removed = self
            .conn
            .execute(
                "DELETE FROM tracks WHERE id IN (
                     SELECT ts.track_id FROM track_sources ts
                     WHERE ts.source_id = ?1
                     AND ts.track_id NOT IN (
                         SELECT track_id FROM track_sources WHERE source_id != ?1
                     )
                 )",
                params![db_source_id],
            )
            .map_err(|e| format!("remove_exclusive_federation_tracks: {e}"))?;

        Ok(removed)
    }

    /// Clear all data for a federation source (albums and tracks).
    /// This is called before a full resync to remove stale data.
    pub fn clear_federation_source_data(&self, source_id: &str) -> Result<(), String> {
        let db_source_id = match self.get_federation_source_db_id(source_id)? {
            Some(id) => id,
            None => return Ok(()), // Source doesn't exist, nothing to clear
        };

        // Delete junction rows. Orphaned tracks/albums are cleaned up separately
        // by remove_orphaned_tracks() and remove_orphaned_albums().
        self.conn
            .execute(
                "DELETE FROM track_sources WHERE source_id = ?1",
                params![db_source_id],
            )
            .map_err(|e| format!("clear track_sources: {e}"))?;

        // Delete album sources
        self.conn
            .execute(
                "DELETE FROM album_sources WHERE source_id = ?1",
                params![db_source_id],
            )
            .map_err(|e| format!("clear album_sources: {e}"))?;

        Ok(())
    }
}
