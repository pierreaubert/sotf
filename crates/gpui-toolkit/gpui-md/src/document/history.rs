use std::collections::VecDeque;

use ropey::Rope;

/// Undo/redo history using rope snapshots with cursor position tracking.
///
/// Uses clone-on-write semantics from ropey's Rope for efficient snapshots.
/// Each entry stores both the document state and cursor position at that point.
pub struct EditHistory {
    undo_stack: VecDeque<(Rope, usize)>,
    redo_stack: VecDeque<(Rope, usize)>,
    max_history: usize,
}

impl EditHistory {
    pub fn new(max_history: usize) -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            max_history,
        }
    }

    /// Save a snapshot before making an edit, including the cursor position.
    pub fn push_undo(&mut self, snapshot: Rope, cursor_pos: usize) {
        self.redo_stack.clear();
        self.undo_stack.push_back((snapshot, cursor_pos));
        if self.undo_stack.len() > self.max_history {
            self.undo_stack.pop_front();
        }
    }

    /// Pop the most recent undo snapshot, pushing the current state to redo.
    /// Returns the restored (document, cursor_position).
    pub fn undo(&mut self, current: Rope, current_cursor: usize) -> Option<(Rope, usize)> {
        let entry = self.undo_stack.pop_back()?;
        self.redo_stack.push_back((current, current_cursor));
        if self.redo_stack.len() > self.max_history {
            self.redo_stack.pop_front();
        }
        Some(entry)
    }

    /// Pop the most recent redo snapshot, pushing the current state to undo.
    /// Returns the restored (document, cursor_position).
    pub fn redo(&mut self, current: Rope, current_cursor: usize) -> Option<(Rope, usize)> {
        let entry = self.redo_stack.pop_back()?;
        self.undo_stack.push_back((current, current_cursor));
        Some(entry)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

impl Default for EditHistory {
    fn default() -> Self {
        Self::new(100)
    }
}
