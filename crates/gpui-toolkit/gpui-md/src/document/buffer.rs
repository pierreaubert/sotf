use ropey::Rope;
use std::path::PathBuf;

/// Rope-backed document buffer with file tracking and dirty state.
pub struct DocumentBuffer {
    rope: Rope,
    file_path: Option<PathBuf>,
    dirty: bool,
    version: u64,
}

impl DocumentBuffer {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            file_path: None,
            dirty: false,
            version: 0,
        }
    }

    pub fn from_text(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            file_path: None,
            dirty: false,
            version: 0,
        }
    }

    pub fn from_file(path: PathBuf, content: &str) -> Self {
        Self {
            rope: Rope::from_str(content),
            file_path: Some(path),
            dirty: false,
            version: 0,
        }
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn line(&self, idx: usize) -> ropey::RopeSlice<'_> {
        self.rope.line(idx)
    }

    pub fn char_to_line(&self, char_idx: usize) -> usize {
        self.rope.char_to_line(char_idx)
    }

    pub fn line_to_char(&self, line_idx: usize) -> usize {
        self.rope.line_to_char(line_idx)
    }

    pub fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }

    pub fn set_file_path(&mut self, path: PathBuf) {
        self.file_path = Some(path);
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    /// Insert text at a character offset.
    pub fn insert(&mut self, char_idx: usize, text: &str) {
        self.rope.insert(char_idx, text);
        self.dirty = true;
        self.version += 1;
    }

    /// Remove a range of characters [start..end).
    pub fn remove(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let len = self.rope.len_chars();
        let end = end.min(len);
        let start = start.min(end);
        if start == end {
            return;
        }
        self.rope.remove(start..end);
        self.dirty = true;
        self.version += 1;
    }

    /// Replace text content entirely (e.g., from file load). Marks document clean.
    pub fn set_text(&mut self, text: &str) {
        self.rope = Rope::from_str(text);
        self.dirty = false;
        self.version += 1;
    }

    /// Replace text content as an edit (e.g., replace-all). Marks document dirty.
    pub fn replace_content(&mut self, text: &str) {
        self.rope = Rope::from_str(text);
        self.dirty = true;
        self.version += 1;
    }

    /// Clone the rope for undo snapshots.
    pub fn snapshot(&self) -> Rope {
        self.rope.clone()
    }

    /// Restore from a snapshot.
    pub fn restore(&mut self, snapshot: Rope) {
        self.rope = snapshot;
        self.dirty = true;
        self.version += 1;
    }
}

impl Default for DocumentBuffer {
    fn default() -> Self {
        Self::new()
    }
}
