use super::{
    EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme, rgb, rgba,
};
use gpui::Rgba;

impl Theme {
    /// Dark theme (default)
    pub fn dark() -> Self {
        Self {
            // Base colors
            background: rgb(0x1e1e1e),
            background_secondary: rgb(0x252525),
            background_tertiary: rgb(0x2d2d2d),
            surface: rgb(0x2d2d2d),
            surface_hover: rgb(0x3e3e3e),
            surface_selected: rgb(0x264f78),

            // Text colors
            text_primary: rgb(0xffffff), // Pure white for max contrast on dark background
            text_secondary: rgb(0xeeeeee),
            text_muted: rgb(0xbbbbbb),
            text_disabled: rgb(0x888888), // ~4.6:1 contrast on #1e1e1e (WCAG AA)

            // Border colors
            border: rgb(0x555555),         // More visible border
            border_focused: rgb(0x1c8cd9), // Brighter for hover states

            // Accent colors
            accent: rgb(0x0a84ff), // Bolder, more saturated
            accent_hover: rgb(0x3d9cff),
            accent_muted: rgb(0x264f78),

            // Text on accent
            text_on_accent: rgb(0xffffff),
            text_on_accent_muted: rgba(0xffffffcc),
            icon_on_accent: rgb(0xffffff),

            // Semantic colors
            success: rgb(0x4ec9b0),
            warning: rgb(0xdcdcaa),
            error: rgb(0xf48771),
            info: rgb(0x569cd6),

            // Level meter colors
            meter_normal: rgb(0x22c55e),
            meter_warning: rgb(0xf59e0b),
            meter_clip: rgb(0xdc2626),

            // Button colors
            button_mute_active: rgb(0xdc2626),
            button_solo_active: rgb(0xf59e0b),
            button_dim_active: rgb(0x6366f1),

            // Playback bar
            progress_bar_bg: rgb(0x3e3e3e),
            progress_bar_fill: rgb(0x007acc),

            // Toast backgrounds
            toast_success_bg: rgb(0x1e3a1e),
            toast_error_bg: rgb(0x3a1e1e),
            toast_info_bg: rgb(0x1e2a3a),
            toast_warning_bg: rgb(0x3a2e1e),

            // Plugin colors
            plugin_colors: PluginColorMap {
                eq: rgb(0x2563eb),
                gain: rgb(0x059669),
                upmixer: rgb(0x7c3aed),
                compressor: rgb(0xdc2626),
                limiter: rgb(0xea580c),
                gate: rgb(0xca8a04),
                loudness: rgb(0x0891b2),
                binaural: rgb(0xdb2777),
                convolution: rgb(0x4f46e5),
                monitor: rgb(0x14b8a6),
                spectrum: rgb(0x8b5cf6),
                mute_solo: rgb(0x6366f1),
            },
            graph_colors: GraphLineColors {
                input: rgb(0x6366f1),
                target: rgb(0x22c55e),
                filter_response: rgb(0xf59e0b),
                corrected: rgb(0x3b82f6),
                error: rgb(0xef4444),
                deviation: rgb(0x8b5cf6),
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
                }, // rgb(0xaaaaaa)
                directivity_er: Rgba {
                    r: 0.961,
                    g: 0.443,
                    b: 0.714,
                    a: 1.0,
                }, // rgb(0xf472b6)
                directivity_sp: Rgba {
                    r: 0.753,
                    g: 0.518,
                    b: 0.992,
                    a: 1.0,
                }, // rgb(0xc084fc)
            },
            band_colors: vec![
                rgb(0xef4444), // Red
                rgb(0xf97316), // Orange
                rgb(0xeab308), // Yellow
                rgb(0x22c55e), // Green
                rgb(0x14b8a6), // Teal
                rgb(0x3b82f6), // Blue
                rgb(0x8b5cf6), // Violet
                rgb(0xec4899), // Pink
                rgb(0x6366f1), // Indigo
                rgb(0x06b6d4), // Cyan
                rgb(0x9ca3af), // Gray
            ],
            channel_colors: vec![
                rgb(0x4285f4), // Blue
                rgb(0xea4335), // Red
                rgb(0x34a853), // Green
                rgb(0xfbbc04), // Yellow
                rgb(0x9c27b0), // Purple
                rgb(0x00bcd4), // Cyan
            ],
            eq_curve_colors: EQCurveColors {
                background: rgb(0x1a1a1a),
                grid: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.125,
                }, // rgba(0xffffff20) = 32/255 ≈ 12.5%
                curve_boost: rgb(0x22c55e),
                curve_cut: rgb(0xef4444),
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
                background: rgb(0x000000),
                bass: rgb(0x22c55e),
                mids: rgb(0xeab308),
                treble: rgb(0xef4444),
            },
            meter_colors: MeterColors {
                background: rgb(0x1e1e1e),
                normal: rgb(0x22c55e),
                warning: rgb(0xf59e0b),
                clip: rgb(0xdc2626),
                peak: rgb(0xffffff),
                text: rgb(0x999999),
            },

            // Additional semantic colors
            peak_indicator: rgb(0xffffff),
            drag_over_highlight: Rgba {
                r: 0.231,
                g: 0.510,
                b: 0.961,
                a: 0.251,
            }, // rgba(0x3b82f640) = 64/255 ≈ 25%
            drag_over_border: rgb(0x3b82f6),
            neutral_indicator: rgb(0x6366f1),
            warning_background: Rgba {
                r: 0.961,
                g: 0.616,
                b: 0.067,
                a: 0.2,
            }, // rgba(0xf59e0b33) ≈ 20%
            knob_color: rgb(0xffffff),
            optimization_color: rgb(0x8b5cf6),
            grid_color: Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.083,
            }, // rgba(0xffffff15) = 21/255 ≈ 8.3%
            overlay_bg: Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.5,
            }, // Semi-transparent black for modal backdrops

            // Layout sizes
            separator_size: 20.0,

            // Font family
            font_family: Some("B612".into()),
            design_tokens: Default::default(),
        }
    }
}
