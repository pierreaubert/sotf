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
    #[allow(clippy::should_implement_trait)]
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
    /// Create a high-contrast dark theme (default)
    pub fn dark() -> Self {
        Self {
            // Backgrounds — true black for maximum contrast
            bg_primary: Color::Rgb(0, 0, 0),
            bg_secondary: Color::Rgb(30, 30, 30),
            bg_selected: Color::Rgb(255, 255, 255),
            bg_highlight: Color::Rgb(40, 40, 45),

            // Foregrounds — bright white text on black
            fg_primary: Color::Rgb(255, 255, 255),
            fg_secondary: Color::Rgb(180, 180, 180),
            fg_selected: Color::Rgb(0, 0, 0),
            fg_muted: Color::Rgb(170, 170, 170),

            // Accent colors — vivid, saturated for dark backgrounds
            accent_primary: Color::Rgb(0, 220, 255),
            accent_secondary: Color::Rgb(220, 80, 220),
            accent_success: Color::Rgb(0, 230, 80),
            accent_warning: Color::Rgb(255, 210, 0),
            accent_error: Color::Rgb(255, 60, 60),
            accent_info: Color::Rgb(0, 220, 255),

            // UI elements
            border_color: Color::Rgb(0, 200, 80),
            title_color: Color::Rgb(255, 220, 0),
            playing_indicator: Color::Rgb(0, 230, 80),
            current_track: Color::Rgb(0, 220, 255),
        }
    }

    /// Create a high-contrast light theme
    pub fn light() -> Self {
        Self {
            // Backgrounds — true white for maximum contrast
            bg_primary: Color::Rgb(255, 255, 255),
            bg_secondary: Color::Rgb(235, 235, 235),
            bg_selected: Color::Rgb(0, 0, 0),
            bg_highlight: Color::Rgb(225, 225, 230),

            // Foregrounds — true black text on white
            fg_primary: Color::Rgb(0, 0, 0),
            fg_secondary: Color::Rgb(60, 60, 60),
            fg_selected: Color::Rgb(255, 255, 255),
            fg_muted: Color::Rgb(110, 110, 110),

            // Accent colors — deep, saturated for white backgrounds
            accent_primary: Color::Rgb(0, 40, 200),
            accent_secondary: Color::Rgb(150, 0, 130),
            accent_success: Color::Rgb(0, 120, 30),
            accent_warning: Color::Rgb(180, 100, 0),
            accent_error: Color::Rgb(200, 0, 0),
            accent_info: Color::Rgb(0, 40, 200),

            // UI elements
            border_color: Color::Rgb(60, 60, 80),
            title_color: Color::Rgb(150, 0, 130),
            playing_indicator: Color::Rgb(0, 120, 30),
            current_track: Color::Rgb(0, 40, 200),
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
        assert_eq!(theme.bg_primary, Color::Rgb(0, 0, 0));
        assert_eq!(theme.fg_primary, Color::Rgb(255, 255, 255));
        assert_eq!(theme.border_color, Color::Rgb(0, 200, 80));
    }

    #[test]
    fn test_light_theme() {
        let theme = Theme::light();
        assert_eq!(theme.bg_primary, Color::Rgb(255, 255, 255));
        assert_eq!(theme.fg_primary, Color::Rgb(0, 0, 0));
        assert_eq!(theme.fg_selected, Color::Rgb(255, 255, 255));
        assert_eq!(theme.border_color, Color::Rgb(60, 60, 80));
    }

    #[test]
    fn test_theme_from_type() {
        let dark = Theme::from_type(ThemeType::Dark);
        assert_eq!(dark.bg_primary, Color::Rgb(0, 0, 0));

        let light = Theme::from_type(ThemeType::Light);
        assert_eq!(light.bg_primary, Color::Rgb(255, 255, 255));
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
        let cloned = theme;

        assert_eq!(theme.bg_primary, copied.bg_primary);
        assert_eq!(theme.bg_primary, cloned.bg_primary);
    }
}
