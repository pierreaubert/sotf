//! Queue management methods.
//!
//! Contains methods for managing the playback queue.

use std::path::PathBuf;

use super::state::App;
use super::types::QueueItem;

impl App {
    pub fn add_album_to_queue(&mut self) -> Option<PathBuf> {
        let was_empty = self.queue.is_empty();
        let was_not_playing = !self.playback.is_playing;

        // Get the selected album from the grid view
        let albums = self.filtered_albums();
        let selected_album = albums.get(self.library_state.selected_index).cloned();

        if let Some(album) = selected_album {
            self.queue.push(QueueItem::new(album.clone()));
            self.expanded_queue_items.push(false);

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
        // Get the selected album from the grid view
        let albums = self.filtered_albums();
        let selected_album = albums.get(self.library_state.selected_index).cloned();

        if let Some(album) = selected_album {
            // Add to queue
            self.queue.push(QueueItem::new(album.clone()));
            self.expanded_queue_items.push(false);

            // Jump to the newly added album (last in queue)
            let new_index = self.queue.len() - 1;
            self.playback.current_queue_index = Some(new_index);
            self.playback.is_playing = true;

            // Get the first track
            return self.queue[new_index]
                .current_track()
                .map(|track| track.path.clone());
        }
        None
    }

    pub fn start_queue(&mut self) -> Option<PathBuf> {
        if self.queue.is_empty() {
            return None;
        }

        // Set current index to 0
        self.playback.current_queue_index = Some(0);
        self.playback.is_playing = true;

        // Get the first track of the first album
        self.queue
            .first()
            .and_then(|item| item.current_track())
            .map(|track| track.path.clone())
    }

    pub fn next_track(&mut self) -> Option<PathBuf> {
        let current_idx = self.playback.current_queue_index?;
        let item = self.queue.get_mut(current_idx)?;

        // Try to advance to next track in current album
        if let Some(track) = item.next_track() {
            return Some(track.path.clone());
        }

        // No more tracks in current album, try next album
        if current_idx + 1 < self.queue.len() {
            self.playback.current_queue_index = Some(current_idx + 1);
            self.queue[current_idx + 1].current_track_index = 0;
            return self.queue[current_idx + 1]
                .current_track()
                .map(|t| t.path.clone());
        }

        // No more albums
        None
    }

    pub fn previous_track(&mut self) -> Option<PathBuf> {
        if let Some(idx) = self.playback.current_queue_index
            && let Some(item) = self.queue.get_mut(idx)
        {
            if let Some(track) = item.previous_track() {
                return Some(track.path.clone());
            } else {
                // Move to previous album in queue
                if idx > 0 {
                    self.playback.current_queue_index = Some(idx - 1);
                    // Go to last track of previous album
                    if let Some(prev_item) = self.queue.get_mut(idx - 1) {
                        prev_item.current_track_index =
                            prev_item.album.tracks.len().saturating_sub(1);
                        return prev_item.current_track().map(|t| t.path.clone());
                    }
                }
            }
        }
        None
    }

    pub fn remove_from_queue(&mut self, index: usize) {
        if index < self.queue.len() {
            self.queue.remove(index);

            // Safely remove from expanded_queue_items, handling potential sync issues
            if index < self.expanded_queue_items.len() {
                self.expanded_queue_items.remove(index);
            } else {
                // If vectors are out of sync, resync them
                log::warn!(
                    "Queue sync issue detected: queue.len()={}, expanded.len()={}",
                    self.queue.len(),
                    self.expanded_queue_items.len()
                );
                // Resize expanded_queue_items to match queue
                self.expanded_queue_items.resize(self.queue.len(), false);
            }

            // Adjust current queue index if needed
            if let Some(current_idx) = self.playback.current_queue_index {
                if current_idx == index {
                    // We deleted the currently playing album
                    if self.queue.is_empty() {
                        // Queue is now empty
                        self.playback.current_queue_index = None;
                        self.playback.is_playing = false;
                    } else if index < self.queue.len() {
                        // There are albums after the deleted one, stay at same index
                        // (items have shifted down, so index now points to the next album)
                        self.playback.current_queue_index = Some(index);
                        // Reset to first track of the new album at this position
                        if let Some(item) = self.queue.get_mut(index) {
                            item.current_track_index = 0;
                        }
                    } else if index > 0 {
                        // Deleted last album, move to previous album
                        self.playback.current_queue_index = Some(index - 1);
                        // Stay on whatever track was playing in that album
                    } else {
                        // Queue is empty
                        self.playback.current_queue_index = None;
                        self.playback.is_playing = false;
                    }
                } else if current_idx > index {
                    // Deleted an album before the current one, adjust index
                    self.playback.current_queue_index = Some(current_idx - 1);
                }
            }
            if self.selected_queue_index >= self.queue.len() && self.selected_queue_index > 0 {
                self.selected_queue_index = self.queue.len() - 1;
            }
        }
    }

    pub fn clear_queue(&mut self) {
        self.queue.clear();
        self.expanded_queue_items.clear();
        self.playback.current_queue_index = None;
        self.selected_queue_index = 0;
        self.playback.is_playing = false;
    }

    /// Get the duration of the currently playing track in seconds
    pub fn get_current_track_duration(&self) -> f64 {
        self.playback.current_queue_index
            .and_then(|idx| self.queue.get(idx))
            .and_then(|item| item.current_track())
            .and_then(|track| track.duration_secs)
            .map(|d| d as f64)
            .unwrap_or(0.0)
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

        // Set current queue index to the selected item
        self.playback.current_queue_index = Some(self.selected_queue_index);
        self.playback.is_playing = true;

        // Reset to first track of the album
        if let Some(item) = self.queue.get_mut(self.selected_queue_index) {
            item.current_track_index = 0;
            return item.current_track().map(|track| track.path.clone());
        }

        None
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

        // Add recommended tracks to queue
        for path in recommendations {
            // Find the album containing this track
            // We search in the loaded library albums
            // Note: This linear search might be slow for very large libraries.
            // Optimization: Build a map of path -> album_index if needed.
            let found_album = self
                .library_state.library
                .albums
                .iter()
                .find(|album| album.tracks.iter().any(|t| t.path == path));

            if let Some(album) = found_album {
                // Clone the album and keep only the recommended track
                let mut single_track_album: sotf_audio_player::Album = album.clone();
                single_track_album.tracks.retain(|t| t.path == path);

                if !single_track_album.tracks.is_empty() {
                    self.queue.push(QueueItem::new(single_track_album));
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

        Ok(added_count)
    }
}
