use super::{
    EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme, rgb, rgba,
};
use gpui::Rgba;

impl Theme {
    /// Onyx theme (near-black with warm amber/gold accent)
    pub fn onyx() -> Self {
        Self {
            // Base colors
            background: rgb(0x0c0c0e),
            background_secondary: rgb(0x111114),
            background_tertiary: rgb(0x141416),
            surface: rgb(0x1a1a1e),
            surface_hover: rgb(0x242428),
            surface_selected: rgb(0x3d2e0a),

            // Text colors
            text_primary: rgb(0xfafaf9),
            text_secondary: rgb(0xd6d3d1),
            text_muted: rgb(0xa8a29e),
            text_disabled: rgb(0x78716c), // ~4.5:1 contrast on #0c0c0e (WCAG AA)

            // Border colors
            border: rgb(0x3a3a3e), // Increased from 0x2a2a2e for visibility
            border_focused: rgb(0xf59e0b),

            // Accent colors
            accent: rgb(0xf59e0b),
            accent_hover: rgb(0xfbbf24),
            accent_muted: rgb(0x78350f),

            // Text on accent (dark on bright amber)
            text_on_accent: rgb(0x0c0c0e),
            text_on_accent_muted: rgba(0x0c0c0ecc),
            icon_on_accent: rgb(0x0c0c0e),

            // Semantic colors
            success: rgb(0x4ade80),
            warning: rgb(0xfb923c),
            error: rgb(0xef4444),
            info: rgb(0x38bdf8),

            // Level meter colors
            meter_normal: rgb(0x4ade80),
            meter_warning: rgb(0xfb923c),
            meter_clip: rgb(0xef4444),

            // Button colors
            button_mute_active: rgb(0xef4444),
            button_solo_active: rgb(0xfb923c),
            button_dim_active: rgb(0xa78bfa),

            // Playback bar
            progress_bar_bg: rgb(0x242428),
            progress_bar_fill: rgb(0xf59e0b),

            // Toast backgrounds
            toast_success_bg: rgb(0x14532d),
            toast_error_bg: rgb(0x450a0a),
            toast_info_bg: rgb(0x0c2d48),
            toast_warning_bg: rgb(0x451a03),

            // Plugin colors (warm-shifted)
            plugin_colors: PluginColorMap {
                eq: rgb(0xfbbf24),
                gain: rgb(0x4ade80),
                upmixer: rgb(0xa78bfa),
                compressor: rgb(0xef4444),
                limiter: rgb(0xfb923c),
                gate: rgb(0xf59e0b),
                loudness: rgb(0x38bdf8),
                binaural: rgb(0xf97316),
                convolution: rgb(0xc084fc),
                monitor: rgb(0x4ade80),
                spectrum: rgb(0xa78bfa),
                mute_solo: rgb(0xf59e0b),
            },
            graph_colors: GraphLineColors {
                input: rgb(0xfbbf24),
                target: rgb(0x4ade80),
                filter_response: rgb(0xfb923c),
                corrected: rgb(0xf59e0b),
                error: rgb(0xef4444),
                deviation: rgb(0xa78bfa),
                grid: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.083,
                },
                secondary_line: rgb(0xa8a29e),
                directivity_er: rgb(0xfb923c),
                directivity_sp: rgb(0xc084fc),
            },
            band_colors: vec![
                rgb(0xef4444), // Red
                rgb(0xf97316), // Orange
                rgb(0xfb923c), // Amber
                rgb(0x4ade80), // Green
                rgb(0x38bdf8), // Sky
                rgb(0xfbbf24), // Gold
                rgb(0xa78bfa), // Violet
                rgb(0xf59e0b), // Amber
                rgb(0x60a5fa), // Blue
                rgb(0x34d399), // Emerald
                rgb(0xa8a29e), // Stone
            ],
            eq_curve_colors: EQCurveColors {
                background: rgb(0x0c0c0e),
                grid: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.083,
                },
                curve_boost: rgb(0x4ade80),
                curve_cut: rgb(0xef4444),
                fill_boost: Rgba {
                    r: 0.290,
                    g: 0.871,
                    b: 0.502,
                    a: 0.251,
                },
                fill_cut: Rgba {
                    r: 0.937,
                    g: 0.267,
                    b: 0.267,
                    a: 0.251,
                },
                zero_line: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.251,
                },
            },
            spectrum_colors: SpectrumColors {
                background: rgb(0x0a0a0c),
                bass: rgb(0xf59e0b),
                mids: rgb(0xfbbf24),
                treble: rgb(0xef4444),
            },
            meter_colors: MeterColors {
                background: rgb(0x0c0c0e),
                normal: rgb(0x4ade80),
                warning: rgb(0xfb923c),
                clip: rgb(0xef4444),
                peak: rgb(0xfafaf9),
                text: rgb(0xa8a29e),
            },

            // Additional semantic colors
            peak_indicator: rgb(0xfafaf9),
            drag_over_highlight: Rgba {
                r: 0.961,
                g: 0.620,
                b: 0.043,
                a: 0.251,
            },
            drag_over_border: rgb(0xf59e0b),
            neutral_indicator: rgb(0xf59e0b),
            warning_background: Rgba {
                r: 0.984,
                g: 0.573,
                b: 0.235,
                a: 0.2,
            },
            knob_color: rgb(0xfafaf9),
            optimization_color: rgb(0xa78bfa),
            grid_color: Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.083,
            },
            overlay_bg: Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.65,
            },

            // Layout sizes
            separator_size: 20.0,

            // Font family
            font_family: "B612".into(),
        }
    }
}
