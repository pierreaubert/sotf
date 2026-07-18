//! UI utility functions for text formatting and helper methods

/// Format channel count as common surround notation (e.g., Mono, 2.0, 5.1, 7.1)
pub fn format_channel_count(n: u32) -> String {
    sotf_audio_player::format_channel_count(n)
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
pub fn get_keybindings_for_screen(
    screen: crate::app::Screen,
    language: crate::i18n::Language,
) -> Vec<(&'static str, &'static str)> {
    use crate::app::Screen;

    let bindings = match screen {
        Screen::Library => vec![
            ("/", "Search"),
            ("↑↓ or k/j", "Browse"),
            ("a/Enter", "Queue album"),
            ("q", "Queue"),
            ("s", "Sort"),
            ("c", "Filter"),
            ("t", "Tree/flat view"),
        ],
        Screen::Queue => vec![
            ("↑↓ or k/j", "Browse"),
            ("Enter", "Play selection"),
            ("d", "Remove track"),
            ("c", "Clear"),
            ("p/Space", "Play/pause"),
            ("A", "Add active playlist"),
        ],
        Screen::Plugins => vec![
            ("↑↓ or k/j", "Browse"),
            ("a", "Add plugin"),
            ("d", "Remove plugin"),
            ("e/Enter", "Edit"),
            ("t", "Enable/disable"),
            ("s", "Save"),
            ("l", "Load"),
        ],
        Screen::Devices => vec![
            ("↑↓ or k/j", "Browse"),
            ("Enter/Space", "Select"),
            ("r", "Rescan"),
        ],
        Screen::Configure => vec![
            ("←→", "Navigate tabs"),
            ("Enter", "Open tab"),
            ("Esc", "Back"),
            ("1-8", "Jump to tab"),
            ("?", "Help"),
        ],
        Screen::Playlists => vec![
            ("↑↓ or k/j", "Browse"),
            ("Enter/l", "Open"),
            ("Esc/h", "Back"),
            ("n/r/d", "Create/rename/delete"),
            ("p", "Play all"),
        ],
        Screen::Loading => vec![],
    };
    let text = crate::i18n::TuiTranslations::for_language(language);
    bindings
        .into_iter()
        .map(|(key, action)| (key, text.action_description(action)))
        .collect()
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[test]
    fn compact_help_covers_every_interactive_screen_without_stale_library_actions() {
        for language in crate::i18n::Language::ALL {
            for screen in [
                crate::app::Screen::Library,
                crate::app::Screen::Queue,
                crate::app::Screen::Playlists,
                crate::app::Screen::Plugins,
                crate::app::Screen::Devices,
                crate::app::Screen::Configure,
            ] {
                assert!(
                    !get_keybindings_for_screen(screen, language).is_empty(),
                    "{screen:?} must have contextual keyboard help for {}",
                    language.code()
                );
            }
        }

        let library =
            get_keybindings_for_screen(crate::app::Screen::Library, crate::i18n::Language::English);
        assert!(library.contains(&("a/Enter", "Queue album")));
        assert!(library.contains(&("q", "Queue")));
        assert!(!library.iter().any(|binding| binding.1 == "Add dir"));
        assert!(!library.iter().any(|binding| binding.0 == "p"));
    }

    #[test]
    fn test_clean_text() {
        // Test leading/trailing whitespace
        assert_eq!(clean_text("  Text  "), "Text");

        // Test multiple spaces
        assert_eq!(clean_text("Text    Name"), "Text Name");

        // Test tabs
        assert_eq!(clean_text("Text\tName"), "Text Name");

        // Test newlines
        assert_eq!(clean_text("Text\nName"), "Text Name");

        // Test combination
        assert_eq!(clean_text("  \t Text   Name\n  "), "Text Name");

        // Test normal string (no change needed)
        assert_eq!(clean_text("Text Name"), "Text Name");

        // Test empty string
        assert_eq!(clean_text(""), "");

        // Test only whitespace
        assert_eq!(clean_text("   \t\n  "), "");
    }

    #[test]
    fn test_clean_track_name() {
        // Verify clean_track_name wraps clean_text correctly
        assert_eq!(clean_track_name("  Track Name  "), "Track Name");
        assert_eq!(clean_track_name("Track\tName"), "Track Name");
    }

    #[test]
    fn test_truncate_with_ellipsis() {
        // Test no truncation needed
        assert_eq!(truncate_with_ellipsis("Short", 10), "Short");

        // Test exact length
        assert_eq!(truncate_with_ellipsis("Exact", 5), "Exact");

        // Test truncation
        assert_eq!(
            truncate_with_ellipsis("This is a very long track name", 15),
            "This is a ve..."
        );

        // Test truncation at edge
        assert_eq!(truncate_with_ellipsis("12345678", 5), "12...");

        // Test very short max_len
        assert_eq!(truncate_with_ellipsis("Test", 3), "...");

        // Test empty string
        assert_eq!(truncate_with_ellipsis("", 10), "");
    }
}
