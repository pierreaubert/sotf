//! Queue management methods.
//!
//! Thin UI layer that delegates to `QueueState` for queue operations
//! and bridges with playback state.

use std::path::PathBuf;

use sotf_audio_player::QueuePlaybackEffect;

use super::state::App;

impl App {
    /// Sync playback.current_queue_index from queue.current_index.
    #[inline]
    fn sync_queue_index(&mut self) {
        self.playback.current_queue_index = self.queue_state.current_index();
    }

    pub fn add_album_to_queue(&mut self) -> Option<sotf_audio::decoder::AudioSource> {
        let was_empty = self.queue_state.is_empty();
        let was_not_playing = !self.playback.is_playing;

        let albums = self.filtered_albums();
        let selected_album = albums.get(self.library_state.selected_index).copied();

        if let Some(album) = selected_album {
            if album
                .id
                .is_some_and(|id| self.queue_state.iter().any(|item| item.album.id == Some(id)))
            {
                return None;
            }

            self.queue_state.add_album(album.clone());

            if was_empty || was_not_playing {
                return self.start_queue();
            }
        }
        None
    }

    /// Add album to queue and immediately jump to it and start playing
    pub fn play_album_now(&mut self) -> Option<sotf_audio::decoder::AudioSource> {
        let albums = self.filtered_albums();
        let selected_album = albums.get(self.library_state.selected_index).copied();

        if let Some(album) = selected_album {
            if let Some(existing_index) = album
                .id
                .and_then(|id| self.queue_state.iter().position(|item| item.album.id == Some(id)))
            {
                self.queue_state.current_index = Some(existing_index);
                self.queue_state.items[existing_index].current_track_index = 0;
                self.sync_queue_index();
                if let Some(source) = self.queue_state.current_track_source() {
                    self.playback.is_playing = true;
                    return Some(source);
                }
                return None;
            }

            let effect = self.queue_state.play_album_now(album.clone());
            self.sync_queue_index();

            if let QueuePlaybackEffect::Play(source) = effect {
                self.playback.is_playing = true;
                return Some(source);
            }
        }
        None
    }

    pub fn start_queue(&mut self) -> Option<sotf_audio::decoder::AudioSource> {
        let effect = self.queue_state.start();
        self.sync_queue_index();
        if let QueuePlaybackEffect::Play(source) = effect {
            self.playback.is_playing = true;
            return Some(source);
        }
        None
    }

    pub fn next_track(&mut self) -> Option<sotf_audio::decoder::AudioSource> {
        let effect = self.queue_state.next_track();
        self.sync_queue_index();
        match effect {
            QueuePlaybackEffect::Play(source) => Some(source),
            _ => None,
        }
    }

    pub fn previous_track(&mut self) -> Option<sotf_audio::decoder::AudioSource> {
        let effect = self.queue_state.previous_track();
        self.sync_queue_index();
        match effect {
            QueuePlaybackEffect::Play(source) => Some(source),
            _ => None,
        }
    }

    pub fn remove_from_queue(&mut self, index: usize) {
        if index >= self.queue_state.len() {
            return;
        }

        let (effect, was_current) = self.queue_state.remove(index);
        self.sync_queue_index();

        if was_current && matches!(effect, QueuePlaybackEffect::Stop) {
            self.playback.is_playing = false;
        }
    }

    pub fn clear_queue(&mut self) {
        self.queue_state.clear();
        self.sync_queue_index();
        self.playback.is_playing = false;
    }

    /// Get the duration of the currently playing track in seconds
    pub fn get_current_track_duration(&self) -> f64 {
        self.queue_state.current_track_duration()
    }

    /// Get the path of the currently playing track
    pub fn get_current_track_path(&self) -> Option<PathBuf> {
        self.queue_state.current_track_path()
    }

    pub fn toggle_queue_item_expansion(&mut self) {
        self.queue_state.toggle_expansion();
    }

    /// Play the selected queue item (album) from the beginning
    pub fn play_selected_queue_item(&mut self) -> Option<sotf_audio::decoder::AudioSource> {
        let selected = self.queue_state.selected_index;
        if selected >= self.queue_state.len() {
            return None;
        }

        let effect = self.queue_state.jump_to(selected);
        self.sync_queue_index();
        if let QueuePlaybackEffect::Play(source) = effect {
            self.playback.is_playing = true;
            return Some(source);
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
            .queue_state
            .fill_magic(db, &self.library_state.library.albums)?;
        Ok(added_albums.len())
    }
}
