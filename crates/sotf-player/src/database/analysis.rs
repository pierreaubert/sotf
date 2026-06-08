//! Replay gain, waveform, and bliss analysis database operations.
use rusqlite::{Result as SqlResult, params};
use std::path::{Path, PathBuf};

use super::MusicDatabase;
use super::current_timestamp;

#[derive(Debug, Clone)]
pub struct ReplayGainAlbumTrackData {
    pub path: PathBuf,
    pub peak: f64,
    pub gating_block_count: u64,
    pub energy: f64,
}

impl MusicDatabase {
    /// Update ReplayGain values for a track
    pub fn update_replay_gain(&self, path: &Path, gain: f64, peak: f64) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE tracks SET replay_gain = ?1, replay_peak = ?2 WHERE path = ?3",
            params![gain, peak, path.to_str().unwrap()],
        )?;
        Ok(())
    }

    /// Update ReplayGain values plus extended data needed for album gain.
    pub fn update_replay_gain_analysis(
        &self,
        path: &Path,
        gain: f64,
        peak: f64,
        gating_block_count: u64,
        energy: f64,
    ) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE tracks
             SET replay_gain = ?1,
                 replay_peak = ?2,
                 replay_gain_block_count = ?3,
                 replay_gain_energy = ?4
             WHERE path = ?5",
            params![
                gain,
                peak,
                gating_block_count as i64,
                energy,
                path.to_str().unwrap()
            ],
        )?;
        Ok(())
    }

    /// Clear all ReplayGain data so a full rescan can be performed.
    pub fn clear_all_replay_gain(&self) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE tracks SET replay_gain = NULL, replay_peak = NULL, replay_gain_block_count = NULL, replay_gain_energy = NULL, album_gain = NULL, album_peak = NULL, replay_gain_error = NULL",
            [],
        )?;
        Ok(())
    }

    /// Clear all waveform data so a full rescan can be performed.
    pub fn clear_all_waveform(&self) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE tracks SET waveform = NULL, waveform_error = NULL",
            [],
        )?;
        Ok(())
    }

    /// Clear all bliss analysis data so a full rescan can be performed.
    pub fn clear_all_bliss(&self) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE tracks SET bliss_tempo = NULL, bliss_zcr = NULL, bliss_loudness = NULL, bliss_features = NULL, bliss_analyzed_at = NULL, bliss_error = NULL",
            [],
        )?;
        Ok(())
    }

    /// Mark a track as having a ReplayGain scan error
    pub fn mark_replay_gain_error(&self, path: &Path, error: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE tracks SET replay_gain_error = ?1 WHERE path = ?2",
            params![error, path.to_str().unwrap()],
        )?;
        Ok(())
    }

    /// Mark a track as having a waveform scan error
    pub fn mark_waveform_error(&self, path: &Path, error: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE tracks SET waveform_error = ?1 WHERE path = ?2",
            params![error, path.to_str().unwrap()],
        )?;
        Ok(())
    }

    /// Mark a track as having a bliss scan error
    pub fn mark_bliss_error(&self, path: &Path, error: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE tracks SET bliss_error = ?1 WHERE path = ?2",
            params![error, path.to_str().unwrap()],
        )?;
        Ok(())
    }

    /// Get tracks that don't have ReplayGain values yet
    pub fn get_tracks_without_replay_gain(&self) -> SqlResult<Vec<PathBuf>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path FROM tracks WHERE (replay_gain IS NULL OR replay_peak IS NULL) AND replay_gain_error IS NULL")?;

        let paths = stmt
            .query_map([], |row| {
                let path_str: String = row.get(0)?;
                Ok(PathBuf::from(path_str))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(paths)
    }

    /// Update album-level ReplayGain for a track
    pub fn update_album_gain(
        &self,
        path: &Path,
        album_gain: f64,
        album_peak: f64,
    ) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE tracks SET album_gain = ?1, album_peak = ?2 WHERE path = ?3",
            params![album_gain, album_peak, path.to_str().unwrap()],
        )?;
        Ok(())
    }

    /// Get albums whose tracks are missing album-level ReplayGain.
    /// Returns `(album_id, Vec<track_path>)` for each album that has at least one track
    /// without album_gain.
    pub fn get_albums_without_album_gain(&self) -> SqlResult<Vec<(i64, Vec<PathBuf>)>> {
        // Find album_ids that have any track missing album_gain
        let mut album_stmt = self
            .conn
            .prepare("SELECT DISTINCT album_id FROM tracks WHERE album_gain IS NULL")?;
        let album_ids: Vec<i64> = album_stmt
            .query_map([], |row| row.get(0))?
            .collect::<SqlResult<Vec<_>>>()?;

        // For each album, get all track paths (we need all tracks to compute album gain)
        let mut track_stmt = self.conn.prepare(
            "SELECT path FROM tracks WHERE album_id = ?1 ORDER BY disc_number, track_number",
        )?;

        let mut result = Vec::new();
        for album_id in album_ids {
            let paths: Vec<PathBuf> = track_stmt
                .query_map(params![album_id], |row| {
                    let path_str: String = row.get(0)?;
                    Ok(PathBuf::from(path_str))
                })?
                .collect::<SqlResult<Vec<_>>>()?;
            if !paths.is_empty() {
                result.push((album_id, paths));
            }
        }

        Ok(result)
    }

    /// Return cached album ReplayGain inputs for all paths, or `None` if any are missing.
    pub fn get_replay_gain_album_track_data(
        &self,
        paths: &[PathBuf],
    ) -> SqlResult<Option<Vec<ReplayGainAlbumTrackData>>> {
        let mut stmt = self.conn.prepare(
            "SELECT replay_peak, replay_gain_block_count, replay_gain_energy
             FROM tracks
             WHERE path = ?1",
        )?;

        let mut tracks = Vec::with_capacity(paths.len());
        for path in paths {
            let data = stmt.query_row(params![path.to_str().unwrap()], |row| {
                let peak: Option<f64> = row.get(0)?;
                let block_count: Option<i64> = row.get(1)?;
                let energy: Option<f64> = row.get(2)?;
                Ok((peak, block_count, energy))
            })?;

            let (Some(peak), Some(block_count), Some(energy)) = data else {
                return Ok(None);
            };
            if block_count <= 0 {
                return Ok(None);
            }

            tracks.push(ReplayGainAlbumTrackData {
                path: path.clone(),
                peak,
                gating_block_count: block_count as u64,
                energy,
            });
        }

        Ok(Some(tracks))
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
            .prepare("SELECT path FROM tracks WHERE waveform IS NULL AND waveform_error IS NULL")?;

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

    /// Get total track count in the database
    pub fn get_track_count(&self) -> SqlResult<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM tracks", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Count tracks that have already been analyzed for bliss (succeeded, failed)
    pub fn get_bliss_done_counts(&self) -> SqlResult<(usize, usize)> {
        let succeeded: i64 = self.conn.query_row(
            "SELECT count(*) FROM tracks WHERE bliss_analyzed_at IS NOT NULL",
            [],
            |row| row.get(0),
        )?;
        let failed: i64 = self.conn.query_row(
            "SELECT count(*) FROM tracks WHERE bliss_error IS NOT NULL",
            [],
            |row| row.get(0),
        )?;
        Ok((succeeded as usize, failed as usize))
    }

    /// Count tracks that have already been analyzed for waveform (succeeded, failed)
    pub fn get_waveform_done_counts(&self) -> SqlResult<(usize, usize)> {
        let succeeded: i64 = self.conn.query_row(
            "SELECT count(*) FROM tracks WHERE waveform IS NOT NULL",
            [],
            |row| row.get(0),
        )?;
        let failed: i64 = self.conn.query_row(
            "SELECT count(*) FROM tracks WHERE waveform_error IS NOT NULL",
            [],
            |row| row.get(0),
        )?;
        Ok((succeeded as usize, failed as usize))
    }

    /// Count tracks that have already been analyzed for ReplayGain (succeeded, failed)
    pub fn get_replay_gain_done_counts(&self) -> SqlResult<(usize, usize)> {
        let succeeded: i64 = self.conn.query_row(
            "SELECT count(*) FROM tracks WHERE replay_gain IS NOT NULL AND replay_peak IS NOT NULL",
            [],
            |row| row.get(0),
        )?;
        let failed: i64 = self.conn.query_row(
            "SELECT count(*) FROM tracks WHERE replay_gain_error IS NOT NULL",
            [],
            |row| row.get(0),
        )?;
        Ok((succeeded as usize, failed as usize))
    }

    /// Get tracks that don't have bliss analysis yet
    pub fn get_tracks_without_bliss(&self) -> SqlResult<Vec<PathBuf>> {
        let mut stmt = self.conn.prepare(
            "SELECT path FROM tracks WHERE bliss_analyzed_at IS NULL AND bliss_error IS NULL",
        )?;

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

    /// Get all tracks that have bliss analysis data
    /// Returns a vector of (path, analysis, duration_secs) tuples
    pub fn get_all_bliss_features(
        &self,
    ) -> SqlResult<Vec<(PathBuf, crate::bliss::BlissAnalysis, u64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT path, bliss_features, duration_secs FROM tracks WHERE bliss_features IS NOT NULL",
        )?;

        let rows = stmt
            .query_map([], |row| {
                let path = PathBuf::from(row.get::<_, String>(0)?);
                let features_blob: Vec<u8> = row.get(1)?;
                let duration: u64 = row.get::<_, Option<i64>>(2)?.unwrap_or(0) as u64;
                Ok((path, features_blob, duration))
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        let mut results = Vec::with_capacity(rows.len());
        for (path, blob, duration) in rows {
            if let Some(analysis) = crate::bliss::BlissAnalysis::from_bytes(&blob) {
                results.push((path, analysis, duration));
            }
        }

        Ok(results)
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
}
