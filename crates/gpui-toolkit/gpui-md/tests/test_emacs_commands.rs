//! Tests for upcase_word, downcase_word, and abort.

use gpui_md::state::MdAppState;

fn state_with(text: &str) -> MdAppState {
    let mut s = MdAppState::new();
    s.document.set_text(text);
    s.cursor.position = 0;
    s.cursor.clear_selection();
    s
}

// ---------------------------------------------------------------------------
// upcase_word (M-u)
// ---------------------------------------------------------------------------

#[test]
fn upcase_word_at_start() {
    // Emacs M-u: uppercase from cursor to end of word (M-f position).
    // With emacs word movement: cursor at 0, M-f lands at 5 (end of "hello").
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    s.upcase_word();
    assert_eq!(s.document.text(), "HELLO world");
    assert_eq!(s.cursor.position, 5);
}

#[test]
fn upcase_word_mid_word() {
    let mut s = state_with("hello world");
    s.cursor.position = 3; // between 'l' and 'l'
    s.upcase_word();
    assert_eq!(s.document.text(), "helLO world");
    assert_eq!(s.cursor.position, 5);
}

#[test]
fn upcase_word_at_end_of_doc_is_noop() {
    let mut s = state_with("hello");
    s.cursor.position = 5;
    s.upcase_word();
    assert_eq!(s.document.text(), "hello");
    assert_eq!(s.cursor.position, 5);
}

#[test]
fn upcase_word_on_empty_doc() {
    let mut s = state_with("");
    s.cursor.position = 0;
    s.upcase_word();
    assert_eq!(s.document.text(), "");
    assert_eq!(s.cursor.position, 0);
}

#[test]
fn upcase_word_unicode() {
    let mut s = state_with("cafe\u{0301} latte");
    s.cursor.position = 0;
    s.upcase_word();
    let text = s.document.text();
    // The word "cafe\u{0301}" should be uppercased
    assert!(text.starts_with("CAFE\u{0301}") || text.starts_with("CAF\u{00C9}"),
        "expected uppercase of cafe\\u{{0301}}, got: {}", text);
}

#[test]
fn upcase_word_already_upper() {
    let mut s = state_with("HELLO world");
    s.cursor.position = 0;
    s.upcase_word();
    assert_eq!(s.document.text(), "HELLO world");
    assert_eq!(s.cursor.position, 5);
}

// ---------------------------------------------------------------------------
// downcase_word (M-l)
// ---------------------------------------------------------------------------

#[test]
fn downcase_word_at_start() {
    // Emacs M-l: lowercase from cursor to end of word.
    let mut s = state_with("HELLO world");
    s.cursor.position = 0;
    s.downcase_word();
    assert_eq!(s.document.text(), "hello world");
    assert_eq!(s.cursor.position, 5);
}

#[test]
fn downcase_word_mid_word() {
    let mut s = state_with("HELLO WORLD");
    s.cursor.position = 3;
    s.downcase_word();
    assert_eq!(s.document.text(), "HELlo WORLD");
    assert_eq!(s.cursor.position, 5);
}

#[test]
fn downcase_word_at_end_of_doc_is_noop() {
    let mut s = state_with("HELLO");
    s.cursor.position = 5;
    s.downcase_word();
    assert_eq!(s.document.text(), "HELLO");
    assert_eq!(s.cursor.position, 5);
}

#[test]
fn downcase_word_on_empty_doc() {
    let mut s = state_with("");
    s.cursor.position = 0;
    s.downcase_word();
    assert_eq!(s.document.text(), "");
    assert_eq!(s.cursor.position, 0);
}

#[test]
fn downcase_word_already_lower() {
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    s.downcase_word();
    assert_eq!(s.document.text(), "hello world");
    assert_eq!(s.cursor.position, 5);
}

// ---------------------------------------------------------------------------
// abort (C-g)
// ---------------------------------------------------------------------------

#[test]
fn abort_clears_selection() {
    let mut s = state_with("hello world");
    s.cursor.anchor = Some(0);
    s.cursor.position = 5;
    assert!(s.cursor.selection().is_some());

    s.abort();
    assert!(s.cursor.selection().is_none(), "abort should clear the selection");
}

#[test]
fn abort_closes_find_bar() {
    let mut s = state_with("hello world");
    s.find_bar_visible = true;
    s.find_query = "hello".to_string();
    s.replace_bar_visible = true;

    s.abort();
    assert!(!s.find_bar_visible, "abort should close find bar");
    assert!(s.find_query.is_empty(), "abort should clear find query");
    assert!(!s.replace_bar_visible, "abort should close replace bar");
}

#[test]
fn abort_cancels_universal_arg() {
    let mut s = state_with("hello");
    s.universal_argument(); // sets universal_arg to Some(4)
    assert!(s.universal_arg.is_some());

    s.abort();
    assert!(s.universal_arg.is_none(), "abort should cancel universal arg");
    assert!(!s.universal_arg_accumulating, "abort should stop digit accumulation");
}

#[test]
fn abort_cancels_isearch() {
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    s.isearch_start(gpui_md::state::IsearchDirection::Forward);
    s.isearch_add_char('w');
    // Cursor should have moved to 'w' in "world"
    let pos_during_search = s.cursor.position;
    assert!(pos_during_search > 0);

    s.abort();
    assert!(!s.isearch.active, "abort should deactivate isearch");
    assert_eq!(
        s.cursor.position, 0,
        "abort should restore cursor to start_position"
    );
}

#[test]
fn abort_dismisses_command_palette() {
    let mut s = state_with("hello");
    s.toggle_command_palette();
    assert!(s.command_palette.visible);

    s.abort();
    assert!(!s.command_palette.visible, "abort should close command palette");
}
