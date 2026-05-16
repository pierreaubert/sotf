//! Queue controller — wraps the low-level `Queue` with higher-level operations.
//!
//! Mutations return `QueuePlaybackEffect` so the UI knows what to do with the
//! Player without the controller owning it.

use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use sotf_audio::decoder::AudioSource;

use crate::{Album, Queue, QueueItem, Track};

/// Effect that a queue mutation has on playback.
/// The UI inspects this to decide whether to start/stop the Player.
#[derive(Debug, Clone, PartialEq)]
pub enum QueuePlaybackEffect {
    /// No playback change needed.
    None,
    /// Start playing from this audio source.
    Play(AudioSource),
    /// Stop playback (queue is empty or current item was removed).
    Stop,
    /// The currently-playing item changed identity (e.g. it was removed and
    /// another item shifted into its slot). UI must reload the player from
    /// this source so playback follows the new `current_index`.
    Reload(AudioSource),
}

#[derive(Debug, Clone, Default)]
pub struct QueueController {
    queue: Queue,
    pub selected_index: usize,
}

/// Deref to `Queue` so callers can use `.get()`, `.iter()`, `.len()`,
/// `.is_empty()`, `.current_index`, indexing, etc. directly on the controller.
/// Queue itself derefs to `Vec<QueueItem>`.
impl Deref for QueueController {
    type Target = Queue;
    fn deref(&self) -> &Self::Target {
        &self.queue
    }
}

impl DerefMut for QueueController {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.queue
    }
}

impl<'a> IntoIterator for &'a QueueController {
    type Item = &'a QueueItem;
    type IntoIter = std::slice::Iter<'a, QueueItem>;
    fn into_iter(self) -> Self::IntoIter {
        self.queue.items.iter()
    }
}

impl<'a> IntoIterator for &'a mut QueueController {
    type Item = &'a mut QueueItem;
    type IntoIter = std::slice::IterMut<'a, QueueItem>;
    fn into_iter(self) -> Self::IntoIter {
        self.queue.items.iter_mut()
    }
}

impl QueueController {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an album to the end of the queue. Returns the insertion index.
    ///
    /// Returns an error if none of the album's tracks exist on disk.
    pub fn add_album(&mut self, album: Album) -> Result<usize, String> {
        #[cfg(not(feature = "testing"))]
        validate_album_has_files(&album)?;
        Ok(self.queue.add(album))
    }

    /// Add album to queue and immediately jump to it for playback.
    ///
    /// Returns an error if none of the album's tracks exist on disk.
    pub fn play_album_now(&mut self, album: Album) -> Result<QueuePlaybackEffect, String> {
        #[cfg(not(feature = "testing"))]
        validate_album_has_files(&album)?;
        let new_index = self.queue.add(album);
        self.queue.current_index = Some(new_index);
        match self.queue.current_track_source() {
            Some(source) => Ok(QueuePlaybackEffect::Play(source)),
            None => Ok(QueuePlaybackEffect::None),
        }
    }

    /// Start playback from the first album in the queue.
    pub fn start(&mut self) -> QueuePlaybackEffect {
        match self.queue.start() {
            Some(source) => QueuePlaybackEffect::Play(source),
            None => QueuePlaybackEffect::None,
        }
    }

    /// Peek at the next track without mutating state (for gapless pre-queuing).
    pub fn peek_next_track(&self) -> Option<&Track> {
        self.queue.peek_next_track()
    }

    /// Advance to the next track (crosses album boundaries).
    pub fn next_track(&mut self) -> QueuePlaybackEffect {
        match self.queue.next_track() {
            Some(source) => QueuePlaybackEffect::Play(source),
            None => QueuePlaybackEffect::Stop,
        }
    }

    /// Go to the previous track (crosses album boundaries).
    pub fn previous_track(&mut self) -> QueuePlaybackEffect {
        match self.queue.previous_track() {
            Some(source) => QueuePlaybackEffect::Play(source),
            None => QueuePlaybackEffect::None,
        }
    }

    /// Jump to a specific album index and reset to its first track.
    pub fn jump_to(&mut self, index: usize) -> QueuePlaybackEffect {
        match self.queue.jump_to(index) {
            Some(source) => QueuePlaybackEffect::Play(source),
            None => QueuePlaybackEffect::None,
        }
    }

    /// Remove the album at `index`.
    /// Returns `(effect, was_current)`.
    ///
    /// When the removed album was the currently-playing one:
    ///   - Empty queue → `Stop`
    ///   - Otherwise → `Reload(<new current source>)` so the UI replaces the
    ///     player's source with whatever shifted into `current_index`.
    ///
    /// When the removed album was *not* current, returns `None` (the existing
    /// playback continues; `Queue::remove` already adjusted `current_index`).
    pub fn remove(&mut self, index: usize) -> (QueuePlaybackEffect, bool) {
        if index >= self.queue.len() {
            return (QueuePlaybackEffect::None, false);
        }

        let was_current = self.queue.remove(index);

        // Adjust selected_index
        if self.selected_index >= self.queue.len() && self.selected_index > 0 {
            self.selected_index = self.queue.len() - 1;
        }

        if was_current {
            if self.queue.is_empty() {
                (QueuePlaybackEffect::Stop, true)
            } else {
                // The successor item is now at `current_index`. Tell the UI to
                // reload the player from it. If for some reason no source can
                // be produced (album with zero tracks), fall back to Stop so
                // the UI doesn't keep playing the now-removed audio.
                match self.queue.current_track_source() {
                    Some(source) => (QueuePlaybackEffect::Reload(source), true),
                    None => (QueuePlaybackEffect::Stop, true),
                }
            }
        } else {
            (QueuePlaybackEffect::None, false)
        }
    }

    /// Remove all items from the queue.
    pub fn clear(&mut self) {
        self.queue.clear();
        self.selected_index = 0;
    }

    /// Get the path of the currently playing track.
    pub fn current_track_path(&self) -> Option<PathBuf> {
        self.queue.current_track_path()
    }

    /// Get a reference to the currently playing track.
    pub fn current_track(&self) -> Option<&Track> {
        self.queue.current_track()
    }

    /// Get the duration of the current track in seconds.
    pub fn current_track_duration(&self) -> f64 {
        self.queue.current_track_duration()
    }

    /// Get the current album index.
    pub fn current_index(&self) -> Option<usize> {
        self.queue.current_index
    }

    /// Set the current index directly (for sync with external state).
    pub fn set_current_index(&mut self, index: Option<usize>) {
        self.queue.current_index = index;
    }

    /// Get the queue items.
    pub fn items(&self) -> &[QueueItem] {
        &self.queue.items
    }

    /// Get a mutable reference to the queue items.
    pub fn items_mut(&mut self) -> &mut Vec<QueueItem> {
        &mut self.queue.items
    }

    /// Get the number of items in the queue.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Fill queue with "magic" recommendations (~1h of music).
    ///
    /// Returns the albums that were added, so the UI can manage its own
    /// expansion state.
    pub fn fill_magic(
        &mut self,
        db: &crate::MusicDatabase,
        library_albums: &[Album],
    ) -> Result<Vec<Album>, String> {
        // Collect current queue paths
        let current_queue_paths: Vec<PathBuf> = self
            .queue
            .items
            .iter()
            .flat_map(|item| item.album.tracks.iter().map(|t| t.path.clone()))
            .collect();

        log::info!(
            "[QueueController] fill_magic: Found {} existing tracks in queue",
            current_queue_paths.len()
        );

        // Get recommendations (target 1 hour = 3600 seconds)
        let recommendations =
            crate::recommendation::recommend_tracks(db, &current_queue_paths, 3600)
                .map_err(|e| format!("Recommendation error: {}", e))?;

        log::info!(
            "[QueueController] fill_magic: Received {} recommendations",
            recommendations.len()
        );

        if recommendations.is_empty() {
            return Ok(Vec::new());
        }

        // Build a lookup map of track path to album index
        let mut path_to_album = std::collections::HashMap::new();
        for (idx, album) in library_albums.iter().enumerate() {
            for track in &album.tracks {
                path_to_album.insert(&track.path, idx);
            }
        }

        let mut added_albums = Vec::new();
        for path in recommendations {
            let found_album = path_to_album
                .get(&path)
                .and_then(|&idx| library_albums.get(idx));

            if let Some(album) = found_album {
                let mut single_track_album = album.clone();
                single_track_album.tracks.retain(|t| t.path == path);

                if !single_track_album.tracks.is_empty() {
                    self.queue.add(single_track_album.clone());
                    added_albums.push(single_track_album);
                }
            } else {
                log::warn!(
                    "[QueueController] fill_magic: Could not find album for recommended track: {:?}",
                    path
                );
            }
        }

        log::info!(
            "[QueueController] fill_magic: Added {} tracks to queue",
            added_albums.len()
        );

        Ok(added_albums)
    }
}

/// Check that at least one track in the album has a file that exists on disk.
#[cfg(not(feature = "testing"))]
fn validate_album_has_files(album: &Album) -> Result<(), String> {
    if album.tracks.is_empty() {
        return Err("Album has no tracks".to_string());
    }
    if album.tracks.iter().any(|t| t.path.exists()) {
        return Ok(());
    }
    Err(format!(
        "None of the files for \"{}\" exist on disk",
        album.title,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_album(title: &str, track_count: usize) -> Album {
        Album {
            title: title.to_string(),
            tracks: (0..track_count)
                .map(|i| Track {
                    path: PathBuf::from(format!("/music/{}/track_{}.flac", title, i + 1)),
                    title: Some(format!("Track {}", i + 1)),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    /// Helper: add album directly to the underlying queue, bypassing
    /// file-existence validation (test paths are fake).
    fn add_test_album(ctrl: &mut QueueController, album: Album) {
        ctrl.queue.add(album);
    }

    #[test]
    fn test_add_and_start() {
        let mut ctrl = QueueController::new();
        assert!(ctrl.is_empty());

        add_test_album(&mut ctrl, make_album("A", 3));
        add_test_album(&mut ctrl, make_album("B", 2));
        assert_eq!(ctrl.len(), 2);

        let effect = ctrl.start();
        assert!(matches!(effect, QueuePlaybackEffect::Play(_)));
    }

    #[test]
    fn test_next_track_stop_at_end() {
        let mut ctrl = QueueController::new();
        add_test_album(&mut ctrl, make_album("A", 1));
        ctrl.start();

        let effect = ctrl.next_track();
        assert_eq!(effect, QueuePlaybackEffect::Stop);
    }

    #[test]
    fn test_play_album_now_via_queue() {
        let mut ctrl = QueueController::new();
        add_test_album(&mut ctrl, make_album("A", 2));
        ctrl.start();

        // Use low-level queue to add + jump (bypasses validation)
        let new_idx = ctrl.queue.add(make_album("B", 3));
        ctrl.queue.current_index = Some(new_idx);
        let source = ctrl.queue.current_track_source();
        assert!(source.is_some());
        assert_eq!(ctrl.current_index(), Some(1));
    }

    #[test]
    #[cfg(not(feature = "testing"))]
    fn test_add_album_rejects_missing_files() {
        let mut ctrl = QueueController::new();
        let result = ctrl.add_album(make_album("Missing", 2));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("None of the files"));
    }

    #[test]
    fn test_remove_current_stops_when_empty() {
        let mut ctrl = QueueController::new();
        add_test_album(&mut ctrl, make_album("A", 1));
        ctrl.start();

        let (effect, was_current) = ctrl.remove(0);
        assert!(was_current);
        assert_eq!(effect, QueuePlaybackEffect::Stop);
    }

    #[test]
    fn test_remove_current_reloads_successor() {
        // Removing the playing album with a successor must emit Reload(source)
        // so the UI knows to swap the player's source to the new current item.
        let mut ctrl = QueueController::new();
        add_test_album(&mut ctrl, make_album("A", 2));
        add_test_album(&mut ctrl, make_album("B", 3));
        ctrl.start(); // current = album A, track 1

        let (effect, was_current) = ctrl.remove(0);
        assert!(was_current, "removed the currently-playing album");
        assert_eq!(
            ctrl.current_index(),
            Some(0),
            "B shifted into index 0 and is now current"
        );

        match effect {
            QueuePlaybackEffect::Reload(source) => {
                assert!(
                    source.to_string().contains("/B/track_1"),
                    "Reload source should point at first track of new current album, got: {}",
                    source
                );
            }
            other => panic!(
                "expected Reload(<B/track_1>) when removing the playing album, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_remove_non_current_keeps_playing() {
        // Removing a non-current item leaves playback alone: effect=None.
        let mut ctrl = QueueController::new();
        add_test_album(&mut ctrl, make_album("A", 1));
        add_test_album(&mut ctrl, make_album("B", 1));
        ctrl.start();

        let (effect, was_current) = ctrl.remove(1); // remove B
        assert!(!was_current);
        assert_eq!(effect, QueuePlaybackEffect::None);
        assert_eq!(ctrl.current_index(), Some(0));
    }

    #[test]
    fn test_clear() {
        let mut ctrl = QueueController::new();
        add_test_album(&mut ctrl, make_album("A", 2));
        add_test_album(&mut ctrl, make_album("B", 2));
        ctrl.start();

        ctrl.clear();
        assert!(ctrl.is_empty());
        assert_eq!(ctrl.selected_index, 0);
    }
}
