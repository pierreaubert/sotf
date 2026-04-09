use gpui_md::markdown::syntax_highlight::highlight_line;
use gpui_md::markdown::theme_colors::MdThemeColors;

fn dark_colors() -> MdThemeColors {
    // Use a dark theme for testing
    let theme = gpui_ui_kit::theme::Theme::dark();
    MdThemeColors::from_theme(&theme)
}

#[test]
fn heading_line() {
    let colors = dark_colors();
    let (spans, in_code) = highlight_line("# Hello", false, &colors);
    assert!(!in_code);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].text, "# Hello");
    assert_eq!(spans[0].color, colors.syn_heading);
}

#[test]
fn heading_level_2() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("## Title", false, &colors);
    assert_eq!(spans[0].color, colors.syn_heading);
}

#[test]
fn not_a_heading_no_space() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("#nospace", false, &colors);
    // Should NOT be colored as heading (no space after #)
    assert_ne!(spans[0].color, colors.syn_heading);
}

#[test]
fn code_fence_toggles() {
    let colors = dark_colors();
    let (spans, in_code) = highlight_line("```rust", false, &colors);
    assert!(in_code); // entered code block
    assert_eq!(spans[0].color, colors.syn_fence);

    let (spans2, in_code2) = highlight_line("let x = 1;", true, &colors);
    assert!(in_code2); // still in code block
    assert_eq!(spans2[0].color, colors.syn_code);

    let (_, in_code3) = highlight_line("```", true, &colors);
    assert!(!in_code3); // exited code block
}

#[test]
fn bold_markers() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("hello **bold** world", false, &colors);
    assert!(spans.len() >= 3);
    // Find the bold span
    let bold_span = spans.iter().find(|s| s.text == "**bold**");
    assert!(bold_span.is_some(), "Should find bold span");
    assert_eq!(bold_span.unwrap().color, colors.syn_bold);
}

#[test]
fn italic_markers() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("hello *italic* world", false, &colors);
    let italic_span = spans.iter().find(|s| s.text == "*italic*");
    assert!(italic_span.is_some(), "Should find italic span");
}

#[test]
fn inline_code() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("use `code` here", false, &colors);
    let code_span = spans.iter().find(|s| s.text == "`code`");
    assert!(code_span.is_some(), "Should find code span");
    assert_eq!(code_span.unwrap().color, colors.syn_code);
}

#[test]
fn link_syntax() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("see [link](url) here", false, &colors);
    let link_span = spans.iter().find(|s| s.text == "[link](url)");
    assert!(link_span.is_some(), "Should find link span");
    assert_eq!(link_span.unwrap().color, colors.syn_link);
}

#[test]
fn unordered_list_marker() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("- item text", false, &colors);
    assert_eq!(spans[0].text, "- ");
    assert_eq!(spans[0].color, colors.syn_list_marker);
}

#[test]
fn task_list_unchecked() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("- [ ] todo item", false, &colors);
    assert_eq!(spans[0].text, "- [ ] ");
    assert_eq!(spans[0].color, colors.syn_checkbox);
}

#[test]
fn task_list_checked() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("- [x] done item", false, &colors);
    assert_eq!(spans[0].text, "- [x] ");
    assert_eq!(spans[0].color, colors.syn_checkbox);
}

#[test]
fn blockquote() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("> quoted text", false, &colors);
    assert_eq!(spans[0].color, colors.syn_blockquote);
}

#[test]
fn horizontal_rule() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("---", false, &colors);
    assert_eq!(spans[0].color, colors.text_muted);
}

#[test]
fn plain_text_gets_default_color() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("just plain text", false, &colors);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].text, "just plain text");
    assert_eq!(spans[0].color, colors.text);
}

#[test]
fn empty_line() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("", false, &colors);
    assert!(spans.is_empty());
}

#[test]
fn strikethrough() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("hello ~~strike~~ world", false, &colors);
    let strike_span = spans.iter().find(|s| s.text == "~~strike~~");
    assert!(strike_span.is_some(), "Should find strikethrough span");
}

#[test]
fn indented_list() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("  - nested item", false, &colors);
    assert_eq!(spans[0].text, "  - ");
    assert_eq!(spans[0].color, colors.syn_list_marker);
}

#[test]
fn ordered_list() {
    let colors = dark_colors();
    let (spans, _) = highlight_line("1. first item", false, &colors);
    assert_eq!(spans[0].text, "1. ");
    assert_eq!(spans[0].color, colors.syn_list_marker);
}
