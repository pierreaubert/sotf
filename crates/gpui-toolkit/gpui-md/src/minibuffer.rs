//! Unified mini-buffer abstraction.
//!
//! Replaces ad-hoc prompt state (command palette, isearch overlays, goto-line)
//! with a single state machine that can drive any kind of modal user input.
//!
//! Design notes:
//! - The mini-buffer lives at the `MdAppState` level. Only one prompt is active
//!   at a time.
//! - Each prompt kind has its own completion candidates and submission semantics.
//! - History is per-prompt-kind, keyed by a discriminant string.
//! - Live prompts (isearch) trigger an `on_input_changed` hook on every edit.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

use crate::state::IsearchDirection;

/// One kind of mini-buffer prompt. Each variant determines the label, the
/// candidate source, and the submission semantics.
#[derive(Clone, Debug)]
pub enum MiniBufferPrompt {
    /// `M-x ` — complete over the command registry (Phase 5 will populate candidates).
    Command,
    /// `I-search: ` — live search, every keystroke updates the cursor.
    Isearch { direction: IsearchDirection },
    /// `Switch to buffer: ` — complete over buffer names.
    SwitchBuffer,
    /// `Kill buffer: ` — complete over buffer names.
    KillBuffer,
    /// `Find file: ` — file-path completion (Phase 3 will make use of this).
    FindFile { base_dir: PathBuf },
    /// `Goto line: ` — numeric input.
    GotoLine,
    /// Generic label — free-text input.
    FreeText { label: String },
    /// Single-key yes/no confirmation.
    YesNo { label: String },
}

impl MiniBufferPrompt {
    /// The label shown at the start of the mini-buffer.
    pub fn label(&self) -> String {
        match self {
            Self::Command => "M-x ".to_string(),
            Self::Isearch { direction } => match direction {
                IsearchDirection::Forward => "I-search: ".to_string(),
                IsearchDirection::Backward => "I-search backward: ".to_string(),
            },
            Self::SwitchBuffer => "Switch to buffer: ".to_string(),
            Self::KillBuffer => "Kill buffer: ".to_string(),
            Self::FindFile { .. } => "Find file: ".to_string(),
            Self::GotoLine => "Goto line: ".to_string(),
            Self::FreeText { label } => format!("{}: ", label),
            Self::YesNo { label } => format!("{} (y or n) ", label),
        }
    }

    /// Whether the prompt should fire an `on_input_changed` callback on every
    /// edit (i.e. live prompts).
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Isearch { .. })
    }

    /// Whether the prompt consumes only y/n/RET/ESC (YesNo prompts).
    pub fn is_yes_no(&self) -> bool {
        matches!(self, Self::YesNo { .. })
    }

    /// A stable key used for history lookup.
    pub fn history_key(&self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Isearch { .. } => "isearch",
            Self::SwitchBuffer => "switch-buffer",
            Self::KillBuffer => "kill-buffer",
            Self::FindFile { .. } => "find-file",
            Self::GotoLine => "goto-line",
            Self::FreeText { .. } => "free-text",
            Self::YesNo { .. } => "yes-no",
        }
    }
}

/// The result of submitting a mini-buffer.
#[derive(Clone, Debug)]
pub enum MiniBufferResult {
    /// User submitted a string (possibly empty).
    Submitted(String),
    /// User cancelled with C-g / Escape.
    Cancelled,
    /// YesNo prompt: user chose yes (true) or no (false).
    YesNoAnswer(bool),
}

/// State for the unified mini-buffer. Only one prompt is active at a time.
#[derive(Default)]
pub struct MiniBufferState {
    pub active: bool,
    pub prompt: Option<MiniBufferPrompt>,
    pub input: String,
    pub cursor_pos: usize, // character position within input
    pub candidates: Vec<String>,
    pub filtered: Vec<usize>, // indices into candidates
    pub selected: usize,      // index into filtered
    /// Per-prompt-kind history keyed by `history_key()`.
    pub history: HashMap<String, VecDeque<String>>,
    pub history_pos: Option<usize>,
}

impl MiniBufferState {
    pub fn open(&mut self, prompt: MiniBufferPrompt, candidates: Vec<String>) {
        self.active = true;
        self.input.clear();
        self.cursor_pos = 0;
        self.filtered = (0..candidates.len()).collect();
        self.candidates = candidates;
        self.selected = 0;
        self.history_pos = None;
        self.prompt = Some(prompt);
    }

    pub fn close(&mut self) {
        self.active = false;
        self.input.clear();
        self.cursor_pos = 0;
        self.candidates.clear();
        self.filtered.clear();
        self.selected = 0;
        self.history_pos = None;
        self.prompt = None;
    }

    pub fn add_char(&mut self, ch: char) {
        // Insert at cursor position, counted in chars.
        let byte_idx = self
            .input
            .char_indices()
            .nth(self.cursor_pos)
            .map(|(b, _)| b)
            .unwrap_or(self.input.len());
        self.input.insert(byte_idx, ch);
        self.cursor_pos += 1;
        self.refilter();
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let start_char = self.cursor_pos - 1;
        let chars: Vec<(usize, char)> = self.input.char_indices().collect();
        if let Some(&(byte_start, _)) = chars.get(start_char) {
            let byte_end = chars
                .get(self.cursor_pos)
                .map(|&(b, _)| b)
                .unwrap_or(self.input.len());
            self.input.drain(byte_start..byte_end);
            self.cursor_pos -= 1;
            self.refilter();
        }
    }

    pub fn delete_forward(&mut self) {
        let total = self.input.chars().count();
        if self.cursor_pos >= total {
            return;
        }
        let chars: Vec<(usize, char)> = self.input.char_indices().collect();
        if let Some(&(byte_start, _)) = chars.get(self.cursor_pos) {
            let byte_end = chars
                .get(self.cursor_pos + 1)
                .map(|&(b, _)| b)
                .unwrap_or(self.input.len());
            self.input.drain(byte_start..byte_end);
            self.refilter();
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    pub fn move_right(&mut self) {
        let total = self.input.chars().count();
        if self.cursor_pos < total {
            self.cursor_pos += 1;
        }
    }

    pub fn move_to_start(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn move_to_end(&mut self) {
        self.cursor_pos = self.input.chars().count();
    }

    pub fn select_next(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1) % self.filtered.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = self
                .selected
                .checked_sub(1)
                .unwrap_or(self.filtered.len() - 1);
        }
    }

    /// Refilter candidates against the current input (substring match).
    pub fn refilter(&mut self) {
        let query = self.input.to_lowercase();
        self.filtered = self
            .candidates
            .iter()
            .enumerate()
            .filter(|(_, cand)| query.is_empty() || cand.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        self.selected = 0;
    }

    /// Extend input to the longest common prefix of the filtered candidates.
    pub fn complete(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        let first = &self.candidates[self.filtered[0]];
        let mut prefix_len = first.len();
        for &idx in &self.filtered[1..] {
            let cand = &self.candidates[idx];
            let common = first
                .chars()
                .zip(cand.chars())
                .take_while(|(a, b)| a.to_lowercase().eq(b.to_lowercase()))
                .count();
            // Convert char count to byte offset
            let bytes = first
                .char_indices()
                .nth(common)
                .map(|(b, _)| b)
                .unwrap_or(first.len());
            if bytes < prefix_len {
                prefix_len = bytes;
            }
        }
        if prefix_len > self.input.len() {
            self.input = first[..prefix_len].to_string();
            self.cursor_pos = self.input.chars().count();
            self.refilter();
        }
    }

    /// Return the selected candidate, or the typed input if none match.
    pub fn current_value(&self) -> String {
        if let Some(&idx) = self.filtered.get(self.selected) {
            if let Some(cand) = self.candidates.get(idx) {
                return cand.clone();
            }
        }
        self.input.clone()
    }

    pub fn push_history(&mut self, value: String) {
        if let Some(prompt) = &self.prompt {
            let key = prompt.history_key().to_string();
            let entry = self.history.entry(key).or_default();
            // Dedupe: don't push if same as last entry
            if entry.back().map(|s| s.as_str()) == Some(value.as_str()) {
                return;
            }
            entry.push_back(value);
            while entry.len() > 50 {
                entry.pop_front();
            }
        }
    }

    pub fn history_prev(&mut self) {
        let Some(prompt) = &self.prompt else { return };
        let Some(entries) = self.history.get(prompt.history_key()) else {
            return;
        };
        if entries.is_empty() {
            return;
        }
        let new_pos = match self.history_pos {
            None => entries.len() - 1,
            Some(0) => 0,
            Some(n) => n - 1,
        };
        self.history_pos = Some(new_pos);
        if let Some(value) = entries.get(new_pos) {
            self.input = value.clone();
            self.cursor_pos = self.input.chars().count();
            self.refilter();
        }
    }

    pub fn history_next(&mut self) {
        let Some(prompt) = &self.prompt else { return };
        let Some(entries) = self.history.get(prompt.history_key()) else {
            return;
        };
        match self.history_pos {
            None => {}
            Some(n) if n + 1 >= entries.len() => {
                self.history_pos = None;
                self.input.clear();
                self.cursor_pos = 0;
                self.refilter();
            }
            Some(n) => {
                let new_pos = n + 1;
                self.history_pos = Some(new_pos);
                if let Some(value) = entries.get(new_pos) {
                    self.input = value.clone();
                    self.cursor_pos = self.input.chars().count();
                    self.refilter();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_sets_prompt_and_candidates() {
        let mut mb = MiniBufferState::default();
        mb.open(
            MiniBufferPrompt::SwitchBuffer,
            vec!["foo".to_string(), "bar".to_string()],
        );
        assert!(mb.active);
        assert_eq!(mb.candidates.len(), 2);
        assert_eq!(mb.filtered.len(), 2);
    }

    #[test]
    fn add_char_filters_candidates() {
        let mut mb = MiniBufferState::default();
        mb.open(
            MiniBufferPrompt::Command,
            vec![
                "forward-char".to_string(),
                "forward-word".to_string(),
                "backward-char".to_string(),
            ],
        );
        // Substring filter: "for" matches both "forward-*" entries
        mb.add_char('f');
        assert_eq!(mb.filtered.len(), 2);
        mb.add_char('o');
        assert_eq!(mb.filtered.len(), 2);
        mb.add_char('r');
        assert_eq!(mb.filtered.len(), 2);
        // "forw" still matches both
        mb.add_char('w');
        assert_eq!(mb.filtered.len(), 2);
        // "forwa" still matches both
        mb.add_char('a');
        assert_eq!(mb.filtered.len(), 2);
        // "forward-w" only matches forward-word
        mb.add_char('r');
        mb.add_char('d');
        mb.add_char('-');
        mb.add_char('w');
        assert_eq!(mb.filtered.len(), 1);
    }

    #[test]
    fn backspace_widens_filter() {
        let mut mb = MiniBufferState::default();
        mb.open(
            MiniBufferPrompt::Command,
            vec!["abc".to_string(), "abd".to_string(), "xyz".to_string()],
        );
        mb.add_char('a');
        assert_eq!(mb.filtered.len(), 2);
        mb.backspace();
        assert_eq!(mb.filtered.len(), 3);
    }

    #[test]
    fn complete_extends_to_common_prefix() {
        let mut mb = MiniBufferState::default();
        mb.open(
            MiniBufferPrompt::Command,
            vec![
                "forward-char".to_string(),
                "forward-word".to_string(),
                "forward-line".to_string(),
            ],
        );
        mb.add_char('f');
        mb.complete();
        assert_eq!(mb.input, "forward-");
    }

    #[test]
    fn select_next_wraps() {
        let mut mb = MiniBufferState::default();
        mb.open(
            MiniBufferPrompt::Command,
            vec!["a".to_string(), "b".to_string()],
        );
        assert_eq!(mb.selected, 0);
        mb.select_next();
        assert_eq!(mb.selected, 1);
        mb.select_next();
        assert_eq!(mb.selected, 0);
    }

    #[test]
    fn close_resets_state() {
        let mut mb = MiniBufferState::default();
        mb.open(
            MiniBufferPrompt::Command,
            vec!["foo".to_string()],
        );
        mb.add_char('f');
        mb.close();
        assert!(!mb.active);
        assert!(mb.input.is_empty());
        assert!(mb.candidates.is_empty());
    }

    #[test]
    fn history_prev_and_next() {
        let mut mb = MiniBufferState::default();
        mb.open(MiniBufferPrompt::Command, vec![]);
        mb.push_history("first".to_string());
        mb.push_history("second".to_string());
        mb.push_history("third".to_string());

        mb.history_prev();
        assert_eq!(mb.input, "third");
        mb.history_prev();
        assert_eq!(mb.input, "second");
        mb.history_next();
        assert_eq!(mb.input, "third");
        mb.history_next();
        assert_eq!(mb.input, ""); // past end
    }

    #[test]
    fn current_value_returns_selected_or_typed() {
        let mut mb = MiniBufferState::default();
        mb.open(
            MiniBufferPrompt::SwitchBuffer,
            vec!["foo.md".to_string(), "bar.md".to_string()],
        );
        assert_eq!(mb.current_value(), "foo.md");
        mb.select_next();
        assert_eq!(mb.current_value(), "bar.md");
        // Typed input with no match
        mb.close();
        mb.open(MiniBufferPrompt::SwitchBuffer, vec![]);
        mb.add_char('x');
        assert_eq!(mb.current_value(), "x");
    }
}
