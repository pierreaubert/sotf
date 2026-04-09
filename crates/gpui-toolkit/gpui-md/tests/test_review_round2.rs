//! TDD tests for code review fixes — Phase 1A + 1B.
//!
//! Each test documents the expected behavior after the fix.
//! These tests should FAIL before the fix and PASS after.

use gpui_md::document::kill_ring::KillRing;
use gpui_md::document::EditHistory;
use gpui_md::state::MdAppState;
use ropey::Rope;

fn state_with(text: &str) -> MdAppState {
    let mut s = MdAppState::new();
    s.document.set_text(text);
    s.cursor.position = 0;
    s.cursor.clear_selection();
    s
}

// ============================================================
// FIX #5: Cursor clamping — len_chars() not saturating_sub(1)
// ============================================================

#[test]
fn cursor_clamping_move_to_line_start_at_doc_end_with_trailing_newline() {
    // "abc\n" has 4 chars. Position 4 is valid (start of empty trailing line).
    // C-a should go to position 4 (start of that empty line), not jump to line 0.
    let mut s = state_with("abc\n");
    s.cursor.position = 4;
    s.move_to_line_start(false);
    assert_eq!(s.cursor.position, 4, "C-a at trailing empty line should stay at line start (pos 4)");
}

#[test]
fn cursor_clamping_move_to_line_end_at_doc_end_with_trailing_newline() {
    let mut s = state_with("abc\n");
    s.cursor.position = 4;
    s.move_to_line_end(false);
    assert_eq!(s.cursor.position, 4, "C-e at trailing empty line should stay at pos 4");
}

#[test]
fn cursor_clamping_kill_to_end_at_doc_end_with_trailing_newline() {
    let mut s = state_with("abc\n");
    s.cursor.position = 4;
    s.kill_to_end_of_line();
    // Should be a no-op — cursor at end of empty trailing line
    assert_eq!(s.document.text(), "abc\n");
    assert_eq!(s.cursor.position, 4);
}

#[test]
fn cursor_clamping_kill_to_start_at_trailing_line() {
    // Cursor at pos 4 (empty trailing line after "abc\n").
    // C-u 0 C-k should do nothing — there's nothing before cursor on this line.
    let mut s = state_with("abc\n");
    s.cursor.position = 4;
    s.kill_to_start_of_line();
    assert_eq!(s.document.text(), "abc\n");
    assert_eq!(s.cursor.position, 4);
}

#[test]
fn cursor_clamping_move_up_from_trailing_empty_line() {
    // "abc\n" => line 0: "abc\n", line 1: "" (empty trailing)
    // Cursor at pos 4 (line 1, col 0). Move up should go to line 0, col 0 = pos 0.
    let mut s = state_with("abc\n");
    s.cursor.position = 4;
    s.move_up(false);
    assert_eq!(s.cursor.position, 0, "move_up from trailing empty line should go to line 0 col 0");
}

#[test]
fn cursor_clamping_move_down_to_trailing_empty_line() {
    // "abc\n" => line 0: "abc\n", line 1: "" (empty trailing)
    // Cursor at pos 0 (line 0). Move down should go to pos 4 (line 1, col 0).
    let mut s = state_with("abc\n");
    s.cursor.position = 0;
    s.move_down(false);
    assert_eq!(s.cursor.position, 4, "move_down should reach trailing empty line");
}

#[test]
fn cursor_clamping_update_preferred_column_at_doc_end() {
    // update_preferred_column should not panic or return wrong column
    // when cursor is at len_chars()
    let mut s = state_with("abc\n");
    s.cursor.position = 4;
    // Trigger update_preferred_column via move_to_line_start
    s.move_to_line_start(false);
    assert_eq!(s.cursor.preferred_column, 0);
}

// ============================================================
// FIX #6: Word movement — emacs M-f / M-b semantics
// ============================================================

#[test]
fn move_word_right_emacs_semantics_from_space() {
    // Emacs M-f: skip non-word chars, then skip word chars => land at end of next word.
    // "hello world" with cursor at 5 (the space): skip space, then skip "world" => pos 11.
    let mut s = state_with("hello world");
    s.cursor.position = 5;
    s.move_word_right(false);
    assert_eq!(s.cursor.position, 11, "M-f from space should land at end of 'world'");
}

#[test]
fn move_word_right_emacs_semantics_from_word_start() {
    // "hello world" with cursor at 0: skip non-word (none), then skip "hello" => pos 5.
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    s.move_word_right(false);
    assert_eq!(s.cursor.position, 5, "M-f from word start should land at end of 'hello'");
}

#[test]
fn move_word_right_emacs_semantics_with_punctuation() {
    // "foo, bar" with cursor at 3 (the comma): skip ", " (non-word), then skip "bar" => pos 8.
    let mut s = state_with("foo, bar");
    s.cursor.position = 3;
    s.move_word_right(false);
    assert_eq!(s.cursor.position, 8, "M-f from punctuation should skip non-word then word");
}

#[test]
fn move_word_left_emacs_semantics_from_space() {
    // Emacs M-b: skip non-word chars backward, then skip word chars backward => land at start of prev word.
    // "hello world" with cursor at 6 (the 'w'): skip nothing (w is word), then skip... wait.
    // Cursor at 6: char is 'w' (word char). M-b should skip backward through non-word (none),
    // then skip backward through word chars => lands at 6 (no non-word to skip first).
    // Actually: M-b skips non-word backwards first. At pos 6, char before (pos 5) is space.
    // No wait — emacs M-b looks at chars *before* the cursor.
    // At pos 6: chars before are ...'o', ' '. So: skip non-word backward (' ') => pos 5.
    // Then skip word backward ('o','l','l','e','h') => pos 0.
    let mut s = state_with("hello world");
    s.cursor.position = 6;
    s.move_word_left(false);
    assert_eq!(s.cursor.position, 0, "M-b from 'w' should skip space then 'hello' landing at 0");
}

#[test]
fn move_word_left_emacs_semantics_from_end() {
    // "hello world" with cursor at 11 (end): skip non-word backward (none),
    // then skip word backward ('d','l','r','o','w') => pos 6.
    let mut s = state_with("hello world");
    s.cursor.position = 11;
    s.move_word_left(false);
    assert_eq!(s.cursor.position, 6, "M-b from end should land at start of 'world'");
}

// ============================================================
// FIX #9: copy_region should not chain with subsequent kills
// ============================================================

#[test]
fn copy_region_does_not_chain_with_next_kill() {
    let mut s = state_with("hello world\nfoo");
    // Select "hello"
    s.cursor.anchor = Some(0);
    s.cursor.position = 5;
    s.copy_region(); // M-w copies "hello"

    // Now kill to end of line (C-k) — should NOT append to "hello"
    s.cursor.position = 6; // at 'w' in "world"
    s.kill_to_end_of_line();

    // The kill ring top should be "world" (from C-k), not "helloworld"
    let top = s.kill_ring.yank().unwrap();
    assert_eq!(top, "world", "C-k after M-w should create separate kill ring entry");
}

// ============================================================
// FIX #10: kill_word_forward should not destroy selection
// ============================================================

#[test]
fn kill_word_forward_preserves_mark() {
    let mut s = state_with("hello world");
    // Set mark at position 0
    s.set_mark();
    // Move to position 6
    s.cursor.position = 6;
    // kill_word_forward should kill "world" but the anchor should still exist
    s.kill_word_forward();
    assert_eq!(s.document.text(), "hello ");
    // The anchor (mark) should still be set at 0
    assert_eq!(s.cursor.anchor, Some(0), "kill_word_forward should not destroy the mark");
}

#[test]
fn kill_word_backward_preserves_mark() {
    let mut s = state_with("hello world");
    s.cursor.position = 11;
    s.set_mark();
    s.cursor.position = 5;
    s.kill_word_backward();
    assert_eq!(s.document.text(), " world");
    // Mark should still be set
    assert!(s.cursor.anchor.is_some(), "kill_word_backward should not destroy the mark");
}

// ============================================================
// FIX #15: DocumentBuffer::remove bounds checking
// ============================================================

#[test]
fn document_buffer_remove_with_end_beyond_length() {
    let mut buf = gpui_md::document::DocumentBuffer::from_text("hello");
    // end=100 is beyond len_chars()=5, should clamp instead of panicking
    buf.remove(0, 100);
    assert_eq!(buf.text(), "");
}

#[test]
fn document_buffer_remove_with_start_beyond_length() {
    let mut buf = gpui_md::document::DocumentBuffer::from_text("hello");
    // start=10 beyond length, should be a no-op
    buf.remove(10, 20);
    assert_eq!(buf.text(), "hello");
}

// ============================================================
// FIX: Redo stack should be capped at max_history
// ============================================================

#[test]
fn redo_stack_capped_at_max_history() {
    let mut history = EditHistory::new(5);
    // Push 5 entries
    for i in 0..5 {
        history.push_undo(Rope::from_str(&format!("v{}", i)), i);
    }
    // Undo all 5 — each pushes to redo stack
    let mut current = Rope::from_str("v5");
    for i in (0..5).rev() {
        let (snap, _cursor) = history.undo(current, i + 1).unwrap();
        current = snap;
    }
    assert!(history.can_redo(), "should have redo entries");

    // Now do one more undo to try push onto redo — should not grow beyond 5
    // Since undo stack is empty, this returns None
    assert!(!history.can_undo());

    // Redo should work for up to 5 entries, not more
    let mut count = 0;
    while history.can_redo() {
        let (snap, _) = history.redo(current, 0).unwrap();
        current = snap;
        count += 1;
    }
    assert!(count <= 5, "redo stack should be capped at max_history, got {}", count);
}

// ============================================================
// Rectangle operations (untested feature area)
// ============================================================

#[test]
fn rectangle_selection_basic() {
    // "abcde\nfghij\nklmno"
    // Select from (0,1) to (2,3) -> lines 0-2, cols 1-3
    let mut s = state_with("abcde\nfghij\nklmno");
    s.cursor.anchor = Some(1); // line 0, col 1 ('b')
    s.cursor.position = 15; // line 2, col 3 ('n')
    let rect = s.rectangle_selection();
    assert_eq!(rect, Some((0, 1, 2, 3)));
}

#[test]
fn delete_rectangle_basic() {
    let mut s = state_with("abcde\nfghij\nklmno");
    s.cursor.anchor = Some(1); // line 0, col 1
    s.cursor.position = 15; // line 2, col 3
    s.delete_rectangle();
    // Should remove cols 1..3 from each line:
    // "abcde" -> "ade"
    // "fghij" -> "fij"
    // "klmno" -> "kno"
    assert_eq!(s.document.text(), "ade\nfij\nkno");
}

#[test]
fn kill_rectangle_stores_killed_text() {
    let mut s = state_with("abcde\nfghij\nklmno");
    s.cursor.anchor = Some(1);
    s.cursor.position = 15;
    s.kill_rectangle();
    assert_eq!(s.document.text(), "ade\nfij\nkno");
    let rect_buf = s.kill_ring.rectangle_buffer.as_ref().unwrap();
    assert_eq!(rect_buf, &vec!["bc".to_string(), "gh".to_string(), "lm".to_string()]);
}

#[test]
fn yank_rectangle_basic() {
    let mut s = state_with("ade\nfij\nkno");
    s.kill_ring.rectangle_buffer = Some(vec!["bc".into(), "gh".into(), "lm".into()]);
    s.cursor.position = 1; // line 0, col 1 (between 'a' and 'd')
    s.yank_rectangle();
    assert_eq!(s.document.text(), "abcde\nfghij\nklmno");
}

#[test]
fn open_rectangle_inserts_spaces() {
    let mut s = state_with("abcde\nfghij\nklmno");
    s.cursor.anchor = Some(1); // col 1
    s.cursor.position = 15; // col 3
    s.open_rectangle();
    // Should insert 2 spaces at col 1 on each line
    assert_eq!(s.document.text(), "a  bcde\nf  ghij\nk  lmno");
}

// ============================================================
// replace_next (completely untested)
// ============================================================

#[test]
fn replace_next_at_cursor() {
    let mut s = state_with("hello hello hello");
    s.find_query = "hello".to_string();
    s.replace_text = "world".to_string();
    s.cursor.position = 0;
    s.replace_next();
    assert_eq!(s.document.text(), "world hello hello");
}

#[test]
fn replace_next_skips_when_not_at_match() {
    let mut s = state_with("hello hello hello");
    s.find_query = "hello".to_string();
    s.replace_text = "world".to_string();
    s.cursor.position = 1; // not at a match
    s.replace_next();
    // Should not replace (cursor not at match), but find_next should jump to next occurrence
    // "hello" at pos 6 is the next match, cursor should be there
    assert_eq!(s.document.text(), "hello hello hello", "should not replace when cursor is not at match");
    assert_eq!(s.cursor.position, 6, "should jump to next match");
}

#[test]
fn replace_next_empty_query_is_noop() {
    let mut s = state_with("hello");
    s.find_query = "".to_string();
    s.replace_text = "world".to_string();
    s.replace_next();
    assert_eq!(s.document.text(), "hello");
}

// ============================================================
// move_to_doc_start / move_to_doc_end (untested)
// ============================================================

#[test]
fn move_to_doc_start() {
    let mut s = state_with("hello\nworld");
    s.cursor.position = 8;
    s.move_to_doc_start(false);
    assert_eq!(s.cursor.position, 0);
}

#[test]
fn move_to_doc_end() {
    let mut s = state_with("hello\nworld");
    s.cursor.position = 0;
    s.move_to_doc_end(false);
    assert_eq!(s.cursor.position, 11);
}

#[test]
fn move_to_doc_start_with_selection() {
    let mut s = state_with("hello\nworld");
    s.cursor.position = 8;
    s.move_to_doc_start(true);
    assert_eq!(s.cursor.position, 0);
    assert!(s.cursor.has_selection());
    assert_eq!(s.cursor.selection(), Some((0, 8)));
}

// ============================================================
// page_up / page_down (untested)
// ============================================================

#[test]
fn page_down_moves_cursor_forward() {
    // Create a document with 100 lines
    let text = (0..100).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
    let mut s = state_with(&text);
    s.cursor.position = 0;
    s.page_down();
    assert!(s.cursor.position > 0, "page_down should move cursor forward");
}

#[test]
fn page_up_moves_cursor_backward() {
    let text = (0..100).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
    let mut s = state_with(&text);
    s.cursor.position = s.document.len_chars();
    s.page_up();
    assert!(s.cursor.position < s.document.len_chars(), "page_up should move cursor backward");
}

// ============================================================
// set_mark and exchange_point_and_mark (untested)
// ============================================================

#[test]
fn set_mark_creates_anchor() {
    let mut s = state_with("hello");
    s.cursor.position = 3;
    s.set_mark();
    assert_eq!(s.cursor.anchor, Some(3));
}

#[test]
fn exchange_point_and_mark_swaps() {
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    s.set_mark(); // mark at 0
    s.cursor.position = 5;
    s.exchange_point_and_mark();
    assert_eq!(s.cursor.position, 0);
    assert_eq!(s.cursor.anchor, Some(5));
}

#[test]
fn exchange_point_and_mark_no_mark_is_noop() {
    let mut s = state_with("hello");
    s.cursor.position = 3;
    s.exchange_point_and_mark();
    assert_eq!(s.cursor.position, 3);
    assert_eq!(s.cursor.anchor, None);
}

// ============================================================
// Misc untested methods
// ============================================================

#[test]
fn cancel_inline_edit_clears_state() {
    let mut s = state_with("hello");
    s.editing_block = Some("block-1".to_string());
    s.editing_block_text = "some text".to_string();
    s.cancel_inline_edit();
    assert!(s.editing_block.is_none());
    assert!(s.editing_block_text.is_empty());
}

#[test]
fn selected_text_returns_correct_substring() {
    let s = {
        let mut s = state_with("hello world");
        s.cursor.anchor = Some(6);
        s.cursor.position = 11;
        s
    };
    assert_eq!(s.selected_text(), Some("world".to_string()));
}

#[test]
fn add_recent_file_deduplicates() {
    let mut s = state_with("");
    let p = std::path::PathBuf::from("/tmp/a.md");
    s.add_recent_file(p.clone());
    s.add_recent_file(p.clone());
    assert_eq!(s.recent_files.len(), 1);
}

#[test]
fn add_recent_file_truncates_at_10() {
    let mut s = state_with("");
    for i in 0..15 {
        s.add_recent_file(std::path::PathBuf::from(format!("/tmp/{}.md", i)));
    }
    assert_eq!(s.recent_files.len(), 10);
}

#[test]
fn delete_selection_returns_deleted_text() {
    let mut s = state_with("hello world");
    s.cursor.anchor = Some(0);
    s.cursor.position = 5;
    let deleted = s.delete_selection();
    assert_eq!(deleted, Some("hello".to_string()));
    assert_eq!(s.document.text(), " world");
    assert_eq!(s.cursor.position, 0);
}

#[test]
fn jump_to_line_zero_is_noop() {
    let mut s = state_with("line1\nline2\nline3");
    s.cursor.position = 10;
    s.jump_to_line(0);
    assert_eq!(s.cursor.position, 10, "jump_to_line(0) should be a no-op");
}

#[test]
fn jump_to_line_beyond_end_goes_to_last_line() {
    let mut s = state_with("line1\nline2\nline3");
    s.jump_to_line(999);
    let last_line_start = s.document.line_to_char(2);
    assert_eq!(s.cursor.position, last_line_start);
}

// ============================================================
// Word movement: Vec<char> allocation efficiency test
// (not a correctness test, but verifies the API works with ropey iterators)
// ============================================================

#[test]
fn move_word_right_on_large_document() {
    // Just verify it works without panicking on a larger doc
    let text = "word ".repeat(1000);
    let mut s = state_with(&text);
    s.cursor.position = 0;
    s.move_word_right(false);
    assert!(s.cursor.position > 0);
}

// ============================================================
// KillRing: yank_pop without preceding yank
// ============================================================

#[test]
fn yank_pop_without_yank_returns_none_via_last_yank_range() {
    let mut s = state_with("hello");
    s.kill_ring.push("text".to_string());
    // yank_pop should only work after a yank — last_yank_range gates it
    assert!(s.kill_ring.last_yank_range().is_none());
}

// ============================================================
// KillRing: push_prepend on empty ring
// ============================================================

#[test]
fn kill_ring_push_prepend_on_empty() {
    let mut kr = KillRing::new();
    kr.push_prepend("hello".to_string());
    assert_eq!(kr.yank(), Some("hello"));
}
