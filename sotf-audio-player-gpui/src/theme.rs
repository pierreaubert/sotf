//! Theme system for the GPUI audio player.
//!
//! Provides color definitions for different UI themes.

use gpui::Rgba;
use serde::{Deserialize, Serialize};

/// Available theme identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeId {
    #[default]
    Dark,
    Light,
    Midnight,
    Forest,
}

impl ThemeId {
    pub fn all() -> &'static [ThemeId] {
        &[ThemeId::Dark, ThemeId::Light, ThemeId::Midnight, ThemeId::Forest]
    }

    pub fn name(&self) -> &'static str {
        match self {
            ThemeId::Dark => "Dark",
            ThemeId::Light => "Light",
            ThemeId::Midnight => "Midnight",
            ThemeId::Forest => "Forest",
        }
    }

    pub fn next(&self) -> ThemeId {
        match self {
            ThemeId::Dark => ThemeId::Light,
            ThemeId::Light => ThemeId::Midnight,
            ThemeId::Midnight => ThemeId::Forest,
            ThemeId::Forest => ThemeId::Dark,
        }
    }
}

/// Complete theme definition with all UI colors
#[derive(Debug, Clone)]
pub struct Theme {
    // Base colors
    pub background: Rgba,
    pub background_secondary: Rgba,
    pub background_tertiary: Rgba,
    pub surface: Rgba,
    pub surface_hover: Rgba,
    pub surface_selected: Rgba,

    // Text colors
    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,
    pub text_disabled: Rgba,

    // Border colors
    pub border: Rgba,
    pub border_focused: Rgba,

    // Accent colors
    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub accent_muted: Rgba,

    // Semantic colors
    pub success: Rgba,
    pub warning: Rgba,
    pub error: Rgba,
    pub info: Rgba,

    // Level meter colors
    pub meter_normal: Rgba,
    pub meter_warning: Rgba,
    pub meter_clip: Rgba,

    // Button colors
    pub button_mute_active: Rgba,
    pub button_solo_active: Rgba,
    pub button_dim_active: Rgba,

    // Playback bar
    pub progress_bar_bg: Rgba,
    pub progress_bar_fill: Rgba,

    // Toast backgrounds
    pub toast_success_bg: Rgba,
    pub toast_error_bg: Rgba,
    pub toast_info_bg: Rgba,
    pub toast_warning_bg: Rgba,
}

impl Theme {
    /// Create theme from ThemeId
    pub fn from_id(id: ThemeId) -> Self {
        match id {
            ThemeId::Dark => Self::dark(),
            ThemeId::Light => Self::light(),
            ThemeId::Midnight => Self::midnight(),
            ThemeId::Forest => Self::forest(),
        }
    }

    /// Dark theme (default)
    pub fn dark() -> Self {
        Self {
            // Base colors
            background: rgba(0x1e1e1e),
            background_secondary: rgba(0x252525),
            background_tertiary: rgba(0x2d2d2d),
            surface: rgba(0x2d2d2d),
            surface_hover: rgba(0x3e3e3e),
            surface_selected: rgba(0x264f78),

            // Text colors
            text_primary: rgba(0xcccccc),
            text_secondary: rgba(0x999999),
            text_muted: rgba(0x666666),
            text_disabled: rgba(0x444444),

            // Border colors
            border: rgba(0x3e3e3e),
            border_focused: rgba(0x007acc),

            // Accent colors
            accent: rgba(0x007acc),
            accent_hover: rgba(0x1c8cd9),
            accent_muted: rgba(0x264f78),

            // Semantic colors
            success: rgba(0x4ec9b0),
            warning: rgba(0xdcdcaa),
            error: rgba(0xf48771),
            info: rgba(0x569cd6),

            // Level meter colors
            meter_normal: rgba(0x22c55e),
            meter_warning: rgba(0xf59e0b),
            meter_clip: rgba(0xdc2626),

            // Button colors
            button_mute_active: rgba(0xdc2626),
            button_solo_active: rgba(0xf59e0b),
            button_dim_active: rgba(0x6366f1),

            // Playback bar
            progress_bar_bg: rgba(0x3e3e3e),
            progress_bar_fill: rgba(0x007acc),

            // Toast backgrounds
            toast_success_bg: rgba(0x1e3a1e),
            toast_error_bg: rgba(0x3a1e1e),
            toast_info_bg: rgba(0x1e2a3a),
            toast_warning_bg: rgba(0x3a2e1e),
        }
    }

    /// Light theme
    pub fn light() -> Self {
        Self {
            // Base colors
            background: rgba(0xf5f5f5),
            background_secondary: rgba(0xeeeeee),
            background_tertiary: rgba(0xe0e0e0),
            surface: rgba(0xffffff),
            surface_hover: rgba(0xf0f0f0),
            surface_selected: rgba(0xcce5ff),

            // Text colors
            text_primary: rgba(0x1e1e1e),
            text_secondary: rgba(0x555555),
            text_muted: rgba(0x888888),
            text_disabled: rgba(0xaaaaaa),

            // Border colors
            border: rgba(0xcccccc),
            border_focused: rgba(0x0066cc),

            // Accent colors
            accent: rgba(0x0066cc),
            accent_hover: rgba(0x0077ee),
            accent_muted: rgba(0xb3d4fc),

            // Semantic colors
            success: rgba(0x28a745),
            warning: rgba(0xffc107),
            error: rgba(0xdc3545),
            info: rgba(0x17a2b8),

            // Level meter colors
            meter_normal: rgba(0x28a745),
            meter_warning: rgba(0xffc107),
            meter_clip: rgba(0xdc3545),

            // Button colors
            button_mute_active: rgba(0xdc3545),
            button_solo_active: rgba(0xffc107),
            button_dim_active: rgba(0x6f42c1),

            // Playback bar
            progress_bar_bg: rgba(0xcccccc),
            progress_bar_fill: rgba(0x0066cc),

            // Toast backgrounds
            toast_success_bg: rgba(0xd4edda),
            toast_error_bg: rgba(0xf8d7da),
            toast_info_bg: rgba(0xd1ecf1),
            toast_warning_bg: rgba(0xfff3cd),
        }
    }

    /// Midnight theme (deep blue)
    pub fn midnight() -> Self {
        Self {
            // Base colors
            background: rgba(0x0d1117),
            background_secondary: rgba(0x161b22),
            background_tertiary: rgba(0x21262d),
            surface: rgba(0x21262d),
            surface_hover: rgba(0x30363d),
            surface_selected: rgba(0x1f6feb33),

            // Text colors
            text_primary: rgba(0xc9d1d9),
            text_secondary: rgba(0x8b949e),
            text_muted: rgba(0x6e7681),
            text_disabled: rgba(0x484f58),

            // Border colors
            border: rgba(0x30363d),
            border_focused: rgba(0x58a6ff),

            // Accent colors
            accent: rgba(0x58a6ff),
            accent_hover: rgba(0x79b8ff),
            accent_muted: rgba(0x1f6feb),

            // Semantic colors
            success: rgba(0x3fb950),
            warning: rgba(0xd29922),
            error: rgba(0xf85149),
            info: rgba(0x58a6ff),

            // Level meter colors
            meter_normal: rgba(0x3fb950),
            meter_warning: rgba(0xd29922),
            meter_clip: rgba(0xf85149),

            // Button colors
            button_mute_active: rgba(0xf85149),
            button_solo_active: rgba(0xd29922),
            button_dim_active: rgba(0x8957e5),

            // Playback bar
            progress_bar_bg: rgba(0x30363d),
            progress_bar_fill: rgba(0x58a6ff),

            // Toast backgrounds
            toast_success_bg: rgba(0x1b4721),
            toast_error_bg: rgba(0x490202),
            toast_info_bg: rgba(0x0d2140),
            toast_warning_bg: rgba(0x4a3219),
        }
    }

    /// Forest theme (green tones)
    pub fn forest() -> Self {
        Self {
            // Base colors
            background: rgba(0x1a2418),
            background_secondary: rgba(0x222d1f),
            background_tertiary: rgba(0x2a3627),
            surface: rgba(0x2a3627),
            surface_hover: rgba(0x3a4a35),
            surface_selected: rgba(0x3d5a3a),

            // Text colors
            text_primary: rgba(0xd4e4d1),
            text_secondary: rgba(0xa8c4a2),
            text_muted: rgba(0x7a9a73),
            text_disabled: rgba(0x556b50),

            // Border colors
            border: rgba(0x3a4a35),
            border_focused: rgba(0x6abf69),

            // Accent colors
            accent: rgba(0x6abf69),
            accent_hover: rgba(0x7dd07c),
            accent_muted: rgba(0x3d5a3a),

            // Semantic colors
            success: rgba(0x6abf69),
            warning: rgba(0xe0c062),
            error: rgba(0xd96c6c),
            info: rgba(0x6cb2d9),

            // Level meter colors
            meter_normal: rgba(0x6abf69),
            meter_warning: rgba(0xe0c062),
            meter_clip: rgba(0xd96c6c),

            // Button colors
            button_mute_active: rgba(0xd96c6c),
            button_solo_active: rgba(0xe0c062),
            button_dim_active: rgba(0x9b7fd9),

            // Playback bar
            progress_bar_bg: rgba(0x3a4a35),
            progress_bar_fill: rgba(0x6abf69),

            // Toast backgrounds
            toast_success_bg: rgba(0x1e3a1e),
            toast_error_bg: rgba(0x3a1e1e),
            toast_info_bg: rgba(0x1e2a3a),
            toast_warning_bg: rgba(0x3a321e),
        }
    }
}

/// Helper function to create Rgba from hex value
fn rgba(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 16) & 0xFF) as f32 / 255.0,
        g: ((hex >> 8) & 0xFF) as f32 / 255.0,
        b: (hex & 0xFF) as f32 / 255.0,
        a: 1.0,
    }
}

impl Theme {
    /// Convert to ButtonTheme for use with ui_kit Button component
    pub fn to_button_theme(&self) -> gpui_ui_kit::ButtonTheme {
        gpui_ui_kit::ButtonTheme {
            accent: self.accent,
            accent_hover: self.accent_hover,
            surface: self.surface,
            surface_hover: self.surface_hover,
            text_primary: self.text_primary,
            text_secondary: self.text_secondary,
            error: self.error,
            border: self.border,
        }
    }

    /// Convert to AccordionTheme for use with ui_kit Accordion component
    pub fn to_accordion_theme(&self) -> gpui_ui_kit::AccordionTheme {
        gpui_ui_kit::AccordionTheme {
            header_bg: self.surface,
            header_hover_bg: self.surface_hover,
            content_bg: self.background,
            border: self.border,
            title_color: self.text_primary,
            indicator_color: self.text_muted,
        }
    }
}
