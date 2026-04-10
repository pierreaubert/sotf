//! Buffer list management for multi-file editing.
//!
//! `MdAppState` keeps the currently active buffer's data as live fields
//! (`document`, `cursor`, `history`). Inactive buffers are stored in
//! `Vec<StoredBuffer>`. Switching buffers swaps the active fields with
//! a stored entry — this keeps the rest of the codebase unchanged.

use std::path::{Path, PathBuf};

use super::{DocumentBuffer, EditHistory, EditorCursor};

/// Stable identifier for a buffer, monotonically increasing.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BufferId(pub u64);

/// What kind of content a buffer holds. Determines key handling and rendering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BufferKind {
    /// Regular text editor buffer backed by a file or scratch content.
    File,
    /// The built-in `*scratch*` buffer.
    Scratch,
    /// A directory listing (dired). Details stored in `MdAppState::dired`
    /// keyed by buffer id (kept external to avoid cloning DiredState on swap).
    Dired,
    /// `*Buffer List*` — read-only listing of all buffers.
    BufferList,
}

impl BufferKind {
    pub fn is_special(&self) -> bool {
        !matches!(self, BufferKind::File | BufferKind::Scratch)
    }
}

/// A buffer that is not currently active. The active buffer's data lives
/// directly on `MdAppState` as `document`/`cursor`/`history`.
pub struct StoredBuffer {
    pub id: BufferId,
    pub name: String,
    pub document: DocumentBuffer,
    pub history: EditHistory,
    pub cursor: EditorCursor,
    pub kind: BufferKind,
    pub read_only: bool,
    pub scroll_line: usize,
    pub last_edit_was_char_insert: bool,
    pub last_move_was_vertical: bool,
}

impl StoredBuffer {
    pub fn new_scratch(id: BufferId) -> Self {
        Self {
            id,
            name: "*scratch*".to_string(),
            document: DocumentBuffer::from_text(""),
            history: EditHistory::default(),
            cursor: EditorCursor::new(),
            kind: BufferKind::Scratch,
            read_only: false,
            scroll_line: 0,
            last_edit_was_char_insert: false,
            last_move_was_vertical: false,
        }
    }

    pub fn new_file(id: BufferId, name: String, path: PathBuf, content: &str) -> Self {
        Self {
            id,
            name,
            document: DocumentBuffer::from_file(path, content),
            history: EditHistory::default(),
            cursor: EditorCursor::new(),
            kind: BufferKind::File,
            read_only: false,
            scroll_line: 0,
            last_edit_was_char_insert: false,
            last_move_was_vertical: false,
        }
    }

    pub fn new_buffer_list(id: BufferId) -> Self {
        Self {
            id,
            name: "*Buffer List*".to_string(),
            document: DocumentBuffer::from_text(""),
            history: EditHistory::default(),
            cursor: EditorCursor::new(),
            kind: BufferKind::BufferList,
            read_only: true,
            scroll_line: 0,
            last_edit_was_char_insert: false,
            last_move_was_vertical: false,
        }
    }
}

/// Derive a display name from a file path, falling back to "untitled".
pub fn name_from_path(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".to_string())
}

/// Given a list of existing names, return `base` or `base<N>` that is unique.
pub fn unique_name(base: &str, existing: &[&str]) -> String {
    if !existing.contains(&base) {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let candidate = format!("{}<{}>", base, n);
        if !existing.iter().any(|x| *x == candidate) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_name_no_collision() {
        let existing = vec!["foo.md"];
        assert_eq!(unique_name("bar.md", &existing), "bar.md");
    }

    #[test]
    fn unique_name_with_collision() {
        let existing = vec!["foo.md"];
        assert_eq!(unique_name("foo.md", &existing), "foo.md<2>");
    }

    #[test]
    fn unique_name_multiple_collisions() {
        let existing = vec!["foo.md", "foo.md<2>"];
        assert_eq!(unique_name("foo.md", &existing), "foo.md<3>");
    }

    #[test]
    fn name_from_path_basic() {
        let p = PathBuf::from("/tmp/foo.md");
        assert_eq!(name_from_path(&p), "foo.md");
    }
}
