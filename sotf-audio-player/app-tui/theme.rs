//! Theme system for SOTF TUI Player
//!
//! Provides color schemes for dark and light themes.

use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeType {
    Dark,
    Light,
}

impl ThemeType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "dark" => Some(ThemeType::Dark),
            "light" => Some(ThemeType::Light),
            _ => None,
        }
    }
}

/// Color scheme for the TUI
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    // Backgrounds
    pub bg_primary: Color,
    pub bg_secondary: Color,
    pub bg_selected: Color,
    pub bg_highlight: Color,

    // Foregrounds
    pub fg_primary: Color,
    pub fg_secondary: Color,
    pub fg_selected: Color,
    pub fg_muted: Color,

    // Accent colors
    pub accent_primary: Color,
    pub accent_secondary: Color,
    pub accent_success: Color,
    pub accent_warning: Color,
    pub accent_error: Color,
    pub accent_info: Color,

    // UI elements
    pub border_color: Color,
    pub title_color: Color,
    pub playing_indicator: Color,
    pub current_track: Color,
}

impl Theme {
    /// Create a dark theme (default)
    pub fn dark() -> Self {
        Self {
            // Backgrounds
            bg_primary: Color::Black,
            bg_secondary: Color::DarkGray,
            bg_selected: Color::White,
            bg_highlight: Color::DarkGray,

            // Foregrounds
            fg_primary: Color::White,
            fg_secondary: Color::Gray,
            fg_selected: Color::Black,
            fg_muted: Color::DarkGray,

            // Accent colors
            accent_primary: Color::Cyan,
            accent_secondary: Color::Magenta,
            accent_success: Color::Green,
            accent_warning: Color::Yellow,
            accent_error: Color::Red,
            accent_info: Color::Cyan,

            // UI elements
            border_color: Color::Green,
            title_color: Color::Yellow,
            playing_indicator: Color::Green,
            current_track: Color::Cyan,
        }
    }

    /// Create a light theme
    pub fn light() -> Self {
        Self {
            // Backgrounds
            bg_primary: Color::White,
            bg_secondary: Color::LightBlue,
            bg_selected: Color::Black,
            bg_highlight: Color::LightCyan,

            // Foregrounds
            fg_primary: Color::Black,
            fg_secondary: Color::DarkGray,
            fg_selected: Color::White,
            fg_muted: Color::Gray,

            // Accent colors
            accent_primary: Color::Blue,
            accent_secondary: Color::Magenta,
            accent_success: Color::Green,
            accent_warning: Color::Yellow,
            accent_error: Color::Red,
            accent_info: Color::Blue,

            // UI elements
            border_color: Color::Blue,
            title_color: Color::Magenta,
            playing_indicator: Color::Green,
            current_track: Color::Blue,
        }
    }

    /// Create a theme based on type
    pub fn from_type(theme_type: ThemeType) -> Self {
        match theme_type {
            ThemeType::Dark => Self::dark(),
            ThemeType::Light => Self::light(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_type_from_str() {
        assert_eq!(ThemeType::from_str("dark"), Some(ThemeType::Dark));
        assert_eq!(ThemeType::from_str("Dark"), Some(ThemeType::Dark));
        assert_eq!(ThemeType::from_str("DARK"), Some(ThemeType::Dark));

        assert_eq!(ThemeType::from_str("light"), Some(ThemeType::Light));
        assert_eq!(ThemeType::from_str("Light"), Some(ThemeType::Light));
        assert_eq!(ThemeType::from_str("LIGHT"), Some(ThemeType::Light));

        assert_eq!(ThemeType::from_str("invalid"), None);
        assert_eq!(ThemeType::from_str(""), None);
    }

    #[test]
    fn test_dark_theme() {
        let theme = Theme::dark();
        assert_eq!(theme.bg_primary, Color::Black);
        assert_eq!(theme.fg_primary, Color::White);
        assert_eq!(theme.border_color, Color::Green);
    }

    #[test]
    fn test_light_theme() {
        let theme = Theme::light();
        assert_eq!(theme.bg_primary, Color::White);
        assert_eq!(theme.fg_primary, Color::Black);
        assert_eq!(theme.border_color, Color::Blue);
    }

    #[test]
    fn test_theme_from_type() {
        let dark = Theme::from_type(ThemeType::Dark);
        assert_eq!(dark.bg_primary, Color::Black);

        let light = Theme::from_type(ThemeType::Light);
        assert_eq!(light.bg_primary, Color::White);
    }

    #[test]
    fn test_theme_default() {
        let default_theme = Theme::default();
        let dark_theme = Theme::dark();

        // Default should be dark theme
        assert_eq!(default_theme.bg_primary, dark_theme.bg_primary);
        assert_eq!(default_theme.fg_primary, dark_theme.fg_primary);
        assert_eq!(default_theme.border_color, dark_theme.border_color);
    }

    #[test]
    fn test_theme_colors_are_different() {
        let dark = Theme::dark();
        let light = Theme::light();

        // Themes should have different primary colors
        assert_ne!(dark.bg_primary, light.bg_primary);
        assert_ne!(dark.fg_primary, light.fg_primary);
        assert_ne!(dark.border_color, light.border_color);
    }

    #[test]
    fn test_theme_copy_and_clone() {
        let theme = Theme::dark();
        let copied = theme;
        let cloned = theme.clone();

        assert_eq!(theme.bg_primary, copied.bg_primary);
        assert_eq!(theme.bg_primary, cloned.bg_primary);
    }
}
