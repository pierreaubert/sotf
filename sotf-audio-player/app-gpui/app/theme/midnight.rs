use super::{
    EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme, rgba,
};
use gpui::Rgba;

impl Theme {
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
            text_muted: rgba(0x9ca5af), // Lighter for better readability
            text_disabled: rgba(0x484f58),

            // Border colors
            border: rgba(0x444c56),         // More visible border
            border_focused: rgba(0x79b8ff), // Brighter for hover states

            // Accent colors
            accent: rgba(0x4493f8), // Bolder, more saturated
            accent_hover: rgba(0x6cb6ff),
            accent_muted: rgba(0x1f6feb),

            // Text on accent
            text_on_accent: rgba(0xffffff),
            text_on_accent_muted: rgba(0xffffffcc),
            icon_on_accent: rgba(0x1e1e1e),

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
            overlay_bg: Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.6,
            }, // Semi-transparent black for modal backdrops (darker for midnight theme)

            // Layout sizes
            separator_size: 20.0,

            // Font family
            font_family: ".SystemUI".into(),
        }
    }
}
