use super::{
    EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme, rgb, rgba,
};
use gpui::Rgba;

impl Theme {
    /// Midnight theme (deep blue)
    pub fn midnight() -> Self {
        Self {
            // Base colors
            background: rgb(0x0d1117),
            background_secondary: rgb(0x161b22),
            background_tertiary: rgb(0x21262d),
            surface: rgb(0x21262d),
            surface_hover: rgb(0x30363d),
            surface_selected: rgba(0x1f6feb33),

            // Text colors
            text_primary: rgb(0xffffff), // Pure white for max contrast on dark background
            text_secondary: rgb(0xe0e6ed),
            text_muted: rgb(0xb1bac4),
            text_disabled: rgb(0x8b949e), // ~4.7:1 contrast on #0d1117 (WCAG AA)

            // Border colors
            border: rgb(0x444c56),         // More visible border
            border_focused: rgb(0x79b8ff), // Brighter for hover states

            // Accent colors
            accent: rgb(0x4493f8), // Bolder, more saturated
            accent_hover: rgb(0x6cb6ff),
            accent_muted: rgb(0x1f6feb),

            // Text on accent
            text_on_accent: rgb(0x000000),
            text_on_accent_muted: rgba(0x000000cc),
            icon_on_accent: rgb(0x000000),

            // Semantic colors
            success: rgb(0x3fb950),
            warning: rgb(0xd29922),
            error: rgb(0xf85149),
            info: rgb(0x58a6ff),

            // Level meter colors
            meter_normal: rgb(0x3fb950),
            meter_warning: rgb(0xd29922),
            meter_clip: rgb(0xf85149),

            // Button colors
            button_mute_active: rgb(0xf85149),
            button_solo_active: rgb(0xd29922),
            button_dim_active: rgb(0x8957e5),

            // Playback bar
            progress_bar_bg: rgb(0x30363d),
            progress_bar_fill: rgb(0x58a6ff),

            // Toast backgrounds
            toast_success_bg: rgb(0x1b4721),
            toast_error_bg: rgb(0x490202),
            toast_info_bg: rgb(0x0d2140),
            toast_warning_bg: rgb(0x4a3219),

            // Plugin colors
            plugin_colors: PluginColorMap {
                eq: rgb(0x79c0ff),
                gain: rgb(0x3fb950),
                upmixer: rgb(0xa371f7),
                compressor: rgb(0xf85149),
                limiter: rgb(0xfb8500),
                gate: rgb(0xd29922),
                loudness: rgb(0x56d4dd),
                binaural: rgb(0xf0883e),
                convolution: rgb(0xbc8ef1),
                monitor: rgb(0x3fb950),
                spectrum: rgb(0xa371f7),
                mute_solo: rgb(0x58a6ff),
            },
            graph_colors: GraphLineColors {
                input: rgb(0x79c0ff),
                target: rgb(0x3fb950),
                filter_response: rgb(0xd29922),
                corrected: rgb(0x58a6ff),
                error: rgb(0xf85149),
                deviation: rgb(0xa371f7),
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
                }, // rgb(0x8b949e)
                directivity_er: Rgba {
                    r: 0.941,
                    g: 0.502,
                    b: 0.502,
                    a: 1.0,
                }, // rgb(0xf08080)
                directivity_sp: Rgba {
                    r: 0.855,
                    g: 0.439,
                    b: 0.839,
                    a: 1.0,
                }, // rgb(0xda70d6)
            },
            band_colors: vec![
                rgb(0xf85149), // Red
                rgb(0xfb8500), // Orange
                rgb(0xd29922), // Yellow
                rgb(0x3fb950), // Green
                rgb(0x56d4dd), // Teal
                rgb(0x79c0ff), // Blue
                rgb(0xa371f7), // Violet
                rgb(0xf0883e), // Pink
                rgb(0x58a6ff), // Indigo
                rgb(0x7ee787), // Cyan
                rgb(0x8b949e), // Gray
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
                background: rgb(0x0d1117),
                grid: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.083,
                }, // rgba(0xffffff15) = 21/255 ≈ 8.3%
                curve_boost: rgb(0x3fb950),
                curve_cut: rgb(0xf85149),
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
                background: rgb(0x0a0e27),
                bass: rgb(0x3fb950),
                mids: rgb(0xd29922),
                treble: rgb(0xf85149),
            },
            meter_colors: MeterColors {
                background: rgb(0x0d1117),
                normal: rgb(0x3fb950),
                warning: rgb(0xd29922),
                clip: rgb(0xf85149),
                peak: rgb(0xffffff),
                text: rgb(0x8b949e),
            },

            // Additional semantic colors
            peak_indicator: rgb(0xffffff),
            drag_over_highlight: Rgba {
                r: 0.345,
                g: 0.651,
                b: 1.0,
                a: 0.251,
            }, // rgba(0x58a6ff40) ≈ 25%
            drag_over_border: rgb(0x58a6ff),
            neutral_indicator: rgb(0x58a6ff),
            warning_background: Rgba {
                r: 0.824,
                g: 0.6,
                b: 0.133,
                a: 0.2,
            }, // rgba(0xd2992233) ≈ 20%
            knob_color: rgb(0xffffff),
            optimization_color: rgb(0xa371f7),
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
                a: 0.6,
            }, // Semi-transparent black for modal backdrops (darker for midnight theme)

            // Layout sizes
            separator_size: 20.0,

            // Font family
            font_family: None,
            design_tokens: Default::default(),
        }
    }
}
