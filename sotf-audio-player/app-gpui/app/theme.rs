//! Theme system for the GPUI audio player.
//!
//! Provides color definitions for different UI themes.

// Allow approximate math constants in color values
#![allow(clippy::approx_constant)]

use gpui::{Rgba, SharedString};
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

            // Text on accent
            text_on_accent: rgba(0xffffff),
            text_on_accent_muted: rgba(0xffffffcc),

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

            // Plugin colors
            plugin_colors: PluginColorMap {
                eq: rgba(0x2563eb),
                gain: rgba(0x059669),
                upmixer: rgba(0x7c3aed),
                compressor: rgba(0xdc2626),
                limiter: rgba(0xea580c),
                gate: rgba(0xca8a04),
                loudness: rgba(0x0891b2),
                binaural: rgba(0xdb2777),
                convolution: rgba(0x4f46e5),
                monitor: rgba(0x14b8a6),
                spectrum: rgba(0x8b5cf6),
                mute_solo: rgba(0x6366f1),
            },
            graph_colors: GraphLineColors {
                input: rgba(0x6366f1),
                target: rgba(0x22c55e),
                filter_response: rgba(0xf59e0b),
                corrected: rgba(0x3b82f6),
                error: rgba(0xef4444),
                deviation: rgba(0x8b5cf6),
                grid: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.083,
                }, // rgba(0xffffff15) = 21/255 ≈ 8.3%
                secondary_line: Rgba {
                    r: 0.667,
                    g: 0.667,
                    b: 0.667,
                    a: 1.0,
                }, // rgba(0xaaaaaa)
                directivity_er: Rgba {
                    r: 0.961,
                    g: 0.443,
                    b: 0.714,
                    a: 1.0,
                }, // rgba(0xf472b6)
                directivity_sp: Rgba {
                    r: 0.753,
                    g: 0.518,
                    b: 0.992,
                    a: 1.0,
                }, // rgba(0xc084fc)
            },
            band_colors: vec![
                rgba(0xef4444), // Red
                rgba(0xf97316), // Orange
                rgba(0xeab308), // Yellow
                rgba(0x22c55e), // Green
                rgba(0x14b8a6), // Teal
                rgba(0x3b82f6), // Blue
                rgba(0x8b5cf6), // Violet
                rgba(0xec4899), // Pink
                rgba(0x6366f1), // Indigo
                rgba(0x06b6d4), // Cyan
                rgba(0x9ca3af), // Gray
            ],
            eq_curve_colors: EQCurveColors {
                background: rgba(0x1a1a1a),
                grid: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.125,
                }, // rgba(0xffffff20) = 32/255 ≈ 12.5%
                curve_boost: rgba(0x22c55e),
                curve_cut: rgba(0xef4444),
                fill_boost: Rgba {
                    r: 0.133,
                    g: 0.773,
                    b: 0.369,
                    a: 0.251,
                }, // rgba(0x22c55e40) = 64/255 ≈ 25%
                fill_cut: Rgba {
                    r: 0.939,
                    g: 0.267,
                    b: 0.267,
                    a: 0.251,
                }, // rgba(0xef444440) = 64/255 ≈ 25%
                zero_line: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.251,
                }, // rgba(0xffffff40) = 64/255 ≈ 25%
            },
            spectrum_colors: SpectrumColors {
                background: rgba(0x000000),
                bass: rgba(0x22c55e),
                mids: rgba(0xeab308),
                treble: rgba(0xef4444),
            },
            meter_colors: MeterColors {
                background: rgba(0x1e1e1e),
                normal: rgba(0x22c55e),
                warning: rgba(0xf59e0b),
                clip: rgba(0xdc2626),
                peak: rgba(0xffffff),
                text: rgba(0x999999),
            },

            // Additional semantic colors
            peak_indicator: rgba(0xffffff),
            drag_over_highlight: Rgba {
                r: 0.231,
                g: 0.510,
                b: 0.961,
                a: 0.251,
            }, // rgba(0x3b82f640) = 64/255 ≈ 25%
            drag_over_border: rgba(0x3b82f6),
            neutral_indicator: rgba(0x6366f1),
            warning_background: Rgba {
                r: 0.961,
                g: 0.616,
                b: 0.067,
                a: 0.2,
            }, // rgba(0xf59e0b33) ≈ 20%
            knob_color: rgba(0xffffff),
            optimization_color: rgba(0x8b5cf6),
            grid_color: Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.083,
            }, // rgba(0xffffff15) = 21/255 ≈ 8.3%

            // Layout sizes
            separator_size: 20.0,

            // Font family
            font_family: ".SystemUI".into(),
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

            // Text on accent
            text_on_accent: rgba(0xffffff),
            text_on_accent_muted: rgba(0xffffffcc),

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

            // Plugin colors
            plugin_colors: PluginColorMap {
                eq: rgba(0x0066cc),
                gain: rgba(0x28a745),
                upmixer: rgba(0x6f42c1),
                compressor: rgba(0xdc3545),
                limiter: rgba(0xfd7e14),
                gate: rgba(0xffc107),
                loudness: rgba(0x17a2b8),
                binaural: rgba(0xe83e8c),
                convolution: rgba(0x6610f2),
                monitor: rgba(0x20c997),
                spectrum: rgba(0x9b59b6),
                mute_solo: rgba(0x007bff),
            },
            graph_colors: GraphLineColors {
                input: rgba(0x007bff),
                target: rgba(0x28a745),
                filter_response: rgba(0xffc107),
                corrected: rgba(0x0066cc),
                error: rgba(0xdc3545),
                deviation: rgba(0x9b59b6),
                grid: Rgba {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.063,
                }, // rgba(0x00000010) = 16/255 ≈ 6.3%
                secondary_line: Rgba {
                    r: 0.333,
                    g: 0.333,
                    b: 0.333,
                    a: 1.0,
                }, // rgba(0x555555)
                directivity_er: Rgba {
                    r: 0.910,
                    g: 0.118,
                    b: 0.388,
                    a: 1.0,
                }, // rgba(0xe91e63)
                directivity_sp: Rgba {
                    r: 0.612,
                    g: 0.153,
                    b: 0.690,
                    a: 1.0,
                }, // rgba(0x9c27b0)
            },
            band_colors: vec![
                rgba(0xdc3545), // Red
                rgba(0xfd7e14), // Orange
                rgba(0xffc107), // Yellow
                rgba(0x28a745), // Green
                rgba(0x20c997), // Teal
                rgba(0x0066cc), // Blue
                rgba(0x6f42c1), // Violet
                rgba(0xe83e8c), // Pink
                rgba(0x007bff), // Indigo
                rgba(0x00bcd4), // Cyan
                rgba(0x999999), // Gray
            ],
            eq_curve_colors: EQCurveColors {
                background: rgba(0xfafafa),
                grid: Rgba {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.063,
                }, // rgba(0x00000010) = 16/255 ≈ 6.3%
                curve_boost: rgba(0x28a745),
                curve_cut: rgba(0xdc3545),
                fill_boost: Rgba {
                    r: 0.157,
                    g: 0.655,
                    b: 0.267,
                    a: 0.188,
                }, // rgba(0x28a74430) ≈ 19%
                fill_cut: Rgba {
                    r: 0.863,
                    g: 0.208,
                    b: 0.208,
                    a: 0.188,
                }, // rgba(0xdc354530) ≈ 19%
                zero_line: Rgba {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.188,
                }, // rgba(0x00000030) ≈ 19%
            },
            spectrum_colors: SpectrumColors {
                background: rgba(0xffffff),
                bass: rgba(0x28a745),
                mids: rgba(0xffc107),
                treble: rgba(0xdc3545),
            },
            meter_colors: MeterColors {
                background: rgba(0xf5f5f5),
                normal: rgba(0x28a745),
                warning: rgba(0xffc107),
                clip: rgba(0xdc3545),
                peak: rgba(0x000000),
                text: rgba(0x555555),
            },

            // Additional semantic colors
            peak_indicator: rgba(0x000000),
            drag_over_highlight: Rgba {
                r: 0.0,
                g: 0.4,
                b: 0.8,
                a: 0.251,
            }, // rgba(0x0066cc40) ≈ 25%
            drag_over_border: rgba(0x0066cc),
            neutral_indicator: rgba(0x007bff),
            warning_background: Rgba {
                r: 1.0,
                g: 0.753,
                b: 0.0,
                a: 0.188,
            }, // rgba(0xffc10730) ≈ 19%
            knob_color: rgba(0x000000),
            optimization_color: rgba(0x9b59b6),
            grid_color: Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.063,
            }, // rgba(0x00000010) = 16/255 ≈ 6.3%

            // Layout sizes
            separator_size: 20.0,

            // Font family
            font_family: ".SystemUI".into(),
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

            // Text on accent
            text_on_accent: rgba(0xffffff),
            text_on_accent_muted: rgba(0xffffffcc),

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

            // Plugin colors
            plugin_colors: PluginColorMap {
                eq: rgba(0x79c0ff),
                gain: rgba(0x3fb950),
                upmixer: rgba(0xa371f7),
                compressor: rgba(0xf85149),
                limiter: rgba(0xfb8500),
                gate: rgba(0xd29922),
                loudness: rgba(0x56d4dd),
                binaural: rgba(0xf0883e),
                convolution: rgba(0xbc8ef1),
                monitor: rgba(0x3fb950),
                spectrum: rgba(0xa371f7),
                mute_solo: rgba(0x58a6ff),
            },
            graph_colors: GraphLineColors {
                input: rgba(0x79c0ff),
                target: rgba(0x3fb950),
                filter_response: rgba(0xd29922),
                corrected: rgba(0x58a6ff),
                error: rgba(0xf85149),
                deviation: rgba(0xa371f7),
                grid: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.083,
                }, // rgba(0xffffff15) = 21/255 ≈ 8.3%
                secondary_line: Rgba {
                    r: 0.545,
                    g: 0.576,
                    b: 0.620,
                    a: 1.0,
                }, // rgba(0x8b949e)
                directivity_er: Rgba {
                    r: 0.941,
                    g: 0.502,
                    b: 0.502,
                    a: 1.0,
                }, // rgba(0xf08080)
                directivity_sp: Rgba {
                    r: 0.855,
                    g: 0.439,
                    b: 0.839,
                    a: 1.0,
                }, // rgba(0xda70d6)
            },
            band_colors: vec![
                rgba(0xf85149), // Red
                rgba(0xfb8500), // Orange
                rgba(0xd29922), // Yellow
                rgba(0x3fb950), // Green
                rgba(0x56d4dd), // Teal
                rgba(0x79c0ff), // Blue
                rgba(0xa371f7), // Violet
                rgba(0xf0883e), // Pink
                rgba(0x58a6ff), // Indigo
                rgba(0x7ee787), // Cyan
                rgba(0x8b949e), // Gray
            ],
            eq_curve_colors: EQCurveColors {
                background: rgba(0x0d1117),
                grid: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.083,
                }, // rgba(0xffffff15) = 21/255 ≈ 8.3%
                curve_boost: rgba(0x3fb950),
                curve_cut: rgba(0xf85149),
                fill_boost: Rgba {
                    r: 0.247,
                    g: 0.729,
                    b: 0.314,
                    a: 0.251,
                }, // rgba(0x3fb95040) ≈ 25%
                fill_cut: Rgba {
                    r: 0.973,
                    g: 0.318,
                    b: 0.290,
                    a: 0.251,
                }, // rgba(0xf8514940) ≈ 25%
                zero_line: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.251,
                }, // rgba(0xffffff40) ≈ 25%
            },
            spectrum_colors: SpectrumColors {
                background: rgba(0x0a0e27),
                bass: rgba(0x3fb950),
                mids: rgba(0xd29922),
                treble: rgba(0xf85149),
            },
            meter_colors: MeterColors {
                background: rgba(0x0d1117),
                normal: rgba(0x3fb950),
                warning: rgba(0xd29922),
                clip: rgba(0xf85149),
                peak: rgba(0xffffff),
                text: rgba(0x8b949e),
            },

            // Additional semantic colors
            peak_indicator: rgba(0xffffff),
            drag_over_highlight: Rgba {
                r: 0.345,
                g: 0.651,
                b: 1.0,
                a: 0.251,
            }, // rgba(0x58a6ff40) ≈ 25%
            drag_over_border: rgba(0x58a6ff),
            neutral_indicator: rgba(0x58a6ff),
            warning_background: Rgba {
                r: 0.824,
                g: 0.6,
                b: 0.133,
                a: 0.2,
            }, // rgba(0xd2992233) ≈ 20%
            knob_color: rgba(0xffffff),
            optimization_color: rgba(0xa371f7),
            grid_color: Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.083,
            }, // rgba(0xffffff15) = 21/255 ≈ 8.3%

            // Layout sizes
            separator_size: 20.0,

            // Font family
            font_family: ".SystemUI".into(),
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

            // Text on accent
            text_on_accent: rgba(0xffffff),
            text_on_accent_muted: rgba(0xffffffcc),

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

            // Plugin colors
            plugin_colors: PluginColorMap {
                eq: rgba(0x6abf69),
                gain: rgba(0x7dd07c),
                upmixer: rgba(0x9b7fd9),
                compressor: rgba(0xd96c6c),
                limiter: rgba(0xdb8f4f),
                gate: rgba(0xe0c062),
                loudness: rgba(0x6cb2d9),
                binaural: rgba(0xd96cb0),
                convolution: rgba(0x9b7fd9),
                monitor: rgba(0x6abf69),
                spectrum: rgba(0x9b7fd9),
                mute_solo: rgba(0x6cb2d9),
            },
            graph_colors: GraphLineColors {
                input: rgba(0x6cb2d9),
                target: rgba(0x6abf69),
                filter_response: rgba(0xe0c062),
                corrected: rgba(0x6abf69),
                error: rgba(0xd96c6c),
                deviation: rgba(0x9b7fd9),
                grid: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.083,
                }, // rgba(0xffffff15) = 21/255 ≈ 8.3%
                secondary_line: Rgba {
                    r: 0.475,
                    g: 0.604,
                    b: 0.451,
                    a: 1.0,
                }, // rgba(0x7a9a73)
                directivity_er: Rgba {
                    r: 0.851,
                    g: 0.424,
                    b: 0.690,
                    a: 1.0,
                }, // rgba(0xd96cb0)
                directivity_sp: Rgba {
                    r: 0.608,
                    g: 0.498,
                    b: 0.851,
                    a: 1.0,
                }, // rgba(0x9b7fd9)
            },
            band_colors: vec![
                rgba(0xd96c6c), // Red
                rgba(0xdb8f4f), // Orange
                rgba(0xe0c062), // Yellow
                rgba(0x6abf69), // Green
                rgba(0x6cb2d9), // Teal
                rgba(0x6abf69), // Blue
                rgba(0x9b7fd9), // Violet
                rgba(0xd96cb0), // Pink
                rgba(0x6cb2d9), // Indigo
                rgba(0x7dd07c), // Cyan
                rgba(0x7a9a73), // Gray
            ],
            eq_curve_colors: EQCurveColors {
                background: rgba(0x1a2418),
                grid: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.083,
                }, // rgba(0xffffff15) = 21/255 ≈ 8.3%
                curve_boost: rgba(0x6abf69),
                curve_cut: rgba(0xd96c6c),
                fill_boost: Rgba {
                    r: 0.416,
                    g: 0.749,
                    b: 0.408,
                    a: 0.251,
                }, // rgba(0x6abf6940) ≈ 25%
                fill_cut: Rgba {
                    r: 0.851,
                    g: 0.424,
                    b: 0.424,
                    a: 0.251,
                }, // rgba(0xd96c6c40) ≈ 25%
                zero_line: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.251,
                }, // rgba(0xffffff40) ≈ 25%
            },
            spectrum_colors: SpectrumColors {
                background: rgba(0x1a2418),
                bass: rgba(0x6abf69),
                mids: rgba(0xe0c062),
                treble: rgba(0xd96c6c),
            },
            meter_colors: MeterColors {
                background: rgba(0x1a2418),
                normal: rgba(0x6abf69),
                warning: rgba(0xe0c062),
                clip: rgba(0xd96c6c),
                peak: rgba(0xffffff),
                text: rgba(0xa8c4a2),
            },

            // Additional semantic colors
            peak_indicator: rgba(0xffffff),
            drag_over_highlight: Rgba {
                r: 0.416,
                g: 0.749,
                b: 0.408,
                a: 0.251,
            }, // rgba(0x6abf6940) ≈ 25%
            drag_over_border: rgba(0x6abf69),
            neutral_indicator: rgba(0x6cb2d9),
            warning_background: Rgba {
                r: 0.878,
                g: 0.753,
                b: 0.384,
                a: 0.2,
            }, // rgba(0xe0c06233) ≈ 20%
            knob_color: rgba(0xffffff),
            optimization_color: rgba(0x9b7fd9),
            grid_color: Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.083,
            }, // rgba(0xffffff15) = 21/255 ≈ 8.3%

            // Layout sizes
            separator_size: 20.0,

            // Font family
            font_family: ".SystemUI".into(),
        }
    }

    /// Black & White theme (monochrome high contrast)
    pub fn black_and_white() -> Self {
        Self {
            // Base colors
            background: rgba(0x000000),
            background_secondary: rgba(0x0a0a0a),
            background_tertiary: rgba(0x141414),
            surface: rgba(0x141414),
            surface_hover: rgba(0x222222),
            surface_selected: rgba(0x333333),

            // Text colors
            text_primary: rgba(0xffffff),
            text_secondary: rgba(0x888888),
            text_muted: rgba(0x555555),
            text_disabled: rgba(0x333333),

            // Border colors (white for high contrast)
            border: rgba(0xffffff),
            border_focused: rgba(0xffffff),

            // Accent colors (black background with white text for buttons)
            accent: rgba(0x000000),
            accent_hover: rgba(0x222222),
            accent_muted: rgba(0x333333),

            // Text on accent (white text on black background)
            text_on_accent: rgba(0xffffff),
            text_on_accent_muted: rgba(0xcccccc),

            // Semantic colors (grayscale for B&W theme)
            success: rgba(0xaaaaaa),
            warning: rgba(0x888888),
            error: rgba(0x666666),
            info: rgba(0x999999),

            // Level meter colors
            meter_normal: rgba(0x666666),
            meter_warning: rgba(0xaaaaaa),
            meter_clip: rgba(0xffffff),

            // Button colors
            button_mute_active: rgba(0x666666), // Dark Grey
            button_solo_active: rgba(0xaaaaaa), // Light Grey
            button_dim_active: rgba(0x333333),  // Grey

            // Playback bar
            progress_bar_bg: rgba(0x222222),
            progress_bar_fill: rgba(0xffffff),

            // Toast backgrounds
            toast_success_bg: rgba(0x111111),
            toast_error_bg: rgba(0x111111),
            toast_info_bg: rgba(0x111111),
            toast_warning_bg: rgba(0x111111),

            // Plugin colors (monochrome grayscale)
            plugin_colors: PluginColorMap {
                eq: rgba(0xcccccc),
                gain: rgba(0xaaaaaa),
                upmixer: rgba(0x888888),
                compressor: rgba(0x666666),
                limiter: rgba(0x777777),
                gate: rgba(0x999999),
                loudness: rgba(0xbbbbbb),
                binaural: rgba(0x808080),
                convolution: rgba(0x909090),
                monitor: rgba(0xababab),
                spectrum: rgba(0x8a8a8a),
                mute_solo: rgba(0x999999),
            },
            graph_colors: GraphLineColors {
                input: rgba(0xcccccc),
                target: rgba(0x999999),
                filter_response: rgba(0xaaaaaa),
                corrected: rgba(0xbbbbbb),
                error: rgba(0x666666),
                deviation: rgba(0x888888),
                grid: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.083,
                }, // rgba(0xffffff15) = 21/255 ≈ 8.3%
                secondary_line: Rgba {
                    r: 0.533,
                    g: 0.533,
                    b: 0.533,
                    a: 1.0,
                }, // rgba(0x888888)
                directivity_er: Rgba {
                    r: 0.667,
                    g: 0.667,
                    b: 0.667,
                    a: 1.0,
                }, // rgba(0xaaaaaa)
                directivity_sp: Rgba {
                    r: 0.733,
                    g: 0.733,
                    b: 0.733,
                    a: 1.0,
                }, // rgba(0xbbbbbb)
            },
            band_colors: vec![
                rgba(0x333333), // Dark gray
                rgba(0x444444), //
                rgba(0x555555), //
                rgba(0x666666), //
                rgba(0x777777), //
                rgba(0x888888), // Medium gray
                rgba(0x999999), //
                rgba(0xaaaaaa), //
                rgba(0xbbbbbb), //
                rgba(0xcccccc), //
                rgba(0xdddddd), // Light gray
            ],
            eq_curve_colors: EQCurveColors {
                background: rgba(0x000000),
                grid: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.063,
                }, // rgba(0xffffff10) = 16/255 ≈ 6.3%
                curve_boost: rgba(0xbbbbbb),
                curve_cut: rgba(0x666666),
                fill_boost: Rgba {
                    r: 0.733,
                    g: 0.733,
                    b: 0.733,
                    a: 0.188,
                }, // rgba(0xbbbbbb30) ≈ 19%
                fill_cut: Rgba {
                    r: 0.4,
                    g: 0.4,
                    b: 0.4,
                    a: 0.188,
                }, // rgba(0x66666630) ≈ 19%
                zero_line: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.188,
                }, // rgba(0xffffff30) ≈ 19%
            },
            spectrum_colors: SpectrumColors {
                background: rgba(0x000000),
                bass: rgba(0xbbbbbb),
                mids: rgba(0x888888),
                treble: rgba(0x444444),
            },
            meter_colors: MeterColors {
                background: rgba(0x000000),
                normal: rgba(0x888888),
                warning: rgba(0xaaaaaa),
                clip: rgba(0xffffff),
                peak: rgba(0xffffff),
                text: rgba(0x888888),
            },

            // Additional semantic colors
            peak_indicator: rgba(0xffffff),
            drag_over_highlight: Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.251,
            }, // rgba(0xffffff40) ≈ 25%
            drag_over_border: rgba(0xffffff),
            neutral_indicator: rgba(0xcccccc),
            warning_background: Rgba {
                r: 0.667,
                g: 0.667,
                b: 0.667,
                a: 0.2,
            }, // rgba(0xaaaaaa33) ≈ 20%
            knob_color: rgba(0xffffff),
            optimization_color: rgba(0xbbbbbb),
            grid_color: Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.083,
            }, // rgba(0xffffff15) = 21/255 ≈ 8.3%

            // Layout sizes
            separator_size: 20.0,

            // Font family
            font_family: "DM Serif Display".into(),
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
            // Use text_on_accent for primary buttons (accent background)
            text_primary: self.text_on_accent,
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
            unchecked_bg: self.border,
            knob: self.text_primary,
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
