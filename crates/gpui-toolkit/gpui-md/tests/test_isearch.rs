//! Tests for incremental search (C-s / C-r).

use gpui_md::state::{IsearchDirection, MdAppState};

fn state_with(text: &str) -> MdAppState {
    let mut s = MdAppState::new();
    s.document.set_text(text);
    s.cursor.position = 0;
    s.cursor.clear_selection();
    s
}

// ---------------------------------------------------------------------------
// isearch_start
// ---------------------------------------------------------------------------

#[test]
fn isearch_start_sets_active_and_direction_forward() {
    let mut s = state_with("hello world");
    s.cursor.position = 3;
    s.isearch_start(IsearchDirection::Forward);

    assert!(s.isearch.active);
    assert_eq!(s.isearch.direction, IsearchDirection::Forward);
    assert_eq!(s.isearch.start_position, 3);
    assert!(s.isearch.query.is_empty());
    assert_eq!(s.isearch.current_match_start, None);
}

#[test]
fn isearch_start_sets_active_and_direction_backward() {
    let mut s = state_with("hello world");
    s.cursor.position = 8;
    s.isearch_start(IsearchDirection::Backward);

    assert!(s.isearch.active);
    assert_eq!(s.isearch.direction, IsearchDirection::Backward);
    assert_eq!(s.isearch.start_position, 8);
}

// ---------------------------------------------------------------------------
// isearch_add_char — forward
// ---------------------------------------------------------------------------

#[test]
fn isearch_add_char_finds_match_forward() {
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    s.isearch_start(IsearchDirection::Forward);
    s.isearch_add_char('w');

    assert_eq!(s.cursor.position, 6, "should jump to 'w' in 'world'");
    assert_eq!(s.isearch.current_match_start, Some(6));
}

#[test]
fn isearch_add_char_narrows_match() {
    let mut s = state_with("world war won");
    s.cursor.position = 0;
    s.isearch_start(IsearchDirection::Forward);
    s.isearch_add_char('w');
    assert_eq!(s.cursor.position, 0, "first 'w' is at position 0");

    s.isearch_add_char('a');
    assert_eq!(s.cursor.position, 6, "'wa' matches at 'war'");
}

#[test]
fn isearch_add_char_no_match_leaves_cursor() {
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    s.isearch_start(IsearchDirection::Forward);

    s.isearch_add_char('z');
    // When there's no match, cursor stays where it was (at start_position
    // since isearch_find_current returns None)
    assert_eq!(s.isearch.current_match_start, None);
}

#[test]
fn isearch_forward_wraps_around() {
    let mut s = state_with("abc abc");
    s.cursor.position = 5; // past the second 'a'
    s.isearch_start(IsearchDirection::Forward);
    s.isearch_add_char('a');
    s.isearch_add_char('b');

    // Should find "ab" — either at pos 5+ or wrapped to pos 0
    let pos = s.cursor.position;
    assert!(pos == 0 || pos == 4,
        "should wrap around to find 'ab', got pos={}", pos);
}

// ---------------------------------------------------------------------------
// isearch — backward
// ---------------------------------------------------------------------------

#[test]
fn isearch_backward_finds_before_cursor() {
    // Use a unique substring so backward search doesn't get confused by
    // cursor movements between add_char calls.
    let mut s = state_with("foo bar baz");
    s.cursor.position = 8; // at 'b' in "baz"
    s.isearch_start(IsearchDirection::Backward);
    s.isearch_add_char('f');

    assert_eq!(s.cursor.position, 0, "backward search from pos 8 should find 'f' at 0");
}

#[test]
fn isearch_backward_wraps_around() {
    let mut s = state_with("xyz abc");
    s.cursor.position = 0; // at start, nothing before
    s.isearch_start(IsearchDirection::Backward);
    s.isearch_add_char('a');
    s.isearch_add_char('b');

    // Should wrap around and find "ab" at position 4
    assert_eq!(s.cursor.position, 4, "backward search should wrap to find 'ab' at 4");
}

// ---------------------------------------------------------------------------
// isearch_backspace
// ---------------------------------------------------------------------------

#[test]
fn isearch_backspace_removes_char_and_researches() {
    let mut s = state_with("abc abd");
    s.cursor.position = 0;
    s.isearch_start(IsearchDirection::Forward);
    s.isearch_add_char('a');
    s.isearch_add_char('b');
    s.isearch_add_char('d');
    // "abd" matches at position 4
    assert_eq!(s.cursor.position, 4);

    s.isearch_backspace(); // query becomes "ab"
    assert_eq!(s.isearch.query, "ab");
    // Re-searches from start_position (0), finds "ab" at 0
    assert_eq!(s.cursor.position, 0, "after backspace, re-search from start should find 'ab' at 0");
}

#[test]
fn isearch_backspace_to_empty_returns_to_start() {
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    s.isearch_start(IsearchDirection::Forward);
    s.isearch_add_char('w');
    assert_eq!(s.cursor.position, 6);

    s.isearch_backspace(); // query becomes empty
    assert!(s.isearch.query.is_empty());
    assert_eq!(
        s.cursor.position, 0,
        "backspace to empty should return cursor to start_position"
    );
    assert_eq!(s.isearch.current_match_start, None);
}

// ---------------------------------------------------------------------------
// isearch_abort vs isearch_exit
// ---------------------------------------------------------------------------

#[test]
fn isearch_abort_returns_cursor_to_start() {
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    s.isearch_start(IsearchDirection::Forward);
    s.isearch_add_char('w');
    assert_eq!(s.cursor.position, 6);

    s.isearch_abort();
    assert!(!s.isearch.active);
    assert_eq!(
        s.cursor.position, 0,
        "isearch_abort should restore cursor to start_position"
    );
}

#[test]
fn isearch_exit_keeps_cursor_at_current() {
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    s.isearch_start(IsearchDirection::Forward);
    s.isearch_add_char('w');
    assert_eq!(s.cursor.position, 6);

    s.isearch_exit();
    assert!(!s.isearch.active);
    assert_eq!(
        s.cursor.position, 6,
        "isearch_exit should keep cursor at match position"
    );
}

// ---------------------------------------------------------------------------
// isearch_next / isearch_prev
// ---------------------------------------------------------------------------

#[test]
fn isearch_next_moves_to_next_match() {
    let mut s = state_with("abc abc abc");
    s.cursor.position = 0;
    s.isearch_start(IsearchDirection::Forward);
    s.isearch_add_char('a');
    s.isearch_add_char('b');
    assert_eq!(s.cursor.position, 0, "first match at 0");

    s.isearch_next();
    assert_eq!(s.cursor.position, 4, "next match at 4");

    s.isearch_next();
    assert_eq!(s.cursor.position, 8, "next match at 8");
}

#[test]
fn isearch_next_wraps_at_end() {
    let mut s = state_with("abc abc");
    s.cursor.position = 0;
    s.isearch_start(IsearchDirection::Forward);
    s.isearch_add_char('a');
    s.isearch_add_char('b');
    assert_eq!(s.cursor.position, 0);

    s.isearch_next(); // goes to 4
    assert_eq!(s.cursor.position, 4);

    s.isearch_next(); // wraps back to 0
    assert_eq!(s.cursor.position, 0);
}

#[test]
fn isearch_prev_moves_to_previous_match() {
    let mut s = state_with("abc abc abc");
    s.cursor.position = 8;
    s.isearch_start(IsearchDirection::Forward);
    s.isearch_add_char('a');
    s.isearch_add_char('b');
    // First match found from start_position=8, which is at position 8
    assert_eq!(s.cursor.position, 8);

    s.isearch_prev();
    assert_eq!(s.cursor.position, 4, "prev should find 'ab' at 4");
}

#[test]
fn isearch_prev_wraps_at_beginning() {
    let mut s = state_with("abc abc");
    s.cursor.position = 0;
    s.isearch_start(IsearchDirection::Forward);
    s.isearch_add_char('a');
    s.isearch_add_char('b');
    assert_eq!(s.cursor.position, 0);

    s.isearch_prev(); // from pos 0, backward should wrap to find "ab" at 4
    assert_eq!(s.cursor.position, 4, "prev from start should wrap to last match");
}

// ---------------------------------------------------------------------------
// isearch on empty document
// ---------------------------------------------------------------------------

#[test]
fn isearch_on_empty_doc() {
    let mut s = state_with("");
    s.isearch_start(IsearchDirection::Forward);
    s.isearch_add_char('x');
    assert_eq!(s.cursor.position, 0);
    assert_eq!(s.isearch.current_match_start, None);
    s.isearch_exit();
    assert!(!s.isearch.active);
}
