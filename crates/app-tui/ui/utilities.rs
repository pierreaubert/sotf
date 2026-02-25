//! UI utility functions for text formatting and helper methods

/// Format channel count as common surround notation (e.g., Mono, 2.0, 5.1, 7.1)
pub fn format_channel_count(n: u32) -> String {
    match n {
        1 => "Mono".to_string(),
        2 => "2.0".to_string(),
        4 => "4.0".to_string(),
        5 => "5.0".to_string(),
        6 => "5.1".to_string(),
        8 => "7.1".to_string(),
        10 => "7.1.2".to_string(),
        12 => "7.1.4".to_string(),
        14 => "9.1.4".to_string(),
        16 => "9.1.6".to_string(),
        _ => format!("{}ch", n),
    }
}

/// Clean up track/song titles by:
/// - Trimming ALL leading/trailing whitespace
/// - Replacing multiple consecutive spaces with a single space
/// - Removing tabs, newlines, and other control characters
pub fn clean_track_name(name: &str) -> String {
    clean_text(name)
}

/// Clean up any text field (artist, album, track) by:
/// - Trimming ALL leading/trailing whitespace
/// - Replacing multiple consecutive spaces with a single space
/// - Removing tabs, newlines, and other control characters
pub fn clean_text(text: &str) -> String {
    // First, replace all control characters (tabs, newlines, etc.) with spaces
    let normalized: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();

    // Then split by whitespace and rejoin with single spaces (handles multiple spaces)
    // This also trims all leading and trailing whitespace
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Truncate a string to a maximum length, adding "..." if truncated
pub fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

/// Get a human-readable name for a path config JSON (for A/B Compare plugin)
pub fn path_config_to_display_name(config: &str) -> String {
    if config.is_empty() || config == r#"{"type":"None"}"# {
        "None (passthrough)".to_string()
    } else if config.contains(r#""plugin_type":"EQ""#) {
        "EQ".to_string()
    } else if config.contains(r#""plugin_type":"gain""#) {
        "Gain".to_string()
    } else if config.contains(r#""plugin_type":"compressor""#) {
        "Compressor".to_string()
    } else if config.contains(r#""plugin_type":"limiter""#) {
        "Limiter".to_string()
    } else if config.contains(r#""plugin_type":"gate""#) {
        "Gate".to_string()
    } else if config.contains(r#""plugin_type":"expander""#) {
        "Expander".to_string()
    } else if config.contains(r#""plugin_type":"denoiser""#) {
        "Denoiser".to_string()
    } else if config.contains(r#""plugin_type":"loudness_compensation""#) {
        "Loudness Comp".to_string()
    } else if config.contains(r#""type":"Rack""#) {
        "Rack (chain)".to_string()
    } else if config.contains(r#""type":"Graph""#) {
        "Graph".to_string()
    } else {
        "Custom".to_string()
    }
}

/// Wrap text to a maximum width, returning lines
pub fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= max_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            if !current_line.is_empty() {
                lines.push(current_line);
            }
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}

/// Get keybindings for a given screen
pub fn get_keybindings_for_screen(screen: crate::app::Screen) -> Vec<(&'static str, &'static str)> {
    use crate::app::Screen;

    match screen {
        Screen::Library => vec![
            ("/", "Search"),
            ("↑↓", "Browse"),
            ("a", "Add dir"),
            ("q", "Queue"),
            ("p", "Play"),
        ],
        Screen::Queue => vec![
            ("↑↓", "Browse"),
            ("d", "Remove track"),
            ("c", "Clear"),
            ("p", "Play"),
        ],
        Screen::Plugins => vec![
            ("↑↓", "Browse"),
            ("a", "Add plugin"),
            ("d", "Remove plugin"),
            ("e", "Edit"),
            ("s", "Save"),
            ("l", "Load"),
        ],
        Screen::Devices => vec![
            ("↑↓", "Browse"),
            ("Enter", "Select"),
        ],
        Screen::Configure => vec![
            ("←→", "Navigate tabs"),
            ("↑↓", "Navigate fields"),
            ("Tab", "Enter/Exit"),
            ("1-5", "Jump to tab"),
            ("?", "Help"),
        ],
        _ => vec![],
    }
}
