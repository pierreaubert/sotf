//! Library loading, saving, scan tracking, and file cleanup.

use rusqlite::{Result as SqlResult, TransactionBehavior, params};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{
    MusicDatabase, current_timestamp, get_file_mtime, split_and_normalize_genres,
    split_metadata_value,
};
use crate::library::{Album, Track, normalize_waveform_samples};

impl MusicDatabase {
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
        let t0 = std::time::Instant::now();

        // Note: We still select artist from albums for backwards compatibility with old databases,
        // but we don't use it - artist is now derived from tracks
        let mut albums_stmt = self.conn.prepare(
            "SELECT id, title, year, album_art_path, album_art_thumbnail,
                    COALESCE(is_favorite, 0), uuid
             FROM albums ORDER BY title",
        )?;

        let mut albums = Vec::new();
        let album_rows = albums_stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,             // id
                row.get::<_, String>(1)?,          // title
                row.get::<_, Option<i64>>(2)?,     // year
                row.get::<_, Option<String>>(3)?,  // album_art_path
                row.get::<_, Option<Vec<u8>>>(4)?, // album_art_thumbnail
                row.get::<_, i64>(5)?,             // is_favorite
                row.get::<_, Option<String>>(6)?,  // uuid
            ))
        })?;

        // Collect album rows first so we can release the albums_stmt borrow,
        // then reuse a single prepared statement for all track queries.
        let album_data: Vec<_> = album_rows.collect::<SqlResult<Vec<_>>>()?;

        let t1 = std::time::Instant::now();

        // Get all play counts at once for efficiency
        let play_counts = self.get_all_album_play_counts()?;
        let track_play_counts = self.get_all_track_play_counts()?;

        let t2 = std::time::Instant::now();

        // Prepare the tracks statement ONCE outside the loop (was N+1 before —
        // preparing per album is expensive for large libraries)
        let mut tracks_stmt = self.conn.prepare(
            "SELECT t.path, t.title, t.artist, t.track_number, t.duration_secs, t.channels,
                    t.sample_rate, t.bit_depth,
                    t.replay_gain, t.replay_peak, t.album_gain, t.album_peak, t.waveform,
                    t.genre, t.composer, t.disc_number, t.conductor, t.performer,
                    t.isrc, t.album_artist, t.ensemble,
                    COALESCE(t.is_favorite, 0), t.uuid,
                    (SELECT ts.audio_source_json FROM track_sources ts
                     WHERE ts.track_id = t.id AND ts.audio_source_json IS NOT NULL
                     LIMIT 1)
             FROM tracks t
             WHERE t.album_id = ?1
             ORDER BY t.disc_number, t.track_number",
        )?;

        albums.reserve(album_data.len());

        for (
            album_id,
            title,
            year,
            album_art_path,
            album_art_thumbnail,
            album_is_favorite,
            album_uuid,
        ) in album_data
        {
            let tracks = tracks_stmt
                .query_map(params![album_id], |row| {
                    let path_str = row.get::<_, String>(0)?;
                    let is_fav = row.get::<_, i64>(21)? != 0;
                    let play_count = track_play_counts.get(&path_str).copied().unwrap_or(0);
                    let waveform = row
                        .get::<_, Option<Vec<u8>>>(12)?
                        .and_then(|bytes| normalize_waveform_samples(&bytes));
                    Ok(Track {
                        path: PathBuf::from(path_str),
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
                        waveform,
                        genre: row.get::<_, Option<String>>(13)?,
                        composer: row.get::<_, Option<String>>(14)?,
                        disc_number: row.get::<_, Option<i64>>(15)?.map(|n| n as u32),
                        conductor: row.get::<_, Option<String>>(16)?,
                        performer: row.get::<_, Option<String>>(17)?,
                        isrc: row.get::<_, Option<String>>(18)?,
                        album_artist: row.get::<_, Option<String>>(19)?,
                        ensemble: row.get::<_, Option<String>>(20)?,
                        edition: None,
                        is_favorite: is_fav,
                        play_count,
                        source: row
                            .get::<_, Option<String>>(23)?
                            .and_then(|json| serde_json::from_str(&json).ok()),
                        uuid: row.get::<_, Option<String>>(22)?,
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
                is_favorite: album_is_favorite != 0,
                uuid: album_uuid,
            });
        }

        log::info!(
            "[startup] load_library: albums_query={:.1}ms play_counts={:.1}ms tracks_loop={:.1}ms total={:.1}ms ({} albums)",
            t1.duration_since(t0).as_secs_f64() * 1000.0,
            t2.duration_since(t1).as_secs_f64() * 1000.0,
            t2.elapsed().as_secs_f64() * 1000.0,
            t0.elapsed().as_secs_f64() * 1000.0,
            albums.len(),
        );

        Ok(albums)
    }

    /// Save albums and tracks to database
    pub fn save_albums(&mut self, albums: &[Album]) -> SqlResult<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = current_timestamp();
        let mut album_key_counts: HashMap<(String, String), usize> = HashMap::new();

        for album in albums {
            *album_key_counts
                .entry((album.artist(), album.title.clone()))
                .or_insert(0) += 1;
        }

        for album in albums {
            // Compute artist from tracks for backwards compatibility with old schema
            // (old schema has artist column with UNIQUE(artist, title) constraint)
            let mut album_artist = album.artist();
            if album_key_counts
                .get(&(album_artist.clone(), album.title.clone()))
                .is_some_and(|count| *count > 1)
            {
                if let Some(parent) = album.tracks.iter().find_map(|track| track.path.parent()) {
                    album_artist = format!("{} [{}]", album_artist, parent.display());
                }
            }

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
                let waveform = track.waveform.as_deref().map(|samples| &samples[..]);

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
                        waveform,
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

        // Clean up orphaned albums (albums with no tracks)
        tx.execute(
            "DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks)",
            [],
        )?;

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

    /// Get aggregate scanner statistics across all tracks.
    /// Returns (total, rg_done, rg_errors, wf_done, wf_errors, bliss_done, bliss_errors).
    pub fn get_scanner_stats(
        &self,
    ) -> SqlResult<(usize, usize, usize, usize, usize, usize, usize)> {
        self.conn.query_row(
            "SELECT COUNT(*) as total,
                    COUNT(replay_gain) as rg_done,
                    COUNT(replay_gain_error) as rg_err,
                    COUNT(waveform) as wf_done,
                    COUNT(waveform_error) as wf_err,
                    COUNT(bliss_analyzed_at) as bliss_done,
                    COUNT(bliss_error) as bliss_err
             FROM tracks",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)? as usize,
                    row.get::<_, i64>(1)? as usize,
                    row.get::<_, i64>(2)? as usize,
                    row.get::<_, i64>(3)? as usize,
                    row.get::<_, i64>(4)? as usize,
                    row.get::<_, i64>(5)? as usize,
                    row.get::<_, i64>(6)? as usize,
                ))
            },
        )
    }

    /// Remove tracks that no longer exist on disk
    pub fn clean_missing_files(&mut self) -> SqlResult<usize> {
        self.clean_missing_files_with_progress(|_, _| {})
    }

    /// Remove tracks whose file extension is no longer supported by the scanner.
    pub fn clean_unsupported_extensions(
        &mut self,
        supported_extensions: &[&str],
    ) -> SqlResult<usize> {
        let tracks: Vec<(i64, String)> = {
            let mut stmt = self.conn.prepare("SELECT id, path FROM tracks")?;
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<SqlResult<Vec<_>>>()?
        };

        let mut to_delete = Vec::new();
        for (id, path_str) in tracks {
            let supported = Path::new(&path_str)
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| {
                    supported_extensions
                        .iter()
                        .any(|supported| supported.eq_ignore_ascii_case(ext))
                });
            if !supported {
                to_delete.push(id);
            }
        }

        let count = to_delete.len();
        if count > 0 {
            let tx = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            for id in to_delete {
                tx.execute("DELETE FROM tracks WHERE id = ?1", params![id])?;
            }
            tx.execute(
                "DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks)",
                [],
            )?;
            tx.commit()?;
        }

        Ok(count)
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
            let tx = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
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

    /// Clear all local library content while preserving app configuration and
    /// saved connection/source records.
    pub fn clear_library_content(&mut self) -> SqlResult<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))?;

        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

        tx.execute("DELETE FROM playlist_tracks", [])?;
        tx.execute("DELETE FROM track_genres", [])?;
        tx.execute("DELETE FROM track_composers", [])?;
        tx.execute("DELETE FROM track_conductors", [])?;
        tx.execute("DELETE FROM track_performers", [])?;
        tx.execute("DELETE FROM track_ensembles", [])?;
        tx.execute("DELETE FROM track_sources", [])?;
        tx.execute("DELETE FROM album_sources", [])?;
        tx.execute("DELETE FROM tracks", [])?;
        tx.execute("DELETE FROM albums", [])?;
        tx.execute("DELETE FROM library_fts", [])?;
        tx.execute("DELETE FROM scan_history", [])?;

        tx.commit()?;
        Ok(count as usize)
    }

    /// Remove all tracks from a specific directory path (and its subdirectories)
    /// This is used when removing a directory from the library
    /// Returns the number of tracks removed
    pub fn remove_tracks_from_directory(&mut self, directory: &Path) -> SqlResult<usize> {
        let dir_str = directory.to_string_lossy();
        // SQLite LIKE pattern: path starts with directory path
        let pattern = format!("{}%", dir_str);

        // First, count how many tracks will be deleted
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tracks WHERE path LIKE ?1",
            params![pattern],
            |row| row.get(0),
        )?;

        if count > 0 {
            // Delete tracks in the directory
            self.conn
                .execute("DELETE FROM tracks WHERE path LIKE ?1", params![pattern])?;

            // Clean up albums with no tracks
            self.conn.execute(
                "DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks)",
                [],
            )?;

            log::info!(
                "Removed {} tracks from directory: {}",
                count,
                directory.display()
            );
        }

        Ok(count as usize)
    }

    /// Get all library sources (id, source_id, source_type, display_name, priority, is_enabled)
    #[allow(clippy::type_complexity)]
    pub fn get_library_sources(&self) -> SqlResult<Vec<(i64, String, String, String, i64, bool)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_id, source_type, display_name, priority, is_enabled FROM library_sources ORDER BY priority DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let is_enabled: i64 = row.get(5)?;
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                is_enabled != 0,
            ))
        })?;
        rows.collect()
    }

    /// Count track_sources entries for a given library source
    pub fn count_track_sources(&self, source_id: i64) -> SqlResult<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM track_sources WHERE source_id = ?1",
            params![source_id],
            |row| row.get(0),
        )
    }

    /// Count album_sources entries for a given library source
    pub fn count_album_sources(&self, source_id: i64) -> SqlResult<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM album_sources WHERE source_id = ?1",
            params![source_id],
            |row| row.get(0),
        )
    }

    /// Set the UUID for an album
    pub fn set_album_uuid(&self, album_id: i64, uuid: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE albums SET uuid = ?1 WHERE id = ?2",
            params![uuid, album_id],
        )?;
        Ok(())
    }

    /// Set the UUID for a track by its path
    pub fn set_track_uuid(&self, track_path: &str, uuid: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE tracks SET uuid = ?1 WHERE path = ?2",
            params![uuid, track_path],
        )?;
        Ok(())
    }
}
