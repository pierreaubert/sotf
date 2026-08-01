//! Full-text search and WAL checkpoint operations.
use super::MusicDatabase;
use rusqlite::{Result as SqlResult, params};

impl MusicDatabase {
    /// Define all database migrations
    pub fn search_library(&self, query: &str) -> SqlResult<Vec<i64>> {
        // If query is empty, return empty
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Build FTS5 query with prefix matching on each term
        // For multi-word queries like "pink floyd", we use AND to require all terms
        // e.g. "pink floyd" -> "pink* AND floyd*"
        // This provides fuzzy matching while being more precise than OR
        let terms: Vec<String> = query
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|s| {
                // Escape double quotes and add wildcard for prefix matching (fuzzy search)
                let escaped = s.replace("\"", "\"\"");
                format!("{}*", escaped)
            })
            .collect();

        if terms.is_empty() {
            return Ok(Vec::new());
        }

        // Join with AND so all terms must match somewhere in artist/album/track fields
        // This makes "pink floyd" find albums where both "pink*" and "floyd*" appear
        // Since FTS5 searches across all indexed columns, this works great for fuzzy search
        let fts_query = terms.join(" AND ");

        // Use rank for relevance-based ordering
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT album_id FROM library_fts WHERE library_fts MATCH ?1 ORDER BY rank",
        )?;

        let album_ids = stmt
            .query_map(params![fts_query], |row| row.get::<_, i64>(0))?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(album_ids)
    }

    /// Rebuild FTS index from current database state
    /// This ensures FTS is in sync after bulk operations like scanning
    pub fn sync_fts_index(&self) -> SqlResult<()> {
        // Clear existing FTS data
        self.conn.execute("DELETE FROM library_fts", [])?;

        // Rebuild from tracks and albums tables
        // Include track_path so filenames are searchable (for files with no metadata tags)
        let has_channel_label = self
            .conn
            .prepare("SELECT channel_label FROM library_fts LIMIT 0")
            .is_ok();
        if has_channel_label {
            self.conn.execute(
                "INSERT INTO library_fts(
                    artist, album_title, track_title, track_path, channel_label, album_id
                )
                 SELECT
                    COALESCE(t.album_artist, t.artist, 'Unknown Artist'),
                    a.title,
                    t.title,
                    t.path,
                    CASE
                        WHEN t.channels IS NULL THEN ''
                        ELSE CAST(t.channels AS TEXT) || ' ' || CASE t.channels
                            WHEN 1 THEN '1.0 Mono'
                            WHEN 2 THEN '2.0 Stereo'
                            WHEN 4 THEN '4.0'
                            WHEN 5 THEN '5.0'
                            WHEN 6 THEN '5.1'
                            WHEN 8 THEN '7.1'
                            WHEN 10 THEN '10ch (5.1.4/7.1.2)'
                            WHEN 12 THEN '12ch (7.1.4)'
                            WHEN 14 THEN '14ch (9.1.4)'
                            WHEN 16 THEN '16ch (9.1.6)'
                            ELSE CAST(t.channels AS TEXT) || 'ch'
                        END
                    END,
                    t.album_id
                 FROM tracks t
                 JOIN albums a ON t.album_id = a.id",
                [],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO library_fts(artist, album_title, track_title, track_path, album_id)
                 SELECT
                    COALESCE(t.album_artist, t.artist, 'Unknown Artist'),
                    a.title,
                    t.title,
                    t.path,
                    t.album_id
                 FROM tracks t
                 JOIN albums a ON t.album_id = a.id",
                [],
            )?;
        }

        log::debug!("FTS index synchronized with database");
        Ok(())
    }

    /// Force a WAL checkpoint to prevent unbounded WAL growth.
    ///
    /// In WAL mode, SQLite auto-checkpoints at 1000 pages, but this only works
    /// when no other connection holds a read snapshot from before the WAL data.
    /// When the TUI keeps a persistent read connection open, the WAL can grow
    /// indefinitely during scanning. This method forces a TRUNCATE checkpoint
    /// which resets the WAL file to zero bytes.
    pub fn checkpoint_wal(&self) -> SqlResult<()> {
        // PRAGMA wal_checkpoint(TRUNCATE) returns (busy, log_pages, checkpointed_pages)
        // TRUNCATE mode checkpoints all frames and truncates the WAL file to zero bytes
        let mut stmt = self.conn.prepare("PRAGMA wal_checkpoint(TRUNCATE)")?;
        let (busy, log_pages, checkpointed): (i32, i32, i32) =
            stmt.query_row([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        if busy != 0 {
            log::warn!("WAL checkpoint was blocked (busy), WAL may still be large");
        } else {
            log::info!(
                "WAL checkpoint complete: {}/{} pages checkpointed",
                checkpointed,
                log_pages
            );
        }
        Ok(())
    }
}
