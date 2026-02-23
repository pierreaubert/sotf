//! Queue management methods.
//!
//! Contains methods for managing the playback queue.

use std::path::PathBuf;

use sotf_audio_player::QueueItem;

use super::state::App;

impl App {
    /// Debug assertion to verify queue and expanded_queue_items are in sync.
    /// Call this after any queue modification in debug builds.
    #[inline]
    fn assert_queue_consistency(&self) {
        debug_assert_eq!(
            self.queue.len(),
            self.expanded_queue_items.len(),
            "Queue desync detected: queue.len()={}, expanded_queue_items.len()={}",
            self.queue.len(),
            self.expanded_queue_items.len()
        );
    }

    /// Sync playback.current_queue_index from queue.current_index.
    /// Must be called after every queue mutation that might change the current index.
    #[inline]
    fn sync_queue_index(&mut self) {
        self.playback.current_queue_index = self.queue.current_index;
    }

    pub fn add_album_to_queue(&mut self) -> Option<PathBuf> {
        let was_empty = self.queue.is_empty();
        let was_not_playing = !self.playback.is_playing;

        // Get the selected album from the grid view (returns references)
        let albums = self.filtered_albums();
        let selected_album = albums.get(self.library_state.selected_index).copied();

        if let Some(album) = selected_album {
            self.queue.add(album.clone());
            self.expanded_queue_items.push(false);
            self.assert_queue_consistency();

            // Auto-play if queue was empty OR if nothing was playing
            if was_empty || was_not_playing {
                return self.start_queue();
            }
        }
        None
    }

    /// Add album to queue and immediately jump to it and start playing
    /// (used for "Play Now" context menu action)
    pub fn play_album_now(&mut self) -> Option<PathBuf> {
        // Get the selected album from the grid view (returns references)
        let albums = self.filtered_albums();
        let selected_album = albums.get(self.library_state.selected_index).copied();

        if let Some(album) = selected_album {
            // Add to queue
            let new_index = self.queue.add(album.clone());
            self.expanded_queue_items.push(false);
            self.assert_queue_consistency();

            // Jump to the newly added album (last in queue)
            self.queue.current_index = Some(new_index);
            self.playback.current_queue_index = Some(new_index);
            self.playback.is_playing = true;

            // Get the first track
            return self.queue.current_track_path();
        }
        None
    }

    pub fn start_queue(&mut self) -> Option<PathBuf> {
        let path = self.queue.start();
        self.sync_queue_index();
        if path.is_some() {
            self.playback.is_playing = true;
        }
        path
    }

    pub fn next_track(&mut self) -> Option<PathBuf> {
        let path = self.queue.next_track();
        self.sync_queue_index();
        path
    }

    pub fn previous_track(&mut self) -> Option<PathBuf> {
        let path = self.queue.previous_track();
        self.sync_queue_index();
        path
    }

    pub fn remove_from_queue(&mut self, index: usize) {
        if index >= self.queue.len() {
            return;
        }

        let was_current = self.queue.remove(index);
        self.sync_queue_index();

        // Safely remove from expanded_queue_items, handling potential sync issues
        if index < self.expanded_queue_items.len() {
            self.expanded_queue_items.remove(index);
        } else {
            log::warn!(
                "Queue sync issue detected: queue.len()={}, expanded.len()={}",
                self.queue.len(),
                self.expanded_queue_items.len()
            );
            self.expanded_queue_items.resize(self.queue.len(), false);
        }

        if was_current && self.queue.is_empty() {
            self.playback.is_playing = false;
        }

        if self.selected_queue_index >= self.queue.len() && self.selected_queue_index > 0 {
            self.selected_queue_index = self.queue.len() - 1;
        }
        self.assert_queue_consistency();
    }

    pub fn clear_queue(&mut self) {
        self.queue.clear();
        self.expanded_queue_items.clear();
        self.sync_queue_index();
        self.selected_queue_index = 0;
        self.playback.is_playing = false;
        self.assert_queue_consistency();
    }

    /// Get the duration of the currently playing track in seconds
    pub fn get_current_track_duration(&self) -> f64 {
        self.queue.current_track_duration()
    }

    /// Get the path of the currently playing track
    pub fn get_current_track_path(&self) -> Option<PathBuf> {
        self.queue.current_track_path()
    }

    pub fn toggle_queue_item_expansion(&mut self) {
        if self.selected_queue_index < self.expanded_queue_items.len() {
            self.expanded_queue_items[self.selected_queue_index] =
                !self.expanded_queue_items[self.selected_queue_index];
        }
    }

    /// Play the selected queue item (album) from the beginning
    /// Returns the path of the first track to play
    pub fn play_selected_queue_item(&mut self) -> Option<PathBuf> {
        if self.selected_queue_index >= self.queue.len() {
            return None;
        }

        let path = self.queue.jump_to(self.selected_queue_index);
        self.sync_queue_index();
        if path.is_some() {
            self.playback.is_playing = true;
        }
        path
    }

    /// Fill queue with "magic" recommendations (1h of music)
    /// Uses bliss audio features to find songs similar to queue or listening history
    pub fn fill_queue_magic(&mut self) -> Result<usize, String> {
        log::info!("[App] Starting fill_queue_magic...");
        let db = match self.library_state.library.get_database() {
            Some(db) => db,
            None => {
                log::error!("[App] fill_queue_magic: Database not available");
                return Err("Database not available".to_string());
            }
        };

        // Collect current queue paths
        let current_queue_paths: Vec<PathBuf> = self
            .queue
            .items
            .iter()
            .flat_map(|item| {
                item.album
                    .tracks
                    .iter()
                    .map(|t| t.path.clone())
                    .collect::<Vec<_>>()
            })
            .collect();

        log::info!(
            "[App] fill_queue_magic: Found {} existing tracks in queue",
            current_queue_paths.len()
        );

        // Get recommendations (target 1 hour = 3600 seconds)
        let recommendations =
            sotf_audio_player::recommendation::recommend_tracks(db, &current_queue_paths, 3600)
                .map_err(|e| format!("Recommendation error: {}", e))?;

        log::info!(
            "[App] fill_queue_magic: Received {} recommendations",
            recommendations.len()
        );

        if recommendations.is_empty() {
            return Ok(0);
        }

        let mut added_count = 0;

        // Optimization: Build a lookup map of track path to album index
        let mut path_to_album = std::collections::HashMap::new();
        for (idx, album) in self.library_state.library.albums.iter().enumerate() {
            for track in &album.tracks {
                path_to_album.insert(&track.path, idx);
            }
        }

        // Add recommended tracks to queue
        for path in recommendations {
            let found_album = path_to_album
                .get(&path)
                .and_then(|&idx| self.library_state.library.albums.get(idx));

            if let Some(album) = found_album {
                let mut single_track_album: sotf_audio_player::Album = album.clone();
                single_track_album.tracks.retain(|t| t.path == path);

                if !single_track_album.tracks.is_empty() {
                    self.queue.add(single_track_album);
                    self.expanded_queue_items.push(false);
                    added_count += 1;
                }
            } else {
                log::warn!(
                    "[App] fill_queue_magic: Could not find album for recommended track: {:?}",
                    path
                );
            }
        }

        log::info!(
            "[App] fill_queue_magic: Added {} tracks to queue",
            added_count
        );

        self.assert_queue_consistency();
        Ok(added_count)
    }
}
