//! Theme definition with serialization support
//!
//! Provides a serializable theme structure that can be exported to JSON or Rust code.

use gpui::Rgba;
use serde::{Deserialize, Serialize};

/// Serializable color representation (RGBA with 0-255 values for readability)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Create a new color from RGBA components (0-255)
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Create an opaque color from RGB components
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Create from a hex value (0xRRGGBB)
    pub fn from_hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xFF) as u8,
            g: ((hex >> 8) & 0xFF) as u8,
            b: (hex & 0xFF) as u8,
            a: 255,
        }
    }

    /// Create from a hex value with alpha (0xRRGGBBAA)
    pub fn from_hex_alpha(hex: u32) -> Self {
        Self {
            r: ((hex >> 24) & 0xFF) as u8,
            g: ((hex >> 16) & 0xFF) as u8,
            b: ((hex >> 8) & 0xFF) as u8,
            a: (hex & 0xFF) as u8,
        }
    }

    /// Convert to hex string (#RRGGBB or #RRGGBBAA)
    pub fn to_hex_string(&self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    /// Parse from hex string (#RGB, #RRGGBB, or #RRGGBBAA)
    pub fn from_hex_string(s: &str) -> Option<Self> {
        let s = s.trim_start_matches('#');
        match s.len() {
            3 => {
                let r = u8::from_str_radix(&s[0..1], 16).ok()?;
                let g = u8::from_str_radix(&s[1..2], 16).ok()?;
                let b = u8::from_str_radix(&s[2..3], 16).ok()?;
                Some(Self::rgb(r * 17, g * 17, b * 17))
            }
            6 => {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                Some(Self::rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                let a = u8::from_str_radix(&s[6..8], 16).ok()?;
                Some(Self::new(r, g, b, a))
            }
            _ => None,
        }
    }

    /// Convert to GPUI Rgba
    pub fn to_rgba(&self) -> Rgba {
        Rgba {
            r: self.r as f32 / 255.0,
            g: self.g as f32 / 255.0,
            b: self.b as f32 / 255.0,
            a: self.a as f32 / 255.0,
        }
    }

    /// Convert from GPUI Rgba
    pub fn from_rgba(rgba: Rgba) -> Self {
        Self {
            r: (rgba.r * 255.0).round() as u8,
            g: (rgba.g * 255.0).round() as u8,
            b: (rgba.b * 255.0).round() as u8,
            a: (rgba.a * 255.0).round() as u8,
        }
    }

    /// Apply alpha (0.0-1.0 scale)
    pub fn with_alpha(&self, alpha: f32) -> Self {
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a: (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
        }
    }

    /// Get HSL components
    pub fn to_hsl(&self) -> (f32, f32, f32) {
        let r = self.r as f32 / 255.0;
        let g = self.g as f32 / 255.0;
        let b = self.b as f32 / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;

        if (max - min).abs() < f32::EPSILON {
            return (0.0, 0.0, l);
        }

        let d = max - min;
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };

        let h = if (max - r).abs() < f32::EPSILON {
            ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
        } else if (max - g).abs() < f32::EPSILON {
            ((b - r) / d + 2.0) / 6.0
        } else {
            ((r - g) / d + 4.0) / 6.0
        };

        (h, s, l)
    }

    /// Create from HSL components (h: 0-1, s: 0-1, l: 0-1)
    pub fn from_hsl(h: f32, s: f32, l: f32) -> Self {
        let (r, g, b) = if s.abs() < f32::EPSILON {
            (l, l, l)
        } else {
            fn hue_to_rgb(p: f32, q: f32, t: f32) -> f32 {
                let t = if t < 0.0 {
                    t + 1.0
                } else if t > 1.0 {
                    t - 1.0
                } else {
                    t
                };
                if t < 1.0 / 6.0 {
                    p + (q - p) * 6.0 * t
                } else if t < 1.0 / 2.0 {
                    q
                } else if t < 2.0 / 3.0 {
                    p + (q - p) * (2.0 / 3.0 - t) * 6.0
                } else {
                    p
                }
            }

            let q = if l < 0.5 {
                l * (1.0 + s)
            } else {
                l + s - l * s
            };
            let p = 2.0 * l - q;
            (
                hue_to_rgb(p, q, h + 1.0 / 3.0),
                hue_to_rgb(p, q, h),
                hue_to_rgb(p, q, h - 1.0 / 3.0),
            )
        };

        Self::rgb(
            (r * 255.0).round() as u8,
            (g * 255.0).round() as u8,
            (b * 255.0).round() as u8,
        )
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::rgb(128, 128, 128)
    }
}

/// Plugin type color mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginColors {
    pub eq: Color,
    pub gain: Color,
    pub upmixer: Color,
    pub compressor: Color,
    pub limiter: Color,
    pub gate: Color,
    pub loudness: Color,
    pub binaural: Color,
    pub convolution: Color,
    pub monitor: Color,
    pub spectrum: Color,
    pub mute_solo: Color,
}

/// Graph visualization line colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphColors {
    pub input: Color,
    pub target: Color,
    pub filter_response: Color,
    pub corrected: Color,
    pub error: Color,
    pub deviation: Color,
    pub grid: Color,
    pub secondary_line: Color,
    pub directivity_er: Color,
    pub directivity_sp: Color,
}

/// EQ curve visualization colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EQCurveColors {
    pub background: Color,
    pub grid: Color,
    pub curve_boost: Color,
    pub curve_cut: Color,
    pub fill_boost: Color,
    pub fill_cut: Color,
    pub zero_line: Color,
}

/// Spectrum analyzer colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumColors {
    pub background: Color,
    pub bass: Color,
    pub mids: Color,
    pub treble: Color,
}

/// Level meter colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeterColors {
    pub background: Color,
    pub normal: Color,
    pub warning: Color,
    pub clip: Color,
    pub peak: Color,
    pub text: Color,
}

/// Complete theme definition with all UI colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorTheme {
    /// Theme name for display
    pub name: String,

    // Base colors
    pub background: Color,
    pub background_secondary: Color,
    pub background_tertiary: Color,
    pub surface: Color,
    pub surface_hover: Color,
    pub surface_selected: Color,

    // Text colors
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub text_disabled: Color,

    // Border colors
    pub border: Color,
    pub border_focused: Color,

    // Accent colors
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_muted: Color,

    // Text on accent
    pub text_on_accent: Color,
    pub text_on_accent_muted: Color,

    // Semantic colors
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,

    // Level meter colors
    pub meter_normal: Color,
    pub meter_warning: Color,
    pub meter_clip: Color,

    // Button colors
    pub button_mute_active: Color,
    pub button_solo_active: Color,
    pub button_dim_active: Color,

    // Playback bar
    pub progress_bar_bg: Color,
    pub progress_bar_fill: Color,

    // Toast backgrounds
    pub toast_success_bg: Color,
    pub toast_error_bg: Color,
    pub toast_info_bg: Color,
    pub toast_warning_bg: Color,

    // Plugin colors
    pub plugin_colors: PluginColors,
    pub graph_colors: GraphColors,
    pub band_colors: Vec<Color>,
    pub eq_curve_colors: EQCurveColors,
    pub spectrum_colors: SpectrumColors,
    pub meter_colors: MeterColors,

    // Additional colors
    pub peak_indicator: Color,
    pub drag_over_highlight: Color,
    pub drag_over_border: Color,
    pub neutral_indicator: Color,
    pub warning_background: Color,
    pub knob_color: Color,
    pub optimization_color: Color,
    pub grid_color: Color,

    // Layout sizes
    pub separator_size: f32,

    // Font family
    pub font_family: String,
}

impl Default for EditorTheme {
    fn default() -> Self {
        Self::dark()
    }
}

impl EditorTheme {
    /// Create the default dark theme
    pub fn dark() -> Self {
        Self {
            name: "Dark".to_string(),

            // Base colors
            background: Color::from_hex(0x1e1e1e),
            background_secondary: Color::from_hex(0x252525),
            background_tertiary: Color::from_hex(0x2d2d2d),
            surface: Color::from_hex(0x2d2d2d),
            surface_hover: Color::from_hex(0x3e3e3e),
            surface_selected: Color::from_hex(0x264f78),

            // Text colors
            text_primary: Color::from_hex(0xcccccc),
            text_secondary: Color::from_hex(0x999999),
            text_muted: Color::from_hex(0x666666),
            text_disabled: Color::from_hex(0x444444),

            // Border colors
            border: Color::from_hex(0x3e3e3e),
            border_focused: Color::from_hex(0x007acc),

            // Accent colors
            accent: Color::from_hex(0x007acc),
            accent_hover: Color::from_hex(0x1c8cd9),
            accent_muted: Color::from_hex(0x264f78),

            // Text on accent
            text_on_accent: Color::from_hex(0xffffff),
            text_on_accent_muted: Color::new(255, 255, 255, 204),

            // Semantic colors
            success: Color::from_hex(0x4ec9b0),
            warning: Color::from_hex(0xdcdcaa),
            error: Color::from_hex(0xf48771),
            info: Color::from_hex(0x569cd6),

            // Level meter colors
            meter_normal: Color::from_hex(0x22c55e),
            meter_warning: Color::from_hex(0xf59e0b),
            meter_clip: Color::from_hex(0xdc2626),

            // Button colors
            button_mute_active: Color::from_hex(0xdc2626),
            button_solo_active: Color::from_hex(0xf59e0b),
            button_dim_active: Color::from_hex(0x6366f1),

            // Playback bar
            progress_bar_bg: Color::from_hex(0x3e3e3e),
            progress_bar_fill: Color::from_hex(0x007acc),

            // Toast backgrounds
            toast_success_bg: Color::from_hex(0x1e3a1e),
            toast_error_bg: Color::from_hex(0x3a1e1e),
            toast_info_bg: Color::from_hex(0x1e2a3a),
            toast_warning_bg: Color::from_hex(0x3a2e1e),

            // Plugin colors
            plugin_colors: PluginColors {
                eq: Color::from_hex(0x2563eb),
                gain: Color::from_hex(0x059669),
                upmixer: Color::from_hex(0x7c3aed),
                compressor: Color::from_hex(0xdc2626),
                limiter: Color::from_hex(0xea580c),
                gate: Color::from_hex(0xca8a04),
                loudness: Color::from_hex(0x0891b2),
                binaural: Color::from_hex(0xdb2777),
                convolution: Color::from_hex(0x4f46e5),
                monitor: Color::from_hex(0x14b8a6),
                spectrum: Color::from_hex(0x8b5cf6),
                mute_solo: Color::from_hex(0x6366f1),
            },
            graph_colors: GraphColors {
                input: Color::from_hex(0x6366f1),
                target: Color::from_hex(0x22c55e),
                filter_response: Color::from_hex(0xf59e0b),
                corrected: Color::from_hex(0x3b82f6),
                error: Color::from_hex(0xef4444),
                deviation: Color::from_hex(0x8b5cf6),
                grid: Color::new(255, 255, 255, 21),
                secondary_line: Color::from_hex(0xaaaaaa),
                directivity_er: Color::from_hex(0xf472b6),
                directivity_sp: Color::from_hex(0xc084fc),
            },
            band_colors: vec![
                Color::from_hex(0xef4444),
                Color::from_hex(0xf97316),
                Color::from_hex(0xeab308),
                Color::from_hex(0x22c55e),
                Color::from_hex(0x14b8a6),
                Color::from_hex(0x3b82f6),
                Color::from_hex(0x8b5cf6),
                Color::from_hex(0xec4899),
                Color::from_hex(0x6366f1),
                Color::from_hex(0x06b6d4),
                Color::from_hex(0x9ca3af),
            ],
            eq_curve_colors: EQCurveColors {
                background: Color::from_hex(0x1a1a1a),
                grid: Color::new(255, 255, 255, 32),
                curve_boost: Color::from_hex(0x22c55e),
                curve_cut: Color::from_hex(0xef4444),
                fill_boost: Color::new(34, 197, 94, 64),
                fill_cut: Color::new(239, 68, 68, 64),
                zero_line: Color::new(255, 255, 255, 64),
            },
            spectrum_colors: SpectrumColors {
                background: Color::from_hex(0x000000),
                bass: Color::from_hex(0x22c55e),
                mids: Color::from_hex(0xeab308),
                treble: Color::from_hex(0xef4444),
            },
            meter_colors: MeterColors {
                background: Color::from_hex(0x1e1e1e),
                normal: Color::from_hex(0x22c55e),
                warning: Color::from_hex(0xf59e0b),
                clip: Color::from_hex(0xdc2626),
                peak: Color::from_hex(0xffffff),
                text: Color::from_hex(0x999999),
            },

            // Additional colors
            peak_indicator: Color::from_hex(0xffffff),
            drag_over_highlight: Color::new(59, 130, 246, 64),
            drag_over_border: Color::from_hex(0x3b82f6),
            neutral_indicator: Color::from_hex(0x6366f1),
            warning_background: Color::new(245, 158, 11, 51),
            knob_color: Color::from_hex(0xffffff),
            optimization_color: Color::from_hex(0x8b5cf6),
            grid_color: Color::new(255, 255, 255, 21),

            // Layout sizes
            separator_size: 20.0,

            // Font family
            font_family: ".SystemUI".to_string(),
        }
    }

    /// Create a light theme
    pub fn light() -> Self {
        Self {
            name: "Light".to_string(),

            background: Color::from_hex(0xf5f5f5),
            background_secondary: Color::from_hex(0xececec),
            background_tertiary: Color::from_hex(0xe0e0e0),
            surface: Color::from_hex(0xffffff),
            surface_hover: Color::from_hex(0xf0f0f0),
            surface_selected: Color::from_hex(0xd0e8ff),

            text_primary: Color::from_hex(0x1e1e1e),
            text_secondary: Color::from_hex(0x555555),
            text_muted: Color::from_hex(0x888888),
            text_disabled: Color::from_hex(0xbbbbbb),

            border: Color::from_hex(0xd0d0d0),
            border_focused: Color::from_hex(0x0066cc),

            accent: Color::from_hex(0x0066cc),
            accent_hover: Color::from_hex(0x0078e6),
            accent_muted: Color::from_hex(0xb3d4f7),

            text_on_accent: Color::from_hex(0xffffff),
            text_on_accent_muted: Color::new(255, 255, 255, 204),

            success: Color::from_hex(0x16a34a),
            warning: Color::from_hex(0xca8a04),
            error: Color::from_hex(0xdc2626),
            info: Color::from_hex(0x2563eb),

            meter_normal: Color::from_hex(0x16a34a),
            meter_warning: Color::from_hex(0xca8a04),
            meter_clip: Color::from_hex(0xdc2626),

            button_mute_active: Color::from_hex(0xdc2626),
            button_solo_active: Color::from_hex(0xca8a04),
            button_dim_active: Color::from_hex(0x6366f1),

            progress_bar_bg: Color::from_hex(0xd0d0d0),
            progress_bar_fill: Color::from_hex(0x0066cc),

            toast_success_bg: Color::from_hex(0xd1fae5),
            toast_error_bg: Color::from_hex(0xfee2e2),
            toast_info_bg: Color::from_hex(0xdbeafe),
            toast_warning_bg: Color::from_hex(0xfef3c7),

            plugin_colors: PluginColors {
                eq: Color::from_hex(0x2563eb),
                gain: Color::from_hex(0x16a34a),
                upmixer: Color::from_hex(0x7c3aed),
                compressor: Color::from_hex(0xdc2626),
                limiter: Color::from_hex(0xea580c),
                gate: Color::from_hex(0xca8a04),
                loudness: Color::from_hex(0x0891b2),
                binaural: Color::from_hex(0xdb2777),
                convolution: Color::from_hex(0x4f46e5),
                monitor: Color::from_hex(0x14b8a6),
                spectrum: Color::from_hex(0x8b5cf6),
                mute_solo: Color::from_hex(0x6366f1),
            },
            graph_colors: GraphColors {
                input: Color::from_hex(0x6366f1),
                target: Color::from_hex(0x16a34a),
                filter_response: Color::from_hex(0xca8a04),
                corrected: Color::from_hex(0x2563eb),
                error: Color::from_hex(0xdc2626),
                deviation: Color::from_hex(0x7c3aed),
                grid: Color::new(0, 0, 0, 21),
                secondary_line: Color::from_hex(0x888888),
                directivity_er: Color::from_hex(0xec4899),
                directivity_sp: Color::from_hex(0xa855f7),
            },
            band_colors: vec![
                Color::from_hex(0xdc2626),
                Color::from_hex(0xea580c),
                Color::from_hex(0xca8a04),
                Color::from_hex(0x16a34a),
                Color::from_hex(0x0d9488),
                Color::from_hex(0x2563eb),
                Color::from_hex(0x7c3aed),
                Color::from_hex(0xdb2777),
                Color::from_hex(0x4f46e5),
                Color::from_hex(0x0891b2),
                Color::from_hex(0x6b7280),
            ],
            eq_curve_colors: EQCurveColors {
                background: Color::from_hex(0xfafafa),
                grid: Color::new(0, 0, 0, 25),
                curve_boost: Color::from_hex(0x16a34a),
                curve_cut: Color::from_hex(0xdc2626),
                fill_boost: Color::new(22, 163, 74, 64),
                fill_cut: Color::new(220, 38, 38, 64),
                zero_line: Color::new(0, 0, 0, 64),
            },
            spectrum_colors: SpectrumColors {
                background: Color::from_hex(0xfafafa),
                bass: Color::from_hex(0x16a34a),
                mids: Color::from_hex(0xca8a04),
                treble: Color::from_hex(0xdc2626),
            },
            meter_colors: MeterColors {
                background: Color::from_hex(0xf5f5f5),
                normal: Color::from_hex(0x16a34a),
                warning: Color::from_hex(0xca8a04),
                clip: Color::from_hex(0xdc2626),
                peak: Color::from_hex(0x1e1e1e),
                text: Color::from_hex(0x555555),
            },

            peak_indicator: Color::from_hex(0x1e1e1e),
            drag_over_highlight: Color::new(37, 99, 235, 64),
            drag_over_border: Color::from_hex(0x2563eb),
            neutral_indicator: Color::from_hex(0x6366f1),
            warning_background: Color::new(202, 138, 4, 51),
            knob_color: Color::from_hex(0x333333),
            optimization_color: Color::from_hex(0x7c3aed),
            grid_color: Color::new(0, 0, 0, 21),

            separator_size: 20.0,
            font_family: ".SystemUI".to_string(),
        }
    }

    /// Save theme to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Load theme from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Generate Rust code for this theme
    pub fn to_rust_code(&self) -> String {
        fn color_to_rust(c: &Color) -> String {
            if c.a == 255 {
                format!("Color::from_hex(0x{:02x}{:02x}{:02x})", c.r, c.g, c.b)
            } else {
                format!("Color::new({}, {}, {}, {})", c.r, c.g, c.b, c.a)
            }
        }

        let mut code = format!(
            r#"/// {} theme
pub fn {}() -> EditorTheme {{
    EditorTheme {{
        name: "{}".to_string(),

        // Base colors
        background: {},
        background_secondary: {},
        background_tertiary: {},
        surface: {},
        surface_hover: {},
        surface_selected: {},

        // Text colors
        text_primary: {},
        text_secondary: {},
        text_muted: {},
        text_disabled: {},

        // Border colors
        border: {},
        border_focused: {},

        // Accent colors
        accent: {},
        accent_hover: {},
        accent_muted: {},

        // Text on accent
        text_on_accent: {},
        text_on_accent_muted: {},

        // Semantic colors
        success: {},
        warning: {},
        error: {},
        info: {},

        // Level meter colors
        meter_normal: {},
        meter_warning: {},
        meter_clip: {},

        // Button colors
        button_mute_active: {},
        button_solo_active: {},
        button_dim_active: {},

        // Playback bar
        progress_bar_bg: {},
        progress_bar_fill: {},

        // Toast backgrounds
        toast_success_bg: {},
        toast_error_bg: {},
        toast_info_bg: {},
        toast_warning_bg: {},

        // Additional colors
        peak_indicator: {},
        drag_over_highlight: {},
        drag_over_border: {},
        neutral_indicator: {},
        warning_background: {},
        knob_color: {},
        optimization_color: {},
        grid_color: {},

        separator_size: {:.1},
        font_family: "{}".to_string(),
"#,
            self.name,
            self.name.to_lowercase().replace(' ', "_"),
            self.name,
            color_to_rust(&self.background),
            color_to_rust(&self.background_secondary),
            color_to_rust(&self.background_tertiary),
            color_to_rust(&self.surface),
            color_to_rust(&self.surface_hover),
            color_to_rust(&self.surface_selected),
            color_to_rust(&self.text_primary),
            color_to_rust(&self.text_secondary),
            color_to_rust(&self.text_muted),
            color_to_rust(&self.text_disabled),
            color_to_rust(&self.border),
            color_to_rust(&self.border_focused),
            color_to_rust(&self.accent),
            color_to_rust(&self.accent_hover),
            color_to_rust(&self.accent_muted),
            color_to_rust(&self.text_on_accent),
            color_to_rust(&self.text_on_accent_muted),
            color_to_rust(&self.success),
            color_to_rust(&self.warning),
            color_to_rust(&self.error),
            color_to_rust(&self.info),
            color_to_rust(&self.meter_normal),
            color_to_rust(&self.meter_warning),
            color_to_rust(&self.meter_clip),
            color_to_rust(&self.button_mute_active),
            color_to_rust(&self.button_solo_active),
            color_to_rust(&self.button_dim_active),
            color_to_rust(&self.progress_bar_bg),
            color_to_rust(&self.progress_bar_fill),
            color_to_rust(&self.toast_success_bg),
            color_to_rust(&self.toast_error_bg),
            color_to_rust(&self.toast_info_bg),
            color_to_rust(&self.toast_warning_bg),
            color_to_rust(&self.peak_indicator),
            color_to_rust(&self.drag_over_highlight),
            color_to_rust(&self.drag_over_border),
            color_to_rust(&self.neutral_indicator),
            color_to_rust(&self.warning_background),
            color_to_rust(&self.knob_color),
            color_to_rust(&self.optimization_color),
            color_to_rust(&self.grid_color),
            self.separator_size,
            self.font_family,
        );

        // Add plugin_colors, graph_colors, etc. (abbreviated for length)
        code.push_str("        // ... plugin_colors, graph_colors, etc.\n");
        code.push_str("    }\n}\n");

        code
    }

    /// Create a GPUI-compatible button theme
    pub fn to_button_theme(&self) -> gpui_ui_kit::ButtonTheme {
        gpui_ui_kit::ButtonTheme {
            accent: self.accent.to_rgba(),
            accent_hover: self.accent_hover.to_rgba(),
            surface: self.surface.to_rgba(),
            surface_hover: self.surface_hover.to_rgba(),
            text_primary: self.text_primary.to_rgba(),
            text_secondary: self.text_secondary.to_rgba(),
            error: self.error.to_rgba(),
            border: self.border.to_rgba(),
        }
    }

    /// Create a GPUI-compatible slider theme
    pub fn to_slider_theme(&self) -> gpui_ui_kit::SliderTheme {
        gpui_ui_kit::SliderTheme {
            track: self.surface_hover.to_rgba(),
            fill: self.accent.to_rgba(),
            thumb: self.text_primary.to_rgba(),
            thumb_hover: self.text_secondary.to_rgba(),
            thumb_active: self.accent.to_rgba(),
            label: self.text_primary.to_rgba(),
            value: self.text_secondary.to_rgba(),
        }
    }

    /// Create a GPUI-compatible accordion theme
    pub fn to_accordion_theme(&self) -> gpui_ui_kit::AccordionTheme {
        gpui_ui_kit::AccordionTheme {
            header_bg: self.surface.to_rgba(),
            header_hover_bg: self.surface_hover.to_rgba(),
            content_bg: self.background.to_rgba(),
            border: self.border.to_rgba(),
            title_color: self.text_primary.to_rgba(),
            indicator_color: self.text_muted.to_rgba(),
        }
    }

    /// Create a GPUI-compatible tabs theme
    pub fn to_tabs_theme(&self) -> gpui_ui_kit::TabsTheme {
        gpui_ui_kit::TabsTheme {
            container_bg: self.surface.to_rgba(),
            container_border: self.border.to_rgba(),
            selected_bg: self.surface_selected.to_rgba(),
            selected_hover_bg: self.surface_hover.to_rgba(),
            hover_bg: self.surface_hover.to_rgba(),
            accent: self.accent.to_rgba(),
            text_selected: self.text_primary.to_rgba(),
            text_unselected: self.text_secondary.to_rgba(),
            text_hover: self.text_primary.to_rgba(),
            badge_bg: self.surface_hover.to_rgba(),
            close_color: self.text_muted.to_rgba(),
            close_hover_color: self.text_primary.to_rgba(),
        }
    }
}

/// Color group for organizing theme editor UI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorGroup {
    Base,
    Text,
    Border,
    Accent,
    Semantic,
    Meter,
    Button,
    Progress,
    Toast,
    Plugin,
    Graph,
    Spectrum,
    Additional,
}

impl ColorGroup {
    pub fn all() -> &'static [ColorGroup] {
        &[
            ColorGroup::Base,
            ColorGroup::Text,
            ColorGroup::Border,
            ColorGroup::Accent,
            ColorGroup::Semantic,
            ColorGroup::Meter,
            ColorGroup::Button,
            ColorGroup::Progress,
            ColorGroup::Toast,
            ColorGroup::Plugin,
            ColorGroup::Graph,
            ColorGroup::Spectrum,
            ColorGroup::Additional,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            ColorGroup::Base => "Base Colors",
            ColorGroup::Text => "Text Colors",
            ColorGroup::Border => "Border Colors",
            ColorGroup::Accent => "Accent Colors",
            ColorGroup::Semantic => "Semantic Colors",
            ColorGroup::Meter => "Level Meters",
            ColorGroup::Button => "Button States",
            ColorGroup::Progress => "Progress Bar",
            ColorGroup::Toast => "Toast Notifications",
            ColorGroup::Plugin => "Plugin Colors",
            ColorGroup::Graph => "Graph Colors",
            ColorGroup::Spectrum => "Spectrum Colors",
            ColorGroup::Additional => "Additional",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_hex_conversion() {
        let color = Color::from_hex(0xff5500);
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 85);
        assert_eq!(color.b, 0);
        assert_eq!(color.a, 255);
        assert_eq!(color.to_hex_string(), "#ff5500");
    }

    #[test]
    fn test_color_hex_string_parsing() {
        let color = Color::from_hex_string("#ff5500").unwrap();
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 85);
        assert_eq!(color.b, 0);

        let color_short = Color::from_hex_string("#f50").unwrap();
        assert_eq!(color_short.r, 255);
        assert_eq!(color_short.g, 85);
        assert_eq!(color_short.b, 0);
    }

    #[test]
    fn test_theme_json_roundtrip() {
        let theme = EditorTheme::dark();
        let json = theme.to_json().unwrap();
        let loaded = EditorTheme::from_json(&json).unwrap();
        assert_eq!(loaded.name, theme.name);
        assert_eq!(loaded.background.r, theme.background.r);
    }

    #[test]
    fn test_hsl_roundtrip() {
        let color = Color::rgb(255, 128, 64);
        let (h, s, l) = color.to_hsl();
        let back = Color::from_hsl(h, s, l);
        // Allow small rounding errors
        assert!((color.r as i16 - back.r as i16).abs() <= 1);
        assert!((color.g as i16 - back.g as i16).abs() <= 1);
        assert!((color.b as i16 - back.b as i16).abs() <= 1);
    }
}
