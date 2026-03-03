//! Queue management methods.
//!
//! Thin UI layer that delegates to `QueueController` via `self.queue`.

use std::path::PathBuf;

use sotf_audio_player::QueuePlaybackEffect;

use super::state::App;

impl App {
    /// Debug assertion to verify queue and expanded_queue_items are in sync.
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
    #[inline]
    fn sync_queue_index(&mut self) {
        self.playback.current_queue_index = self.queue.current_index();
    }

    pub fn add_album_to_queue(&mut self) -> Option<PathBuf> {
        let was_empty = self.queue.is_empty();
        let was_not_playing = !self.playback.is_playing;

        let albums = self.filtered_albums();
        let selected_album = albums.get(self.library_state.selected_index).copied();

        if let Some(album) = selected_album {
            self.queue.add_album(album.clone());
            self.expanded_queue_items.push(false);
            self.assert_queue_consistency();

            if was_empty || was_not_playing {
                return self.start_queue();
            }
        }
        None
    }

    /// Add album to queue and immediately jump to it and start playing
    pub fn play_album_now(&mut self) -> Option<PathBuf> {
        let albums = self.filtered_albums();
        let selected_album = albums.get(self.library_state.selected_index).copied();

        if let Some(album) = selected_album {
            let effect = self.queue.play_album_now(album.clone());
            self.expanded_queue_items.push(false);
            self.sync_queue_index();
            self.assert_queue_consistency();

            if let QueuePlaybackEffect::Play(path) = effect {
                self.playback.is_playing = true;
                return Some(path);
            }
        }
        None
    }

    pub fn start_queue(&mut self) -> Option<PathBuf> {
        let effect = self.queue.start();
        self.sync_queue_index();
        if let QueuePlaybackEffect::Play(path) = effect {
            self.playback.is_playing = true;
            return Some(path);
        }
        None
    }

    pub fn next_track(&mut self) -> Option<PathBuf> {
        match self.queue.next_track() {
            QueuePlaybackEffect::Play(path) => {
                self.sync_queue_index();
                Some(path)
            }
            QueuePlaybackEffect::Stop => {
                self.sync_queue_index();
                None
            }
            QueuePlaybackEffect::None => {
                self.sync_queue_index();
                None
            }
        }
    }

    pub fn previous_track(&mut self) -> Option<PathBuf> {
        match self.queue.previous_track() {
            QueuePlaybackEffect::Play(path) => {
                self.sync_queue_index();
                Some(path)
            }
            _ => {
                self.sync_queue_index();
                None
            }
        }
    }

    pub fn remove_from_queue(&mut self, index: usize) {
        if index >= self.queue.len() {
            return;
        }

        let (effect, was_current) = self.queue.remove(index);
        self.sync_queue_index();

        // Safely remove from expanded_queue_items
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

        if was_current && matches!(effect, QueuePlaybackEffect::Stop) {
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
    pub fn play_selected_queue_item(&mut self) -> Option<PathBuf> {
        if self.selected_queue_index >= self.queue.len() {
            return None;
        }

        let effect = self.queue.jump_to(self.selected_queue_index);
        self.sync_queue_index();
        if let QueuePlaybackEffect::Play(path) = effect {
            self.playback.is_playing = true;
            return Some(path);
        }
        None
    }

    /// Fill queue with "magic" recommendations (~1h of music)
    pub fn fill_queue_magic(&mut self) -> Result<usize, String> {
        log::info!("[App] Starting fill_queue_magic...");
        let db = match self.library_state.library.get_database() {
            Some(db) => db,
            None => {
                log::error!("[App] fill_queue_magic: Database not available");
                return Err("Database not available".to_string());
            }
        };

        let added_albums = self
            .queue
            .fill_magic(db, &self.library_state.library.albums)?;

        let added_count = added_albums.len();
        for _ in &added_albums {
            self.expanded_queue_items.push(false);
        }

        self.assert_queue_consistency();
        Ok(added_count)
    }
}
