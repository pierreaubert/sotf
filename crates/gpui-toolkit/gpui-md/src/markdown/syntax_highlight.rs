//! Markdown syntax highlighting for the source editor.
//!
//! Tokenizes a line of markdown into colored spans for rendering.
//! Uses theme-aware colors for proper light/dark mode support.

use gpui::Rgba;

use super::theme_colors::MdThemeColors;

/// A colored text span within a single line.
#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub text: String,
    pub color: Rgba,
}

/// Highlight a single line of markdown source.
///
/// `in_code_block` indicates whether we're inside a fenced code block.
/// Returns the highlighted spans and updated code block state.
pub fn highlight_line(
    line: &str,
    in_code_block: bool,
    colors: &MdThemeColors,
) -> (Vec<HighlightSpan>, bool) {
    let trimmed = line.trim_start();

    // Code fence toggle (``` or ~~~)
    if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
        return (
            vec![HighlightSpan {
                text: line.to_string(),
                color: colors.syn_fence,
            }],
            !in_code_block,
        );
    }

    // Inside code block
    if in_code_block {
        return (
            vec![HighlightSpan {
                text: line.to_string(),
                color: colors.syn_code,
            }],
            true,
        );
    }

    // Heading
    if trimmed.starts_with('#') {
        let hash_count = trimmed.chars().take_while(|c| *c == '#').count();
        if hash_count <= 6 && trimmed.chars().nth(hash_count) == Some(' ') {
            return (
                vec![HighlightSpan {
                    text: line.to_string(),
                    color: colors.syn_heading,
                }],
                false,
            );
        }
    }

    // Horizontal rule (CommonMark: 3+ of same char from {-, *, _}, optional spaces)
    if is_horizontal_rule(trimmed) {
        return (
            vec![HighlightSpan {
                text: line.to_string(),
                color: colors.text_muted,
            }],
            false,
        );
    }

    // Block quote
    if trimmed.starts_with('>') {
        return (
            vec![HighlightSpan {
                text: line.to_string(),
                color: colors.syn_blockquote,
            }],
            false,
        );
    }

    // Inline highlighting
    let spans = highlight_inline(line, colors);
    (spans, false)
}

fn highlight_inline(line: &str, colors: &MdThemeColors) -> Vec<HighlightSpan> {
    let mut spans = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut current_text = String::new();

    let trimmed = line.trim_start();
    let indent_len = line.chars().count() - trimmed.chars().count();

    // Task list
    if trimmed.starts_with("- [x] ") || trimmed.starts_with("- [ ] ") {
        let marker_end = indent_len + 6;
        let marker: String = chars[..marker_end.min(len)].iter().collect();
        spans.push(HighlightSpan {
            text: marker,
            color: colors.syn_checkbox,
        });
        i = marker_end.min(len);
    } else if trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("+ ")
    {
        let marker_end = indent_len + 2;
        let marker: String = chars[..marker_end.min(len)].iter().collect();
        spans.push(HighlightSpan {
            text: marker,
            color: colors.syn_list_marker,
        });
        i = marker_end.min(len);
    } else if let Some(num_end) = detect_ordered_list(trimmed) {
        let marker_end = indent_len + num_end;
        let marker: String = chars[..marker_end.min(len)].iter().collect();
        spans.push(HighlightSpan {
            text: marker,
            color: colors.syn_list_marker,
        });
        i = marker_end.min(len);
    }

    while i < len {
        let ch = chars[i];

        // Inline code
        if ch == '`' {
            flush_text(&mut current_text, colors.text, &mut spans);
            let end = find_closing(&chars, i + 1, '`');
            if let Some(end) = end {
                let code: String = chars[i..=end].iter().collect();
                spans.push(HighlightSpan {
                    text: code,
                    color: colors.syn_code,
                });
                i = end + 1;
                continue;
            }
        }

        // Bold **text** or __text__
        if i + 1 < len
            && ((ch == '*' && chars[i + 1] == '*') || (ch == '_' && chars[i + 1] == '_'))
        {
            let marker = ch;
            if let Some(end) = find_double_closing(&chars, i + 2, marker) {
                flush_text(&mut current_text, colors.text, &mut spans);
                let bold: String = chars[i..=end + 1].iter().collect();
                spans.push(HighlightSpan {
                    text: bold,
                    color: colors.syn_bold,
                });
                i = end + 2;
                continue;
            }
        }

        // Italic *text* or _text_
        if (ch == '*' || ch == '_')
            && i + 1 < len
            && chars[i + 1] != ch
            && let Some(end) = find_closing(&chars, i + 1, ch)
        {
            flush_text(&mut current_text, colors.text, &mut spans);
            let italic: String = chars[i..=end].iter().collect();
            spans.push(HighlightSpan {
                text: italic,
                color: colors.syn_italic,
            });
            i = end + 1;
            continue;
        }

        // Strikethrough ~~text~~
        if i + 1 < len
            && ch == '~'
            && chars[i + 1] == '~'
            && let Some(end) = find_double_closing(&chars, i + 2, '~')
        {
            flush_text(&mut current_text, colors.text, &mut spans);
            let strike: String = chars[i..=end + 1].iter().collect();
            spans.push(HighlightSpan {
                text: strike,
                color: colors.syn_strikethrough,
            });
            i = end + 2;
            continue;
        }

        // Link [text](url)
        if ch == '['
            && let Some(link_end) = find_link_end(&chars, i)
        {
            flush_text(&mut current_text, colors.text, &mut spans);
            let link: String = chars[i..=link_end].iter().collect();
            spans.push(HighlightSpan {
                text: link,
                color: colors.syn_link,
            });
            i = link_end + 1;
            continue;
        }

        current_text.push(ch);
        i += 1;
    }

    flush_text(&mut current_text, colors.text, &mut spans);
    spans
}

fn flush_text(current: &mut String, color: Rgba, spans: &mut Vec<HighlightSpan>) {
    if !current.is_empty() {
        spans.push(HighlightSpan {
            text: std::mem::take(current),
            color,
        });
    }
}

fn find_closing(chars: &[char], start: usize, marker: char) -> Option<usize> {
    chars[start..]
        .iter()
        .position(|&c| c == marker)
        .map(|i| start + i)
}

fn find_double_closing(chars: &[char], start: usize, marker: char) -> Option<usize> {
    let mut i = start;
    while i + 1 < chars.len() {
        if chars[i] == marker && chars[i + 1] == marker {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_link_end(chars: &[char], start: usize) -> Option<usize> {
    let bracket_close = find_closing(chars, start + 1, ']')?;
    if bracket_close + 1 < chars.len() && chars[bracket_close + 1] == '(' {
        let paren_close = find_closing(chars, bracket_close + 2, ')')?;
        return Some(paren_close);
    }
    None
}

/// CommonMark thematic break: a line containing 3 or more of the same
/// character from {`-`, `*`, `_`}, optionally interspersed with spaces/tabs,
/// and nothing else.
fn is_horizontal_rule(trimmed: &str) -> bool {
    if trimmed.is_empty() {
        return false;
    }
    // Determine the rule character (first non-space char)
    let rule_char = match trimmed.chars().find(|c| !c.is_whitespace()) {
        Some(c @ ('-' | '*' | '_')) => c,
        _ => return false,
    };
    let count = trimmed.chars().filter(|&c| c == rule_char).count();
    let all_valid = trimmed.chars().all(|c| c == rule_char || c == ' ' || c == '\t');
    count >= 3 && all_valid
}

fn detect_ordered_list(trimmed: &str) -> Option<usize> {
    let mut i = 0;
    let chars: Vec<char> = trimmed.chars().collect();
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0
        && i < chars.len()
        && (chars[i] == '.' || chars[i] == ')')
        && i + 1 < chars.len()
        && chars[i + 1] == ' '
    {
        return Some(i + 2);
    }
    None
}
