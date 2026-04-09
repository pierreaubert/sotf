//! TDD tests for syntax highlighting bugs.
//!
//! Issue #27:  tilde code fences (~~~) not recognized
//! Issue #27b: horizontal rule detection too strict (only matches exactly ---, ***, ___)

use gpui_md::markdown::syntax_highlight::highlight_line;
use gpui_md::markdown::theme_colors::MdThemeColors;

fn dark_colors() -> MdThemeColors {
    let theme = gpui_ui_kit::theme::Theme::dark();
    MdThemeColors::from_theme(&theme)
}

// ---------------------------------------------------------------------------
// Issue #27: tilde code fences not recognized
//
// The code fence toggle only checks for ```. CommonMark also supports ~~~.
// Content inside ~~~ fences is highlighted as regular text instead of code.
// ---------------------------------------------------------------------------

#[test]
fn issue_27_tilde_code_fence_is_highlighted_as_fence() {
    let colors = dark_colors();
    let (spans, in_code) = highlight_line("~~~", false, &colors);
    assert!(
        in_code,
        "~~~ should toggle into code block mode, like ```"
    );
    assert_eq!(spans.len(), 1);
    assert_eq!(
        spans[0].color, colors.syn_fence,
        "~~~ should be highlighted as a code fence"
    );
}

#[test]
fn issue_27_tilde_code_fence_with_language_tag() {
    let colors = dark_colors();
    let (spans, in_code) = highlight_line("~~~rust", false, &colors);
    assert!(in_code, "~~~rust should enter code block mode");
    assert_eq!(
        spans[0].color, colors.syn_fence,
        "~~~rust should be highlighted as a code fence"
    );
}

#[test]
fn issue_27_tilde_fence_round_trip() {
    let colors = dark_colors();

    // Enter code block with ~~~
    let (_, in_code) = highlight_line("~~~", false, &colors);
    assert!(in_code);

    // Content inside should be code
    let (spans, still_in) = highlight_line("let x = 42;", true, &colors);
    assert!(still_in);
    assert_eq!(spans[0].color, colors.syn_code);

    // Exit code block with ~~~
    let (_, exited) = highlight_line("~~~", true, &colors);
    assert!(!exited, "~~~ should exit the code block");
}

// ---------------------------------------------------------------------------
// Issue #27b: horizontal rule detection too strict
//
// The highlighter only matches exactly "---", "***", "___".
// CommonMark allows >=3 of these characters with optional spaces:
// "----", "- - -", "****", "_ _ _", etc.
// ---------------------------------------------------------------------------

#[test]
fn issue_27b_four_dashes_is_horizontal_rule() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("----", false, &colors);
    assert_eq!(spans.len(), 1);
    assert_eq!(
        spans[0].color, colors.text_muted,
        "---- should be recognized as a horizontal rule"
    );
}

#[test]
fn issue_27b_spaced_dashes_is_horizontal_rule() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("- - -", false, &colors);
    assert_eq!(spans.len(), 1);
    assert_eq!(
        spans[0].color, colors.text_muted,
        "'- - -' should be recognized as a horizontal rule"
    );
}

#[test]
fn issue_27b_many_asterisks_is_horizontal_rule() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("*****", false, &colors);
    assert_eq!(spans.len(), 1);
    assert_eq!(
        spans[0].color, colors.text_muted,
        "***** should be recognized as a horizontal rule"
    );
}

#[test]
fn issue_27b_spaced_underscores_is_horizontal_rule() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("_ _ _", false, &colors);
    assert_eq!(spans.len(), 1);
    assert_eq!(
        spans[0].color, colors.text_muted,
        "'_ _ _' should be recognized as a horizontal rule"
    );
}
