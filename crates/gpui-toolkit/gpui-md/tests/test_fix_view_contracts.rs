//! TDD tests for view-layer bugs — contracts and preconditions.
//!
//! Many view-layer issues cannot be fully tested without a GPUI rendering
//! context. These tests verify the underlying contracts, preconditions,
//! and building blocks that the view fixes depend on.
//!
//! Issue #1:  keybinding actions defined but never wired to handlers
//! Issue #2:  find bar has no text input (find_query can never be set from UI)
//! Issue #4:  hardcoded char_width = 8.4 breaks click positioning
//! Issue #5:  thread_local TEXT_ORIGIN_X corrupts multi-instance positioning
//! Issue #6:  mouse drag fires without button held
//! Issue #7:  silent data loss on file save failure
//! Issue #16: preview pane mutates state during render
//! Issue #19: ExportPdf menu item shown when feature is disabled
//! Issue #20: "About" menu item triggers TogglePreview
//! Issue #24: page up/down hardcoded to 30 lines
//! Issue #25: gutter width uses hardcoded 9.0px per digit
//! Issue #26: scroll sync uses proportional mapping, not source-map-based
//! Issue #30: redundant text.clone() in save-as path

use gpui_md::markdown::{SourceMap, SourceSpan};
use gpui_md::state::MdAppState;

fn state_with(text: &str) -> MdAppState {
    let mut s = MdAppState::new();
    s.document.set_text(text);
    s.cursor.position = 0;
    s.cursor.clear_selection();
    s
}

// ---------------------------------------------------------------------------
// Issue #1: keybinding actions — verify state-level operations work
//
// The actions are defined in actions.rs and bindings in keybindings.rs,
// but no on_action handlers exist in the view. This test verifies the
// state API that the handlers should call. The GPUI wiring is a view fix.
// ---------------------------------------------------------------------------

#[test]
fn issue_01_undo_redo_work_at_state_level() {
    let mut s = state_with("hello");
    s.cursor.position = 5; // end of "hello"
    s.insert_text(" world");
    assert_eq!(s.document.text(), "hello world");

    s.undo();
    assert_eq!(s.document.text(), "hello");

    s.redo();
    assert_eq!(s.document.text(), "hello world");
    // FIX NEEDED: wire on_action handlers in EditorPane/MainView for
    // Undo, Redo, SaveFile, ToggleBold, Find, etc.
}

#[test]
fn issue_01_toggle_bold_works_at_state_level() {
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    s.cursor.anchor = Some(5); // select "hello"
    s.toggle_format("**", "**");
    assert_eq!(s.document.text(), "**hello** world");
}

#[test]
fn issue_01_find_works_at_state_level() {
    let mut s = state_with("hello world hello");
    s.find_query = "hello".to_string();
    s.find_next();
    // Cursor should move to first occurrence
    assert!(
        s.cursor.position > 0,
        "find_next should move cursor to match"
    );
}

// ---------------------------------------------------------------------------
// Issue #2: find bar — verify find/replace state works when query is set
//
// The view's find bar renders as static divs (no text input). This test
// verifies that the state operations work when find_query IS populated.
// The view fix requires adding focusable text input widgets.
// ---------------------------------------------------------------------------

#[test]
fn issue_02_find_and_replace_work_when_query_set_programmatically() {
    let mut s = state_with("foo bar foo baz foo");
    s.find_query = "foo".to_string();
    s.replace_text = "qux".to_string();

    s.replace_all();
    assert_eq!(s.document.text(), "qux bar qux baz qux");
    // FIX NEEDED: FindBar needs actual text inputs so users can type the query
}

// ---------------------------------------------------------------------------
// Issue #6: mouse drag — verify cursor state tracks selection correctly
//
// The view's on_mouse_move checks anchor.is_some() but not if a button
// is pressed. This test verifies that cursor selection mechanics work.
// The view fix needs a pressed-button check in the move handler.
// ---------------------------------------------------------------------------

#[test]
fn issue_06_selection_via_anchor_and_position() {
    let mut s = state_with("hello world");
    // Simulate mouse down at position 3
    s.cursor.anchor = Some(3);
    s.cursor.position = 3;
    assert!(!s.cursor.has_selection());

    // Simulate drag to position 8
    s.cursor.position = 8;
    assert!(s.cursor.has_selection());
    assert_eq!(s.cursor.selection(), Some((3, 8)));

    // Simulate mouse up — selection should remain
    // (but view should stop updating position on hover)
    assert_eq!(s.cursor.selection(), Some((3, 8)));
    // FIX NEEDED: EditorPane.on_mouse_move must check ev.pressed_button
}

// ---------------------------------------------------------------------------
// Issue #26: scroll sync — verify source_map find_block_at_line works
//
// Proportional scroll mapping (50% editor → 50% preview) is semantically
// wrong. The fix should use source_map.find_block_at_line() to map
// editor line → preview block. This test verifies the API works.
// ---------------------------------------------------------------------------

#[test]
fn issue_26_source_map_find_block_at_line_for_scroll_sync() {
    let mut sm = SourceMap::new();
    sm.insert(
        "heading".to_string(),
        SourceSpan {
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 10,
        },
    );
    sm.insert(
        "para1".to_string(),
        SourceSpan {
            start_line: 3,
            start_col: 1,
            end_line: 5,
            end_col: 20,
        },
    );
    sm.insert(
        "para2".to_string(),
        SourceSpan {
            start_line: 7,
            start_col: 1,
            end_line: 9,
            end_col: 15,
        },
    );

    // Should find the block at or before line 4 (within para1's span)
    let block = sm.find_block_at_line(4);
    assert!(
        block.is_some(),
        "source map should find the block containing line 4"
    );

    // Should find block at line 1 (heading)
    let block = sm.find_block_at_line(1);
    assert!(block.is_some(), "source map should find block at line 1");

    // Between blocks (line 6) — should find nearest preceding block
    let block = sm.find_block_at_line(6);
    assert!(
        block.is_some(),
        "source map should find nearest block before line 6"
    );
    // FIX NEEDED: MainView.sync_scroll should use find_block_at_line
    // instead of proportional mapping
}

// ---------------------------------------------------------------------------
// Issues #4, #5, #7, #16, #19, #20, #24, #25, #30
//
// These are purely view-layer / binary-level issues that cannot be
// meaningfully unit tested. Summary of required fixes:
//
// #4:  EditorPane — measure char_width from actual font metrics via GPUI
//      text measurement API instead of hardcoding 8.4
//
// #5:  EditorPane — store text_origin_x on the EditorPane struct, not in
//      a thread_local. Recompute on layout changes.
//
// #7:  gpui_md_editor binary — change `if std::fs::write(...).is_ok()`
//      to show an error dialog on failure (rfd::MessageDialog or log)
//
// #16: PreviewPane — move source_map computation out of render() into a
//      subscription that triggers when document text changes
//
// #19: gpui_md_editor binary — feature-gate the ExportPdf menu item
//      behind #[cfg(feature = "pdf-export")]
//
// #20: gpui_md_editor binary — change "About" menu item action from
//      TogglePreview to a proper About dialog action
//
// #24: EditorPane — use actual visible line count instead of hardcoded 30
//      for page up/down
//
// #25: EditorPane — compute gutter width from font metrics instead of
//      hardcoded 9.0 pixels per digit
//
// #30: gpui_md_editor binary — remove redundant text.clone() in save-as
//      (line 96); use &text directly
// ---------------------------------------------------------------------------
