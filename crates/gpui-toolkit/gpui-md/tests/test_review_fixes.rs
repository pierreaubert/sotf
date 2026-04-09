//! Tests for bugs found during code review.

use gpui_md::state::MdAppState;

fn state_with(text: &str) -> MdAppState {
    let mut s = MdAppState::new();
    s.document.set_text(text);
    s.cursor.position = 0;
    s.cursor.clear_selection();
    s
}

// ---------------------------------------------------------------------------
// BUG A: kill_to_end_of_line on last line leaves the final character
// ---------------------------------------------------------------------------

#[test]
fn kill_to_end_of_line_on_last_line_kills_everything() {
    let mut s = state_with("abc");
    s.cursor.position = 0;
    s.kill_to_end_of_line();
    assert_eq!(
        s.document.text(),
        "",
        "kill_to_end_of_line on the last line should kill all remaining chars"
    );
    assert_eq!(s.kill_ring.yank(), Some("abc"));
}

#[test]
fn kill_to_end_of_line_from_middle_of_last_line() {
    let mut s = state_with("abc");
    s.cursor.position = 1;
    s.kill_to_end_of_line();
    assert_eq!(s.document.text(), "a");
    assert_eq!(s.kill_ring.yank(), Some("bc"));
}

// ---------------------------------------------------------------------------
// BUG B: undo coalescing skips snapshot when replacing a selection
// ---------------------------------------------------------------------------

#[test]
fn coalescing_breaks_when_selection_is_active() {
    let mut s = state_with("ABC");
    s.cursor.position = 0;
    s.insert_text("x"); // snapshot pushed, doc = "xABC", cursor = 1
    assert_eq!(s.document.text(), "xABC");

    // Select "AB" (positions 1..3)
    s.cursor.anchor = Some(1);
    s.cursor.position = 3;

    // Type "y" — this replaces the selection
    s.insert_text("y"); // doc = "xyC"
    assert_eq!(s.document.text(), "xyC");

    // Undo should restore to "xABC" (before the selection replacement)
    s.undo();
    assert_eq!(
        s.document.text(),
        "xABC",
        "undo after coalesced selection replacement should restore the intermediate state"
    );
}

// ---------------------------------------------------------------------------
// BUG C: kill_word_forward captures wrong cursor pos in undo snapshot
// ---------------------------------------------------------------------------

#[test]
fn kill_word_forward_undo_restores_cursor_to_original_position() {
    let mut s = state_with("hello world");
    s.cursor.position = 6; // before "world"
    s.kill_word_forward();
    assert_eq!(s.document.text(), "hello ");
    assert_eq!(s.cursor.position, 6);

    s.undo();
    assert_eq!(s.document.text(), "hello world");
    assert_eq!(
        s.cursor.position, 6,
        "undo after kill_word_forward should restore cursor to before the kill"
    );
}

// ---------------------------------------------------------------------------
// BUG D: kill_word_backward captures wrong cursor pos in undo snapshot
// ---------------------------------------------------------------------------

#[test]
fn kill_word_backward_undo_restores_cursor_to_original_position() {
    let mut s = state_with("hello world");
    s.cursor.position = 5; // after "hello"
    s.kill_word_backward();
    assert_eq!(s.document.text(), " world");
    assert_eq!(s.cursor.position, 0);

    s.undo();
    assert_eq!(s.document.text(), "hello world");
    assert_eq!(
        s.cursor.position, 5,
        "undo after kill_word_backward should restore cursor to before the kill"
    );
}

// ---------------------------------------------------------------------------
// BUG E: replace_all double-bumps document version
// ---------------------------------------------------------------------------

#[test]
fn replace_all_increments_version_once() {
    let mut s = state_with("aaa");
    let v0 = s.document.version();
    s.find_query = "a".to_string();
    s.replace_text = "b".to_string();
    s.replace_all();
    // replace_content() increments version once; push_undo snapshots the
    // rope but doesn't touch version. So delta should be exactly 1.
    let delta = s.document.version() - v0;
    assert_eq!(
        delta, 1,
        "replace_all should bump version exactly once, got delta={}",
        delta
    );
}
