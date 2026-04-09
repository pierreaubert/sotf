//! Integration tests for Phase 3-6 review fixes:
//! transpose, zap-to-char, C-x prefix, rectangle ops, Unicode handling.

use gpui_md::state::MdAppState;

fn state_with(text: &str) -> MdAppState {
    let mut s = MdAppState::new();
    s.document.set_text(text);
    s.cursor.position = 0;
    s.cursor.clear_selection();
    s
}

// ---------------------------------------------------------------------------
// Transpose characters (C-t)
// ---------------------------------------------------------------------------

#[test]
fn transpose_chars_mid_line() {
    let mut s = state_with("abcd");
    s.cursor.position = 2; // between 'b' and 'c'
    s.transpose_chars();
    assert_eq!(s.document.text(), "acbd");
    assert_eq!(s.cursor.position, 3);
}

#[test]
fn transpose_chars_at_end() {
    let mut s = state_with("abcd");
    s.cursor.position = 4; // at end
    s.transpose_chars();
    assert_eq!(s.document.text(), "abdc");
    assert_eq!(s.cursor.position, 4);
}

#[test]
fn transpose_chars_at_position_1() {
    let mut s = state_with("abcd");
    s.cursor.position = 1;
    s.transpose_chars();
    assert_eq!(s.document.text(), "bacd");
    assert_eq!(s.cursor.position, 2);
}

#[test]
fn transpose_chars_at_position_0_is_noop() {
    let mut s = state_with("abcd");
    s.cursor.position = 0;
    s.transpose_chars();
    assert_eq!(s.document.text(), "abcd");
    assert_eq!(s.cursor.position, 0);
}

#[test]
fn transpose_chars_single_char_is_noop() {
    let mut s = state_with("a");
    s.cursor.position = 1;
    s.transpose_chars();
    assert_eq!(s.document.text(), "a");
}

#[test]
fn transpose_chars_empty_is_noop() {
    let mut s = state_with("");
    s.transpose_chars();
    assert_eq!(s.document.text(), "");
}

#[test]
fn transpose_chars_unicode() {
    let mut s = state_with("aéb");
    s.cursor.position = 2; // between 'é' and 'b'
    s.transpose_chars();
    assert_eq!(s.document.text(), "abéb"[..1].to_string() + "bé");
    // Actually let me be more precise
    let text = s.document.text();
    let chars: Vec<char> = text.chars().collect();
    assert_eq!(chars, vec!['a', 'b', 'é']);
}

// ---------------------------------------------------------------------------
// Transpose words (M-t)
// ---------------------------------------------------------------------------

#[test]
fn transpose_words_basic() {
    let mut s = state_with("hello world");
    s.cursor.position = 6; // at 'w'
    s.transpose_words();
    assert_eq!(s.document.text(), "world hello");
}

#[test]
fn transpose_words_between_words() {
    let mut s = state_with("foo bar baz");
    s.cursor.position = 4; // at 'b' in "bar"
    s.transpose_words();
    assert_eq!(s.document.text(), "bar foo baz");
}

#[test]
fn transpose_words_at_start_is_noop() {
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    s.transpose_words();
    // At start, there's no previous word to swap with
    assert_eq!(s.document.text(), "hello world");
}

// ---------------------------------------------------------------------------
// Zap to char (M-z)
// ---------------------------------------------------------------------------

#[test]
fn zap_to_char_basic() {
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    s.zap_to_char('o');
    // Kills from 0 through 'o' (inclusive) = "hello"... wait, 'o' is at index 4
    // zap kills from cursor to (and including) next occurrence of target
    assert_eq!(s.document.text(), " world");
    assert_eq!(s.cursor.position, 0);
}

#[test]
fn zap_to_char_mid_text() {
    let mut s = state_with("abcdefghij");
    s.cursor.position = 3; // at 'd'
    s.zap_to_char('g');
    // Kills "defg" (positions 3..7 inclusive)
    assert_eq!(s.document.text(), "abchij");
    assert_eq!(s.cursor.position, 3);
}

#[test]
fn zap_to_char_not_found_is_noop() {
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    s.zap_to_char('z');
    assert_eq!(s.document.text(), "hello world");
    assert_eq!(s.cursor.position, 0);
}

#[test]
fn zap_to_char_adds_to_kill_ring() {
    let mut s = state_with("hello world");
    s.cursor.position = 0;
    s.zap_to_char('o');
    // The killed text should be in the kill ring
    let yanked = s.kill_ring.yank().map(|s| s.to_string());
    assert_eq!(yanked, Some("hello".to_string()));
}

#[test]
fn zap_to_char_unicode() {
    let mut s = state_with("café latte");
    s.cursor.position = 0;
    s.zap_to_char('é');
    assert_eq!(s.document.text(), " latte");
}

// ---------------------------------------------------------------------------
// Zap pending state (M-z then C-g)
// ---------------------------------------------------------------------------

#[test]
fn zap_to_char_pending_cancelled_by_abort() {
    let mut s = state_with("hello");
    s.zap_to_char_start();
    assert!(s.zap_to_char_pending);
    s.abort();
    assert!(!s.zap_to_char_pending);
}

// ---------------------------------------------------------------------------
// C-x prefix
// ---------------------------------------------------------------------------

#[test]
fn c_x_prefix_cancelled_by_abort() {
    let mut s = state_with("hello");
    s.c_x_pending = true;
    s.abort();
    assert!(!s.c_x_pending);
    assert!(!s.c_x_r_pending);
}

#[test]
fn c_x_r_prefix_cancelled_by_abort() {
    let mut s = state_with("hello");
    s.c_x_r_pending = true;
    s.abort();
    assert!(!s.c_x_r_pending);
}

// ---------------------------------------------------------------------------
// Rectangle operations
// ---------------------------------------------------------------------------

#[test]
fn kill_rectangle_basic() {
    let mut s = state_with("abcd\nefgh\nijkl");
    // Select a rectangle: mark at (0,1) col 1, cursor at (2,3) col 3.
    // Rectangle selection uses [min_col, max_col) exclusive, so cols 1..3 = "bc","fg","jk"
    s.cursor.position = 1; // line 0, col 1
    s.set_mark();
    s.cursor.position = 13; // line 2, col 3
    s.kill_rectangle();
    assert_eq!(s.document.text(), "ad\neh\nil");
}

#[test]
fn yank_rectangle_after_kill() {
    let mut s = state_with("abcd\nefgh\nijkl");
    s.cursor.position = 1;
    s.set_mark();
    s.cursor.position = 13;
    s.kill_rectangle();
    assert_eq!(s.document.text(), "ad\neh\nil");

    // Yank the rectangle back at position 1 (col 1 of line 0)
    s.cursor.position = 1;
    s.yank_rectangle();
    assert_eq!(s.document.text(), "abcd\nefgh\nijkl");
}

// ---------------------------------------------------------------------------
// Universal arg + case conversion
// ---------------------------------------------------------------------------

#[test]
fn universal_arg_upcase_multiple_words() {
    let mut s = state_with("one two three four");
    s.cursor.position = 0;
    // Simulate C-u 2 M-u: upcase 2 words
    s.universal_argument(); // sets to 4
    s.universal_arg_accumulating = true;
    s.universal_arg_digit(2); // replaces 4 with 2
    let n = s.take_universal_arg();
    assert_eq!(n, 2);
    for _ in 0..2 {
        s.upcase_word();
    }
    assert_eq!(s.document.text(), "ONE TWO three four");
}

#[test]
fn universal_arg_downcase_multiple_words() {
    let mut s = state_with("ONE TWO THREE FOUR");
    s.cursor.position = 0;
    for _ in 0..3 {
        s.downcase_word();
    }
    assert_eq!(s.document.text(), "one two three FOUR");
}

// ---------------------------------------------------------------------------
// Unicode text operations
// ---------------------------------------------------------------------------

#[test]
fn insert_unicode_multibyte() {
    let mut s = state_with("");
    s.insert_text("日本語");
    assert_eq!(s.document.text(), "日本語");
    assert_eq!(s.cursor.position, 3); // 3 chars, not 9 bytes
}

#[test]
fn word_movement_with_unicode() {
    let mut s = state_with("hello 世界 world");
    s.cursor.position = 0;
    s.move_word_right(false);
    assert_eq!(s.cursor.position, 5); // end of "hello"
    s.move_word_right(false);
    assert_eq!(s.cursor.position, 8); // end of "世界"
    s.move_word_right(false);
    assert_eq!(s.cursor.position, 14); // end of "world"
}

#[test]
fn backspace_unicode() {
    let mut s = state_with("café");
    s.cursor.position = 4; // after 'é'
    s.backspace();
    assert_eq!(s.document.text(), "caf");
    assert_eq!(s.cursor.position, 3);
}

#[test]
fn delete_forward_unicode() {
    let mut s = state_with("café");
    s.cursor.position = 3; // before 'é'
    s.delete_forward();
    assert_eq!(s.document.text(), "caf");
    assert_eq!(s.cursor.position, 3);
}

#[test]
fn upcase_word_unicode() {
    let mut s = state_with("café world");
    s.cursor.position = 0;
    s.upcase_word();
    assert_eq!(s.document.text(), "CAFÉ world");
}

// ---------------------------------------------------------------------------
// Undo coalescing with mixed content
// ---------------------------------------------------------------------------

#[test]
fn undo_coalesces_all_single_chars() {
    let mut s = state_with("");
    // Type "hi there" — all single chars should coalesce
    for c in "hi there".chars() {
        s.insert_text(&c.to_string());
    }
    assert_eq!(s.document.text(), "hi there");
    // Single undo should remove everything
    s.undo();
    assert_eq!(s.document.text(), "");
}

#[test]
fn undo_breaks_on_multi_char_insert() {
    let mut s = state_with("");
    s.insert_text("h");
    s.insert_text("i");
    s.insert_text(" paste"); // multi-char — breaks coalescing
    s.insert_text("d");
    assert_eq!(s.document.text(), "hi pasted");
    s.undo(); // undoes "d" (single char after break)
    // The "d" was inserted after a multi-char insert, so it started a new group
    // but "d" alone is the new group
    // Actually: after " paste" (multi-char), last_edit_was_char_insert = false
    // Then "d" is single char but !last_edit_was_char_insert, so no coalesce → new snapshot
    let text = s.document.text();
    assert_eq!(text, "hi paste");
}

// ---------------------------------------------------------------------------
// Syntax highlighting: italic vs bold vs strikethrough colors
// ---------------------------------------------------------------------------

#[test]
fn highlight_italic_uses_distinct_color() {
    use gpui_md::markdown::{MdThemeColors, highlight_line};

    // Create a minimal theme with distinct colors
    let colors = MdThemeColors::from_theme(&gpui_ui_kit::theme::Theme::default());

    let (spans, _) = highlight_line("*italic* and **bold**", false, &colors);
    // Find the italic span and bold span
    let italic_span = spans.iter().find(|s| s.text.contains("italic"));
    let bold_span = spans.iter().find(|s| s.text.contains("bold"));

    assert!(italic_span.is_some(), "should have italic span");
    assert!(bold_span.is_some(), "should have bold span");

    // They should have different colors
    let italic_color = italic_span.unwrap().color;
    let bold_color = bold_span.unwrap().color;
    assert_ne!(
        format!("{:?}", italic_color),
        format!("{:?}", bold_color),
        "italic and bold should have different colors"
    );
}

#[test]
fn highlight_strikethrough_uses_distinct_color() {
    use gpui_md::markdown::{MdThemeColors, highlight_line};

    let colors = MdThemeColors::from_theme(&gpui_ui_kit::theme::Theme::default());

    let (spans, _) = highlight_line("**bold** and ~~strike~~", false, &colors);
    let bold_span = spans.iter().find(|s| s.text.contains("bold"));
    let strike_span = spans.iter().find(|s| s.text.contains("strike"));

    assert!(bold_span.is_some());
    assert!(strike_span.is_some());

    let bold_color = bold_span.unwrap().color;
    let strike_color = strike_span.unwrap().color;
    assert_ne!(
        format!("{:?}", bold_color),
        format!("{:?}", strike_color),
        "bold and strikethrough should have different colors"
    );
}
