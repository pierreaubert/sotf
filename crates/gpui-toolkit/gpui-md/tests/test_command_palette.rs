//! Tests for the M-x command palette.

use gpui_md::state::{MdAppState, PaletteMode};

fn state_with(text: &str) -> MdAppState {
    let mut s = MdAppState::new();
    s.document.set_text(text);
    s.cursor.position = 0;
    s.cursor.clear_selection();
    s
}

// ---------------------------------------------------------------------------
// Palette visibility
// ---------------------------------------------------------------------------

#[test]
fn palette_starts_hidden() {
    let s = state_with("hello");
    assert!(!s.command_palette.visible);
}

#[test]
fn toggle_opens_and_closes() {
    let mut s = state_with("hello");
    s.toggle_command_palette();
    assert!(s.command_palette.visible);

    s.toggle_command_palette();
    assert!(!s.command_palette.visible);
}

#[test]
fn toggle_resets_query_and_selection() {
    let mut s = state_with("hello");
    s.toggle_command_palette();
    s.command_palette_add_char('g');
    s.command_palette_select_next();
    assert!(!s.command_palette.query.is_empty());

    // Close and reopen
    s.toggle_command_palette();
    s.toggle_command_palette();
    assert!(s.command_palette.query.is_empty());
    assert_eq!(s.command_palette.selected, 0);
}

// ---------------------------------------------------------------------------
// Filtering
// ---------------------------------------------------------------------------

#[test]
fn filter_empty_query_shows_all_commands() {
    let mut s = state_with("hello");
    s.toggle_command_palette();
    // Empty query: all commands should be visible
    assert_eq!(
        s.command_palette.filtered_indices.len(),
        s.command_palette.commands.len(),
        "empty query should show all commands"
    );
}

#[test]
fn filter_with_query_narrows_results() {
    let mut s = state_with("hello");
    s.toggle_command_palette();

    s.command_palette_add_char('g');
    s.command_palette_add_char('o');
    s.command_palette_add_char('t');
    s.command_palette_add_char('o');

    // Should match "goto-line" and possibly "toggle-*" commands
    assert!(!s.command_palette.filtered_indices.is_empty());
    // At minimum "goto-line" should match
    let has_goto = s.command_palette.filtered_indices.iter().any(|&i| {
        s.command_palette.commands[i].id == "goto-line"
    });
    assert!(has_goto, "filtering for 'goto' should include goto-line");
}

#[test]
fn filter_with_no_match() {
    let mut s = state_with("hello");
    s.toggle_command_palette();

    s.command_palette_add_char('z');
    s.command_palette_add_char('z');
    s.command_palette_add_char('z');

    assert!(
        s.command_palette.filtered_indices.is_empty(),
        "query 'zzz' should match no commands"
    );
}

#[test]
fn backspace_widens_results() {
    let mut s = state_with("hello");
    s.toggle_command_palette();
    let all_count = s.command_palette.filtered_indices.len();

    s.command_palette_add_char('g');
    s.command_palette_add_char('o');
    s.command_palette_add_char('t');
    s.command_palette_add_char('o');
    let narrow_count = s.command_palette.filtered_indices.len();
    assert!(narrow_count <= all_count);

    s.command_palette_backspace();
    s.command_palette_backspace();
    s.command_palette_backspace();
    s.command_palette_backspace();
    assert_eq!(
        s.command_palette.filtered_indices.len(),
        all_count,
        "clearing query should restore all results"
    );
}

// ---------------------------------------------------------------------------
// Navigation (select_next / select_prev)
// ---------------------------------------------------------------------------

#[test]
fn select_next_wraps_around() {
    let mut s = state_with("hello");
    s.toggle_command_palette();
    let count = s.command_palette.filtered_indices.len();
    assert!(count > 0);

    // Advance to last item
    for _ in 0..count - 1 {
        s.command_palette_select_next();
    }
    assert_eq!(s.command_palette.selected, count - 1);

    // One more wraps to 0
    s.command_palette_select_next();
    assert_eq!(s.command_palette.selected, 0, "select_next should wrap around");
}

#[test]
fn select_prev_wraps_around() {
    let mut s = state_with("hello");
    s.toggle_command_palette();
    assert_eq!(s.command_palette.selected, 0);

    // Going back from 0 should wrap to last
    s.command_palette_select_prev();
    let count = s.command_palette.filtered_indices.len();
    assert_eq!(
        s.command_palette.selected,
        count - 1,
        "select_prev from 0 should wrap to last item"
    );
}

#[test]
fn select_prev_then_next_round_trips() {
    let mut s = state_with("hello");
    s.toggle_command_palette();
    s.command_palette_select_next();
    s.command_palette_select_next();
    assert_eq!(s.command_palette.selected, 2);

    s.command_palette_select_prev();
    assert_eq!(s.command_palette.selected, 1);
}

// ---------------------------------------------------------------------------
// Execute
// ---------------------------------------------------------------------------

#[test]
fn execute_returns_command_id_and_hides() {
    let mut s = state_with("hello");
    s.toggle_command_palette();

    // Navigate to "find" command
    s.command_palette_add_char('f');
    s.command_palette_add_char('i');
    s.command_palette_add_char('n');
    s.command_palette_add_char('d');

    // Find the "find" command in the filtered list
    let find_idx = s.command_palette.filtered_indices.iter().position(|&i| {
        s.command_palette.commands[i].id == "find"
    });
    assert!(find_idx.is_some(), "should find 'find' in filtered results");
    s.command_palette.selected = find_idx.unwrap();

    let result = s.command_palette_execute();
    assert_eq!(result, Some("find".to_string()));
    assert!(!s.command_palette.visible, "palette should hide after execute");
}

#[test]
fn execute_when_not_visible_returns_none() {
    let mut s = state_with("hello");
    let result = s.command_palette_execute();
    assert_eq!(result, None);
}

#[test]
fn execute_goto_line_switches_to_goto_mode() {
    let mut s = state_with("hello");
    s.toggle_command_palette();

    // Navigate to goto-line
    let goto_idx = s.command_palette.filtered_indices.iter().position(|&i| {
        s.command_palette.commands[i].id == "goto-line"
    });
    assert!(goto_idx.is_some());
    s.command_palette.selected = goto_idx.unwrap();

    let result = s.command_palette_execute();
    // goto-line switches to GotoLine mode, does not return id
    assert_eq!(result, None);
    assert!(s.command_palette.visible, "palette should stay visible for goto-line prompt");
    assert_eq!(s.command_palette.mode, PaletteMode::GotoLine);
    assert!(s.command_palette.query.is_empty(), "query should be cleared for line number input");
}

#[test]
fn execute_in_goto_line_mode_jumps_to_line() {
    let mut s = state_with("line one\nline two\nline three");
    s.toggle_command_palette();

    // Switch to GotoLine mode
    let goto_idx = s.command_palette.filtered_indices.iter().position(|&i| {
        s.command_palette.commands[i].id == "goto-line"
    });
    s.command_palette.selected = goto_idx.unwrap();
    s.command_palette_execute();

    // Now type a line number
    s.command_palette_add_char('2');
    let result = s.command_palette_execute();
    assert_eq!(result, None);
    assert!(!s.command_palette.visible, "palette should close after goto-line execute");

    // Cursor should be at the start of line 2 (0-indexed line 1)
    let line2_start = s.document.line_to_char(1);
    assert_eq!(
        s.cursor.position, line2_start,
        "cursor should be at start of line 2"
    );
}

// ---------------------------------------------------------------------------
// Dismiss
// ---------------------------------------------------------------------------

#[test]
fn dismiss_hides_and_clears() {
    let mut s = state_with("hello");
    s.toggle_command_palette();
    s.command_palette_add_char('x');
    assert!(s.command_palette.visible);

    s.command_palette_dismiss();
    assert!(!s.command_palette.visible);
    assert!(s.command_palette.query.is_empty());
    assert_eq!(s.command_palette.mode, PaletteMode::Command);
}

// ---------------------------------------------------------------------------
// dispatch_palette_command
// ---------------------------------------------------------------------------

#[test]
fn dispatch_palette_command_handles_toggle_preview() {
    let mut s = state_with("hello");
    let original = s.show_preview;
    let handled = s.dispatch_palette_command("toggle-preview");
    assert!(handled);
    assert_ne!(s.show_preview, original);
}

#[test]
fn dispatch_palette_command_handles_toggle_line_numbers() {
    let mut s = state_with("hello");
    let original = s.show_line_numbers;
    let handled = s.dispatch_palette_command("toggle-line-numbers");
    assert!(handled);
    assert_ne!(s.show_line_numbers, original);
}

#[test]
fn dispatch_palette_command_handles_select_all() {
    let mut s = state_with("hello world");
    let handled = s.dispatch_palette_command("select-all");
    assert!(handled);
    assert!(s.cursor.selection().is_some());
    let (start, end) = s.cursor.selection().unwrap();
    assert_eq!(start, 0);
    assert_eq!(end, 11);
}

#[test]
fn dispatch_palette_command_handles_upcase_word() {
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    let handled = s.dispatch_palette_command("upcase-word");
    assert!(handled);
    assert_eq!(s.document.text(), "HELLO world");
}

#[test]
fn dispatch_palette_command_handles_downcase_word() {
    let mut s = state_with("HELLO world");
    s.cursor.position = 0;
    let handled = s.dispatch_palette_command("downcase-word");
    assert!(handled);
    assert_eq!(s.document.text(), "hello world");
}

#[test]
fn dispatch_palette_command_unknown_returns_false() {
    let mut s = state_with("hello");
    let handled = s.dispatch_palette_command("nonexistent-command");
    assert!(!handled, "unknown command should return false");
}

#[test]
fn dispatch_palette_command_handles_find() {
    let mut s = state_with("hello");
    assert!(!s.find_bar_visible);
    let handled = s.dispatch_palette_command("find");
    assert!(handled);
    assert!(s.find_bar_visible);
}

#[test]
fn dispatch_palette_command_handles_exchange_point_and_mark() {
    let mut s = state_with("hello world");
    s.cursor.position = 5;
    s.cursor.anchor = Some(0);

    let handled = s.dispatch_palette_command("exchange-point-and-mark");
    assert!(handled);
    assert_eq!(s.cursor.position, 0);
    assert_eq!(s.cursor.anchor, Some(5));
}
