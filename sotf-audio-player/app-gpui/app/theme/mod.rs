//! Theme system for the GPUI audio player.
//!
//! Provides color definitions for different UI themes.

// Allow approximate math constants in color values
#![allow(clippy::approx_constant)]

use gpui::{Rgba, SharedString};
use gpui_ui_kit::theme::{Theme as UiKitTheme, ThemeVariant as UiKitThemeVariant};
use serde::{Deserialize, Serialize};

/// Available theme identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeId {
    #[default]
    Dark,
    Light,
    Midnight,
    Forest,
    BlackAndWhite,
}

pub mod black;
pub mod black_and_white;
pub mod forest;
pub mod light;
pub mod midnight;

impl ThemeId {
    pub fn all() -> &'static [ThemeId] {
        &[
            ThemeId::Dark,
            ThemeId::Light,
            ThemeId::Midnight,
            ThemeId::Forest,
            ThemeId::BlackAndWhite,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            ThemeId::Dark => "Dark",
            ThemeId::Light => "Light",
            ThemeId::Midnight => "Midnight",
            ThemeId::Forest => "Forest",
            ThemeId::BlackAndWhite => "Black & White",
        }
    }

    pub fn next(&self) -> ThemeId {
        match self {
            ThemeId::Dark => ThemeId::Light,
            ThemeId::Light => ThemeId::Midnight,
            ThemeId::Midnight => ThemeId::Forest,
            ThemeId::Forest => ThemeId::BlackAndWhite,
            ThemeId::BlackAndWhite => ThemeId::Dark,
        }
    }
}

impl From<ThemeId> for UiKitThemeVariant {
    fn from(id: ThemeId) -> Self {
        match id {
            ThemeId::Dark => UiKitThemeVariant::Dark,
            ThemeId::Light => UiKitThemeVariant::Light,
            ThemeId::Midnight => UiKitThemeVariant::Midnight,
            ThemeId::Forest => UiKitThemeVariant::Forest,
            ThemeId::BlackAndWhite => UiKitThemeVariant::BlackAndWhite,
        }
    }
}

/// Plugin type color mapping
#[derive(Debug, Clone)]
pub struct PluginColorMap {
    pub eq: Rgba,
    pub gain: Rgba,
    pub upmixer: Rgba,
    pub compressor: Rgba,
    pub limiter: Rgba,
    pub gate: Rgba,
    pub loudness: Rgba,
    pub binaural: Rgba,
    pub convolution: Rgba,
    pub monitor: Rgba,
    pub spectrum: Rgba,
    pub mute_solo: Rgba,
}

/// Graph visualization line colors
#[derive(Debug, Clone)]
pub struct GraphLineColors {
    pub input: Rgba,
    pub target: Rgba,
    pub filter_response: Rgba,
    pub corrected: Rgba,
    pub error: Rgba,
    pub deviation: Rgba,
    pub grid: Rgba,
    pub secondary_line: Rgba,
    pub directivity_er: Rgba,
    pub directivity_sp: Rgba,
}

/// EQ curve visualization colors
#[derive(Debug, Clone)]
pub struct EQCurveColors {
    pub background: Rgba,
    pub grid: Rgba,
    pub curve_boost: Rgba,
    pub curve_cut: Rgba,
    pub fill_boost: Rgba,
    pub fill_cut: Rgba,
    pub zero_line: Rgba,
}

/// Spectrum analyzer colors
#[derive(Debug, Clone)]
pub struct SpectrumColors {
    pub background: Rgba,
    pub bass: Rgba,   // Low frequency
    pub mids: Rgba,   // Mid frequency
    pub treble: Rgba, // High frequency
}

/// Level meter colors
#[derive(Debug, Clone)]
pub struct MeterColors {
    pub background: Rgba,
    pub normal: Rgba,
    pub warning: Rgba,
    pub clip: Rgba,
    pub peak: Rgba,
    pub text: Rgba,
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

    // Text on accent (for contrast on accent backgrounds)
    pub text_on_accent: Rgba,
    pub text_on_accent_muted: Rgba,
    pub icon_on_accent: Rgba,

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

    // Plugin colors
    pub plugin_colors: PluginColorMap,
    pub graph_colors: GraphLineColors,
    pub band_colors: Vec<Rgba>,
    pub eq_curve_colors: EQCurveColors,
    pub spectrum_colors: SpectrumColors,
    pub meter_colors: MeterColors,

    // Additional semantic colors
    pub peak_indicator: Rgba,
    pub drag_over_highlight: Rgba,
    pub drag_over_border: Rgba,
    pub neutral_indicator: Rgba,
    pub warning_background: Rgba,
    pub knob_color: Rgba,
    pub optimization_color: Rgba,
    pub grid_color: Rgba,

    // Layout sizes
    pub separator_size: f32,

    // Font family
    pub font_family: SharedString,
}

impl Theme {
    /// Convert the app theme to the ui-kit theme so defaults are consistent without per-call overrides.
    pub fn to_ui_kit_theme(&self, id: ThemeId) -> UiKitTheme {
        UiKitTheme {
            variant: UiKitThemeVariant::from(id),
            background: self.background,
            surface: self.surface,
            surface_hover: self.surface_hover,
            muted: self.background_secondary,
            text_primary: self.text_primary,
            text_secondary: self.text_secondary,
            text_muted: self.text_muted,
            accent: self.accent,
            accent_hover: self.accent_hover,
            accent_muted: self.accent_muted,
            success: self.success,
            warning: self.warning,
            error: self.error,
            info: self.info,
            border: self.border,
            border_hover: self.border_focused,
        }
    }

    /// Create theme from ThemeId
    pub fn from_id(id: ThemeId) -> Self {
        match id {
            ThemeId::Dark => Self::dark(),
            ThemeId::Light => Self::light(),
            ThemeId::Midnight => Self::midnight(),
            ThemeId::Forest => Self::forest(),
            ThemeId::BlackAndWhite => Self::black_and_white(),
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
    /// Apply opacity to a color (0.0 = transparent, 1.0 = opaque)
    pub fn with_opacity(color: Rgba, opacity: f32) -> Rgba {
        let mut c = color;
        c.a = opacity.clamp(0.0, 1.0);
        c
    }

    /// Common opacity: 8% (~21 alpha)
    pub fn opacity_8pct(color: Rgba) -> Rgba {
        Self::with_opacity(color, 0.08)
    }

    /// Common opacity: 20% (~51 alpha)
    pub fn opacity_20pct(color: Rgba) -> Rgba {
        Self::with_opacity(color, 0.2)
    }

    /// Common opacity: 25% (~64 alpha)
    pub fn opacity_25pct(color: Rgba) -> Rgba {
        Self::with_opacity(color, 0.25)
    }

    /// Convert to ButtonTheme for use with ui_kit Button component
    pub fn to_button_theme(&self) -> gpui_ui_kit::ButtonTheme {
        gpui_ui_kit::ButtonTheme {
            accent: self.accent,
            accent_hover: self.accent_hover,
            surface: self.surface,
            surface_hover: self.surface_hover,
            text_primary: self.text_primary,
            text_secondary: self.text_secondary,
            // Use text_on_accent for Primary variant buttons (on accent background)
            text_on_accent: self.text_on_accent,
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

    /// Convert to SliderTheme for use with ui_kit Slider component
    pub fn to_slider_theme(&self) -> gpui_ui_kit::SliderTheme {
        gpui_ui_kit::SliderTheme {
            track: self.surface_hover,
            fill: self.accent,
            thumb: self.text_primary,
            thumb_hover: self.text_secondary,
            thumb_active: self.accent,
            label: self.text_primary,
            value: self.text_secondary,
        }
    }

    /// Convert to IconButtonTheme for use with ui_kit IconButton component
    pub fn to_icon_button_theme(&self) -> gpui_ui_kit::IconButtonTheme {
        gpui_ui_kit::IconButtonTheme {
            ghost_bg: Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
            ghost_hover_bg: self.surface_hover,
            selected_bg: self.surface_selected,
            selected_hover_bg: self.surface_hover,
            filled_bg: self.surface,
            filled_hover_bg: self.surface_hover,
            accent: self.accent,
            accent_hover: self.accent_hover,
            text: self.text_primary,
            text_on_accent: self.text_primary,
            border: self.border,
        }
    }

    /// Convert to TabsTheme for use with ui_kit Tabs component
    pub fn to_tabs_theme(&self) -> gpui_ui_kit::TabsTheme {
        gpui_ui_kit::TabsTheme {
            container_bg: self.surface,
            container_border: self.border,
            selected_bg: self.surface_selected,
            selected_hover_bg: self.surface_hover,
            hover_bg: self.surface_hover,
            accent: self.accent,
            // Use text_on_accent for selected text since accent is used as background
            text_selected: self.text_on_accent,
            text_unselected: self.text_secondary,
            text_hover: self.text_primary,
            badge_bg: self.surface_hover,
            close_color: self.text_muted,
            close_hover_color: self.text_primary,
            icon_selected: Some(self.icon_on_accent),
            icon_unselected: None,
        }
    }

    /// Convert to MenuTheme for use with ui_kit Menu component
    pub fn to_menu_theme(&self) -> gpui_ui_kit::MenuTheme {
        gpui_ui_kit::MenuTheme {
            background: self.surface,
            border: self.border,
            separator: self.border,
            text: self.text_secondary,
            text_hover: self.text_primary,
            text_disabled: self.text_disabled,
            text_shortcut: self.text_muted,
            hover_bg: self.surface_hover,
            danger_hover_bg: self.error,
        }
    }

    /// Convert to PotentiometerTheme for use with ui_kit Potentiometer component
    pub fn to_potentiometer_theme(&self) -> gpui_ui_kit::PotentiometerTheme {
        gpui_ui_kit::PotentiometerTheme {
            surface: self.surface,
            surface_hover: self.surface_hover,
            knob_bg: self.background_secondary,
            accent: self.accent,
            accent_muted: self.accent_muted,
            border: self.border,
            text_secondary: self.text_secondary,
            text_primary: self.text_primary,
            text_muted: self.text_muted,
            text_on_accent: self.text_on_accent,
            background_secondary: self.background_secondary,
        }
    }

    /// Convert to ToggleTheme for use with ui_kit Toggle component
    pub fn to_toggle_theme(&self) -> gpui_ui_kit::ToggleTheme {
        gpui_ui_kit::ToggleTheme {
            checked_bg: self.accent,
            unchecked_bg: self.surface,
            knob: self.text_primary,
            knob_on_checked: self.text_on_accent,
            track_border: self.border,
            label: self.text_secondary,
            accent: self.accent,
            accent_muted: self.accent_muted,
            success: self.success,
            border: self.border,
            text_on_accent: self.text_on_accent,
            text_muted: self.text_muted,
        }
    }
}
