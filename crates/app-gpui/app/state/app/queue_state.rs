use sotf_audio_player::{QueueController, QueuePlaybackEffect};
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

/// Queue state — wraps `QueueController` with per-item UI expansion tracking.
///
/// Deref/DerefMut to `QueueController` so `.len()`, `.iter()`, `.current_index`,
/// `.peek_next_track()`, etc. work transparently. Mutations that change item count
/// (add, remove, clear, fill_magic) are shadowed to keep `expanded` in sync.
#[derive(Debug, Clone)]
struct ClearedQueueSnapshot {
    ctrl: QueueController,
    expanded: Vec<bool>,
    selected_index: usize,
}

/// Exact state before the most recent queue-item removal. Keeping a complete
/// controller snapshot preserves the current album/track selection when an
/// item before the playing item is restored.
#[derive(Debug, Clone)]
struct RemovedQueueSnapshot {
    ctrl: QueueController,
    expanded: Vec<bool>,
    selected_index: usize,
    removed_was_current: bool,
}

#[derive(Debug)]
pub struct QueueState {
    pub(super) ctrl: QueueController,
    /// Per-queue-item UI expansion state (true = expanded to show tracks)
    pub expanded: Vec<bool>,
    /// Currently selected queue item index in the UI
    pub selected_index: usize,
    last_cleared: Option<ClearedQueueSnapshot>,
    last_removed: Option<RemovedQueueSnapshot>,
}

impl Deref for QueueState {
    type Target = QueueController;
    fn deref(&self) -> &Self::Target {
        &self.ctrl
    }
}

impl DerefMut for QueueState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ctrl
    }
}

impl Default for QueueState {
    fn default() -> Self {
        Self::new()
    }
}

impl QueueState {
    pub fn new() -> Self {
        Self {
            ctrl: QueueController::new(),
            expanded: Vec::new(),
            selected_index: 0,
            last_cleared: None,
            last_removed: None,
        }
    }

    /// A later mutation makes a previously offered undo ambiguous: applying it
    /// would discard the newer user action. Only the latest destructive action
    /// remains recoverable.
    fn invalidate_undo(&mut self) {
        self.last_cleared = None;
        self.last_removed = None;
    }

    /// Add an album to the queue, tracking its expansion state.
    pub fn add_album(&mut self, album: sotf_audio_player::Album) -> Result<usize, String> {
        self.invalidate_undo();
        let idx = self.ctrl.add_album(album)?;
        self.expanded.push(false);
        Ok(idx)
    }

    /// Add album and immediately jump to it for playback.
    pub fn play_album_now(
        &mut self,
        album: sotf_audio_player::Album,
    ) -> Result<QueuePlaybackEffect, String> {
        self.invalidate_undo();
        let effect = self.ctrl.play_album_now(album)?;
        self.expanded.push(false);
        Ok(effect)
    }

    /// Append playlist tracks while keeping UI expansion state aligned with
    /// the shared queue controller.
    pub fn enqueue_playlist_tracks(
        &mut self,
        library: &sotf_audio_player::MusicLibrary,
        track_paths: &[PathBuf],
    ) -> sotf_audio_player::PlaylistQueueAppend {
        self.invalidate_undo();
        let outcome = self.ctrl.enqueue_playlist_tracks(library, track_paths);
        self.expanded
            .resize(self.expanded.len() + outcome.added, false);
        outcome
    }

    /// Remove the album at `index`, keeping expansion in sync.
    pub fn remove(&mut self, index: usize) -> (QueuePlaybackEffect, bool) {
        if index >= self.ctrl.len() {
            return (QueuePlaybackEffect::None, false);
        }
        self.last_cleared = None;
        self.last_removed = Some(RemovedQueueSnapshot {
            ctrl: self.ctrl.clone(),
            expanded: self.expanded.clone(),
            selected_index: self.selected_index,
            removed_was_current: self.ctrl.current_index() == Some(index),
        });
        let result = self.ctrl.remove(index);
        if index < self.expanded.len() {
            self.expanded.remove(index);
        } else {
            self.expanded.resize(self.ctrl.len(), false);
        }
        if self.selected_index >= self.ctrl.len() && self.selected_index > 0 {
            self.selected_index = self.ctrl.len() - 1;
        }
        result
    }

    /// Move an album and its corresponding expansion state together. This is
    /// deliberately not undoable through the destructive-action undo slots.
    pub fn move_item(&mut self, from: usize, to: usize) -> bool {
        if !self.ctrl.move_item(from, to) {
            return false;
        }
        let expanded = self.expanded.remove(from);
        self.expanded.insert(to, expanded);
        self.invalidate_undo();
        true
    }

    /// Clear all items from the queue.
    pub fn clear(&mut self) {
        self.last_removed = None;
        self.last_cleared = (!self.ctrl.is_empty()).then(|| ClearedQueueSnapshot {
            ctrl: self.ctrl.clone(),
            expanded: self.expanded.clone(),
            selected_index: self.selected_index,
        });
        self.ctrl.clear();
        self.expanded.clear();
        self.selected_index = 0;
    }

    pub fn can_undo_clear(&self) -> bool {
        self.last_cleared.is_some()
    }

    /// Restore the exact queue state before the most recent clear.
    pub fn undo_clear(&mut self) -> bool {
        let Some(snapshot) = self.last_cleared.take() else {
            return false;
        };
        self.ctrl = snapshot.ctrl;
        self.expanded = snapshot.expanded;
        self.selected_index = snapshot
            .selected_index
            .min(self.ctrl.len().saturating_sub(1));
        true
    }

    pub fn can_undo_remove(&self) -> bool {
        self.last_removed.is_some()
    }

    /// Restore the exact queue state before the most recent item removal.
    /// Returns whether the removed item was the selected playback item, so the
    /// app can reload audio only when continuing playback requires it.
    pub fn undo_remove(&mut self) -> Option<bool> {
        let snapshot = self.last_removed.take()?;
        self.ctrl = snapshot.ctrl;
        self.expanded = snapshot.expanded;
        self.selected_index = snapshot
            .selected_index
            .min(self.ctrl.len().saturating_sub(1));
        Some(snapshot.removed_was_current)
    }

    /// Fill queue with "magic" recommendations.
    pub fn fill_magic(
        &mut self,
        db: &sotf_audio_player::MusicDatabase,
        library_albums: &[sotf_audio_player::Album],
    ) -> Result<Vec<sotf_audio_player::Album>, String> {
        self.invalidate_undo();
        let added = self.ctrl.fill_magic(db, library_albums)?;
        for _ in &added {
            self.expanded.push(false);
        }
        Ok(added)
    }

    /// Toggle expansion of the currently selected queue item.
    pub fn toggle_expansion(&mut self) {
        if self.selected_index < self.expanded.len() {
            self.expanded[self.selected_index] = !self.expanded[self.selected_index];
        }
    }
}
