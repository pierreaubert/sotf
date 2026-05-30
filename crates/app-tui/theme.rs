//! Theme system for SOTF TUI Player
//!
//! Provides color schemes for dark and light themes.

use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeType {
    Dark,
    Light,
    Solarized,
    Dracula,
    Gruvbox,
    TokyoNight,
}

impl ThemeType {
    pub fn all() -> &'static [Self] {
        &[
            Self::Dark,
            Self::Light,
            Self::Solarized,
            Self::Dracula,
            Self::Gruvbox,
            Self::TokyoNight,
        ]
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match normalize_theme_name(s).as_str() {
            "dark" => Some(ThemeType::Dark),
            "light" => Some(ThemeType::Light),
            "solarized" | "solarized_dark" => Some(ThemeType::Solarized),
            "dracula" => Some(ThemeType::Dracula),
            "gruvbox" | "gruvbox_dark" => Some(ThemeType::Gruvbox),
            "tokyonight" | "tokyo_night" => Some(ThemeType::TokyoNight),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ThemeType::Dark => "Dark",
            ThemeType::Light => "Light",
            ThemeType::Solarized => "Solarized",
            ThemeType::Dracula => "Dracula",
            ThemeType::Gruvbox => "Gruvbox",
            ThemeType::TokyoNight => "Tokyo Night",
        }
    }
}

fn normalize_theme_name(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            if c.is_ascii_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else if c == '-' || c == '_' || c.is_ascii_whitespace() {
                Some('_')
            } else {
                None
            }
        })
        .collect()
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

    /// Create a Solarized Dark terminal preset.
    pub fn solarized() -> Self {
        Self {
            bg_primary: rgb(0x002b36),
            bg_secondary: rgb(0x073642),
            bg_selected: rgb(0x586e75),
            bg_highlight: rgb(0x073642),

            fg_primary: rgb(0xeee8d5),
            fg_secondary: rgb(0x93a1a1),
            fg_selected: rgb(0xfdf6e3),
            fg_muted: rgb(0x839496),

            accent_primary: rgb(0x268bd2),
            accent_secondary: rgb(0x2aa198),
            accent_success: rgb(0x859900),
            accent_warning: rgb(0xb58900),
            accent_error: rgb(0xdc322f),
            accent_info: rgb(0x268bd2),

            border_color: rgb(0x586e75),
            title_color: rgb(0xb58900),
            playing_indicator: rgb(0x859900),
            current_track: rgb(0x268bd2),
        }
    }

    /// Create a Dracula terminal preset.
    pub fn dracula() -> Self {
        Self {
            bg_primary: rgb(0x282a36),
            bg_secondary: rgb(0x21222c),
            bg_selected: rgb(0x44475a),
            bg_highlight: rgb(0x343746),

            fg_primary: rgb(0xf8f8f2),
            fg_secondary: rgb(0xcfcfd8),
            fg_selected: rgb(0xf8f8f2),
            fg_muted: rgb(0x6272a4),

            accent_primary: rgb(0xbd93f9),
            accent_secondary: rgb(0xff79c6),
            accent_success: rgb(0x50fa7b),
            accent_warning: rgb(0xf1fa8c),
            accent_error: rgb(0xff5555),
            accent_info: rgb(0x8be9fd),

            border_color: rgb(0x6272a4),
            title_color: rgb(0xff79c6),
            playing_indicator: rgb(0x50fa7b),
            current_track: rgb(0x8be9fd),
        }
    }

    /// Create a Gruvbox Dark terminal preset.
    pub fn gruvbox() -> Self {
        Self {
            bg_primary: rgb(0x282828),
            bg_secondary: rgb(0x3c3836),
            bg_selected: rgb(0x504945),
            bg_highlight: rgb(0x3c3836),

            fg_primary: rgb(0xebdbb2),
            fg_secondary: rgb(0xd5c4a1),
            fg_selected: rgb(0xfbf1c7),
            fg_muted: rgb(0xa89984),

            accent_primary: rgb(0x83a598),
            accent_secondary: rgb(0xd3869b),
            accent_success: rgb(0xb8bb26),
            accent_warning: rgb(0xfabd2f),
            accent_error: rgb(0xfb4934),
            accent_info: rgb(0x8ec07c),

            border_color: rgb(0x665c54),
            title_color: rgb(0xfabd2f),
            playing_indicator: rgb(0xb8bb26),
            current_track: rgb(0x83a598),
        }
    }

    /// Create a Tokyo Night terminal preset.
    pub fn tokyo_night() -> Self {
        Self {
            bg_primary: rgb(0x1a1b26),
            bg_secondary: rgb(0x24283b),
            bg_selected: rgb(0x3b4261),
            bg_highlight: rgb(0x292e42),

            fg_primary: rgb(0xc0caf5),
            fg_secondary: rgb(0xa9b1d6),
            fg_selected: rgb(0xffffff),
            fg_muted: rgb(0x565f89),

            accent_primary: rgb(0x7aa2f7),
            accent_secondary: rgb(0xbb9af7),
            accent_success: rgb(0x9ece6a),
            accent_warning: rgb(0xe0af68),
            accent_error: rgb(0xf7768e),
            accent_info: rgb(0x7dcfff),

            border_color: rgb(0x414868),
            title_color: rgb(0xbb9af7),
            playing_indicator: rgb(0x9ece6a),
            current_track: rgb(0x7aa2f7),
        }
    }

    /// Create a theme based on type
    pub fn from_type(theme_type: ThemeType) -> Self {
        match theme_type {
            ThemeType::Dark => Self::dark(),
            ThemeType::Light => Self::light(),
            ThemeType::Solarized => Self::solarized(),
            ThemeType::Dracula => Self::dracula(),
            ThemeType::Gruvbox => Self::gruvbox(),
            ThemeType::TokyoNight => Self::tokyo_night(),
        }
    }
}

fn rgb(hex: u32) -> Color {
    Color::Rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
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

        assert_eq!(
            ThemeType::from_str("solarized-dark"),
            Some(ThemeType::Solarized)
        );
        assert_eq!(ThemeType::from_str("Dracula"), Some(ThemeType::Dracula));
        assert_eq!(ThemeType::from_str("gruvbox"), Some(ThemeType::Gruvbox));
        assert_eq!(
            ThemeType::from_str("Tokyo Night"),
            Some(ThemeType::TokyoNight)
        );

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
    fn test_tui_preset_theme_names_and_colors() {
        assert_eq!(ThemeType::all().len(), 6);
        assert_eq!(ThemeType::TokyoNight.name(), "Tokyo Night");

        let solarized = Theme::from_type(ThemeType::Solarized);
        assert_eq!(solarized.bg_primary, Color::Rgb(0, 43, 54));
        assert_eq!(solarized.accent_primary, Color::Rgb(38, 139, 210));

        let dracula = Theme::from_type(ThemeType::Dracula);
        assert_eq!(dracula.bg_primary, Color::Rgb(40, 42, 54));
        assert_eq!(dracula.accent_secondary, Color::Rgb(255, 121, 198));

        let gruvbox = Theme::from_type(ThemeType::Gruvbox);
        assert_eq!(gruvbox.bg_primary, Color::Rgb(40, 40, 40));
        assert_eq!(gruvbox.title_color, Color::Rgb(250, 189, 47));

        let tokyo = Theme::from_type(ThemeType::TokyoNight);
        assert_eq!(tokyo.bg_primary, Color::Rgb(26, 27, 38));
        assert_eq!(tokyo.accent_info, Color::Rgb(125, 207, 255));
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
