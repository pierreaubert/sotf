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
    /// Midnight theme (deep blue)
    Midnight,
    /// Forest theme (green tones)
    Forest,
    /// Black & White theme (monochrome high contrast)
    BlackAndWhite,
}

impl ThemeVariant {
    /// Get all available variants
    pub fn all() -> &'static [ThemeVariant] {
        &[
            ThemeVariant::Dark,
            ThemeVariant::Light,
            ThemeVariant::Midnight,
            ThemeVariant::Forest,
            ThemeVariant::BlackAndWhite,
        ]
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            ThemeVariant::Dark => "Dark",
            ThemeVariant::Light => "Light",
            ThemeVariant::Midnight => "Midnight",
            ThemeVariant::Forest => "Forest",
            ThemeVariant::BlackAndWhite => "Black & White",
        }
    }

    /// Toggle to next variant
    pub fn toggle(&self) -> Self {
        match self {
            ThemeVariant::Dark => ThemeVariant::Light,
            ThemeVariant::Light => ThemeVariant::Midnight,
            ThemeVariant::Midnight => ThemeVariant::Forest,
            ThemeVariant::Forest => ThemeVariant::BlackAndWhite,
            ThemeVariant::BlackAndWhite => ThemeVariant::Dark,
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

    /// Create a midnight theme (deep blue)
    pub fn midnight() -> Self {
        Self {
            variant: ThemeVariant::Midnight,
            // Backgrounds
            background: rgb(0x0d1117),
            surface: rgb(0x21262d),
            surface_hover: rgb(0x30363d),
            muted: rgb(0x161b22),
            // Text
            text_primary: rgb(0xc9d1d9),
            text_secondary: rgb(0x8b949e),
            text_muted: rgb(0x6e7681),
            // Accent
            accent: rgb(0x58a6ff),
            accent_hover: rgb(0x79b8ff),
            accent_muted: rgba(0x1f6feb33),
            // Semantic
            success: rgb(0x3fb950),
            warning: rgb(0xd29922),
            error: rgb(0xf85149),
            info: rgb(0x58a6ff),
            // Border
            border: rgb(0x30363d),
            border_hover: rgb(0x484f58),
        }
    }

    /// Create a forest theme (green tones)
    pub fn forest() -> Self {
        Self {
            variant: ThemeVariant::Forest,
            // Backgrounds
            background: rgb(0x1a2418),
            surface: rgb(0x2a3627),
            surface_hover: rgb(0x3a4a35),
            muted: rgb(0x222d1f),
            // Text
            text_primary: rgb(0xd4e4d1),
            text_secondary: rgb(0xa8c4a2),
            text_muted: rgb(0x7a9a73),
            // Accent
            accent: rgb(0x6abf69),
            accent_hover: rgb(0x7dd07c),
            accent_muted: rgba(0x3d5a3a33),
            // Semantic
            success: rgb(0x6abf69),
            warning: rgb(0xe0c062),
            error: rgb(0xd96c6c),
            info: rgb(0x6cb2d9),
            // Border
            border: rgb(0x3a4a35),
            border_hover: rgb(0x556b50),
        }
    }

    /// Create a black & white theme (monochrome high contrast)
    pub fn black_and_white() -> Self {
        Self {
            variant: ThemeVariant::BlackAndWhite,
            // Backgrounds
            background: rgb(0x000000),
            surface: rgb(0x141414),
            surface_hover: rgb(0x222222),
            muted: rgb(0x0a0a0a),
            // Text
            text_primary: rgb(0xffffff),
            text_secondary: rgb(0x888888),
            text_muted: rgb(0x555555),
            // Accent (black background with white border for buttons)
            accent: rgb(0x000000),
            accent_hover: rgb(0x222222),
            accent_muted: rgba(0x33333333),
            // Semantic (grayscale for B&W theme)
            success: rgb(0xaaaaaa),
            warning: rgb(0x888888),
            error: rgb(0x666666),
            info: rgb(0x999999),
            // Border (white for high contrast)
            border: rgb(0xffffff),
            border_hover: rgb(0xcccccc),
        }
    }

    /// Get theme for variant
    pub fn for_variant(variant: ThemeVariant) -> Self {
        match variant {
            ThemeVariant::Dark => Self::dark(),
            ThemeVariant::Light => Self::light(),
            ThemeVariant::Midnight => Self::midnight(),
            ThemeVariant::Forest => Self::forest(),
            ThemeVariant::BlackAndWhite => Self::black_and_white(),
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

// Shadow helpers for hover effects

/// Create a glow shadow effect for hover states.
/// This is a shared helper to avoid duplicating shadow construction
/// across button, accordion, menu, tabs, and other components.
pub fn glow_shadow(color: Rgba) -> Vec<BoxShadow> {
    let glow_inner = Hsla::from(color).alpha(0.6);
    let glow_outer = Hsla::from(color).alpha(0.2);
    vec![
        BoxShadow {
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(4.0),
            spread_radius: px(0.0),
            color: glow_inner,
        },
        BoxShadow {
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(25.0),
            spread_radius: px(2.0),
            color: glow_outer,
        },
    ]
}
