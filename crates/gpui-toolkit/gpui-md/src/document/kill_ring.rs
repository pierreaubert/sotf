//! Emacs-style kill ring for the editor.
//!
//! Stores killed (cut) text in a ring buffer. Supports:
//! - Consecutive kills appending to the same entry
//! - Yank (paste most recent) and yank-pop (cycle through ring)
//! - Separate rectangle buffer for column operations

use std::collections::VecDeque;

const MAX_RING_SIZE: usize = 60;

pub struct KillRing {
    ring: VecDeque<String>,
    /// Index for yank-pop cycling (0 = most recent).
    yank_index: usize,
    /// Whether the last operation was a kill (for consecutive-kill appending).
    last_was_kill: bool,
    /// Whether the last operation was a yank (for yank-pop to work).
    last_was_yank: bool,
    /// The char range replaced by the last yank (for yank-pop replacement).
    last_yank_range: Option<(usize, usize)>,
    /// Separate buffer for rectangle (column) operations.
    pub rectangle_buffer: Option<Vec<String>>,
}

impl KillRing {
    pub fn new() -> Self {
        Self {
            ring: VecDeque::new(),
            yank_index: 0,
            last_was_kill: false,
            last_was_yank: false,
            last_yank_range: None,
            rectangle_buffer: None,
        }
    }

    /// Push killed text onto the ring.
    /// If the last operation was also a kill, append to the top entry instead.
    pub fn push(&mut self, text: String) {
        if self.last_was_kill && !self.ring.is_empty() {
            // Append to the most recent entry (consecutive kill)
            self.ring.back_mut().unwrap().push_str(&text);
        } else {
            self.ring.push_back(text);
            if self.ring.len() > MAX_RING_SIZE {
                self.ring.pop_front();
            }
        }
        self.yank_index = 0;
        self.last_was_kill = true;
        self.last_was_yank = false;
        self.last_yank_range = None;
    }

    /// Push killed text that should be prepended (for backward kills).
    pub fn push_prepend(&mut self, text: String) {
        if self.last_was_kill && !self.ring.is_empty() {
            let existing = self.ring.back_mut().unwrap();
            let mut new = text;
            new.push_str(existing);
            *existing = new;
        } else {
            self.ring.push_back(text);
            if self.ring.len() > MAX_RING_SIZE {
                self.ring.pop_front();
            }
        }
        self.yank_index = 0;
        self.last_was_kill = true;
        self.last_was_yank = false;
        self.last_yank_range = None;
    }

    /// Get the most recent kill ring entry for yanking.
    pub fn yank(&self) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let idx = self.ring.len() - 1 - self.yank_index;
        self.ring.get(idx).map(|s| s.as_str())
    }

    /// Cycle to the previous entry in the ring (for yank-pop / Alt+Y).
    /// Returns the new entry text.
    pub fn yank_pop(&mut self) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        self.yank_index = (self.yank_index + 1) % self.ring.len();
        let idx = self.ring.len() - 1 - self.yank_index;
        self.ring.get(idx).map(|s| s.as_str())
    }

    /// Mark that a yank was performed, storing the inserted range for yank-pop.
    pub fn mark_yank(&mut self, start: usize, end: usize) {
        self.last_was_yank = true;
        self.last_was_kill = false;
        self.last_yank_range = Some((start, end));
    }

    /// Get the range of the last yank (for yank-pop replacement).
    pub fn last_yank_range(&self) -> Option<(usize, usize)> {
        if self.last_was_yank {
            self.last_yank_range
        } else {
            None
        }
    }

    /// Reset the consecutive-kill flag (call on any non-kill operation).
    pub fn clear_kill_flag(&mut self) {
        self.last_was_kill = false;
    }

    /// Reset the yank flag (call on any non-yank operation).
    pub fn clear_yank_flag(&mut self) {
        self.last_was_yank = false;
        self.last_yank_range = None;
    }

    /// Reset both flags (call on any operation that is neither kill nor yank).
    pub fn reset_flags(&mut self) {
        self.last_was_kill = false;
        self.last_was_yank = false;
        self.last_yank_range = None;
    }

    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }
}

impl Default for KillRing {
    fn default() -> Self {
        Self::new()
    }
}
