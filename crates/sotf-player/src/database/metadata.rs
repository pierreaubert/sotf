//! Metadata queries: genres, composers, conductors, performers, ensembles, library sources.
use rusqlite::{Result as SqlResult, params};
use std::path::{Path, PathBuf};
use super::MusicDatabase;

impl MusicDatabase {
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
