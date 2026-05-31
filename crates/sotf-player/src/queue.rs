//! Queue management for audio playback.
//!
//! Shared between all app frontends (GPUI, TUI, etc.)

use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use sotf_audio::decoder::AudioSource;

use crate::{Album, Track};

/// A single album in the playback queue with a current track position.
#[derive(Debug, Clone)]
pub struct QueueItem {
    pub album: Album,
    pub current_track_index: usize,
}

impl QueueItem {
    pub fn new(album: Album) -> Self {
        Self {
            album,
            current_track_index: 0,
        }
    }

    pub fn current_track(&self) -> Option<&Track> {
        self.album.tracks.get(self.current_track_index)
    }

    /// Peek at the next track without advancing the index.
    pub fn peek_next_track(&self) -> Option<&Track> {
        self.album.tracks.get(self.current_track_index + 1)
    }

    pub fn next_track(&mut self) -> Option<&Track> {
        if self.current_track_index + 1 < self.album.tracks.len() {
            self.current_track_index += 1;
            self.current_track()
        } else {
            None
        }
    }

    pub fn previous_track(&mut self) -> Option<&Track> {
        if self.current_track_index > 0 {
            self.current_track_index -= 1;
            self.current_track()
        } else {
            None
        }
    }
}

/// Playback queue containing albums with navigation support.
///
/// This struct manages the ordered list of albums and the current playback position.
/// UI-specific state (expansion flags, selection indices) is NOT stored here —
/// each frontend manages that separately.
#[derive(Debug, Clone, Default)]
pub struct Queue {
    pub items: Vec<QueueItem>,
    pub current_index: Option<usize>,
}

/// Deref to `Vec<QueueItem>` so callers can use `.get()`, `.iter()`, `.len()`,
/// `.is_empty()`, indexing, etc. directly on the Queue.
impl Deref for Queue {
    type Target = Vec<QueueItem>;

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

impl DerefMut for Queue {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.items
    }
}

impl<'a> IntoIterator for &'a Queue {
    type Item = &'a QueueItem;
    type IntoIter = std::slice::Iter<'a, QueueItem>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl<'a> IntoIterator for &'a mut Queue {
    type Item = &'a mut QueueItem;
    type IntoIter = std::slice::IterMut<'a, QueueItem>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter_mut()
    }
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an album to the end of the queue. Returns the index it was inserted at.
    pub fn add(&mut self, album: Album) -> usize {
        let idx = self.items.len();
        self.items.push(QueueItem::new(album));
        idx
    }

    /// Remove the album at `index`.
    ///
    /// Adjusts `current_index` to maintain correct playback position:
    /// - If the removed item was the current one: moves to the next item (or previous if last),
    ///   or `None` if the queue becomes empty. Returns `true`.
    /// - If the removed item was before the current one: decrements `current_index`.
    ///   Returns `false`.
    /// - Otherwise: no change to `current_index`. Returns `false`.
    ///
    /// Returns whether the removed item was the currently playing one.
    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }

        self.items.remove(index);

        let Some(current_idx) = self.current_index else {
            return false;
        };

        if current_idx == index {
            // Removed the currently playing album
            if self.items.is_empty() {
                self.current_index = None;
            } else if index < self.items.len() {
                // [A, B, C], current=B, remove(1) -> [A, C];
                // C is now at index 1, so keep the same current_index.
                self.current_index = Some(index);
                // Reset to first track of the new album at this position
                self.items[index].current_track_index = 0;
            } else if index > 0 {
                // Deleted last album, move to previous
                self.current_index = Some(index - 1);
            } else {
                self.current_index = None;
            }
            true
        } else {
            if current_idx > index {
                self.current_index = Some(current_idx - 1);
            }
            false
        }
    }

    /// Remove all items from the queue.
    pub fn clear(&mut self) {
        self.items.clear();
        self.current_index = None;
    }

    /// Start playback from the first album in the queue.
    /// Returns the audio source of the first track, or `None` if the queue is empty.
    pub fn start(&mut self) -> Option<AudioSource> {
        if self.items.is_empty() {
            return None;
        }
        self.current_index = Some(0);
        self.items[0].current_track_index = 0;
        self.current_track_source()
    }

    /// Peek at the next track without mutating state. Crosses album boundaries.
    pub fn peek_next_track(&self) -> Option<&Track> {
        let current_idx = self.current_index?;
        let item = self.items.get(current_idx)?;

        // Try next track in current album
        if let Some(track) = item.peek_next_track() {
            return Some(track);
        }

        // Try first track of next album
        self.items.get(current_idx + 1)?.album.tracks.first()
    }

    /// Advance to the next track. Crosses album boundaries.
    /// Returns the audio source of the next track, or `None` if at the end of the queue.
    pub fn next_track(&mut self) -> Option<AudioSource> {
        let current_idx = self.current_index?;
        let item = self.items.get_mut(current_idx)?;

        // Try to advance within the current album
        if let Some(track) = item.next_track() {
            return Some(track.audio_source());
        }

        // Move to next album
        if current_idx + 1 < self.items.len() {
            self.current_index = Some(current_idx + 1);
            self.items[current_idx + 1].current_track_index = 0;
            return self.items[current_idx + 1]
                .current_track()
                .map(|t| t.audio_source());
        }

        // End of queue
        None
    }

    /// Go to the previous track. Crosses album boundaries.
    /// Returns the audio source of the previous track, or `None` if at the start of the queue.
    pub fn previous_track(&mut self) -> Option<AudioSource> {
        let current_idx = self.current_index?;
        let item = self.items.get_mut(current_idx)?;

        // Try to go back within the current album
        if let Some(track) = item.previous_track() {
            return Some(track.audio_source());
        }

        // Move to previous album (last track)
        if current_idx > 0 {
            self.current_index = Some(current_idx - 1);
            if let Some(prev_item) = self.items.get_mut(current_idx - 1) {
                prev_item.current_track_index = prev_item.album.tracks.len().saturating_sub(1);
                return prev_item.current_track().map(|t| t.audio_source());
            }
        }

        None
    }

    /// Get the audio source of the currently playing track.
    pub fn current_track_source(&self) -> Option<AudioSource> {
        self.current_index
            .and_then(|idx| self.items.get(idx))
            .and_then(|item| item.current_track())
            .map(|track| track.audio_source())
    }

    /// Get the path of the currently playing track (convenience for local files).
    pub fn current_track_path(&self) -> Option<PathBuf> {
        self.current_track_source()
            .and_then(|s| s.as_path().map(|p| p.to_path_buf()))
    }

    /// Get a reference to the currently playing track.
    pub fn current_track(&self) -> Option<&Track> {
        self.current_index
            .and_then(|idx| self.items.get(idx))
            .and_then(|item| item.current_track())
    }

    /// Get the duration of the current track in seconds.
    pub fn current_track_duration(&self) -> f64 {
        self.current_track()
            .and_then(|track| track.duration_secs)
            .map(|d| d as f64)
            .unwrap_or(0.0)
    }

    /// Jump to a specific album index and reset to its first track.
    /// Returns the audio source of the first track, or `None` if the index is invalid.
    pub fn jump_to(&mut self, index: usize) -> Option<AudioSource> {
        if index >= self.items.len() {
            return None;
        }
        self.current_index = Some(index);
        self.items[index].current_track_index = 0;
        self.current_track_source()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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

    #[test]
    fn test_queue_add_and_start() {
        let mut queue = Queue::new();
        assert!(queue.is_empty());

        queue.add(make_album("A", 3));
        queue.add(make_album("B", 2));
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.current_index, None);

        let path = queue.start();
        assert!(path.is_some());
        assert_eq!(queue.current_index, Some(0));
    }

    #[test]
    fn test_queue_next_track_within_album() {
        let mut queue = Queue::new();
        queue.add(make_album("A", 3));
        queue.start();

        let source = queue.next_track().unwrap();
        assert!(source.to_string().contains("track_2"));

        let source = queue.next_track().unwrap();
        assert!(source.to_string().contains("track_3"));
    }

    #[test]
    fn test_queue_next_track_crosses_albums() {
        let mut queue = Queue::new();
        queue.add(make_album("A", 1));
        queue.add(make_album("B", 2));
        queue.start();

        // Album A has 1 track, next should go to album B
        let source = queue.next_track().unwrap();
        assert!(source.to_string().contains("/B/track_1"));
        assert_eq!(queue.current_index, Some(1));
    }

    #[test]
    fn test_queue_next_track_end_of_queue() {
        let mut queue = Queue::new();
        queue.add(make_album("A", 1));
        queue.start();

        assert!(queue.next_track().is_none());
    }

    #[test]
    fn test_queue_previous_track_within_album() {
        let mut queue = Queue::new();
        queue.add(make_album("A", 3));
        queue.start();
        queue.next_track(); // track 2
        queue.next_track(); // track 3

        let source = queue.previous_track().unwrap();
        assert!(source.to_string().contains("track_2"));
    }

    #[test]
    fn test_queue_previous_track_crosses_albums() {
        let mut queue = Queue::new();
        queue.add(make_album("A", 2));
        queue.add(make_album("B", 2));
        queue.start();
        queue.next_track(); // A track 2
        queue.next_track(); // B track 1

        let source = queue.previous_track().unwrap();
        // Should go to last track of album A (track 2)
        assert!(source.to_string().contains("/A/track_2"));
        assert_eq!(queue.current_index, Some(0));
    }

    #[test]
    fn test_queue_remove_current_item() {
        let mut queue = Queue::new();
        queue.add(make_album("A", 1));
        queue.add(make_album("B", 1));
        queue.start();

        let was_current = queue.remove(0);
        assert!(was_current);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.current_index, Some(0));
        // Should now be on album B
        assert!(
            queue
                .current_track_source()
                .unwrap()
                .to_string()
                .contains("/B/")
        );
    }

    #[test]
    fn test_queue_remove_before_current() {
        let mut queue = Queue::new();
        queue.add(make_album("A", 1));
        queue.add(make_album("B", 1));
        queue.add(make_album("C", 1));
        queue.start();
        queue.next_track(); // Move to B

        let was_current = queue.remove(0); // Remove A
        assert!(!was_current);
        assert_eq!(queue.current_index, Some(0)); // B is now at index 0
    }

    #[test]
    fn test_queue_remove_last_makes_empty() {
        let mut queue = Queue::new();
        queue.add(make_album("A", 1));
        queue.start();

        let was_current = queue.remove(0);
        assert!(was_current);
        assert!(queue.is_empty());
        assert_eq!(queue.current_index, None);
    }

    #[test]
    fn test_queue_clear() {
        let mut queue = Queue::new();
        queue.add(make_album("A", 2));
        queue.add(make_album("B", 2));
        queue.start();

        queue.clear();
        assert!(queue.is_empty());
        assert_eq!(queue.current_index, None);
    }

    #[test]
    fn test_queue_jump_to() {
        let mut queue = Queue::new();
        queue.add(make_album("A", 2));
        queue.add(make_album("B", 2));
        queue.start();

        let source = queue.jump_to(1).unwrap();
        assert!(source.to_string().contains("/B/track_1"));
        assert_eq!(queue.current_index, Some(1));
    }
}
