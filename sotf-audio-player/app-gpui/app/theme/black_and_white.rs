use super::{
    EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme, rgba,
};
use gpui::Rgba;

impl Theme {
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
            text_muted: rgba(0xaaaaaa), // Lighter for better readability
            text_disabled: rgba(0x333333),

            // Border colors (white for high contrast)
            border: rgba(0xdddddd), // Slightly softer but still visible
            border_focused: rgba(0xffffff), // Pure white for hover states

            // Accent colors (white accent for high contrast)
            accent: rgba(0xffffff), // Bold white accent
            accent_hover: rgba(0xeeeeee),
            accent_muted: rgba(0x888888),

            // Text on accent
            text_on_accent: rgba(0x000000),
            text_on_accent_muted: rgba(0x333333),
            icon_on_accent: rgba(0x000000),

            // Semantic colors
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
                input: rgba(0x5c77a5),           // Blue
                target: rgba(0x71a152),          // Green
                filter_response: rgba(0xdc842a), // Orange
                corrected: rgba(0x76b7b2),       // Teal-cyan
                error: rgba(0xc85857),           // Red
                deviation: rgba(0xb07aa1),       // Purple
                grid: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.083,
                }, // rgba(0xffffff15) = 21/255 ≈ 8.3%
                secondary_line: rgba(0xbab0ac),  // Gray
                directivity_er: rgba(0xe15759),  // Red-pink
                directivity_sp: rgba(0x89b5b1),  // Teal
            },
            band_colors: vec![
                rgba(0x5c77a5), // Blue
                rgba(0xdc842a), // Orange
                rgba(0xc85857), // Red
                rgba(0x89b5b1), // Teal
                rgba(0x71a152), // Green
                rgba(0xbab0ac), // Gray
                rgba(0xe15759), // Red-pink
                rgba(0xb07aa1), // Purple
                rgba(0x76b7b2), // Teal-cyan
                rgba(0xff9da7), // Pink
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
            overlay_bg: Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.7,
            }, // Semi-transparent black for modal backdrops (higher contrast for B&W)

            // Layout sizes
            separator_size: 20.0,

            // Font family
            font_family: "DM Serif Display".into(),
        }
    }
}
