use gpui_md::state::MdAppState;

fn state_with(text: &str) -> MdAppState {
    let mut s = MdAppState::new();
    s.document.set_text(text);
    s.cursor.position = 0;
    s.cursor.clear_selection();
    s
}

// ---- Basic editing ----

#[test]
fn insert_text_at_start() {
    let mut s = state_with("");
    s.insert_text("hello");
    assert_eq!(s.document.text(), "hello");
    assert_eq!(s.cursor.position, 5);
}

#[test]
fn insert_text_at_middle() {
    let mut s = state_with("hllo");
    s.cursor.position = 1;
    s.insert_text("e");
    assert_eq!(s.document.text(), "hello");
    assert_eq!(s.cursor.position, 2);
}

#[test]
fn insert_replaces_selection() {
    let mut s = state_with("hello world");
    s.cursor.anchor = Some(5);
    s.cursor.position = 11;
    s.insert_text("!");
    assert_eq!(s.document.text(), "hello!");
    assert_eq!(s.cursor.position, 6);
}

#[test]
fn backspace_deletes_before_cursor() {
    let mut s = state_with("hello");
    s.cursor.position = 5;
    s.backspace();
    assert_eq!(s.document.text(), "hell");
    assert_eq!(s.cursor.position, 4);
}

#[test]
fn backspace_at_start_is_noop() {
    let mut s = state_with("hello");
    s.cursor.position = 0;
    s.backspace();
    assert_eq!(s.document.text(), "hello");
}

#[test]
fn backspace_deletes_selection() {
    let mut s = state_with("hello world");
    s.cursor.anchor = Some(5);
    s.cursor.position = 11;
    s.backspace();
    assert_eq!(s.document.text(), "hello");
    assert_eq!(s.cursor.position, 5);
}

#[test]
fn delete_forward() {
    let mut s = state_with("hello");
    s.cursor.position = 0;
    s.delete_forward();
    assert_eq!(s.document.text(), "ello");
}

#[test]
fn delete_forward_at_end_is_noop() {
    let mut s = state_with("hello");
    s.cursor.position = 5;
    s.delete_forward();
    assert_eq!(s.document.text(), "hello");
}

// ---- Cursor movement ----

#[test]
fn move_left() {
    let mut s = state_with("abc");
    s.cursor.position = 2;
    s.move_left(false);
    assert_eq!(s.cursor.position, 1);
}

#[test]
fn move_left_at_start() {
    let mut s = state_with("abc");
    s.cursor.position = 0;
    s.move_left(false);
    assert_eq!(s.cursor.position, 0);
}

#[test]
fn move_left_collapses_selection() {
    let mut s = state_with("abcdef");
    s.cursor.anchor = Some(2);
    s.cursor.position = 5;
    s.move_left(false);
    // Should collapse to selection start
    assert_eq!(s.cursor.position, 2);
    assert!(!s.cursor.has_selection());
}

#[test]
fn move_right() {
    let mut s = state_with("abc");
    s.cursor.position = 1;
    s.move_right(false);
    assert_eq!(s.cursor.position, 2);
}

#[test]
fn move_right_collapses_selection() {
    let mut s = state_with("abcdef");
    s.cursor.anchor = Some(2);
    s.cursor.position = 5;
    s.move_right(false);
    // Should collapse to selection end
    assert_eq!(s.cursor.position, 5);
    assert!(!s.cursor.has_selection());
}

#[test]
fn move_up_from_second_line() {
    let mut s = state_with("abc\ndef");
    s.cursor.position = 6; // 'e' on second line (col 2)
    s.move_up(false);
    assert_eq!(s.cursor.position, 2); // col 2 on first line = 'c'
}

#[test]
fn move_up_from_first_line_goes_to_start() {
    let mut s = state_with("abc\ndef");
    s.cursor.position = 2;
    s.move_up(false);
    assert_eq!(s.cursor.position, 0);
}

#[test]
fn move_down_from_first_line() {
    let mut s = state_with("abc\ndef");
    s.cursor.position = 1; // col 1 on first line
    s.move_down(false);
    assert_eq!(s.cursor.position, 5); // col 1 on second line = 'e'
}

#[test]
fn move_down_from_last_line_goes_to_end() {
    let mut s = state_with("abc\ndef");
    s.cursor.position = 5;
    s.move_down(false);
    assert_eq!(s.cursor.position, 7); // end of doc
}

#[test]
fn move_down_clamps_to_shorter_line() {
    let mut s = state_with("abcdef\nhi");
    s.cursor.position = 5; // col 5 on first line
    s.move_down(false);
    // Second line only has 2 chars, so clamp to col 2
    assert_eq!(s.cursor.position, 9); // line_start(1)=7, col=min(5,2)=2 → 9
}

#[test]
fn move_to_line_start() {
    let mut s = state_with("abc\ndef");
    s.cursor.position = 6;
    s.move_to_line_start(false);
    assert_eq!(s.cursor.position, 4); // start of "def"
}

#[test]
fn move_to_line_end() {
    let mut s = state_with("abc\ndef");
    s.cursor.position = 4;
    s.move_to_line_end(false);
    assert_eq!(s.cursor.position, 7); // end of "def"
}

#[test]
fn move_word_right() {
    let mut s = state_with("hello world foo");
    s.cursor.position = 0;
    s.move_word_right(false);
    assert_eq!(s.cursor.position, 5); // emacs M-f: end of "hello"
}

#[test]
fn move_word_left() {
    let mut s = state_with("hello world");
    s.cursor.position = 11;
    s.move_word_left(false);
    assert_eq!(s.cursor.position, 6); // before "world"
}

#[test]
fn move_with_extend_creates_selection() {
    let mut s = state_with("hello");
    s.cursor.position = 0;
    s.move_right(true);
    s.move_right(true);
    s.move_right(true);
    assert_eq!(s.cursor.position, 3);
    assert_eq!(s.cursor.anchor, Some(0));
    assert_eq!(s.cursor.selection(), Some((0, 3)));
}

// ---- Selection operations ----

#[test]
fn select_all() {
    let mut s = state_with("hello world");
    s.select_all();
    assert_eq!(s.cursor.anchor, Some(0));
    assert_eq!(s.cursor.position, 11);
}

#[test]
fn select_word_at_middle_of_word() {
    let mut s = state_with("hello world");
    s.select_word_at(7); // 'o' in "world"
    assert_eq!(s.cursor.anchor, Some(6));
    assert_eq!(s.cursor.position, 11);
}

#[test]
fn select_word_at_start_of_word() {
    let mut s = state_with("hello world");
    s.select_word_at(6); // 'w' in "world"
    assert_eq!(s.cursor.anchor, Some(6));
    assert_eq!(s.cursor.position, 11);
}

#[test]
fn select_word_at_end_of_word() {
    let mut s = state_with("hello world");
    s.select_word_at(4); // 'o' in "hello" (last char)
    assert_eq!(s.cursor.anchor, Some(0));
    assert_eq!(s.cursor.position, 5);
}

#[test]
fn select_word_at_space() {
    let mut s = state_with("hello world");
    s.select_word_at(5); // space between words
    // Should select the space run
    assert_eq!(s.cursor.anchor, Some(5));
    assert_eq!(s.cursor.position, 6);
}

#[test]
fn select_word_at_first_char() {
    let mut s = state_with("hello world");
    s.select_word_at(0); // 'h' in "hello"
    assert_eq!(s.cursor.anchor, Some(0));
    assert_eq!(s.cursor.position, 5);
}

#[test]
fn select_word_at_last_char() {
    let mut s = state_with("hello world");
    s.select_word_at(10); // 'd' in "world"
    assert_eq!(s.cursor.anchor, Some(6));
    assert_eq!(s.cursor.position, 11);
}

#[test]
fn select_word_with_underscores() {
    let mut s = state_with("foo_bar baz");
    s.select_word_at(3); // '_' in "foo_bar"
    assert_eq!(s.cursor.anchor, Some(0));
    assert_eq!(s.cursor.position, 7);
}

#[test]
fn select_word_at_punctuation() {
    let mut s = state_with("hello, world");
    s.select_word_at(5); // ',' after "hello"
    // comma is not alphanumeric/_  so it should select just the comma
    assert_eq!(s.cursor.anchor, Some(5));
    assert_eq!(s.cursor.position, 6);
}

#[test]
fn select_word_multiple_spaces() {
    let mut s = state_with("hello   world");
    s.select_word_at(6); // middle of triple-space
    // Should select all three spaces
    assert_eq!(s.cursor.anchor, Some(5));
    assert_eq!(s.cursor.position, 8);
}

#[test]
fn select_word_on_second_line() {
    let mut s = state_with("first\nsecond word");
    s.select_word_at(6); // 's' in "second"
    assert_eq!(s.cursor.anchor, Some(6));
    assert_eq!(s.cursor.position, 12);
}

#[test]
fn select_word_at_pos_beyond_text() {
    // Cursor at end of text (past last char)
    let mut s = state_with("hello");
    s.select_word_at(5); // one past last char — should clamp
    // Should select "hello" (pos clamped to 4, which is in the word)
    assert_eq!(s.cursor.anchor, Some(0));
    assert_eq!(s.cursor.position, 5);
}

#[test]
fn select_line_at() {
    let mut s = state_with("line1\nline2\nline3");
    s.select_line_at(7); // somewhere in "line2"
    assert_eq!(s.cursor.anchor, Some(6));
    assert_eq!(s.cursor.position, 12); // includes the '\n'
}

#[test]
fn select_last_line() {
    let mut s = state_with("line1\nline2");
    s.select_line_at(8); // somewhere in "line2"
    assert_eq!(s.cursor.anchor, Some(6));
    assert_eq!(s.cursor.position, 11); // end of doc
}

// ---- Undo/Redo ----

#[test]
fn undo_redo_cycle() {
    let mut s = state_with("hello");
    s.cursor.position = 5;
    s.insert_text(" world");
    assert_eq!(s.document.text(), "hello world");

    s.undo();
    assert_eq!(s.document.text(), "hello");

    s.redo();
    assert_eq!(s.document.text(), "hello world");
}

#[test]
fn undo_clamps_cursor() {
    let mut s = state_with("hi");
    s.cursor.position = 2;
    s.insert_text(" there you go");
    assert_eq!(s.cursor.position, 15);
    s.undo();
    // Cursor should be clamped to end of restored text
    assert!(s.cursor.position <= s.document.len_chars());
}

// ---- Kill ring operations ----

#[test]
fn kill_to_end_of_line() {
    let mut s = state_with("hello world\nbye");
    s.cursor.position = 5; // after "hello"
    s.kill_to_end_of_line();
    assert_eq!(s.document.text(), "hello\nbye");
    assert_eq!(s.kill_ring.yank(), Some(" world"));
}

#[test]
fn kill_to_end_of_line_at_eol_kills_newline() {
    let mut s = state_with("hello\nworld");
    s.cursor.position = 5; // at the newline
    s.kill_to_end_of_line();
    assert_eq!(s.document.text(), "helloworld");
    assert_eq!(s.kill_ring.yank(), Some("\n"));
}

#[test]
fn kill_to_start_of_line() {
    let mut s = state_with("hello world");
    s.cursor.position = 5;
    s.kill_to_start_of_line();
    assert_eq!(s.document.text(), " world");
    assert_eq!(s.cursor.position, 0);
    assert_eq!(s.kill_ring.yank(), Some("hello"));
}

#[test]
fn kill_word_forward() {
    // Emacs M-d: kills from cursor to end of word (not including trailing space)
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    s.kill_word_forward();
    assert_eq!(s.document.text(), " world");
    assert_eq!(s.kill_ring.yank(), Some("hello"));
}

#[test]
fn kill_word_backward() {
    let mut s = state_with("hello world");
    s.cursor.position = 11;
    s.kill_word_backward();
    assert_eq!(s.document.text(), "hello ");
    assert_eq!(s.kill_ring.yank(), Some("world"));
}

#[test]
fn kill_region() {
    let mut s = state_with("hello world");
    s.cursor.anchor = Some(5);
    s.cursor.position = 11;
    s.kill_region();
    assert_eq!(s.document.text(), "hello");
    assert_eq!(s.kill_ring.yank(), Some(" world"));
}

#[test]
fn copy_region_does_not_delete() {
    let mut s = state_with("hello world");
    s.cursor.anchor = Some(6);
    s.cursor.position = 11;
    s.copy_region();
    assert_eq!(s.document.text(), "hello world"); // unchanged
    assert_eq!(s.kill_ring.yank(), Some("world"));
    assert!(!s.cursor.has_selection()); // selection cleared
}

#[test]
fn yank_inserts_text() {
    let mut s = state_with("abc");
    s.kill_ring.push("XYZ".to_string());
    s.cursor.position = 3;
    s.yank();
    assert_eq!(s.document.text(), "abcXYZ");
    assert_eq!(s.cursor.position, 6);
}

#[test]
fn yank_pop_replaces_last_yank() {
    let mut s = state_with("");
    s.kill_ring.push("first".to_string());
    s.kill_ring.clear_kill_flag();
    s.kill_ring.push("second".to_string());

    s.yank(); // inserts "second"
    assert_eq!(s.document.text(), "second");

    s.yank_pop(); // replaces "second" with "first"
    assert_eq!(s.document.text(), "first");
}

// ---- Formatting ----

#[test]
fn toggle_bold_no_selection() {
    let mut s = state_with("hello");
    s.cursor.position = 5;
    s.toggle_format("**", "**");
    assert_eq!(s.document.text(), "hello****");
    assert_eq!(s.cursor.position, 7); // between the markers
}

#[test]
fn toggle_bold_with_selection() {
    let mut s = state_with("hello world");
    s.cursor.anchor = Some(6);
    s.cursor.position = 11;
    s.toggle_format("**", "**");
    assert_eq!(s.document.text(), "hello **world**");
}

// ---- Find ----

#[test]
fn find_next_basic() {
    let mut s = state_with("hello world hello");
    s.find_query = "hello".to_string();
    s.cursor.position = 0;
    s.find_next();
    // Should find the second "hello" (skips current position)
    assert_eq!(s.cursor.position, 12);
}

#[test]
fn find_next_wraps_around() {
    let mut s = state_with("hello world");
    s.find_query = "hello".to_string();
    s.cursor.position = 5; // past the only match
    s.find_next();
    assert_eq!(s.cursor.position, 0); // wraps to beginning
}

#[test]
fn replace_all() {
    let mut s = state_with("foo bar foo baz foo");
    s.find_query = "foo".to_string();
    s.replace_text = "X".to_string();
    s.replace_all();
    assert_eq!(s.document.text(), "X bar X baz X");
}

#[test]
fn replace_all_clamps_cursor() {
    let mut s = state_with("aaa bbb ccc");
    s.cursor.position = 11;
    s.find_query = "bbb ccc".to_string();
    s.replace_text = "x".to_string();
    s.replace_all();
    assert_eq!(s.document.text(), "aaa x");
    assert!(s.cursor.position <= s.document.len_chars());
}

// ---- Bug regression: double-click "here" selects space before it ----

#[test]
fn select_word_here_in_default_text() {
    // Simulates double-clicking "here" in "Start typing your markdown here."
    let text = "# Welcome to gpui-md\n\nStart typing your markdown here.\n";
    let mut s = state_with(text);

    // Find the position of 'h' in "here"
    let here_pos = text.find("here").unwrap();
    // "here" should be at chars: find byte pos, convert to char pos
    let char_pos = text[..here_pos].chars().count();

    s.select_word_at(char_pos);

    let selected = &text[s.cursor.anchor.unwrap()..s.cursor.position]
        .chars()
        .collect::<String>();
    assert_eq!(
        selected, "here",
        "Double-clicking on 'h' in 'here' should select 'here', not '{}'",
        selected
    );
}

#[test]
fn select_word_here_click_on_e() {
    // Click on the 'e' at the end of "here"
    let text = "Start typing your markdown here.\n";
    let mut s = state_with(text);

    let here_pos = text.find("here").unwrap();
    let char_pos = text[..here_pos].chars().count() + 3; // 'e' at end of "here"

    s.select_word_at(char_pos);

    let anchor = s.cursor.anchor.unwrap();
    let pos = s.cursor.position;
    let selected: String = text.chars().skip(anchor).take(pos - anchor).collect();
    assert_eq!(selected, "here");
}

#[test]
fn select_word_does_not_include_leading_space() {
    let mut s = state_with("foo bar baz");
    // Click on 'b' in "bar" (char 4)
    s.select_word_at(4);
    let anchor = s.cursor.anchor.unwrap();
    let pos = s.cursor.position;
    let selected: String = "foo bar baz".chars().skip(anchor).take(pos - anchor).collect();
    assert_eq!(selected, "bar", "Should select 'bar', got '{}'", selected);
    assert_eq!(anchor, 4); // 'b' starts at 4
    assert_eq!(pos, 7);    // after 'r'
}

// ---- Edge cases ----

#[test]
fn operations_on_empty_document() {
    let mut s = state_with("");
    s.move_left(false);
    s.move_right(false);
    s.move_up(false);
    s.move_down(false);
    s.move_to_line_start(false);
    s.move_to_line_end(false);
    s.backspace();
    s.delete_forward();
    s.kill_to_end_of_line();
    s.kill_to_start_of_line();
    assert_eq!(s.document.text(), "");
    assert_eq!(s.cursor.position, 0);
}

#[test]
fn unicode_cursor_movement() {
    let mut s = state_with("café");
    s.cursor.position = 0;
    s.move_right(false);
    assert_eq!(s.cursor.position, 1); // 'c'
    s.move_right(false);
    assert_eq!(s.cursor.position, 2); // 'a'
    s.move_right(false);
    assert_eq!(s.cursor.position, 3); // 'f'
    s.move_right(false);
    assert_eq!(s.cursor.position, 4); // 'é'
}

#[test]
fn multiline_cursor_boundary() {
    let mut s = state_with("ab\ncd");
    s.cursor.position = 2; // at '\n'
    s.move_right(false);
    assert_eq!(s.cursor.position, 3); // 'c' on next line
    s.move_left(false);
    assert_eq!(s.cursor.position, 2); // back to '\n'
}
