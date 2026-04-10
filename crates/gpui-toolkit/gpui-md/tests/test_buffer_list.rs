//! Tests for the buffer list / multi-buffer state machine.

use std::path::PathBuf;

use gpui_md::state::MdAppState;

fn dummy_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

#[test]
fn new_state_has_one_buffer() {
    let state = MdAppState::new();
    assert_eq!(state.buffer_count(), 1);
    assert_eq!(state.current_buffer_name, "*scratch*");
}

#[test]
fn from_file_sets_buffer_name() {
    let state = MdAppState::from_file(dummy_path("foo.md"), "# Hello");
    assert_eq!(state.buffer_count(), 1);
    assert_eq!(state.current_buffer_name, "foo.md");
}

#[test]
fn create_file_buffer_adds_to_stored() {
    let mut state = MdAppState::new();
    state.create_file_buffer(dummy_path("a.md"), "content a");
    assert_eq!(state.buffer_count(), 2);
    let names = state.buffer_names();
    assert_eq!(names[0], "*scratch*"); // current (still active)
    assert_eq!(names[1], "a.md");      // newly stored
}

#[test]
fn create_file_buffer_dedupes_by_path() {
    let mut state = MdAppState::new();
    let path = dummy_path("dedupe.md");
    let id1 = state.create_file_buffer(path.clone(), "first");
    let id2 = state.create_file_buffer(path.clone(), "second");
    assert_eq!(id1, id2);
    assert_eq!(state.buffer_count(), 2); // scratch + one file buffer
}

#[test]
fn switch_to_buffer_by_name() {
    let mut state = MdAppState::from_file(dummy_path("first.md"), "first content");
    state.create_file_buffer(dummy_path("second.md"), "second content");
    assert_eq!(state.current_buffer_name, "first.md");

    let ok = state.switch_to_buffer_by_name("second.md");
    assert!(ok);
    assert_eq!(state.current_buffer_name, "second.md");
    assert!(state.document.text().contains("second content"));
}

#[test]
fn switch_preserves_other_buffer_contents() {
    let mut state = MdAppState::from_file(dummy_path("a.md"), "content of a");
    state.create_file_buffer(dummy_path("b.md"), "content of b");

    // Active: a.md. Switch to b.md.
    state.switch_to_buffer_by_name("b.md");
    assert!(state.document.text().contains("content of b"));

    // Now switch back — a.md should still have its content
    state.switch_to_buffer_by_name("a.md");
    assert!(state.document.text().contains("content of a"));
}

#[test]
fn kill_buffer_removes_from_list() {
    let mut state = MdAppState::from_file(dummy_path("a.md"), "a");
    state.create_file_buffer(dummy_path("b.md"), "b");
    assert_eq!(state.buffer_count(), 2);

    let killed = state.kill_buffer_by_name("b.md");
    assert!(killed);
    assert_eq!(state.buffer_count(), 1);
}

#[test]
fn cannot_kill_last_buffer() {
    let mut state = MdAppState::new();
    assert_eq!(state.buffer_count(), 1);
    let killed = state.kill_current_buffer();
    assert!(!killed, "killing the last buffer must be refused");
    assert_eq!(state.buffer_count(), 1);
}

#[test]
fn killing_active_buffer_switches_to_another() {
    let mut state = MdAppState::from_file(dummy_path("a.md"), "a");
    state.create_file_buffer(dummy_path("b.md"), "b");
    let killed = state.kill_current_buffer();
    assert!(killed);
    assert_eq!(state.buffer_count(), 1);
    assert_eq!(state.current_buffer_name, "b.md");
    assert!(state.document.text().contains("b"));
}

#[test]
fn switch_to_next_buffer_cycles() {
    let mut state = MdAppState::from_file(dummy_path("a.md"), "a");
    state.create_file_buffer(dummy_path("b.md"), "b");
    state.create_file_buffer(dummy_path("c.md"), "c");
    // Active: a.md, stored: [b, c]
    state.switch_to_next_buffer();
    // After swap with index 0, active: b, stored: [c, a]
    assert_eq!(state.current_buffer_name, "b.md");
    state.switch_to_next_buffer();
    assert_eq!(state.current_buffer_name, "c.md");
}

#[test]
fn find_buffer_by_path_finds_active() {
    let path = dummy_path("active.md");
    let state = MdAppState::from_file(path.clone(), "hello");
    assert!(state.find_buffer_by_path(&path).is_some());
}

#[test]
fn find_buffer_by_path_finds_stored() {
    let mut state = MdAppState::new();
    let path = dummy_path("stored.md");
    state.create_file_buffer(path.clone(), "hi");
    assert!(state.find_buffer_by_path(&path).is_some());
}

#[test]
fn duplicate_name_collisions_are_disambiguated() {
    let mut state = MdAppState::new();
    // Create two buffers with the same base name but different paths
    state.create_file_buffer(PathBuf::from("/tmp/a/README.md"), "one");
    state.create_file_buffer(PathBuf::from("/tmp/b/README.md"), "two");
    let names = state.buffer_names();
    // First one gets plain README.md, second gets README.md<2>
    assert!(names.iter().any(|n| n == "README.md"));
    assert!(names.iter().any(|n| n == "README.md<2>"));
}

#[test]
fn switch_preserves_selection() {
    let mut state = MdAppState::from_file(dummy_path("sel_a.md"), "content of a");
    state.create_file_buffer(dummy_path("sel_b.md"), "content of b");

    // Make a selection in a
    state.cursor.position = 5;
    state.cursor.anchor = Some(0);
    assert!(state.cursor.selection().is_some());

    // Switch to b — its own cursor should have no selection
    state.switch_to_buffer_by_name("sel_b.md");
    assert!(state.cursor.selection().is_none());

    // Switch back to a — selection must be restored
    state.switch_to_buffer_by_name("sel_a.md");
    assert_eq!(state.cursor.selection(), Some((0, 5)));
}

#[test]
fn kill_buffer_cleans_up_dired_state() {
    let mut state = MdAppState::new();
    // Open a dired buffer on the temp directory
    state.dired_open(std::env::temp_dir());
    let dired_id = state.current_buffer_id;
    assert!(state.dired_states.contains_key(&dired_id));

    // Create another buffer to make killing possible
    state.create_file_buffer(dummy_path("other.md"), "other");

    // Kill the dired buffer
    state.kill_buffer_id(dired_id);
    assert!(
        !state.dired_states.contains_key(&dired_id),
        "dired_states should be cleaned up on kill"
    );
}

#[test]
fn edit_is_scoped_to_active_buffer() {
    let mut state = MdAppState::from_file(dummy_path("edit_a.md"), "aaa");
    state.create_file_buffer(dummy_path("edit_b.md"), "bbb");

    // Edit active buffer (a)
    state.cursor.position = state.document.len_chars();
    state.insert_text("!");
    assert_eq!(state.document.text(), "aaa!");

    // Switch to b — a's edit must persist, b must be untouched
    state.switch_to_buffer_by_name("edit_b.md");
    assert_eq!(state.document.text(), "bbb");

    // Switch back to a — edit is still there
    state.switch_to_buffer_by_name("edit_a.md");
    assert_eq!(state.document.text(), "aaa!");
}
