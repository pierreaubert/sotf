use super::{
    EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme, rgb,
};
use gpui::Rgba;

impl Theme {
    /// Black & White theme (monochrome high contrast)
    pub fn black_and_white() -> Self {
        Self {
            // Base colors
            background: rgb(0x000000),
            background_secondary: rgb(0x0a0a0a),
            background_tertiary: rgb(0x141414),
            surface: rgb(0x141414),
            surface_hover: rgb(0x222222),
            surface_selected: rgb(0x333333),

            // Text colors
            text_primary: rgb(0xffffff),
            text_secondary: rgb(0xffffff),
            text_muted: rgb(0xb0b0b0), // Lighter for better readability, contrast ~4.3:1 on surface
            text_disabled: rgb(0x595959), // ~4.6:1 contrast on #000000 (WCAG AA)

            // Border colors (white for high contrast)
            border: rgb(0xdddddd),         // Slightly softer but still visible
            border_focused: rgb(0xffffff), // Pure white for hover states

            // Accent colors (white accent for high contrast)
            accent: rgb(0xffffff), // Bold white accent
            accent_hover: rgb(0xeeeeee),
            accent_muted: rgb(0x888888),

            // Text on accent
            text_on_accent: rgb(0x000000),
            text_on_accent_muted: rgb(0x333333),
            icon_on_accent: rgb(0x000000),

            // Semantic colors
            success: rgb(0xaaaaaa),
            warning: rgb(0x888888),
            error: rgb(0x666666),
            info: rgb(0x999999),

            // Level meter colors
            meter_normal: rgb(0x666666),
            meter_warning: rgb(0xaaaaaa),
            meter_clip: rgb(0xffffff),

            // Button colors
            button_mute_active: rgb(0x666666), // Dark Grey
            button_solo_active: rgb(0xaaaaaa), // Light Grey
            button_dim_active: rgb(0x333333),  // Grey

            // Playback bar
            progress_bar_bg: rgb(0x222222),
            progress_bar_fill: rgb(0xffffff),

            // Toast backgrounds
            toast_success_bg: rgb(0x111111),
            toast_error_bg: rgb(0x111111),
            toast_info_bg: rgb(0x111111),
            toast_warning_bg: rgb(0x111111),

            // Plugin colors (monochrome grayscale)
            plugin_colors: PluginColorMap {
                eq: rgb(0xcccccc),
                gain: rgb(0xaaaaaa),
                upmixer: rgb(0x888888),
                compressor: rgb(0x666666),
                limiter: rgb(0x777777),
                gate: rgb(0x999999),
                loudness: rgb(0xbbbbbb),
                binaural: rgb(0x808080),
                convolution: rgb(0x909090),
                monitor: rgb(0xababab),
                spectrum: rgb(0x8a8a8a),
                mute_solo: rgb(0x999999),
            },
            graph_colors: GraphLineColors {
                input: rgb(0x5c77a5),           // Blue
                target: rgb(0x71a152),          // Green
                filter_response: rgb(0xdc842a), // Orange
                corrected: rgb(0x76b7b2),       // Teal-cyan
                error: rgb(0xc85857),           // Red
                deviation: rgb(0xb07aa1),       // Purple
                grid: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.083,
                }, // rgba(0xffffff15) = 21/255 ≈ 8.3%
                secondary_line: rgb(0xbab0ac),  // Gray
                directivity_er: rgb(0xe15759),  // Red-pink
                directivity_sp: rgb(0x89b5b1),  // Teal
            },
            band_colors: vec![
                rgb(0x5c77a5), // Blue
                rgb(0xdc842a), // Orange
                rgb(0xc85857), // Red
                rgb(0x89b5b1), // Teal
                rgb(0x71a152), // Green
                rgb(0xbab0ac), // Gray
                rgb(0xe15759), // Red-pink
                rgb(0xb07aa1), // Purple
                rgb(0x76b7b2), // Teal-cyan
                rgb(0xff9da7), // Pink
            ],
            channel_colors: vec![
                rgb(0x5c77a5), // Blue
                rgb(0xc44e52), // Red
                rgb(0x55a868), // Green
                rgb(0xccb974), // Yellow
                rgb(0xb07aa1), // Purple
                rgb(0x76b7b2), // Teal
            ],
            eq_curve_colors: EQCurveColors {
                background: rgb(0x000000),
                grid: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.063,
                }, // rgba(0xffffff10) = 16/255 ≈ 6.3%
                curve_boost: rgb(0xbbbbbb),
                curve_cut: rgb(0x666666),
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
                background: rgb(0x000000),
                bass: rgb(0xbbbbbb),
                mids: rgb(0x888888),
                treble: rgb(0x444444),
            },
            meter_colors: MeterColors {
                background: rgb(0x000000),
                normal: rgb(0x888888),
                warning: rgb(0xaaaaaa),
                clip: rgb(0xffffff),
                peak: rgb(0xffffff),
                text: rgb(0x888888),
            },

            // Additional semantic colors
            peak_indicator: rgb(0xffffff),
            drag_over_highlight: Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.251,
            }, // rgba(0xffffff40) ≈ 25%
            drag_over_border: rgb(0xffffff),
            neutral_indicator: rgb(0xcccccc),
            warning_background: Rgba {
                r: 0.667,
                g: 0.667,
                b: 0.667,
                a: 0.2,
            }, // rgba(0xaaaaaa33) ≈ 20%
            knob_color: rgb(0xffffff),
            optimization_color: rgb(0xbbbbbb),
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
            font_family: Some("B612".into()),
            design_tokens: Default::default(),
        }
    }
}
