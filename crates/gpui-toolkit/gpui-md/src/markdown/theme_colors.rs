//! Theme-aware color palette for the markdown editor.
//!
//! Extracted from the active gpui-ui-kit Theme so that the renderer
//! and syntax highlighter produce correct colors in both light and dark modes.

use gpui::Rgba;

/// Colors used by the markdown renderer and syntax highlighter.
#[derive(Debug, Clone, Copy)]
pub struct MdThemeColors {
    // Text
    pub text: Rgba,
    pub text_muted: Rgba,

    // Backgrounds
    pub code_block_bg: Rgba,
    pub table_bg: Rgba,
    pub table_header_bg: Rgba,

    // Borders
    pub border: Rgba,
    pub hr: Rgba,

    // Syntax highlighting (editor pane)
    pub syn_heading: Rgba,
    pub syn_bold: Rgba,
    pub syn_italic: Rgba,
    pub syn_strikethrough: Rgba,
    pub syn_code: Rgba,
    pub syn_link: Rgba,
    pub syn_list_marker: Rgba,
    pub syn_blockquote: Rgba,
    pub syn_fence: Rgba,
    pub syn_checkbox: Rgba,

    // Cursor
    pub cursor_bg: Rgba,
    pub cursor_text: Rgba,

    // Selection highlight
    pub selection_bg: Rgba,

    // Find highlight
    pub find_match_bg: Rgba,
    pub find_match_text: Rgba,
}

impl MdThemeColors {
    /// Build from a gpui-ui-kit Theme (reads current theme state).
    pub fn from_theme(theme: &gpui_ui_kit::theme::Theme) -> Self {
        let is_light = is_light_theme(theme);

        if is_light {
            Self {
                text: theme.text_primary,
                text_muted: theme.text_muted,
                code_block_bg: rgba(0xf0f0f0ff),
                table_bg: rgba(0xe8e8e8ff),
                table_header_bg: rgba(0xdcdcdcff),
                border: theme.border,
                hr: rgba(0xccccccff),
                syn_heading: rgba(0x0550aeff),
                syn_bold: rgba(0x953800ff),
                syn_italic: rgba(0x6f42c1ff),
                syn_strikethrough: rgba(0x808080ff),
                syn_code: rgba(0xa0522dff),
                syn_link: rgba(0x116329ff),
                syn_list_marker: rgba(0x636c76ff),
                syn_blockquote: rgba(0x636c76ff),
                syn_fence: rgba(0xa0522dff),
                syn_checkbox: rgba(0x116329ff),
                cursor_bg: rgba(0x000000ff),
                cursor_text: rgba(0xffffffff),
                selection_bg: rgba(0xb4d5feff),
                find_match_bg: rgba(0xfff2ccff),
                find_match_text: rgba(0x6e5600ff),
            }
        } else {
            Self {
                text: theme.text_primary,
                text_muted: theme.text_muted,
                code_block_bg: rgba(0x1e1e1eff),
                table_bg: rgba(0x2d2d2dff),
                table_header_bg: rgba(0x383838ff),
                border: theme.border,
                hr: rgba(0x444444ff),
                syn_heading: rgba(0x569cd6ff),
                syn_bold: rgba(0xc78d33ff),
                syn_italic: rgba(0xb392f0ff),
                syn_strikethrough: rgba(0x6a737dff),
                syn_code: rgba(0xce8675ff),
                syn_link: rgba(0x66ba6aff),
                syn_list_marker: rgba(0xdbdbabff),
                syn_blockquote: rgba(0x888888ff),
                syn_fence: rgba(0xce8675ff),
                syn_checkbox: rgba(0x66ba6aff),
                cursor_bg: rgba(0xffffffcc),
                cursor_text: rgba(0x000000ff),
                selection_bg: rgba(0x264f78ff),
                find_match_bg: rgba(0x61502bff),
                find_match_text: rgba(0xffdd57ff),
            }
        }
    }
}

fn is_light_theme(theme: &gpui_ui_kit::theme::Theme) -> bool {
    // Detect light/dark by checking background luminance
    let bg = theme.background;
    let luminance = 0.299 * bg.r + 0.587 * bg.g + 0.114 * bg.b;
    luminance > 0.5
}

fn rgba(hex: u32) -> Rgba {
    gpui::rgba(hex)
}
