//! Playlist controller — manages playlist state and operations.
//!
//! Delegates persistence to `MusicDatabase` and provides navigation,
//! CRUD operations, and track resolution for UI layers.

use std::path::{Path, PathBuf};

use crate::{Album, MusicDatabase, MusicLibrary, Playlist, Track};

#[derive(Debug, Clone)]
pub struct PlaylistController {
    /// All playlists (loaded from DB without track entries)
    playlists: Vec<Playlist>,
    /// Track counts per playlist (parallel to `playlists`)
    track_counts: Vec<usize>,
    /// Currently selected playlist index in the list
    pub selected_playlist_index: usize,
    /// Currently selected track index within the active playlist
    pub selected_track_index: usize,
    /// The currently "open" playlist (with tracks loaded)
    active_playlist: Option<Playlist>,
}

impl Default for PlaylistController {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaylistController {
    pub fn new() -> Self {
        Self {
            playlists: Vec::new(),
            track_counts: Vec::new(),
            selected_playlist_index: 0,
            selected_track_index: 0,
            active_playlist: None,
        }
    }

    // =========================================================================
    // Accessors
    // =========================================================================

    /// Get all playlists (without track entries).
    pub fn playlists(&self) -> &[Playlist] {
        &self.playlists
    }

    /// Get the currently open playlist (with tracks loaded).
    pub fn active_playlist(&self) -> Option<&Playlist> {
        self.active_playlist.as_ref()
    }

    /// Get the ID of the active playlist.
    pub fn active_playlist_id(&self) -> Option<i64> {
        self.active_playlist.as_ref().and_then(|p| p.id)
    }

    // =========================================================================
    // Loading
    // =========================================================================

    /// Load all playlists from the database (without track entries).
    pub fn load_playlists(&mut self, db: &MusicDatabase) -> Result<(), String> {
        self.playlists = db
            .get_playlists()
            .map_err(|e| format!("Failed to load playlists: {}", e))?;
        // Load track counts
        self.track_counts = self
            .playlists
            .iter()
            .map(|p| {
                p.id.and_then(|id| db.get_playlist_track_count(id).ok())
                    .unwrap_or(0)
            })
            .collect();
        // Clamp selected index
        if self.playlists.is_empty() {
            self.selected_playlist_index = 0;
        } else if self.selected_playlist_index >= self.playlists.len() {
            self.selected_playlist_index = self.playlists.len() - 1;
        }
        Ok(())
    }

    /// Get track count for a playlist at the given index.
    pub fn playlist_track_count(&self, index: usize) -> usize {
        self.track_counts.get(index).copied().unwrap_or(0)
    }

    /// Open a playlist by index — loads its tracks from DB.
    pub fn open_playlist(&mut self, db: &MusicDatabase, index: usize) -> Result<(), String> {
        let playlist = self
            .playlists
            .get(index)
            .ok_or_else(|| format!("Playlist index {} out of range", index))?;
        let id = playlist
            .id
            .ok_or_else(|| "Playlist has no ID".to_string())?;
        let full = db
            .get_playlist_with_tracks(id)
            .map_err(|e| format!("Failed to load playlist tracks: {}", e))?
            .ok_or_else(|| "Playlist not found".to_string())?;
        self.active_playlist = Some(full);
        self.selected_track_index = 0;
        Ok(())
    }

    /// Close the active playlist.
    pub fn close_playlist(&mut self) {
        self.active_playlist = None;
        self.selected_track_index = 0;
    }

    /// Reload the active playlist's tracks from DB (after add/remove/move).
    fn reload_active(&mut self, db: &MusicDatabase) -> Result<(), String> {
        if let Some(ref active) = self.active_playlist {
            if let Some(id) = active.id {
                let full = db
                    .get_playlist_with_tracks(id)
                    .map_err(|e| format!("Failed to reload playlist: {}", e))?
                    .ok_or_else(|| "Playlist not found".to_string())?;
                // Clamp track index
                let track_count = full.entries.len();
                if track_count == 0 {
                    self.selected_track_index = 0;
                } else if self.selected_track_index >= track_count {
                    self.selected_track_index = track_count - 1;
                }
                self.active_playlist = Some(full);
            }
        }
        Ok(())
    }

    // =========================================================================
    // CRUD
    // =========================================================================

    /// Create a new playlist.
    pub fn create_playlist(
        &mut self,
        db: &MusicDatabase,
        name: &str,
        description: Option<&str>,
    ) -> Result<i64, String> {
        let id = db
            .create_playlist(name, description)
            .map_err(|e| format!("Failed to create playlist: {}", e))?;
        self.load_playlists(db)?;
        // Select the newly created playlist
        if let Some(pos) = self.playlists.iter().position(|p| p.id == Some(id)) {
            self.selected_playlist_index = pos;
        }
        Ok(id)
    }

    /// Rename the playlist at the given index.
    pub fn rename_playlist(
        &mut self,
        db: &MusicDatabase,
        index: usize,
        new_name: &str,
    ) -> Result<(), String> {
        let playlist = self
            .playlists
            .get(index)
            .ok_or_else(|| format!("Playlist index {} out of range", index))?;
        let id = playlist
            .id
            .ok_or_else(|| "Playlist has no ID".to_string())?;
        db.update_playlist(id, new_name, playlist.description.as_deref())
            .map_err(|e| format!("Failed to rename playlist: {}", e))?;
        self.load_playlists(db)?;
        Ok(())
    }

    /// Delete the playlist at the given index.
    pub fn delete_playlist(&mut self, db: &MusicDatabase, index: usize) -> Result<(), String> {
        let playlist = self
            .playlists
            .get(index)
            .ok_or_else(|| format!("Playlist index {} out of range", index))?;
        let id = playlist
            .id
            .ok_or_else(|| "Playlist has no ID".to_string())?;

        // If we're deleting the active playlist, close it
        if self.active_playlist_id() == Some(id) {
            self.close_playlist();
        }

        db.delete_playlist(id)
            .map_err(|e| format!("Failed to delete playlist: {}", e))?;
        self.load_playlists(db)?;
        Ok(())
    }

    // =========================================================================
    // Track operations (on active playlist)
    // =========================================================================

    /// Add tracks to the active playlist.
    pub fn add_tracks(
        &mut self,
        db: &MusicDatabase,
        track_paths: &[PathBuf],
    ) -> Result<(), String> {
        let id = self
            .active_playlist_id()
            .ok_or_else(|| "No active playlist".to_string())?;
        db.add_tracks_to_playlist(id, track_paths)
            .map_err(|e| format!("Failed to add tracks: {}", e))?;
        self.reload_active(db)?;
        self.load_playlists(db)?; // Refresh updated_at
        Ok(())
    }

    /// Add all tracks from an album to a playlist by index.
    pub fn add_album_to_playlist(
        &mut self,
        db: &MusicDatabase,
        playlist_index: usize,
        album: &Album,
    ) -> Result<(), String> {
        let playlist = self
            .playlists
            .get(playlist_index)
            .ok_or_else(|| format!("Playlist index {} out of range", playlist_index))?;
        let id = playlist
            .id
            .ok_or_else(|| "Playlist has no ID".to_string())?;
        let paths: Vec<PathBuf> = album.tracks.iter().map(|t| t.path.clone()).collect();
        db.add_tracks_to_playlist(id, &paths)
            .map_err(|e| format!("Failed to add album tracks: {}", e))?;
        // Reload active if this is the open playlist
        if self.active_playlist_id() == Some(id) {
            self.reload_active(db)?;
        }
        self.load_playlists(db)?;
        Ok(())
    }

    /// Remove the track at the given index from the active playlist.
    pub fn remove_track(&mut self, db: &MusicDatabase, track_index: usize) -> Result<(), String> {
        let active = self
            .active_playlist
            .as_ref()
            .ok_or_else(|| "No active playlist".to_string())?;
        let id = active.id.ok_or_else(|| "Playlist has no ID".to_string())?;
        let entry = active
            .entries
            .get(track_index)
            .ok_or_else(|| format!("Track index {} out of range", track_index))?;
        db.remove_track_from_playlist(id, &entry.track_path)
            .map_err(|e| format!("Failed to remove track: {}", e))?;
        self.reload_active(db)?;
        self.load_playlists(db)?;
        Ok(())
    }

    /// Move the currently selected track up in the active playlist.
    pub fn move_track_up(&mut self, db: &MusicDatabase) -> Result<(), String> {
        if self.selected_track_index == 0 {
            return Ok(());
        }
        let active = self
            .active_playlist
            .as_ref()
            .ok_or_else(|| "No active playlist".to_string())?;
        let id = active.id.ok_or_else(|| "Playlist has no ID".to_string())?;
        let entry = active
            .entries
            .get(self.selected_track_index)
            .ok_or_else(|| "Track index out of range".to_string())?;
        let new_pos = (self.selected_track_index - 1) as u32;
        db.move_track_in_playlist(id, &entry.track_path, new_pos)
            .map_err(|e| format!("Failed to move track: {}", e))?;
        self.selected_track_index -= 1;
        self.reload_active(db)?;
        Ok(())
    }

    /// Move the currently selected track down in the active playlist.
    pub fn move_track_down(&mut self, db: &MusicDatabase) -> Result<(), String> {
        let active = self
            .active_playlist
            .as_ref()
            .ok_or_else(|| "No active playlist".to_string())?;
        if self.selected_track_index >= active.entries.len().saturating_sub(1) {
            return Ok(());
        }
        let id = active.id.ok_or_else(|| "Playlist has no ID".to_string())?;
        let entry = active
            .entries
            .get(self.selected_track_index)
            .ok_or_else(|| "Track index out of range".to_string())?;
        let new_pos = (self.selected_track_index + 1) as u32;
        db.move_track_in_playlist(id, &entry.track_path, new_pos)
            .map_err(|e| format!("Failed to move track: {}", e))?;
        self.selected_track_index += 1;
        self.reload_active(db)?;
        Ok(())
    }

    // =========================================================================
    // Track resolution
    // =========================================================================

    /// Resolve playlist track paths to Track references from the library.
    /// Returns one entry per playlist entry; None if the track wasn't found.
    pub fn resolve_tracks<'a>(&self, library: &'a MusicLibrary) -> Vec<Option<&'a Track>> {
        let active = match &self.active_playlist {
            Some(p) => p,
            None => return Vec::new(),
        };
        active
            .entries
            .iter()
            .map(|entry| Self::find_track_by_path(library, &entry.track_path))
            .collect()
    }

    /// Find a track in the library by its path.
    fn find_track_by_path<'a>(library: &'a MusicLibrary, path: &Path) -> Option<&'a Track> {
        for album in &library.albums {
            for track in &album.tracks {
                if track.path == path {
                    return Some(track);
                }
            }
        }
        None
    }

    /// Get all track paths from the active playlist (for "play all").
    pub fn active_track_paths(&self) -> Vec<PathBuf> {
        match &self.active_playlist {
            Some(p) => p.entries.iter().map(|e| e.track_path.clone()).collect(),
            None => Vec::new(),
        }
    }

    // =========================================================================
    // Import / Export
    // =========================================================================

    /// Export the active playlist to an M3U8 file.
    pub fn export_playlist(
        &self,
        library: &MusicLibrary,
        output_path: &Path,
    ) -> Result<(), String> {
        let active = self
            .active_playlist
            .as_ref()
            .ok_or_else(|| "No active playlist to export".to_string())?;
        crate::playlist_io::export_m3u8(active, library, output_path)
    }

    /// Import a playlist from an M3U/M3U8 file.
    /// Creates a new playlist in the database and populates it with the resolved tracks.
    pub fn import_playlist(&mut self, db: &MusicDatabase, input_path: &Path) -> Result<(), String> {
        let imported = crate::playlist_io::import_m3u8(input_path)?;
        let id = self.create_playlist(db, &imported.name, None)?;

        let paths: Vec<PathBuf> = imported.entries.into_iter().map(|e| e.path).collect();
        if !paths.is_empty() {
            db.add_tracks_to_playlist(id, &paths)
                .map_err(|e| format!("Failed to add imported tracks: {}", e))?;
        }

        self.load_playlists(db)?;
        // Open the newly imported playlist
        if let Some(pos) = self.playlists.iter().position(|p| p.id == Some(id)) {
            self.open_playlist(db, pos)?;
        }
        Ok(())
    }

    // =========================================================================
    // Navigation
    // =========================================================================

    pub fn select_next_playlist(&mut self) {
        if !self.playlists.is_empty() {
            self.selected_playlist_index =
                (self.selected_playlist_index + 1) % self.playlists.len();
        }
    }

    pub fn select_prev_playlist(&mut self) {
        if !self.playlists.is_empty() {
            self.selected_playlist_index = if self.selected_playlist_index == 0 {
                self.playlists.len() - 1
            } else {
                self.selected_playlist_index - 1
            };
        }
    }

    pub fn select_next_track(&mut self) {
        if let Some(ref active) = self.active_playlist {
            if !active.entries.is_empty() {
                self.selected_track_index = (self.selected_track_index + 1) % active.entries.len();
            }
        }
    }

    pub fn select_prev_track(&mut self) {
        if let Some(ref active) = self.active_playlist {
            if !active.entries.is_empty() {
                self.selected_track_index = if self.selected_track_index == 0 {
                    active.entries.len() - 1
                } else {
                    self.selected_track_index - 1
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::MusicDatabase;

    fn setup_db() -> MusicDatabase {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let db_path = dir.path().join("test.db");
        // Leak the tempdir so it lives for the test duration
        std::mem::forget(dir);
        MusicDatabase::open_for_testing(&db_path).expect("Failed to create test database")
    }

    #[test]
    fn test_create_and_list_playlists() {
        let db = setup_db();
        let mut ctrl = PlaylistController::new();

        ctrl.create_playlist(&db, "Jazz Mix", None).unwrap();
        ctrl.create_playlist(&db, "Road Trip", Some("For driving"))
            .unwrap();

        assert_eq!(ctrl.playlists().len(), 2);
        assert_eq!(ctrl.playlists()[0].name, "Jazz Mix");
        assert_eq!(ctrl.playlists()[1].name, "Road Trip");
    }

    #[test]
    fn test_rename_playlist() {
        let db = setup_db();
        let mut ctrl = PlaylistController::new();

        ctrl.create_playlist(&db, "Old Name", None).unwrap();
        ctrl.rename_playlist(&db, 0, "New Name").unwrap();

        assert_eq!(ctrl.playlists()[0].name, "New Name");
    }

    #[test]
    fn test_delete_playlist() {
        let db = setup_db();
        let mut ctrl = PlaylistController::new();

        ctrl.create_playlist(&db, "To Delete", None).unwrap();
        ctrl.create_playlist(&db, "Keep", None).unwrap();
        assert_eq!(ctrl.playlists().len(), 2);

        ctrl.delete_playlist(&db, 1).unwrap();
        assert_eq!(ctrl.playlists().len(), 1);
        assert_eq!(ctrl.playlists()[0].name, "Keep");
    }

    #[test]
    fn test_open_and_add_tracks() {
        let db = setup_db();
        let mut ctrl = PlaylistController::new();

        ctrl.create_playlist(&db, "Test", None).unwrap();
        ctrl.open_playlist(&db, 0).unwrap();
        assert!(ctrl.active_playlist().is_some());
        assert_eq!(ctrl.active_playlist().unwrap().entries.len(), 0);

        let paths = vec![
            PathBuf::from("/music/track1.flac"),
            PathBuf::from("/music/track2.flac"),
        ];
        ctrl.add_tracks(&db, &paths).unwrap();
        assert_eq!(ctrl.active_playlist().unwrap().entries.len(), 2);
    }

    #[test]
    fn test_remove_track() {
        let db = setup_db();
        let mut ctrl = PlaylistController::new();

        ctrl.create_playlist(&db, "Test", None).unwrap();
        ctrl.open_playlist(&db, 0).unwrap();

        let paths = vec![
            PathBuf::from("/music/a.flac"),
            PathBuf::from("/music/b.flac"),
            PathBuf::from("/music/c.flac"),
        ];
        ctrl.add_tracks(&db, &paths).unwrap();

        ctrl.remove_track(&db, 1).unwrap();
        let entries = &ctrl.active_playlist().unwrap().entries;
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].track_path, PathBuf::from("/music/a.flac"));
        assert_eq!(entries[1].track_path, PathBuf::from("/music/c.flac"));
    }

    #[test]
    fn test_move_track() {
        let db = setup_db();
        let mut ctrl = PlaylistController::new();

        ctrl.create_playlist(&db, "Test", None).unwrap();
        ctrl.open_playlist(&db, 0).unwrap();

        let paths = vec![
            PathBuf::from("/music/a.flac"),
            PathBuf::from("/music/b.flac"),
            PathBuf::from("/music/c.flac"),
        ];
        ctrl.add_tracks(&db, &paths).unwrap();

        // Move second track down
        ctrl.selected_track_index = 1;
        ctrl.move_track_down(&db).unwrap();
        let entries = &ctrl.active_playlist().unwrap().entries;
        assert_eq!(entries[1].track_path, PathBuf::from("/music/c.flac"));
        assert_eq!(entries[2].track_path, PathBuf::from("/music/b.flac"));
        assert_eq!(ctrl.selected_track_index, 2);

        // Move it back up
        ctrl.move_track_up(&db).unwrap();
        let entries = &ctrl.active_playlist().unwrap().entries;
        assert_eq!(entries[1].track_path, PathBuf::from("/music/b.flac"));
        assert_eq!(ctrl.selected_track_index, 1);
    }

    #[test]
    fn test_navigation() {
        let mut ctrl = PlaylistController::new();
        let db = setup_db();

        ctrl.create_playlist(&db, "A", None).unwrap();
        ctrl.create_playlist(&db, "B", None).unwrap();
        ctrl.create_playlist(&db, "C", None).unwrap();

        assert_eq!(ctrl.selected_playlist_index, 2); // Last created is selected
        ctrl.selected_playlist_index = 0;

        ctrl.select_next_playlist();
        assert_eq!(ctrl.selected_playlist_index, 1);
        ctrl.select_next_playlist();
        assert_eq!(ctrl.selected_playlist_index, 2);
        ctrl.select_next_playlist();
        assert_eq!(ctrl.selected_playlist_index, 0); // Wraps

        ctrl.select_prev_playlist();
        assert_eq!(ctrl.selected_playlist_index, 2); // Wraps back
    }

    #[test]
    fn test_close_playlist() {
        let db = setup_db();
        let mut ctrl = PlaylistController::new();

        ctrl.create_playlist(&db, "Test", None).unwrap();
        ctrl.open_playlist(&db, 0).unwrap();
        assert!(ctrl.active_playlist().is_some());

        ctrl.close_playlist();
        assert!(ctrl.active_playlist().is_none());
        assert_eq!(ctrl.selected_track_index, 0);
    }

    #[test]
    fn test_delete_active_playlist_closes_it() {
        let db = setup_db();
        let mut ctrl = PlaylistController::new();

        ctrl.create_playlist(&db, "Test", None).unwrap();
        ctrl.open_playlist(&db, 0).unwrap();
        assert!(ctrl.active_playlist().is_some());

        ctrl.delete_playlist(&db, 0).unwrap();
        assert!(ctrl.active_playlist().is_none());
        assert!(ctrl.playlists().is_empty());
    }

    #[test]
    fn test_selected_index_resets_when_all_playlists_deleted() {
        // Bug: selected_playlist_index stayed stale when list became empty
        let db = setup_db();
        let mut ctrl = PlaylistController::new();

        ctrl.create_playlist(&db, "A", None).unwrap();
        ctrl.create_playlist(&db, "B", None).unwrap();
        ctrl.selected_playlist_index = 1;

        ctrl.delete_playlist(&db, 1).unwrap();
        ctrl.delete_playlist(&db, 0).unwrap();
        assert!(ctrl.playlists().is_empty());
        assert_eq!(ctrl.selected_playlist_index, 0);
    }

    #[test]
    fn test_track_index_resets_when_all_tracks_removed() {
        // Bug: selected_track_index stayed stale when all tracks removed
        let db = setup_db();
        let mut ctrl = PlaylistController::new();

        ctrl.create_playlist(&db, "Test", None).unwrap();
        ctrl.open_playlist(&db, 0).unwrap();

        let paths = vec![PathBuf::from("/music/a.flac")];
        ctrl.add_tracks(&db, &paths).unwrap();
        ctrl.selected_track_index = 0;

        ctrl.remove_track(&db, 0).unwrap();
        assert_eq!(ctrl.active_playlist().unwrap().entries.len(), 0);
        assert_eq!(ctrl.selected_track_index, 0);
    }

    #[test]
    fn test_track_counts_loaded() {
        let db = setup_db();
        let mut ctrl = PlaylistController::new();

        ctrl.create_playlist(&db, "Empty", None).unwrap();
        ctrl.create_playlist(&db, "WithTracks", None).unwrap();

        // Add tracks to second playlist
        ctrl.open_playlist(&db, 1).unwrap();
        let paths = vec![
            PathBuf::from("/music/a.flac"),
            PathBuf::from("/music/b.flac"),
        ];
        ctrl.add_tracks(&db, &paths).unwrap();
        ctrl.close_playlist();

        // Track counts should be available without opening playlists
        assert_eq!(ctrl.playlist_track_count(0), 0);
        assert_eq!(ctrl.playlist_track_count(1), 2);
    }
}
