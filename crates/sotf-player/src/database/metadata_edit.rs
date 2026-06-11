use super::split::{split_and_normalize_genres, split_metadata_value};
use super::{MusicDatabase, current_timestamp};
use crate::metadata::MetadataPatch;
use rusqlite::{Result as SqlResult, Transaction, params};
use std::path::Path;

impl MusicDatabase {
    pub fn update_track_metadata(&mut self, path: &Path, patch: &MetadataPatch) -> SqlResult<()> {
        let tx = self.conn.transaction()?;
        let now = current_timestamp();
        let path_str = path.to_string_lossy().to_string();

        if let Some(album_title) = &patch.album_title {
            tx.execute(
                "UPDATE albums
                 SET title = ?1, updated_at = ?2
                 WHERE id = (SELECT album_id FROM tracks WHERE path = ?3)",
                params![album_title, now, &path_str],
            )?;
        }
        if let Some(year) = patch.year {
            tx.execute(
                "UPDATE albums
                 SET year = ?1, updated_at = ?2
                 WHERE id = (SELECT album_id FROM tracks WHERE path = ?3)",
                params![year as i64, now, &path_str],
            )?;
        }
        if let Some(edition) = &patch.edition {
            tx.execute(
                "UPDATE tracks SET edition = ?1, updated_at = ?2 WHERE path = ?3",
                params![edition, now, &path_str],
            )?;
        }

        tx.execute(
            "UPDATE tracks SET
                title = COALESCE(?1, title),
                artist = COALESCE(?2, artist),
                album_artist = COALESCE(?3, album_artist),
                genre = COALESCE(?4, genre),
                composer = COALESCE(?5, composer),
                disc_number = COALESCE(?6, disc_number),
                track_number = COALESCE(?7, track_number),
                conductor = COALESCE(?8, conductor),
                performer = COALESCE(?9, performer),
                isrc = COALESCE(?10, isrc),
                ensemble = COALESCE(?11, ensemble),
                edition = COALESCE(?12, edition),
                updated_at = ?13
             WHERE path = ?14",
            params![
                patch.title,
                patch.artist,
                patch.album_artist,
                patch.genre,
                patch.composer,
                patch.disc_number.map(|n| n as i64),
                patch.track_number.map(|n| n as i64),
                patch.conductor,
                patch.performer,
                patch.isrc,
                patch.ensemble,
                patch.edition,
                now,
                &path_str,
            ],
        )?;

        let track_id: i64 = tx.query_row(
            "SELECT id FROM tracks WHERE path = ?1",
            params![&path_str],
            |row| row.get(0),
        )?;
        refresh_track_facets(&tx, track_id)?;
        tx.commit()
    }

    pub fn update_album_metadata(&mut self, album_id: i64, patch: &MetadataPatch) -> SqlResult<()> {
        let tx = self.conn.transaction()?;
        let now = current_timestamp();

        tx.execute(
            "UPDATE albums SET
                title = COALESCE(?1, title),
                year = COALESCE(?2, year),
                edition = COALESCE(?3, edition),
                updated_at = ?4
             WHERE id = ?5",
            params![
                patch.album_title,
                patch.year.map(|n| n as i64),
                patch.edition,
                now,
                album_id,
            ],
        )?;

        tx.execute(
            "UPDATE tracks SET
                album_artist = COALESCE(?1, album_artist),
                genre = COALESCE(?2, genre),
                composer = COALESCE(?3, composer),
                conductor = COALESCE(?4, conductor),
                performer = COALESCE(?5, performer),
                ensemble = COALESCE(?6, ensemble),
                edition = COALESCE(?7, edition),
                updated_at = ?8
             WHERE album_id = ?9",
            params![
                patch.album_artist,
                patch.genre,
                patch.composer,
                patch.conductor,
                patch.performer,
                patch.ensemble,
                patch.edition,
                now,
                album_id,
            ],
        )?;

        let track_ids = {
            let mut stmt = tx.prepare("SELECT id FROM tracks WHERE album_id = ?1")?;
            stmt.query_map(params![album_id], |row| row.get::<_, i64>(0))?
                .collect::<SqlResult<Vec<_>>>()?
        };
        for track_id in track_ids {
            refresh_track_facets(&tx, track_id)?;
        }
        tx.commit()
    }
}

fn refresh_track_facets(tx: &Transaction<'_>, track_id: i64) -> SqlResult<()> {
    let (genre, composer, conductor, performer, ensemble): (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = tx.query_row(
        "SELECT genre, composer, conductor, performer, ensemble FROM tracks WHERE id = ?1",
        params![track_id],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        },
    )?;

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

    if let Some(genre) = genre {
        for value in split_and_normalize_genres(&genre) {
            tx.execute(
                "INSERT OR IGNORE INTO genres (name) VALUES (?1)",
                params![&value],
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO track_genres (track_id, genre_id)
                 SELECT ?1, id FROM genres WHERE name = ?2",
                params![track_id, &value],
            )?;
        }
    }

    if let Some(composer) = composer {
        for value in split_metadata_value(&composer) {
            tx.execute(
                "INSERT OR IGNORE INTO composers (name) VALUES (?1)",
                params![&value],
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO track_composers (track_id, composer_id)
                 SELECT ?1, id FROM composers WHERE name = ?2",
                params![track_id, &value],
            )?;
        }
    }

    if let Some(conductor) = conductor
        && !conductor.is_empty()
    {
        tx.execute(
            "INSERT OR IGNORE INTO conductors (name) VALUES (?1)",
            params![&conductor],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO track_conductors (track_id, conductor_id)
             SELECT ?1, id FROM conductors WHERE name = ?2",
            params![track_id, &conductor],
        )?;
    }

    if let Some(performer) = performer {
        for value in split_metadata_value(&performer) {
            tx.execute(
                "INSERT OR IGNORE INTO performers (name) VALUES (?1)",
                params![&value],
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO track_performers (track_id, performer_id)
                 SELECT ?1, id FROM performers WHERE name = ?2",
                params![track_id, &value],
            )?;
        }
    }

    if let Some(ensemble) = ensemble
        && !ensemble.is_empty()
    {
        tx.execute(
            "INSERT OR IGNORE INTO ensembles (name) VALUES (?1)",
            params![&ensemble],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO track_ensembles (track_id, ensemble_id)
             SELECT ?1, id FROM ensembles WHERE name = ?2",
            params![track_id, &ensemble],
        )?;
    }

    Ok(())
}
