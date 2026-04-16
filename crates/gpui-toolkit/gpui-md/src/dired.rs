//! Dired — directory browser as a buffer kind.
//!
//! A dired buffer displays a directory listing. It is fundamentally a
//! read-only buffer whose text content is regenerated from `DiredState`
//! whenever the directory is refreshed. Navigation is done via `n`/`p`
//! keys, `RET` to open, `d` to mark for deletion, `x` to execute marks,
//! `g` to refresh, `^` to go up.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// One entry in a dired listing.
#[derive(Clone, Debug)]
pub struct DiredEntry {
    pub name: String,
    pub path: PathBuf,
    pub kind: DiredEntryKind,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub read_only: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiredEntryKind {
    File,
    Directory,
    Symlink,
}

/// A mark on a dired entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiredMark {
    Delete,
}

/// State for a dired buffer.
#[derive(Clone, Debug)]
pub struct DiredState {
    pub path: PathBuf,
    pub entries: Vec<DiredEntry>,
    pub marks: HashMap<usize, DiredMark>,
    /// Index of the currently highlighted entry.
    pub selected: usize,
}

impl DiredState {
    /// Create a new dired state by reading the given directory.
    pub fn read_dir(path: &Path) -> std::io::Result<Self> {
        let entries = read_entries(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            entries,
            marks: HashMap::new(),
            selected: 0,
        })
    }

    /// Re-read the directory, preserving selection when possible.
    pub fn refresh(&mut self) -> std::io::Result<()> {
        let prev_name = self.entries.get(self.selected).map(|e| e.name.clone());
        self.entries = read_entries(&self.path)?;
        self.marks.clear();
        // Try to restore selection by name; fall back to clamping to length
        if let Some(name) = prev_name
            && let Some(idx) = self.entries.iter().position(|e| e.name == name)
        {
            self.selected = idx;
            return Ok(());
        }
        if self.selected >= self.entries.len() && !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
        }
        Ok(())
    }

    /// Render the dired listing as a text buffer for display in the editor.
    pub fn render_to_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("  {}:\n", self.path.display()));
        out.push_str(&format!("  total {}\n", self.entries.len()));
        for (idx, entry) in self.entries.iter().enumerate() {
            let mark = self.marks.get(&idx).map(|_| 'D').unwrap_or(' ');
            let type_char = match entry.kind {
                DiredEntryKind::Directory => 'd',
                DiredEntryKind::Symlink => 'l',
                DiredEntryKind::File => '-',
            };
            let size = format_size(entry.size);
            let name = match entry.kind {
                DiredEntryKind::Directory => format!("{}/", entry.name),
                _ => entry.name.clone(),
            };
            out.push_str(&format!("{} {} {:>10}  {}\n", mark, type_char, size, name));
        }
        out
    }

    /// Move selection down.
    pub fn move_down(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
        }
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Toggle the delete mark on the currently selected entry.
    pub fn mark_delete(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let idx = self.selected;
        if self.marks.remove(&idx).is_none() {
            self.marks.insert(idx, DiredMark::Delete);
        }
        // Auto-advance like Emacs
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
        }
    }

    /// Remove the mark on the currently selected entry.
    pub fn unmark(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.marks.remove(&self.selected);
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
        }
    }

    /// The entry currently under point, or None.
    pub fn current_entry(&self) -> Option<&DiredEntry> {
        self.entries.get(self.selected)
    }

    /// Paths marked for deletion (in iteration order).
    pub fn marked_paths(&self, mark: DiredMark) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for (idx, m) in &self.marks {
            if *m == mark
                && let Some(entry) = self.entries.get(*idx)
            {
                out.push(entry.path.clone());
            }
        }
        out
    }

    /// Parent directory, if any.
    pub fn parent(&self) -> Option<PathBuf> {
        self.path.parent().map(|p| p.to_path_buf())
    }

    /// The line index where the cursor should be placed for the current
    /// selection. First two lines are header (path + total).
    pub fn cursor_line(&self) -> usize {
        self.selected + 2
    }
}

fn read_entries(path: &Path) -> std::io::Result<Vec<DiredEntry>> {
    let mut entries = Vec::new();
    let rd = fs::read_dir(path)?;
    // Always include ".." (unless at root)
    if path.parent().is_some() {
        entries.push(DiredEntry {
            name: "..".to_string(),
            path: path.parent().unwrap().to_path_buf(),
            kind: DiredEntryKind::Directory,
            size: 0,
            modified: None,
            read_only: true,
        });
    }
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let full_path = entry.path();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let symlink_meta = fs::symlink_metadata(&full_path).ok();
        let is_symlink = symlink_meta
            .as_ref()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        let kind = if is_symlink {
            DiredEntryKind::Symlink
        } else if meta.is_dir() {
            DiredEntryKind::Directory
        } else {
            DiredEntryKind::File
        };
        entries.push(DiredEntry {
            name,
            path: full_path,
            kind,
            size: meta.len(),
            modified: meta.modified().ok(),
            read_only: meta.permissions().readonly(),
        });
    }
    // Sort: directories first, then by name
    entries.sort_by(|a, b| match (a.kind, b.kind) {
        (DiredEntryKind::Directory, DiredEntryKind::Directory) => a.name.cmp(&b.name),
        (DiredEntryKind::Directory, _) => std::cmp::Ordering::Less,
        (_, DiredEntryKind::Directory) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });
    // Keep ".." at the top
    if let Some(idx) = entries.iter().position(|e| e.name == "..")
        && idx != 0
    {
        let dd = entries.remove(idx);
        entries.insert(0, dd);
    }
    Ok(entries)
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} K", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} M", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} G", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_dir_works_on_tmp() {
        let tmp = std::env::temp_dir();
        let state = DiredState::read_dir(&tmp);
        assert!(state.is_ok());
    }

    #[test]
    fn render_to_text_has_header() {
        let tmp = std::env::temp_dir();
        let state = DiredState::read_dir(&tmp).unwrap();
        let text = state.render_to_text();
        assert!(text.contains(&format!("{}:", tmp.display())));
        assert!(text.contains("total"));
    }

    #[test]
    fn mark_delete_toggles() {
        let tmp = std::env::temp_dir();
        let mut state = DiredState::read_dir(&tmp).unwrap();
        if state.entries.is_empty() {
            return;
        }
        state.selected = 0;
        state.mark_delete();
        assert!(state.marks.contains_key(&0));
        // Selection auto-advanced
        if state.entries.len() > 1 {
            assert_eq!(state.selected, 1);
        }
    }

    #[test]
    fn format_size_units() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.0 K");
        assert_eq!(format_size(1024 * 1024), "1.0 M");
    }
}
