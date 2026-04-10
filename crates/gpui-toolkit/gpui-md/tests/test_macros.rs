//! Integration tests for keyboard macro recording and replay.

use gpui_md::macros::RecordedAction;
use gpui_md::state::MdAppState;

#[test]
fn record_and_replay_insert_chars() {
    let mut state = MdAppState::new();
    // Start from an empty document
    state.document.set_text("");
    state.cursor.position = 0;

    state.macro_start_recording();
    state.record_action(RecordedAction::InsertChar('a'));
    state.insert_text("a");
    state.record_action(RecordedAction::InsertChar('b'));
    state.insert_text("b");
    state.record_action(RecordedAction::InsertChar('c'));
    state.insert_text("c");
    state.macro_stop_recording();

    assert_eq!(state.document.text(), "abc");
    assert!(state.macros.last.is_some());
    assert_eq!(state.macros.last.as_ref().unwrap().actions.len(), 3);

    // Replay once
    state.macro_execute_last(1);
    assert_eq!(state.document.text(), "abcabc");
}

#[test]
fn replay_with_count() {
    let mut state = MdAppState::new();
    state.document.set_text("");
    state.cursor.position = 0;

    state.macro_start_recording();
    state.record_action(RecordedAction::InsertChar('x'));
    state.insert_text("x");
    state.macro_stop_recording();

    state.macro_execute_last(5);
    assert_eq!(state.document.text(), "xxxxxx"); // 1 recorded + 5 replays
}

#[test]
fn stop_empty_macro_does_not_save() {
    let mut state = MdAppState::new();
    state.macro_start_recording();
    state.macro_stop_recording();
    assert!(state.macros.last.is_none());
}

#[test]
fn recording_does_not_record_during_replay() {
    let mut state = MdAppState::new();
    state.document.set_text("");
    state.cursor.position = 0;

    state.macro_start_recording();
    state.record_action(RecordedAction::InsertChar('a'));
    state.insert_text("a");
    state.macro_stop_recording();

    // Start a new recording, then replay the old one. The replay should
    // not be recorded into the new session.
    state.macro_start_recording();
    state.macro_execute_last(2);
    state.macro_stop_recording();

    // Without recording the replay, current_macro should be empty.
    assert!(state.macros.last.as_ref().unwrap().actions.is_empty()
        || state.macros.last.as_ref().unwrap().actions.len() == 1);
}

#[test]
fn move_and_insert_sequence() {
    let mut state = MdAppState::new();
    state.document.set_text("hello");
    state.cursor.position = 0;

    state.macro_start_recording();
    state.record_action(RecordedAction::MoveRight);
    state.move_right(false);
    state.record_action(RecordedAction::InsertChar('X'));
    state.insert_text("X");
    state.macro_stop_recording();

    assert_eq!(state.document.text(), "hXello");

    // Replay should: move right one, insert X. From cursor pos 2 (after X)
    // first move right → pos 3, then insert X → "hXeXllo"
    state.macro_execute_last(1);
    assert_eq!(state.document.text(), "hXeXllo");
}
