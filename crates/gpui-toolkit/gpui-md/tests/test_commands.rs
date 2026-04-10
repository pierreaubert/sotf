//! Integration tests for the command registry.

use std::rc::Rc;

use gpui_md::commands::{Command, CommandArgs, CommandCategory, InteractiveSpec};
use gpui_md::state::MdAppState;

#[test]
fn builtin_commands_are_registered() {
    let state = MdAppState::new();
    assert!(state.commands.len() > 20);
    assert!(state.commands.get("forward-char").is_some());
    assert!(state.commands.get("backward-char").is_some());
    assert!(state.commands.get("find-file").is_some());
    assert!(state.commands.get("switch-to-buffer").is_some());
}

#[test]
fn run_forward_char_moves_cursor() {
    let mut state = MdAppState::new();
    state.document.set_text("hello world");
    state.cursor.position = 0;
    state.run_command_by_name("forward-char");
    assert_eq!(state.cursor.position, 1);
}

#[test]
fn run_forward_char_with_prefix_arg() {
    let mut state = MdAppState::new();
    state.document.set_text("hello world");
    state.cursor.position = 0;
    state.run_command_direct("forward-char", CommandArgs::default().with_prefix(5));
    assert_eq!(state.cursor.position, 5);
}

#[test]
fn register_custom_command_at_runtime() {
    let mut state = MdAppState::new();
    state.commands.register(Command {
        name: "insert-banner".to_string(),
        description: "Insert a banner".to_string(),
        category: CommandCategory::Custom,
        handler: Rc::new(|s, _| {
            s.insert_text("*** BANNER ***");
        }),
        interactive: InteractiveSpec::None,
    });
    state.document.set_text("");
    state.cursor.position = 0;
    state.run_command_by_name("insert-banner");
    assert_eq!(state.document.text(), "*** BANNER ***");
}

#[test]
fn run_unknown_command_is_noop() {
    let mut state = MdAppState::new();
    state.document.set_text("hello");
    state.cursor.position = 3;
    state.run_command_by_name("nonexistent-command");
    // Nothing changes
    assert_eq!(state.document.text(), "hello");
    assert_eq!(state.cursor.position, 3);
}

#[test]
fn switch_to_buffer_command_opens_minibuffer() {
    let mut state = MdAppState::new();
    assert!(!state.minibuffer.active);
    state.run_command_by_name("switch-to-buffer");
    assert!(state.minibuffer.active);
}

#[test]
fn registry_filter_returns_matches() {
    let state = MdAppState::new();
    let filtered = state.commands.filter("forward");
    assert!(filtered.contains(&"forward-char".to_string()));
    assert!(filtered.contains(&"forward-word".to_string()));
}

#[test]
fn toggle_preview_command_flips_flag() {
    let mut state = MdAppState::new();
    let before = state.show_preview;
    state.run_command_by_name("toggle-preview");
    assert_eq!(state.show_preview, !before);
}

#[test]
fn undo_command_reverts_edit() {
    let mut state = MdAppState::new();
    state.document.set_text("initial");
    state.cursor.position = 7;
    state.insert_text(" more");
    assert_eq!(state.document.text(), "initial more");
    state.run_command_by_name("undo");
    assert_eq!(state.document.text(), "initial");
}
