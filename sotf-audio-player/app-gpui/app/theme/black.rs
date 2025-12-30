use super::{
    rgba, EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme,
};
use gpui::Rgba;

impl Theme {
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
            text_muted: rgba(0x999999), // Lighter for better readability
            text_disabled: rgba(0x444444),

            // Border colors
            border: rgba(0x555555),         // More visible border
            border_focused: rgba(0x1c8cd9), // Brighter for hover states

            // Accent colors
            accent: rgba(0x0a84ff), // Bolder, more saturated
            accent_hover: rgba(0x3d9cff),
            accent_muted: rgba(0x264f78),

            // Text on accent
            text_on_accent: rgba(0xffffff),
            text_on_accent_muted: rgba(0xffffffcc),
            icon_on_accent: rgba(0x1e1e1e),

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
}
