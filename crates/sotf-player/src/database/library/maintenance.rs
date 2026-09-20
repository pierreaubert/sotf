//! Reviewable local-file maintenance. Run filesystem checks on a worker thread.

use super::super::MusicDatabase;
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use std::path::{Path, PathBuf};

/// A database record observed missing during a read-only review.
/// Identity is private so callers cannot manufacture removal requests.
#[derive(Clone, Debug)]
pub struct MissingFileEntry {
    id: i64,
    path: PathBuf,
    uuid: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{Album, Track};

    #[test]
    fn maintenance_review_revalidates_and_only_removes_reviewed_records() {
        let directory = tempfile::tempdir().unwrap();
        let mut db = MusicDatabase::open_for_testing(directory.path().join("music.db")).unwrap();
        let paths: Vec<_> = ["missing", "restored", "newly-missing", "changed"]
            .map(|name| directory.path().join(name))
            .into();
        std::fs::write(&paths[2], b"audio").unwrap();
        db.save_albums(&[Album {
            title: "Review fixture".into(),
            tracks: paths
                .iter()
                .map(|path| Track {
                    path: path.clone(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }])
        .unwrap();
        let review = db.review_missing_files().unwrap();
        assert_eq!(review.entries().len(), 3);
        assert_eq!(
            db.load_library().unwrap()[0].tracks.len(),
            4,
            "preview must not mutate"
        );
        std::fs::write(&paths[1], b"restored").unwrap();
        std::fs::remove_file(&paths[2]).unwrap();
        db.conn
            .execute(
                "UPDATE tracks SET uuid = 'replacement' WHERE path = ?1",
                [paths[3].to_string_lossy().as_ref()],
            )
            .unwrap();
        assert_eq!(db.remove_reviewed_missing_files(&review).unwrap(), 1);
        assert_eq!(db.remove_reviewed_missing_files(&review).unwrap(), 0);
        let remaining = db.load_library().unwrap();
        let remaining: Vec<_> = remaining[0]
            .tracks
            .iter()
            .map(|track| &track.path)
            .collect();
        assert_eq!(remaining.len(), 3);
        for path in &paths[1..] {
            assert!(remaining.contains(&path));
        }
    }

    #[test]
    fn maintenance_review_preserves_provider_records_and_unrelated_empty_albums() {
        let directory = tempfile::tempdir().unwrap();
        let mut db = MusicDatabase::open_for_testing(directory.path().join("music.db")).unwrap();
        db.save_albums(&[
            Album {
                title: "Provider".into(),
                tracks: vec![Track {
                    path: directory.path().join("remote"),
                    ..Default::default()
                }],
                ..Default::default()
            },
            Album {
                title: "Local".into(),
                tracks: vec![Track {
                    path: directory.path().join("local"),
                    ..Default::default()
                }],
                ..Default::default()
            },
        ])
        .unwrap();
        db.conn.execute("INSERT INTO library_sources (source_id, source_type, display_name, priority, created_at, updated_at) VALUES ('review-provider', 'subsonic', 'Review', 1, 0, 0)", []).unwrap();
        db.conn.execute("INSERT INTO track_sources (track_id, source_id, external_id) SELECT t.id, s.id, 'remote' FROM tracks t, library_sources s WHERE t.path = ?1 AND s.source_id = 'review-provider'", [directory.path().join("remote").to_string_lossy().as_ref()]).unwrap();
        db.conn.execute("INSERT INTO albums (artist, title, created_at, updated_at) VALUES ('', 'Unrelated empty', 0, 0)", []).unwrap();
        let review = db.review_missing_files().unwrap();
        assert_eq!(review.entries().len(), 1);
        assert_eq!(review.entries()[0].path(), directory.path().join("local"));
        assert_eq!(db.remove_reviewed_missing_files(&review).unwrap(), 1);
        let titles: Vec<String> = db
            .conn
            .prepare("SELECT title FROM albums ORDER BY title")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(titles, ["Provider", "Unrelated empty"]);
    }
}

impl MissingFileEntry {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Clone, Debug, Default)]
pub struct MissingFileReview {
    entries: Vec<MissingFileEntry>,
    unavailable_paths: Vec<PathBuf>,
}

impl MissingFileReview {
    pub fn entries(&self) -> &[MissingFileEntry] {
        &self.entries
    }

    /// Paths whose existence could not be established, never removal candidates.
    pub fn unavailable_paths(&self) -> &[PathBuf] {
        &self.unavailable_paths
    }
}

impl MusicDatabase {
    /// Inspect local records without changing the database or touching audio files.
    /// Provider-backed records and relative/virtual paths are not file candidates.
    pub fn review_missing_files(&self) -> rusqlite::Result<MissingFileReview> {
        let mut statement = self.conn.prepare(
            "SELECT id, path, uuid FROM tracks t WHERE NOT EXISTS (
                SELECT 1 FROM track_sources ts JOIN library_sources s ON s.id = ts.source_id
                WHERE ts.track_id = t.id AND s.source_id != 'local'
            ) ORDER BY path, id",
        )?;
        let records = statement.query_map([], |row| {
            Ok(MissingFileEntry {
                id: row.get(0)?,
                path: PathBuf::from(row.get::<_, String>(1)?),
                uuid: row.get(2)?,
            })
        })?;
        let mut review = MissingFileReview::default();
        for record in records {
            let record = record?;
            if !record.path.is_absolute() {
                continue;
            }
            match record.path.try_exists() {
                Ok(false) => review.entries.push(record),
                Ok(true) => {}
                Err(_) => review.unavailable_paths.push(record.path),
            }
        }
        Ok(review)
    }

    /// Remove only reviewed records that still have the same identity and are
    /// still missing. Newly missing, restored and provider-backed tracks survive.
    /// Call only after confirmation; the snapshot can safely be applied twice.
    pub fn remove_reviewed_missing_files(
        &mut self,
        review: &MissingFileReview,
    ) -> rusqlite::Result<usize> {
        let transaction = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut removed = 0;
        for entry in &review.entries {
            if !matches!(entry.path.try_exists(), Ok(false)) {
                continue;
            }
            let path = entry.path.to_string_lossy();
            let album_id: Option<i64> = transaction
                .query_row(
                    "SELECT album_id FROM tracks t WHERE id = ?1 AND path = ?2 AND uuid IS ?3
                 AND NOT EXISTS (SELECT 1 FROM track_sources ts
                     JOIN library_sources s ON s.id = ts.source_id
                     WHERE ts.track_id = t.id AND s.source_id != 'local')",
                    params![entry.id, path.as_ref(), entry.uuid],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(album_id) = album_id {
                removed += transaction.execute("DELETE FROM tracks WHERE id = ?1", [entry.id])?;
                transaction.execute(
                    "DELETE FROM albums WHERE id = ?1
                     AND NOT EXISTS (SELECT 1 FROM tracks WHERE album_id = ?1)
                     AND NOT EXISTS (SELECT 1 FROM album_sources a
                         JOIN library_sources s ON s.id = a.source_id
                         WHERE a.album_id = ?1 AND s.source_id != 'local')",
                    [album_id],
                )?;
            }
        }
        transaction.commit()?;
        Ok(removed)
    }
}
