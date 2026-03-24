use crate::federation_config::{FederationSourceEntry, SourceConnectionConfig};
use rusqlite::params;

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
                "SELECT source_id, display_name, priority, is_enabled, config_json, source_type
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
                Ok((source_id, display_name, priority, is_enabled, config_json, source_type))
            })
            .map_err(|e| format!("query load_federation_sources: {e}"))?;

        let mut sources = Vec::new();
        for row in rows {
            let (source_id, display_name, priority, is_enabled, config_json, source_type) =
                row.map_err(|e| format!("row: {e}"))?;

            let connection = if let Some(json) = &config_json {
                serde_json::from_str(json).unwrap_or_else(|_| {
                    SourceConnectionConfig::default_for_type(&source_type)
                })
            } else {
                SourceConnectionConfig::default_for_type(&source_type)
            };

            sources.push(FederationSourceEntry {
                source_id,
                display_name,
                priority,
                is_enabled,
                connection,
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

        let source_type = match &source.connection {
            SourceConnectionConfig::Subsonic { .. } => "subsonic",
            SourceConnectionConfig::Mpd { .. } => "mpd",
            SourceConnectionConfig::Dlna { .. } => "dlna",
            SourceConnectionConfig::Peer { .. } => "peer",
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);

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
}
