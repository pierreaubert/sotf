//! Queue controller — wraps the low-level `Queue` with higher-level operations.
//!
//! Mutations return `QueuePlaybackEffect` so the UI knows what to do with the
//! Player without the controller owning it.

use sotf_audio::decoder::AudioSource;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use crate::{Album, MusicLibrary, Queue, QueueItem, Track};

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

/// Outcome of appending a playlist to the queue.
///
/// A playlist can refer to files which no longer exist in the local library,
/// and callers may enqueue it repeatedly.  Report both cases explicitly so a
/// UI can offer useful recovery instead of silently doing nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlaylistQueueAppend {
    pub added: usize,
    pub skipped_existing: usize,
    pub skipped_missing: usize,
    pub first_added_index: Option<usize>,
}

/// Address of a track in the album-backed queue. Upcoming addresses exclude
/// the playing track and every already-played item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueTrackPosition {
    pub item: usize,
    pub track: usize,
}

/// Single-use undo record. Restore only the removed track, never a stale
/// playback cursor or an entire queue snapshot.
#[derive(Debug)]
pub struct UpcomingTrackRemoval {
    position: QueueTrackPosition,
    album: Album,
    item_removed: bool,
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
        validate_album_has_tracks(&album)?;
        #[cfg(not(feature = "testing"))]
        validate_album_has_files(&album)?;
        Ok(self.queue.add(album))
    }

    /// Append the active playlist's tracks as single-track queue items.
    ///
    /// Keeping one track per item preserves playlist ordering even when
    /// neighbouring entries came from the same album.  Existing queue tracks
    /// are intentionally not duplicated.
    pub fn enqueue_playlist_tracks(
        &mut self,
        library: &MusicLibrary,
        track_paths: &[PathBuf],
    ) -> PlaylistQueueAppend {
        let mut outcome = PlaylistQueueAppend::default();

        for path in track_paths {
            if self
                .queue
                .items
                .iter()
                .any(|item| item.album.tracks.iter().any(|track| &track.path == path))
            {
                outcome.skipped_existing += 1;
                continue;
            }

            let Some(album) = library
                .albums
                .iter()
                .find(|album| album.tracks.iter().any(|track| &track.path == path))
            else {
                outcome.skipped_missing += 1;
                continue;
            };

            let mut single_track_album = album.clone();
            single_track_album
                .tracks
                .retain(|track| &track.path == path);

            match self.add_album(single_track_album) {
                Ok(index) => {
                    outcome.first_added_index.get_or_insert(index);
                    outcome.added += 1;
                }
                Err(_) => outcome.skipped_missing += 1,
            }
        }

        outcome
    }

    /// Add album to queue and immediately jump to it for playback.
    ///
    /// Returns an error if none of the album's tracks exist on disk.
    pub fn play_album_now(&mut self, album: Album) -> Result<QueuePlaybackEffect, String> {
        validate_album_has_tracks(&album)?;
        #[cfg(not(feature = "testing"))]
        validate_album_has_files(&album)?;
        let new_index = self.queue.add(album);
        self.queue.current_index = Some(new_index);
        match self.queue.current_track_source() {
            Some(source) => Ok(QueuePlaybackEffect::Play(source)),
            None => Ok(QueuePlaybackEffect::None),
        }
    }

    /// Play a single-file selection, reusing its existing queue position when
    /// present. Other queued albums and their order are preserved.
    pub fn play_single_file_now(&mut self, album: Album) -> Result<QueuePlaybackEffect, String> {
        if album.tracks.len() != 1 || album.tracks[0].source.is_some() {
            return Err("A single local audio file is required".into());
        }
        #[cfg(not(feature = "testing"))]
        validate_album_has_files(&album)?;
        let path = &album.tracks[0].path;
        let existing = self
            .queue
            .items
            .iter()
            .enumerate()
            .find_map(|(item_index, item)| {
                item.album
                    .tracks
                    .iter()
                    .position(|track| track.source.is_none() && &track.path == path)
                    .map(|track_index| (item_index, track_index))
            });
        if let Some((item_index, track_index)) = existing {
            self.queue.items[item_index].current_track_index = track_index;
            self.queue.current_index = Some(item_index);
            return Ok(self
                .queue
                .current_track_source()
                .map_or(QueuePlaybackEffect::None, QueuePlaybackEffect::Play));
        }
        self.play_album_now(album)
    }

    /// Schedule an album immediately after the current track, preserving the
    /// remainder of the current album and every previously queued item.
    /// Does not start or interrupt playback. With no current item, insert first.
    pub fn enqueue_next(&mut self, album: Album) -> Result<usize, String> {
        validate_album_has_tracks(&album)?;
        #[cfg(not(feature = "testing"))]
        validate_album_has_files(&album)?;
        let current = self
            .queue
            .current_index
            .filter(|&index| index < self.queue.items.len());
        let insertion = current.map_or(0, |index| index + 1);
        let remainder = current.and_then(|index| {
            let item = &mut self.queue.items[index];
            let split = item.current_track_index.saturating_add(1);
            if split >= item.album.tracks.len() {
                return None;
            }
            let mut tail = item.album.clone();
            tail.tracks = item.album.tracks.split_off(split);
            Some(QueueItem::new(tail))
        });
        let added = if remainder.is_some() { 2 } else { 1 };
        if let Some(remainder) = remainder {
            self.queue.items.insert(insertion, remainder);
        }
        self.queue.items.insert(insertion, QueueItem::new(album));
        if self.selected_index >= insertion && self.queue.items.len() > added {
            self.selected_index += added;
        }
        Ok(insertion)
    }

    pub fn upcoming_track_positions(&self) -> Vec<QueueTrackPosition> {
        let current = self.queue.current_index;
        if let Some(index) = current
            && self
                .queue
                .items
                .get(index)
                .and_then(QueueItem::current_track)
                .is_none()
        {
            return Vec::new();
        }
        self.queue
            .items
            .iter()
            .enumerate()
            .skip(current.unwrap_or(0))
            .flat_map(|(item, queued)| {
                let start = if current == Some(item) {
                    queued.current_track_index + 1
                } else {
                    0
                };
                (start..queued.album.tracks.len())
                    .map(move |track| QueueTrackPosition { item, track })
            })
            .collect()
    }

    /// Resolve a rendered row against the live upcoming queue. Both its album
    /// position and source must still match: source alone is ambiguous when a
    /// track occurs more than once. Playback advancement may change the flat
    /// index without changing this address.
    pub fn resolve_upcoming_track(
        &self,
        position: QueueTrackPosition,
        source: &AudioSource,
    ) -> Option<usize> {
        let track = self
            .queue
            .items
            .get(position.item)?
            .album
            .tracks
            .get(position.track)?;
        if track.audio_source() != *source {
            return None;
        }
        self.upcoming_track_positions()
            .iter()
            .position(|candidate| *candidate == position)
    }

    /// Remove one upcoming track without changing playback. Indices refer to
    /// `upcoming_track_positions`, so the current track cannot be removed here.
    pub fn remove_upcoming_track(&mut self, index: usize) -> Option<UpcomingTrackRemoval> {
        let position = self.upcoming_track_positions().get(index).copied()?;
        let item_removed = self.queue.items[position.item].album.tracks.len() == 1;
        let album = self.take_upcoming_track(position)?;
        Some(UpcomingTrackRemoval {
            position,
            album,
            item_removed,
        })
    }

    /// Undo a removal after arbitrary playback navigation. Callers discard the
    /// record on intervening content edits; navigation alone keeps it valid.
    pub fn undo_upcoming_track_removal(&mut self, mut removal: UpcomingTrackRemoval) -> bool {
        let position = removal.position;
        if removal.item_removed {
            if position.item > self.queue.items.len() {
                return false;
            }
            self.queue
                .items
                .insert(position.item, QueueItem::new(removal.album));
            if let Some(current) = self.queue.current_index.as_mut()
                && *current >= position.item
            {
                *current += 1;
            }
            if self.selected_index >= position.item {
                self.selected_index += 1;
            }
        } else {
            let Some(item) = self.queue.items.get_mut(position.item) else {
                return false;
            };
            if position.track > item.album.tracks.len() {
                return false;
            }
            let Some(track) = removal.album.tracks.pop() else {
                return false;
            };
            item.album.tracks.insert(position.track, track);
            if self.queue.current_index == Some(position.item)
                && item.current_track_index >= position.track
            {
                item.current_track_index += 1;
            }
        }
        self.selected_index = self
            .selected_index
            .min(self.queue.items.len().saturating_sub(1));
        true
    }

    /// Move one upcoming track to its final flat-list index. Split only the
    /// destination album when necessary; keep the moved track's album metadata.
    pub fn move_upcoming_track(&mut self, from: usize, to: usize) -> bool {
        let positions = self.upcoming_track_positions();
        if from == to || to >= positions.len() {
            return false;
        }
        let Some(position) = positions.get(from).copied() else {
            return false;
        };
        let Some(album) = self.take_upcoming_track(position) else {
            return false;
        };
        let target = self.upcoming_track_positions().get(to).copied();
        let insertion = match target {
            Some(target) if target.track == 0 => target.item,
            Some(target) => {
                let item = &mut self.queue.items[target.item];
                let mut remainder = item.album.clone();
                remainder.tracks = item.album.tracks.split_off(target.track);
                self.queue
                    .items
                    .insert(target.item + 1, QueueItem::new(remainder));
                target.item + 1
            }
            None => self.queue.items.len(),
        };
        self.queue.items.insert(insertion, QueueItem::new(album));
        self.selected_index = insertion;
        true
    }

    fn take_upcoming_track(&mut self, position: QueueTrackPosition) -> Option<Album> {
        // This private helper receives an address validated against the current
        // upcoming list; still reject invalid bounds without mutating anything.
        let item = self.queue.items.get_mut(position.item)?;
        if position.track >= item.album.tracks.len() {
            return None;
        }
        let track = item.album.tracks.remove(position.track);
        let mut album = item.album.clone();
        album.tracks = vec![track];
        if item.album.tracks.is_empty() {
            self.queue.items.remove(position.item);
        }
        self.selected_index = self
            .selected_index
            .min(self.queue.items.len().saturating_sub(1));
        Some(album)
    }

    /// Explicitly play an upcoming row without changing the queue contents.
    pub fn play_upcoming_track(&mut self, index: usize) -> QueuePlaybackEffect {
        let Some(position) = self.upcoming_track_positions().get(index).copied() else {
            return QueuePlaybackEffect::None;
        };
        self.queue.current_index = Some(position.item);
        self.queue.items[position.item].current_track_index = position.track;
        self.selected_index = position.item;
        self.queue
            .current_track_source()
            .map(QueuePlaybackEffect::Play)
            .unwrap_or(QueuePlaybackEffect::None)
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

    /// Move a queued album. Reordering does not restart playback: `Queue`
    /// tracks the current item by its moved index, so its source remains valid.
    pub fn move_item(&mut self, from: usize, to: usize) -> bool {
        if !self.queue.move_item(from, to) {
            return false;
        }

        if self.selected_index == from {
            self.selected_index = to;
        } else if from < self.selected_index && self.selected_index <= to {
            self.selected_index -= 1;
        } else if to <= self.selected_index && self.selected_index < from {
            self.selected_index += 1;
        }
        true
    }

    /// Remove all items from the queue.
    pub fn clear(&mut self) {
        self.queue.clear();
        self.selected_index = 0;
    }

    /// Set the current index directly (for sync with external state).
    pub fn set_current_index(&mut self, index: Option<usize>) {
        self.queue.current_index = index;
    }

    /// Get the current album index.
    ///
    /// Kept as a small semantic accessor because several app call sites use it
    /// to sync playback state; other queue accessors are available via `Deref`.
    pub fn current_index(&self) -> Option<usize> {
        self.queue.current_index
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

fn validate_album_has_tracks(album: &Album) -> Result<(), String> {
    if album.tracks.is_empty() {
        return Err("Album has no tracks".to_string());
    }
    Ok(())
}

/// Check that at least one track in the album has a file that exists on disk.
#[cfg(not(feature = "testing"))]
fn validate_album_has_files(album: &Album) -> Result<(), String> {
    if album
        .tracks
        .iter()
        .any(|t| !matches!(t.audio_source(), AudioSource::File(_)) || t.path.exists())
    {
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

    #[test]
    fn single_file_playback_reuses_track_position_without_reordering_queue() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut original = make_album("Original", 2);
        original.tracks[1].path = file.path().into();
        let selected = Album {
            tracks: vec![original.tracks[1].clone()],
            ..Default::default()
        };
        let mut controller = QueueController::new();
        add_test_album(&mut controller, original);
        add_test_album(&mut controller, make_album("Later", 1));
        for _ in 0..2 {
            assert_eq!(
                controller.play_single_file_now(selected.clone()).unwrap(),
                QueuePlaybackEffect::Play(AudioSource::File(file.path().into()))
            );
            assert_eq!(controller.len(), 2);
            assert_eq!(controller.current_index(), Some(0));
            assert_eq!(controller[0].current_track_index, 1);
            assert_eq!(controller[1].album.title, "Later");
        }
        assert!(controller.play_single_file_now(Album::default()).is_err());
        assert_eq!(controller.current_index(), Some(0));
        assert_eq!(controller.len(), 2);
    }

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

    fn next_test_album(title: &str, count: usize) -> Album {
        let mut album = make_album(title, count);
        for (index, track) in album.tracks.iter_mut().enumerate() {
            track.source = Some(AudioSource::Url {
                url: format!("https://example.com/{title}/{index}.flac"),
                format_hint: None,
                seekable: true,
            });
        }
        album
    }

    #[test]
    fn playing_upcoming_track_resolves_flat_position_without_editing_content() {
        let mut ctrl = QueueController::new();
        ctrl.add_album(next_test_album("A", 3)).unwrap();
        ctrl.add_album(next_test_album("B", 2)).unwrap();
        ctrl.queue.current_index = Some(0);
        ctrl.queue.items[0].current_track_index = 1;
        let expected = ctrl.queue.items[1].album.tracks[1].audio_source();
        assert_eq!(
            ctrl.play_upcoming_track(2),
            QueuePlaybackEffect::Play(expected.clone())
        );
        assert_eq!(ctrl.queue.current_index, Some(1));
        assert_eq!(ctrl.queue.items[1].current_track_index, 1);
        assert_eq!(ctrl.selected_index, 1);
        assert_eq!(ctrl.queue.items[0].album.tracks.len(), 3);
        assert_eq!(ctrl.queue.items[1].album.tracks.len(), 2);
        assert_eq!(ctrl.play_upcoming_track(0), QueuePlaybackEffect::None);
        assert_eq!(ctrl.queue.current_track_source(), Some(expected));
    }

    #[test]
    fn rendered_upcoming_address_is_revalidated_after_playback_or_content_changes() {
        let mut ctrl = QueueController::new();
        ctrl.add_album(next_test_album("A", 3)).unwrap();
        ctrl.queue.current_index = Some(0);
        let position = QueueTrackPosition { item: 0, track: 2 };
        let source = ctrl.queue.items[0].album.tracks[2].audio_source();
        assert_eq!(ctrl.resolve_upcoming_track(position, &source), Some(1));
        ctrl.queue.items[0].current_track_index = 1;
        assert_eq!(ctrl.resolve_upcoming_track(position, &source), Some(0));
        ctrl.queue.items[0].current_track_index = 2;
        assert_eq!(ctrl.resolve_upcoming_track(position, &source), None);

        ctrl.queue.current_index = None;
        let first_source = ctrl.queue.items[0].album.tracks[0].audio_source();
        ctrl.remove_upcoming_track(0).unwrap();
        assert_eq!(
            ctrl.resolve_upcoming_track(QueueTrackPosition { item: 0, track: 0 }, &first_source),
            None
        );
        assert_eq!(ctrl.resolve_upcoming_track(position, &source), None);
    }

    #[test]
    fn undo_last_stopped_track_restores_valid_selection_without_playback() {
        let mut ctrl = QueueController::new();
        ctrl.add_album(next_test_album("A", 1)).unwrap();
        let expected = upcoming_sources(&ctrl);
        let removed = ctrl.remove_upcoming_track(0).unwrap();
        assert!(ctrl.queue.items.is_empty());
        assert!(ctrl.undo_upcoming_track_removal(removed));
        assert_eq!(ctrl.selected_index, 0);
        assert_eq!(ctrl.queue.current_index, None);
        assert_eq!(upcoming_sources(&ctrl), expected);
        assert!(matches!(
            ctrl.play_upcoming_track(0),
            QueuePlaybackEffect::Play(_)
        ));
        assert_eq!(ctrl.queue.current_index, Some(0));
    }

    fn upcoming_sources(ctrl: &QueueController) -> Vec<(String, AudioSource)> {
        ctrl.upcoming_track_positions()
            .iter()
            .map(|position| {
                let album = &ctrl.queue.items[position.item].album;
                (
                    album.title.clone(),
                    album.tracks[position.track].audio_source(),
                )
            })
            .collect()
    }

    #[test]
    fn moving_upcoming_tracks_preserves_current_and_album_identity() {
        let mut ctrl = QueueController::new();
        let a = next_test_album("A", 3);
        let b = next_test_album("B", 2);
        let a_tail = a.tracks[2].audio_source();
        let b_first = b.tracks[0].audio_source();
        let b_last = b.tracks[1].audio_source();
        ctrl.add_album(a).unwrap();
        ctrl.add_album(b).unwrap();
        ctrl.set_current_index(Some(0));
        ctrl.queue.items[0].current_track_index = 1;
        let playing = ctrl.queue.current_track_source();
        assert!(ctrl.move_upcoming_track(2, 0));
        assert_eq!(
            upcoming_sources(&ctrl),
            vec![
                ("B".into(), b_last.clone()),
                ("A".into(), a_tail.clone()),
                ("B".into(), b_first.clone()),
            ]
        );
        assert_eq!(ctrl.queue.current_track_source(), playing);
        assert_eq!(ctrl.current_index(), Some(0));
        assert_eq!(ctrl.queue.items[0].current_track_index, 1);
        assert!(ctrl.move_upcoming_track(0, 2));
        assert_eq!(
            upcoming_sources(&ctrl),
            vec![
                ("A".into(), a_tail.clone()),
                ("B".into(), b_first.clone()),
                ("B".into(), b_last.clone()),
            ]
        );
        assert_eq!(ctrl.queue.current_track_source(), playing);
        assert_eq!(ctrl.next_track(), QueuePlaybackEffect::Play(a_tail));
        assert_eq!(ctrl.next_track(), QueuePlaybackEffect::Play(b_first));
        assert_eq!(ctrl.next_track(), QueuePlaybackEffect::Play(b_last));
    }

    #[test]
    fn removing_upcoming_tracks_never_removes_current_or_history() {
        let mut ctrl = QueueController::new();
        ctrl.add_album(next_test_album("History", 1)).unwrap();
        ctrl.add_album(next_test_album("Current", 3)).unwrap();
        ctrl.add_album(next_test_album("Later", 1)).unwrap();
        ctrl.set_current_index(Some(1));
        ctrl.queue.items[1].current_track_index = 1;
        let playing = ctrl.queue.current_track_source();
        assert_eq!(ctrl.upcoming_track_positions().len(), 2);
        assert!(ctrl.remove_upcoming_track(0).is_some());
        assert_eq!(ctrl.queue.items[1].album.tracks.len(), 2);
        assert!(ctrl.remove_upcoming_track(0).is_some());
        assert!(ctrl.remove_upcoming_track(0).is_none());
        assert_eq!(ctrl.queue.items.len(), 2);
        assert_eq!(ctrl.queue.items[0].album.title, "History");
        assert_eq!(ctrl.queue.current_track_source(), playing);
        assert_eq!(ctrl.current_index(), Some(1));
        assert_eq!(ctrl.queue.items[1].current_track_index, 1);
    }

    #[test]
    fn undo_upcoming_removal_preserves_playback_that_advanced() {
        let mut ctrl = QueueController::new();
        ctrl.add_album(next_test_album("A", 4)).unwrap();
        ctrl.add_album(next_test_album("B", 1)).unwrap();
        ctrl.add_album(next_test_album("C", 1)).unwrap();
        ctrl.start();
        let removed = ctrl.remove_upcoming_track(0).unwrap();
        ctrl.next_track();
        let playing = ctrl.current_track_source();
        assert!(ctrl.undo_upcoming_track_removal(removed));
        assert_eq!(ctrl.current_track_source(), playing);
        assert_eq!(ctrl.queue.items[0].current_track_index, 2);
        // B is a whole queue item; undo after crossing its old position must
        // shift the current item index, never rewind C or restart audio.
        let removed_b = ctrl.remove_upcoming_track(1).unwrap();
        ctrl.next_track();
        ctrl.next_track();
        let playing_c = ctrl.current_track_source();
        assert_eq!(ctrl.current_index(), Some(1));
        assert!(ctrl.undo_upcoming_track_removal(removed_b));
        assert_eq!(ctrl.current_index(), Some(2));
        assert_eq!(ctrl.current_track_source(), playing_c);
    }

    #[test]
    fn stopped_upcoming_edits_do_not_start_playback_and_reject_invalid_indices() {
        let mut ctrl = QueueController::new();
        ctrl.add_album(next_test_album("A", 3)).unwrap();
        let before = upcoming_sources(&ctrl);
        assert!(!ctrl.move_upcoming_track(0, 3));
        assert!(!ctrl.move_upcoming_track(3, 0));
        assert!(!ctrl.move_upcoming_track(0, 0));
        assert!(ctrl.remove_upcoming_track(3).is_none());
        assert_eq!(upcoming_sources(&ctrl), before);
        assert!(ctrl.move_upcoming_track(0, 2));
        assert_eq!(
            upcoming_sources(&ctrl),
            vec![before[1].clone(), before[2].clone(), before[0].clone()]
        );
        assert_eq!(ctrl.current_index(), None);
        ctrl.queue.current_index = Some(99);
        assert!(ctrl.upcoming_track_positions().is_empty());
        assert!(ctrl.remove_upcoming_track(0).is_none());
        assert!(!ctrl.move_upcoming_track(0, 1));
    }

    #[test]
    fn enqueue_next_preserves_current_track_and_album_remainder() {
        let mut ctrl = QueueController::new();
        ctrl.add_album(next_test_album("A", 3)).unwrap();
        ctrl.add_album(next_test_album("Later", 1)).unwrap();
        ctrl.start();
        ctrl.next_track();
        ctrl.selected_index = 1;
        let playing = ctrl.current_track_source();
        assert_eq!(ctrl.enqueue_next(next_test_album("Next", 2)).unwrap(), 1);
        assert_eq!(ctrl.current_track_source(), playing);
        assert_eq!(ctrl.current_index(), Some(0));
        assert_eq!(ctrl.selected_index, 3);
        assert_eq!(ctrl[0].album.tracks.len(), 2);
        assert_eq!(ctrl[2].album.title, "A");
        assert_eq!(ctrl[2].album.tracks[0].title.as_deref(), Some("Track 3"));
        assert_eq!(ctrl[3].album.title, "Later");
        for title in ["Next", "Next", "A", "Later"] {
            assert!(matches!(ctrl.next_track(), QueuePlaybackEffect::Play(_)));
            assert_eq!(ctrl[ctrl.current_index().unwrap()].album.title, title);
        }
    }

    #[test]
    fn enqueue_next_without_playback_inserts_first_without_starting() {
        let mut ctrl = QueueController::new();
        assert_eq!(ctrl.enqueue_next(next_test_album("First", 1)).unwrap(), 0);
        assert_eq!(ctrl.current_index(), None);
        assert_eq!(ctrl.selected_index, 0);
        ctrl.enqueue_next(next_test_album("New", 1)).unwrap();
        assert_eq!(ctrl.current_index(), None);
        assert_eq!(ctrl.selected_index, 1);
        assert_eq!(ctrl[0].album.title, "New");
        assert_eq!(ctrl[1].album.title, "First");
    }

    #[test]
    fn enqueue_next_rejection_does_not_split_current_album() {
        let mut ctrl = QueueController::new();
        ctrl.add_album(next_test_album("A", 3)).unwrap();
        ctrl.start();
        let source = ctrl.current_track_source();
        assert!(ctrl.enqueue_next(Album::default()).is_err());
        assert_eq!(ctrl.len(), 1);
        assert_eq!(ctrl[0].album.tracks.len(), 3);
        assert_eq!(ctrl.current_track_source(), source);
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
    fn test_play_album_now_appends_without_displacing_existing_queue() {
        let mut ctrl = QueueController::new();
        add_test_album(&mut ctrl, next_test_album("A", 2));
        add_test_album(&mut ctrl, next_test_album("B", 2));
        ctrl.start();
        assert_eq!(ctrl.current_index(), Some(0));

        let effect = ctrl
            .play_album_now(next_test_album("C", 3))
            .expect("play now accepts a valid album");
        assert!(matches!(effect, QueuePlaybackEffect::Play(_)));

        // Append-and-jump policy: existing items keep identity and order,
        // so play-now displaces nothing and needs no snapshot recovery.
        let titles: Vec<_> = ctrl.iter().map(|item| item.album.title.as_str()).collect();
        assert_eq!(titles, ["A", "B", "C"]);
        assert_eq!(ctrl.current_index(), Some(2));
    }

    #[test]
    fn test_play_album_now_rejects_empty_album() {
        let mut ctrl = QueueController::new();
        let result = ctrl.play_album_now(make_album("Empty", 0));
        assert_eq!(result, Err("Album has no tracks".to_string()));
        assert!(ctrl.is_empty());
        assert_eq!(ctrl.current_index, None);
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
    #[cfg(not(feature = "testing"))]
    fn test_add_album_accepts_url_sources_without_files() {
        let mut ctrl = QueueController::new();
        let stream = AudioSource::Url {
            url: "https://example.com/live.m3u8".to_string(),
            format_hint: Some("hls".to_string()),
        };
        let album = Album {
            title: "Live Stream".to_string(),
            tracks: vec![Track {
                path: PathBuf::from("https://example.com/live.m3u8"),
                source: Some(stream.clone()),
                title: Some("Live".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = ctrl.add_album(album);
        assert_eq!(result, Ok(0));
        assert_eq!(ctrl.start(), QueuePlaybackEffect::Play(stream));
    }

    #[test]
    fn enqueue_playlist_tracks_preserves_order_and_skips_duplicates() {
        let mut album = make_album("Playlist", 2);
        for (index, track) in album.tracks.iter_mut().enumerate() {
            track.source = Some(AudioSource::Url {
                url: format!("https://example.com/{index}.flac"),
                format_hint: None,
                seekable: true,
            });
        }
        let paths = album
            .tracks
            .iter()
            .map(|track| track.path.clone())
            .collect::<Vec<_>>();
        let mut library = MusicLibrary::new();
        library.albums = vec![album];
        let mut controller = QueueController::new();

        let first = controller.enqueue_playlist_tracks(&library, &paths);
        assert_eq!(first.added, 2);
        assert_eq!(first.skipped_existing, 0);
        assert_eq!(first.skipped_missing, 0);
        assert_eq!(first.first_added_index, Some(0));
        assert_eq!(controller.len(), 2);
        assert_eq!(controller[0].album.tracks[0].path, paths[0]);
        assert_eq!(controller[1].album.tracks[0].path, paths[1]);

        let second = controller.enqueue_playlist_tracks(&library, &paths);
        assert_eq!(second.added, 0);
        assert_eq!(second.skipped_existing, 2);
        assert_eq!(controller.len(), 2);
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

    #[test]
    fn move_item_keeps_selected_and_playing_album_identity() {
        let mut ctrl = QueueController::new();
        add_test_album(&mut ctrl, make_album("A", 1));
        add_test_album(&mut ctrl, make_album("B", 1));
        add_test_album(&mut ctrl, make_album("C", 1));
        ctrl.selected_index = 1;
        ctrl.jump_to(1);

        assert!(ctrl.move_item(1, 2));
        assert_eq!(ctrl.selected_index, 2);
        assert_eq!(ctrl.current_index(), Some(2));
        assert_eq!(ctrl[2].album.title, "B");
    }
}
