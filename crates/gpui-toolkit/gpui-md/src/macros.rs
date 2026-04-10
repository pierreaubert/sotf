//! Keyboard macros — record, store, and replay sequences of editor actions.
//!
//! Design: record dispatched actions, not raw keystrokes. Recording actions
//! is deterministic and matches Emacs semantics. The `RecordedAction` enum
//! enumerates the atomic verbs worth recording; a `Command { name }` escape
//! hatch lets Phase 5's command registry plug in later.

use std::collections::HashMap;

/// One recorded step in a keyboard macro.
#[derive(Clone, Debug)]
pub enum RecordedAction {
    InsertChar(char),
    InsertString(String),
    Newline,
    Backspace,
    DeleteForward,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveWordLeft,
    MoveWordRight,
    MoveLineStart,
    MoveLineEnd,
    MoveDocStart,
    MoveDocEnd,
    KillLine,
    KillLineBackward,
    KillWordForward,
    KillWordBackward,
    KillRegion,
    CopyRegion,
    Yank,
    YankPop,
    Undo,
    Redo,
    SetMark,
    ClearSelection,
    ExchangePointAndMark,
    UpcaseWord,
    DowncaseWord,
    TransposeChars,
    TransposeWords,
    /// Escape hatch — named command from the Phase 5 registry.
    Command { name: String, prefix_arg: Option<usize> },
}

/// A recorded keyboard macro.
#[derive(Clone, Debug, Default)]
pub struct KeyboardMacro {
    pub actions: Vec<RecordedAction>,
}

impl KeyboardMacro {
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
    pub fn len(&self) -> usize {
        self.actions.len()
    }
}

/// State for keyboard macro recording and replay.
#[derive(Debug, Default)]
pub struct MacroState {
    /// True when the user has started recording with C-x ( but not yet stopped.
    pub recording: bool,
    /// True while a macro is being replayed. Prevents re-entrant recording.
    pub applying: bool,
    /// Actions in the in-progress recording.
    pub current: Vec<RecordedAction>,
    /// Last completed macro, used by C-x e.
    pub last: Option<KeyboardMacro>,
    /// Named macros registered by the user (name → macro).
    pub named: HashMap<String, KeyboardMacro>,
}

impl MacroState {
    pub fn start_recording(&mut self) {
        self.recording = true;
        self.current.clear();
    }

    pub fn stop_recording(&mut self) {
        if !self.recording {
            return;
        }
        self.recording = false;
        if !self.current.is_empty() {
            self.last = Some(KeyboardMacro {
                actions: std::mem::take(&mut self.current),
            });
        } else {
            self.current.clear();
        }
    }

    pub fn record(&mut self, action: RecordedAction) {
        if self.recording && !self.applying {
            self.current.push(action);
        }
    }

    pub fn name_last(&mut self, name: String) -> bool {
        if let Some(m) = self.last.clone() {
            self.named.insert(name, m);
            true
        } else {
            false
        }
    }

    pub fn get_named(&self, name: &str) -> Option<&KeyboardMacro> {
        self.named.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_stop_empty_macro() {
        let mut ms = MacroState::default();
        ms.start_recording();
        assert!(ms.recording);
        ms.stop_recording();
        assert!(!ms.recording);
        assert!(ms.last.is_none(), "empty macro should not be stored");
    }

    #[test]
    fn record_and_save_macro() {
        let mut ms = MacroState::default();
        ms.start_recording();
        ms.record(RecordedAction::InsertChar('a'));
        ms.record(RecordedAction::MoveRight);
        ms.stop_recording();
        assert!(ms.last.is_some());
        assert_eq!(ms.last.as_ref().unwrap().actions.len(), 2);
    }

    #[test]
    fn record_no_op_when_not_recording() {
        let mut ms = MacroState::default();
        ms.record(RecordedAction::InsertChar('x'));
        assert!(ms.last.is_none());
        assert!(ms.current.is_empty());
    }

    #[test]
    fn record_no_op_when_applying() {
        let mut ms = MacroState::default();
        ms.start_recording();
        ms.applying = true;
        ms.record(RecordedAction::InsertChar('x'));
        assert!(ms.current.is_empty(), "should not record while applying");
        ms.applying = false;
        ms.record(RecordedAction::InsertChar('y'));
        assert_eq!(ms.current.len(), 1);
    }

    #[test]
    fn name_last_stores_named_macro() {
        let mut ms = MacroState::default();
        ms.start_recording();
        ms.record(RecordedAction::InsertChar('z'));
        ms.stop_recording();
        assert!(ms.name_last("my-macro".to_string()));
        assert!(ms.get_named("my-macro").is_some());
    }
}
