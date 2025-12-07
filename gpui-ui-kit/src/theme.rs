//! Theme system for gpui-ui-kit
//!
//! Provides a unified theming system with light and dark themes.

use gpui::*;

/// Available theme variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeVariant {
    /// Dark theme (default)
    #[default]
    Dark,
    /// Light theme
    Light,
}

impl ThemeVariant {
    /// Get all available variants
    pub fn all() -> &'static [ThemeVariant] {
        &[ThemeVariant::Dark, ThemeVariant::Light]
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            ThemeVariant::Dark => "Dark",
            ThemeVariant::Light => "Light",
        }
    }

    /// Toggle to next variant
    pub fn toggle(&self) -> Self {
        match self {
            ThemeVariant::Dark => ThemeVariant::Light,
            ThemeVariant::Light => ThemeVariant::Dark,
        }
    }
}

/// Global theme colors
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme variant
    pub variant: ThemeVariant,

    // Background colors
    /// Main background color
    pub background: Rgba,
    /// Elevated surface background (cards, dialogs)
    pub surface: Rgba,
    /// Surface on hover
    pub surface_hover: Rgba,
    /// Muted background for secondary elements
    pub muted: Rgba,

    // Text colors
    /// Primary text color
    pub text_primary: Rgba,
    /// Secondary/muted text color
    pub text_secondary: Rgba,
    /// Disabled text color
    pub text_muted: Rgba,

    // Accent colors
    /// Primary accent color
    pub accent: Rgba,
    /// Accent on hover
    pub accent_hover: Rgba,
    /// Muted accent for backgrounds
    pub accent_muted: Rgba,

    // Semantic colors
    /// Success color
    pub success: Rgba,
    /// Warning color
    pub warning: Rgba,
    /// Error color
    pub error: Rgba,
    /// Info color
    pub info: Rgba,

    // Border colors
    /// Default border
    pub border: Rgba,
    /// Border on hover/focus
    pub border_hover: Rgba,
}

impl Theme {
    /// Create a dark theme
    pub fn dark() -> Self {
        Self {
            variant: ThemeVariant::Dark,
            // Backgrounds
            background: rgb(0x1e1e1e),
            surface: rgb(0x2a2a2a),
            surface_hover: rgb(0x3a3a3a),
            muted: rgb(0x252525),
            // Text
            text_primary: rgb(0xffffff),
            text_secondary: rgb(0xcccccc),
            text_muted: rgb(0x888888),
            // Accent
            accent: rgb(0x007acc),
            accent_hover: rgb(0x0098ff),
            accent_muted: rgba(0x007acc33),
            // Semantic
            success: rgb(0x22c55e),
            warning: rgb(0xf59e0b),
            error: rgb(0xef4444),
            info: rgb(0x3b82f6),
            // Border
            border: rgb(0x3a3a3a),
            border_hover: rgb(0x555555),
        }
    }

    /// Create a light theme
    pub fn light() -> Self {
        Self {
            variant: ThemeVariant::Light,
            // Backgrounds
            background: rgb(0xf5f5f5),
            surface: rgb(0xffffff),
            surface_hover: rgb(0xf0f0f0),
            muted: rgb(0xeeeeee),
            // Text
            text_primary: rgb(0x1a1a1a),
            text_secondary: rgb(0x4a4a4a),
            text_muted: rgb(0x888888),
            // Accent
            accent: rgb(0x0066cc),
            accent_hover: rgb(0x0055aa),
            accent_muted: rgba(0x0066cc22),
            // Semantic
            success: rgb(0x16a34a),
            warning: rgb(0xd97706),
            error: rgb(0xdc2626),
            info: rgb(0x2563eb),
            // Border
            border: rgb(0xd4d4d4),
            border_hover: rgb(0xaaaaaa),
        }
    }

    /// Get theme for variant
    pub fn for_variant(variant: ThemeVariant) -> Self {
        match variant {
            ThemeVariant::Dark => Self::dark(),
            ThemeVariant::Light => Self::light(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

/// Global state for theme management
pub struct ThemeState {
    pub theme: Theme,
}

impl Global for ThemeState {}

impl ThemeState {
    /// Create new theme state with default (dark) theme
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
        }
    }

    /// Create theme state with specific variant
    pub fn with_variant(variant: ThemeVariant) -> Self {
        Self {
            theme: Theme::for_variant(variant),
        }
    }

    /// Set theme variant
    pub fn set_variant(&mut self, variant: ThemeVariant) {
        self.theme = Theme::for_variant(variant);
    }

    /// Toggle between light and dark themes
    pub fn toggle(&mut self) {
        self.set_variant(self.theme.variant.toggle());
    }
}

impl Default for ThemeState {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension trait for easy theme access
pub trait ThemeExt {
    /// Get the current theme
    fn theme(&self) -> Theme;
}

impl ThemeExt for App {
    fn theme(&self) -> Theme {
        self.try_global::<ThemeState>()
            .map(|s| s.theme.clone())
            .unwrap_or_else(Theme::dark)
    }
}

// Conversions to component themes
use crate::button::ButtonTheme;

impl From<&Theme> for ButtonTheme {
    fn from(theme: &Theme) -> Self {
        ButtonTheme {
            accent: theme.accent,
            accent_hover: theme.accent_hover,
            surface: theme.surface,
            surface_hover: theme.surface_hover,
            text_primary: theme.text_primary,
            text_secondary: theme.text_secondary,
            error: theme.error,
            border: theme.border,
        }
    }
}
