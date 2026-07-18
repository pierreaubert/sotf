//! Queue management methods.
//!
//! Thin UI layer that delegates to `QueueState` for queue operations
//! and bridges with playback state.

use std::path::PathBuf;

use rand::RngExt;
use sotf_audio_player::{Album, QueuePlaybackEffect};

use super::state::App;
use super::types::{ToastAction, ToastMessage, ToastType};

fn is_missing_album_files_error(message: &str) -> bool {
    message.starts_with("None of the files")
}

impl App {
    /// Sync playback.current_queue_index from queue.current_index.
    #[inline]
    fn sync_queue_index(&mut self) {
        self.playback.current_queue_index = self.queue_state.current_index();
    }

    pub fn add_album_to_queue(
        &mut self,
    ) -> Result<Option<sotf_audio::decoder::AudioSource>, String> {
        let albums = self.filtered_albums();
        let selected_album = albums
            .get(self.library_state.selected_index)
            .map(|album| (*album).clone());

        if let Some(album) = selected_album {
            if album.id.is_some_and(|id| {
                self.queue_state
                    .iter()
                    .any(|item| item.album.id == Some(id))
            }) {
                return Ok(None);
            }

            if let Err(err) = self.queue_state.add_album(album.clone()) {
                if is_missing_album_files_error(&err) {
                    self.remove_stale_album_from_view(&album, err);
                    return Ok(None);
                }
                return Err(err);
            }
        }
        Ok(None)
    }

    /// Add album to queue and immediately jump to it and start playing
    pub fn play_album_now(&mut self) -> Result<Option<sotf_audio::decoder::AudioSource>, String> {
        let albums = self.filtered_albums();
        let selected_album = albums
            .get(self.library_state.selected_index)
            .map(|album| (*album).clone());

        if let Some(album) = selected_album {
            if let Some(existing_index) = album.id.and_then(|id| {
                self.queue_state
                    .iter()
                    .position(|item| item.album.id == Some(id))
            }) {
                self.queue_state.current_index = Some(existing_index);
                self.queue_state.items[existing_index].current_track_index = 0;
                self.sync_queue_index();
                if let Some(source) = self.queue_state.current_track_source() {
                    self.playback.is_playing = true;
                    return Ok(Some(source));
                }
                return Ok(None);
            }

            let effect = match self.queue_state.play_album_now(album.clone()) {
                Ok(effect) => effect,
                Err(err) => {
                    if is_missing_album_files_error(&err) {
                        self.remove_stale_album_from_view(&album, err);
                        return Ok(None);
                    }
                    return Err(err);
                }
            };
            self.sync_queue_index();

            if let QueuePlaybackEffect::Play(source) = effect {
                self.playback.is_playing = true;
                return Ok(Some(source));
            }
        }
        Ok(None)
    }

    fn remove_stale_album_from_view(&mut self, album: &Album, message: String) {
        let before = self.library_state.library.albums.len();
        self.library_state.library.albums.retain(|candidate| {
            if let (Some(candidate_id), Some(album_id)) = (candidate.id, album.id) {
                candidate_id != album_id
            } else {
                candidate.title != album.title || candidate.artist() != album.artist()
            }
        });

        if self.library_state.library.albums.len() != before {
            self.library_state.invalidate_cache();
            let len = self.filtered_albums().len();
            if len == 0 {
                self.library_state.selected_index = 0;
            } else if self.library_state.selected_index >= len {
                self.library_state.selected_index = len - 1;
            }
            self.invalidate_library_stats();
        }

        self.ui_state.toast_message = Some(
            ToastMessage::persistent(message, ToastType::Warning)
                .with_action(ToastAction::new("Rescan", "rescan-library")),
        );
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
        if self.ui_state.phone_repeat_enabled {
            self.sync_queue_index();
            return self.queue_state.current_track_source();
        }

        if self.ui_state.phone_shuffle_enabled
            && let Some(source) = self.next_shuffled_track()
        {
            self.sync_queue_index();
            return Some(source);
        }

        let effect = self.queue_state.next_track();
        self.sync_queue_index();
        match effect {
            QueuePlaybackEffect::Play(source) => Some(source),
            _ => None,
        }
    }

    fn next_shuffled_track(&mut self) -> Option<sotf_audio::decoder::AudioSource> {
        let choices = self
            .queue_state
            .items
            .iter()
            .enumerate()
            .flat_map(|(album_idx, item)| {
                item.album
                    .tracks
                    .iter()
                    .enumerate()
                    .map(move |(track_idx, _track)| (album_idx, track_idx))
            })
            .collect::<Vec<_>>();

        if choices.is_empty() {
            return None;
        }

        let current = self.queue_state.current_index().and_then(|album_idx| {
            self.queue_state
                .items
                .get(album_idx)
                .map(|item| (album_idx, item.current_track_index))
        });

        let mut rng = rand::rng();
        let mut target = choices[rng.random_range(0..choices.len())];
        if choices.len() > 1 {
            while Some(target) == current {
                target = choices[rng.random_range(0..choices.len())];
            }
        }

        self.queue_state.current_index = Some(target.0);
        if let Some(item) = self.queue_state.items.get_mut(target.0) {
            item.current_track_index = target.1;
        }
        self.queue_state.current_track_source()
    }

    pub fn previous_track(&mut self) -> Option<sotf_audio::decoder::AudioSource> {
        let effect = self.queue_state.previous_track();
        self.sync_queue_index();
        match effect {
            QueuePlaybackEffect::Play(source) => Some(source),
            _ => None,
        }
    }

    pub fn remove_from_queue(&mut self, index: usize) -> QueuePlaybackEffect {
        if index >= self.queue_state.len() {
            return QueuePlaybackEffect::None;
        }

        let was_playing = self.playback.is_playing;
        let (effect, was_current) = self.queue_state.remove(index);
        self.sync_queue_index();

        match effect {
            QueuePlaybackEffect::Stop if was_current => {
                self.playback.is_playing = false;
                QueuePlaybackEffect::Stop
            }
            QueuePlaybackEffect::Reload(source) if was_current && was_playing => {
                QueuePlaybackEffect::Reload(source)
            }
            _ => QueuePlaybackEffect::None,
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
