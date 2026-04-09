//! TDD tests for state-level bugs.
//!
//! Issue #3:  preferred_column — sticky column for vertical cursor movement
//! Issue #8:  yank with selection pushes double undo
//! Issue #9:  undo does not restore cursor position
//! Issue #10: every keystroke creates separate undo entry (no coalescing)
//! Issue #14: word movement edge cases / correctness at boundaries
//! Issue #15: kill_to_end/start_of_line uses unclamped cursor.position
//! Issue #17: replace_all clears dirty flag via set_text
//! Issue #18: inline edit state API (view needs input widget)
//! Issue #22: insert_text panics when cursor is past document end

use gpui_md::markdown::SourceSpan;
use gpui_md::state::MdAppState;

fn state_with(text: &str) -> MdAppState {
    let mut s = MdAppState::new();
    s.document.set_text(text);
    s.cursor.position = 0;
    s.cursor.clear_selection();
    s
}

// ---------------------------------------------------------------------------
// Issue #3: preferred_column (sticky column for vertical movement)
//
// Moving through a short line should preserve the original column so that
// arriving on a longer line restores the cursor to where it started.
// Current code recomputes column from position each time, permanently
// losing the column when traversing a short line.
// ---------------------------------------------------------------------------

#[test]
fn issue_03_move_down_through_short_line_preserves_column() {
    // Line 0: "abcdefghij\n"  (11 chars, starts at 0)
    // Line 1: "ab\n"          ( 3 chars, starts at 11)
    // Line 2: "abcdefghij"   (10 chars, starts at 14)
    let mut s = state_with("abcdefghij\nab\nabcdefghij");

    s.cursor.position = 7; // col 7 on line 0
    s.move_down(false);
    // Line 1 has only 2 visible chars → cursor clamps to col 2
    assert_eq!(s.cursor.position, 13); // 11 + 2

    s.move_down(false);
    // Line 2 is long enough → cursor should return to col 7
    assert_eq!(
        s.cursor.position, 21, // 14 + 7
        "preferred column should be preserved through a short line"
    );
}

#[test]
fn issue_03_move_up_through_short_line_preserves_column() {
    let mut s = state_with("abcdefghij\nab\nabcdefghij");

    s.cursor.position = 21; // col 7 on line 2
    s.move_up(false);
    assert_eq!(s.cursor.position, 13); // col 2 on line 1

    s.move_up(false);
    assert_eq!(
        s.cursor.position, 7, // col 7 on line 0
        "preferred column should be preserved when moving up through a short line"
    );
}

#[test]
fn issue_03_horizontal_movement_resets_preferred_column() {
    let mut s = state_with("abcdefghij\nab\nabcdefghij");

    s.cursor.position = 7; // col 7 on line 0
    s.move_down(false); // col 2 on line 1, preferred stays at 7
    s.move_left(false); // col 1 on line 1, preferred resets to 1

    s.move_up(false);
    // After horizontal movement, the new column (1) should stick
    assert_eq!(
        s.cursor.position, 1,
        "horizontal movement should reset the preferred column"
    );
}

// ---------------------------------------------------------------------------
// Issue #8: yank with active selection pushes two undo entries
//
// delete_selection() pushes its own undo snapshot, then yank() pushes
// another. A single undo only reverses the insertion, leaving the
// document in a state that never existed.
// ---------------------------------------------------------------------------

#[test]
fn issue_08_yank_with_selection_is_single_undo_operation() {
    let mut s = state_with("hello world");

    // Kill "world" to populate the kill ring
    s.cursor.position = 6;
    s.cursor.anchor = Some(11);
    s.kill_region();
    assert_eq!(s.document.text(), "hello ");

    // Select "hello"
    s.cursor.position = 0;
    s.cursor.anchor = Some(5);

    // Yank: replaces "hello" with "world"
    s.yank();
    assert_eq!(s.document.text(), "world ");

    // A single undo should restore the state *before* yank was called
    s.undo();
    assert_eq!(
        s.document.text(),
        "hello ",
        "single undo after yank-with-selection should restore pre-yank state, \
         not the intermediate state after deletion"
    );
}

// ---------------------------------------------------------------------------
// Issue #9: undo does not restore cursor position
//
// After undo the cursor is clamped to document length but not restored
// to where it was before the edit. Every real editor saves
// (snapshot, cursor) pairs in the undo stack.
// ---------------------------------------------------------------------------

#[test]
fn issue_09_undo_restores_cursor_to_pre_edit_position() {
    let mut s = state_with("hello");

    s.cursor.position = 2; // between 'e' and 'l'
    s.insert_text("X"); // text → "heXllo", cursor → 3
    assert_eq!(s.cursor.position, 3);

    s.undo();
    assert_eq!(s.document.text(), "hello");
    assert_eq!(
        s.cursor.position, 2,
        "undo should restore cursor to the position before the edit"
    );
}

#[test]
fn issue_09_redo_restores_cursor_to_post_edit_position() {
    let mut s = state_with("hello");

    s.cursor.position = 2;
    s.insert_text("X"); // text → "heXllo", cursor → 3

    s.undo(); // text → "hello"
    // Move cursor away from where undo left it
    s.cursor.position = 0;
    s.redo(); // text → "heXllo", cursor should be 3 (post-edit position)

    assert_eq!(s.document.text(), "heXllo");
    assert_eq!(
        s.cursor.position, 3,
        "redo should restore cursor to the post-edit position, not leave it at 0"
    );
}

// ---------------------------------------------------------------------------
// Issue #10: every character insertion creates a separate undo entry
//
// Typing "hello" produces 5 undo entries. Undoing once should remove the
// entire burst of consecutive characters, not just the last one.
// ---------------------------------------------------------------------------

#[test]
fn issue_10_consecutive_character_insertions_coalesce_into_one_undo() {
    let mut s = state_with("");
    s.insert_text("h");
    s.insert_text("e");
    s.insert_text("l");
    s.insert_text("l");
    s.insert_text("o");
    assert_eq!(s.document.text(), "hello");

    s.undo();
    assert_eq!(
        s.document.text(),
        "",
        "a single undo should remove all consecutively-typed characters"
    );
}

#[test]
fn issue_10_cursor_movement_breaks_undo_coalescing() {
    let mut s = state_with("");
    s.insert_text("a");
    s.insert_text("b");
    s.move_left(false); // cursor movement breaks the group
    s.move_right(false);
    s.insert_text("c");
    s.insert_text("d");

    s.undo(); // should undo "cd" only
    assert_eq!(s.document.text(), "ab");
    s.undo(); // should undo "ab"
    assert_eq!(s.document.text(), "");
}

#[test]
fn issue_10_whitespace_coalesces_with_other_chars() {
    let mut s = state_with("");
    s.insert_text("h");
    s.insert_text("i");
    s.insert_text(" "); // space coalesces with surrounding chars
    s.insert_text("y");
    s.insert_text("o");
    assert_eq!(s.document.text(), "hi yo");

    // All single-char inserts coalesce into one undo group, so a single
    // undo removes the entire "hi yo".
    s.undo();
    let text = s.document.text();
    assert_eq!(
        text, "",
        "undo should remove the entire coalesced group; got '{}'",
        text
    );
}

// ---------------------------------------------------------------------------
// Issue #14: word movement correctness at document boundaries
//
// These should never panic even with edge-case positions.
// ---------------------------------------------------------------------------

#[test]
fn issue_14_move_word_right_at_document_end_is_noop() {
    let mut s = state_with("hello");
    s.cursor.position = 5; // at end
    s.move_word_right(false); // should not panic
    assert_eq!(s.cursor.position, 5);
}

#[test]
fn issue_14_move_word_left_at_document_start_is_noop() {
    let mut s = state_with("hello");
    s.cursor.position = 0;
    s.move_word_left(false); // should not panic
    assert_eq!(s.cursor.position, 0);
}

#[test]
fn issue_14_move_word_on_empty_document() {
    let mut s = state_with("");
    s.move_word_right(false);
    assert_eq!(s.cursor.position, 0);
    s.move_word_left(false);
    assert_eq!(s.cursor.position, 0);
}

// ---------------------------------------------------------------------------
// Issue #15: kill_to_end/start_of_line with cursor past document end
//
// `pos` is clamped but `self.cursor.position` is used directly downstream,
// causing ropey to panic on slice() with out-of-bounds indices.
// After the call, cursor.position should also be clamped.
// ---------------------------------------------------------------------------

#[test]
fn issue_15_kill_to_start_of_line_cursor_past_end_does_not_panic() {
    let mut s = state_with("hello");
    s.cursor.position = 100; // way past end
    // Currently panics: rope().slice(0..100) is out of bounds
    s.kill_to_start_of_line();
    assert!(
        s.cursor.position <= s.document.len_chars(),
        "cursor should be clamped to valid range after kill"
    );
}

#[test]
fn issue_15_kill_to_end_of_line_cursor_past_end_clamps() {
    let mut s = state_with("hello");
    s.cursor.position = 100;
    s.kill_to_end_of_line(); // should not panic
    assert!(
        s.cursor.position <= s.document.len_chars(),
        "cursor should be clamped after kill_to_end_of_line"
    );
}

#[test]
fn issue_15_kill_on_empty_last_line_with_cursor_at_newline() {
    // Document ends with \n, creating an empty last line.
    // Cursor at len_chars() is the valid "past-the-end" insert position.
    let mut s = state_with("hello\n");
    s.cursor.position = 6; // one past the \n
    s.kill_to_end_of_line(); // should be a no-op, not crash
    assert_eq!(s.document.text(), "hello\n");
}

// ---------------------------------------------------------------------------
// Issue #17: replace_all uses set_text which clears the dirty flag
//
// After a replace_all, the document should be marked dirty because
// it has unsaved modifications.
// ---------------------------------------------------------------------------

#[test]
fn issue_17_replace_all_marks_document_dirty() {
    let mut s = state_with("hello world hello");
    s.document.mark_clean();
    assert!(!s.document.is_dirty());

    s.find_query = "hello".to_string();
    s.replace_text = "hi".to_string();
    s.replace_all();

    assert_eq!(s.document.text(), "hi world hi");
    assert!(
        s.document.is_dirty(),
        "document should be dirty after replace_all"
    );
}

// ---------------------------------------------------------------------------
// Issue #18: inline edit — state API verification
//
// The view layer renders editing_block_text as a static div with no input.
// This test verifies the state-level API works so the view fix only needs
// to add an input widget.
// ---------------------------------------------------------------------------

#[test]
fn issue_18_inline_edit_commit_applies_modified_text() {
    let mut s = state_with("# Hello\n\nWorld");
    // Source map uses 1-indexed lines
    s.source_map.insert(
        "block-0".to_string(),
        SourceSpan {
            start_line: 3,
            start_col: 1,
            end_line: 3,
            end_col: 6,
        },
    );

    s.start_inline_edit("block-0");
    assert_eq!(s.editing_block_text, "World");

    // Simulate the user modifying the text (view needs input widget for this)
    s.editing_block_text = "Modified".to_string();
    s.commit_inline_edit();

    assert_eq!(s.document.text(), "# Hello\n\nModified");
}

// ---------------------------------------------------------------------------
// Issue #22: insert_text with cursor past document end panics
//
// If cursor.position > len_chars(), ropey panics on insert().
// The function should clamp the cursor before inserting.
// ---------------------------------------------------------------------------

#[test]
fn issue_22_insert_text_with_cursor_past_end_does_not_panic() {
    let mut s = state_with("hi");
    s.cursor.position = 100; // way past end
    s.insert_text("!"); // should clamp and not panic
    assert_eq!(
        s.document.text(),
        "hi!",
        "text should be inserted at clamped position (end of document)"
    );
    assert_eq!(s.cursor.position, 3);
}

#[test]
fn issue_22_insert_text_on_empty_doc_with_stale_cursor() {
    let mut s = state_with("");
    s.cursor.position = 50;
    s.insert_text("a");
    assert_eq!(s.document.text(), "a");
    assert_eq!(s.cursor.position, 1);
}
