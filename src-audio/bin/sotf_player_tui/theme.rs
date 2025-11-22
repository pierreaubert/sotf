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
