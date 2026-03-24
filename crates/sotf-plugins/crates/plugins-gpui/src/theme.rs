//! Plugin UI theme — a self-contained color palette for plugin rendering.
//!
//! This is a subset of app-gpui's full `Theme`, containing only the colors
//! that plugin UIs actually reference. In app-gpui, it's created via
//! `PluginViewTheme::from(&theme)`. In AU, use `PluginViewTheme::default_dark()`.

use gpui::Rgba;

/// Colors for the EQ frequency response chart.
#[derive(Debug, Clone)]
pub struct EqCurveColors {
    pub background: Rgba,
    pub grid: Rgba,
    pub curve_boost: Rgba,
    pub curve_cut: Rgba,
    pub fill_boost: Rgba,
    pub fill_cut: Rgba,
    pub zero_line: Rgba,
}

/// Colors for the spectrum analyzer.
#[derive(Debug, Clone)]
pub struct SpectrumColors {
    pub background: Rgba,
    pub bass: Rgba,
    pub mids: Rgba,
    pub treble: Rgba,
}

/// Colors for level meters.
#[derive(Debug, Clone)]
pub struct MeterColors {
    pub background: Rgba,
    pub normal: Rgba,
    pub warning: Rgba,
    pub clip: Rgba,
    pub peak: Rgba,
    pub text: Rgba,
}

/// Colors for graph lines (transfer curves, etc.).
#[derive(Debug, Clone)]
pub struct GraphColors {
    pub grid: Rgba,
    pub input: Rgba,
    pub target: Rgba,
    pub filter_response: Rgba,
}

/// Complete theme for plugin UI rendering.
///
/// Contains all color fields referenced by plugin render functions in
/// `common.rs`, `level_meters.rs`, `ui_eq.rs`, and other plugin UIs.
#[derive(Debug, Clone)]
pub struct PluginViewTheme {
    // ── Base colors ───────────────────────────────────────────────────────
    pub background: Rgba,
    pub background_secondary: Rgba,
    pub surface: Rgba,
    pub surface_hover: Rgba,

    // ── Text ──────────────────────────────────────────────────────────────
    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,
    pub text_on_accent: Rgba,

    // ── Accent ────────────────────────────────────────────────────────────
    pub accent: Rgba,
    pub accent_muted: Rgba,

    // ── Borders ───────────────────────────────────────────────────────────
    pub border: Rgba,

    // ── Semantic ──────────────────────────────────────────────────────────
    pub success: Rgba,
    pub warning: Rgba,
    pub error: Rgba,
    pub info: Rgba,

    // ── Meters ────────────────────────────────────────────────────────────
    pub meter_normal: Rgba,
    pub meter_warning: Rgba,
    pub meter_clip: Rgba,

    // ── Plugin-specific color groups ──────────────────────────────────────
    pub eq_curve_colors: EqCurveColors,
    pub spectrum_colors: SpectrumColors,
    pub meter_colors: MeterColors,
    pub graph_colors: GraphColors,

    /// Per-band colors for EQ / multiband plugins (10 colors, cycling).
    pub band_colors: [Rgba; 10],

    /// Knob fill color.
    pub knob_color: Rgba,

    /// Peak indicator color.
    pub peak_indicator: Rgba,
}

impl PluginViewTheme {
    /// Convert to `gpui_ui_kit::PotentiometerTheme` for knob rendering.
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

    /// Convert to `gpui_ui_kit::ToggleTheme` for toggle rendering.
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
            text_primary: self.text_primary,
            surface_hover: self.surface_hover,
            background: self.background,
        }
    }

    /// Convert to `gpui_ui_kit::VerticalSliderTheme` for slider rendering.
    pub fn to_vertical_slider_theme(&self) -> gpui_ui_kit::VerticalSliderTheme {
        gpui_ui_kit::VerticalSliderTheme {
            surface: self.surface,
            surface_hover: self.surface_hover,
            track_bg: self.background,
            accent: self.accent,
            accent_muted: self.accent_muted,
            border: self.border,
            text_secondary: self.text_secondary,
            text_primary: self.text_primary,
            text_muted: self.text_muted,
            text_on_accent: self.text_on_accent,
            background_secondary: self.background_secondary,
            peak_marker: self.peak_indicator,
        }
    }

    /// Create a helper to get Rgba with modified opacity (for hover/muted states).
    pub fn with_opacity(color: Rgba, alpha: f32) -> Rgba {
        Rgba {
            r: color.r,
            g: color.g,
            b: color.b,
            a: alpha,
        }
    }

    /// Default dark theme for standalone AU plugin views.
    pub fn default_dark() -> Self {
        Self {
            background: gpui::rgb(0x121218),
            background_secondary: gpui::rgb(0x1a1a24),
            surface: gpui::rgb(0x22222e),
            surface_hover: gpui::rgb(0x2a2a38),
            text_primary: gpui::rgb(0xf0f0f4),
            text_secondary: gpui::rgb(0xa0a0b0),
            text_muted: gpui::rgb(0x606070),
            text_on_accent: gpui::rgb(0xffffff),
            accent: gpui::rgb(0x3b82f6),
            accent_muted: gpui::rgba(0x3b82f640),
            border: gpui::rgb(0x333340),
            success: gpui::rgb(0x22c55e),
            warning: gpui::rgb(0xeab308),
            error: gpui::rgb(0xef4444),
            info: gpui::rgb(0x06b6d4),
            meter_normal: gpui::rgb(0x22c55e),
            meter_warning: gpui::rgb(0xeab308),
            meter_clip: gpui::rgb(0xef4444),
            eq_curve_colors: EqCurveColors {
                background: gpui::rgb(0x0f0f18),
                grid: gpui::rgb(0x2a2a3a),
                curve_boost: gpui::rgb(0x3b82f6),
                curve_cut: gpui::rgb(0xef4444),
                fill_boost: gpui::rgba(0x3b82f620),
                fill_cut: gpui::rgba(0xef444420),
                zero_line: gpui::rgb(0x404050),
            },
            spectrum_colors: SpectrumColors {
                background: gpui::rgb(0x0f0f18),
                bass: gpui::rgb(0xef4444),
                mids: gpui::rgb(0x22c55e),
                treble: gpui::rgb(0x3b82f6),
            },
            meter_colors: MeterColors {
                background: gpui::rgb(0x1a1a24),
                normal: gpui::rgb(0x22c55e),
                warning: gpui::rgb(0xeab308),
                clip: gpui::rgb(0xef4444),
                peak: gpui::rgb(0xffffff),
                text: gpui::rgb(0xa0a0b0),
            },
            graph_colors: GraphColors {
                grid: gpui::rgb(0x333340),
                input: gpui::rgb(0x3b82f6),
                target: gpui::rgb(0x22c55e),
                filter_response: gpui::rgb(0xeab308),
            },
            band_colors: [
                gpui::rgb(0xef4444), // Red
                gpui::rgb(0xf97316), // Orange
                gpui::rgb(0xeab308), // Yellow
                gpui::rgb(0x22c55e), // Green
                gpui::rgb(0x14b8a6), // Teal
                gpui::rgb(0x3b82f6), // Blue
                gpui::rgb(0x8b5cf6), // Violet
                gpui::rgb(0xec4899), // Pink
                gpui::rgb(0x6366f1), // Indigo
                gpui::rgb(0x06b6d4), // Cyan
            ],
            knob_color: gpui::rgb(0x3b82f6),
            peak_indicator: gpui::rgb(0xef4444),
        }
    }
}
