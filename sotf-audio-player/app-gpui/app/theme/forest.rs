use super::{
    EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme, rgba,
};
use gpui::Rgba;

impl Theme {
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
            text_muted: rgba(0xb0cca9), // Lighter for better readability
            text_disabled: rgba(0x556b50),

            // Border colors
            border: rgba(0x4a5a45),         // More visible border
            border_focused: rgba(0x7dd07c), // Brighter for hover states

            // Accent colors
            accent: rgba(0x5cc65b), // Bolder, more saturated
            accent_hover: rgba(0x76e075),
            accent_muted: rgba(0x3d5a3a),

            // Text on accent
            text_on_accent: rgba(0xffffff),
            text_on_accent_muted: rgba(0xffffffcc),
            icon_on_accent: rgba(0x1e1e1e),

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
}
