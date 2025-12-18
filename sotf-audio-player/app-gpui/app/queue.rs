//! Queue management methods.
//!
//! Contains methods for managing the playback queue.

use std::path::PathBuf;

use super::state::App;
use super::types::QueueItem;

impl App {
    pub fn add_album_to_queue(&mut self) -> Option<PathBuf> {
        let was_empty = self.queue.is_empty();
        let was_not_playing = !self.is_playing;

        // Get the selected album from the grid view
        let albums = self.filtered_albums();
        let selected_album = albums.get(self.selected_album_index).cloned();

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
        let selected_album = albums.get(self.selected_album_index).cloned();

        if let Some(album) = selected_album {
            // Add to queue
            self.queue.push(QueueItem::new(album.clone()));
            self.expanded_queue_items.push(false);

            // Jump to the newly added album (last in queue)
            let new_index = self.queue.len() - 1;
            self.current_queue_index = Some(new_index);
            self.is_playing = true;

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
        self.current_queue_index = Some(0);
        self.is_playing = true;

        // Get the first track of the first album
        self.queue
            .first()
            .and_then(|item| item.current_track())
            .map(|track| track.path.clone())
    }

    pub fn next_track(&mut self) -> Option<PathBuf> {
        let current_idx = self.current_queue_index?;
        let item = self.queue.get_mut(current_idx)?;

        // Try to advance to next track in current album
        if let Some(track) = item.next_track() {
            return Some(track.path.clone());
        }

        // No more tracks in current album, try next album
        if current_idx + 1 < self.queue.len() {
            self.current_queue_index = Some(current_idx + 1);
            self.queue[current_idx + 1].current_track_index = 0;
            return self.queue[current_idx + 1]
                .current_track()
                .map(|t| t.path.clone());
        }

        // No more albums
        None
    }

    pub fn previous_track(&mut self) -> Option<PathBuf> {
        if let Some(idx) = self.current_queue_index
            && let Some(item) = self.queue.get_mut(idx)
        {
            if let Some(track) = item.previous_track() {
                return Some(track.path.clone());
            } else {
                // Move to previous album in queue
                if idx > 0 {
                    self.current_queue_index = Some(idx - 1);
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
            if let Some(current_idx) = self.current_queue_index {
                if current_idx == index {
                    // We deleted the currently playing album
                    if self.queue.is_empty() {
                        // Queue is now empty
                        self.current_queue_index = None;
                        self.is_playing = false;
                    } else if index < self.queue.len() {
                        // There are albums after the deleted one, stay at same index
                        // (items have shifted down, so index now points to the next album)
                        self.current_queue_index = Some(index);
                        // Reset to first track of the new album at this position
                        if let Some(item) = self.queue.get_mut(index) {
                            item.current_track_index = 0;
                        }
                    } else if index > 0 {
                        // Deleted last album, move to previous album
                        self.current_queue_index = Some(index - 1);
                        // Stay on whatever track was playing in that album
                    } else {
                        // Queue is empty
                        self.current_queue_index = None;
                        self.is_playing = false;
                    }
                } else if current_idx > index {
                    // Deleted an album before the current one, adjust index
                    self.current_queue_index = Some(current_idx - 1);
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
        self.current_queue_index = None;
        self.selected_queue_index = 0;
        self.is_playing = false;
    }

    /// Get the duration of the currently playing track in seconds
    pub fn get_current_track_duration(&self) -> f64 {
        self.current_queue_index
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
}
