use super::{
    EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme,
    ThemeFeedback, ThemeLayout, ThemePluginPalette, rgb, rgba,
};
use gpui::Rgba;

impl Theme {
    /// Forest theme (green tones)
    pub fn forest() -> Self {
        Self {
            background: rgb(0x1a2418),
            background_secondary: rgb(0x222d1f),
            background_tertiary: rgb(0x2a3627),
            surface: rgb(0x2a3627),
            surface_hover: rgb(0x3a4a35),
            surface_selected: rgb(0x3d5a3a),
            text_primary: rgb(0xffffff),
            text_secondary: rgb(0xe0f2d8),
            text_muted: rgb(0xc8dcc0),
            text_disabled: rgb(0x9aba90),
            border: rgb(0x4a5a45),
            border_focused: rgb(0x7dd07c),
            accent: rgb(0x5cc65b),
            accent_hover: rgb(0x76e075),
            accent_muted: rgb(0x3d5a3a),
            text_on_accent: rgb(0x000000),
            text_on_accent_muted: rgba(0x000000cc),
            icon_on_accent: rgb(0x000000),
            success: rgb(0x6abf69),
            warning: rgb(0xe0c062),
            error: rgb(0xd96c6c),
            info: rgb(0x6cb2d9),
            button_mute_active: rgb(0xd96c6c),
            button_solo_active: rgb(0xe0c062),
            button_dim_active: rgb(0x9b7fd9),
            plugin_palette: ThemePluginPalette {
                plugin_colors: PluginColorMap {
                    eq: rgb(0x6abf69),
                    gain: rgb(0x7dd07c),
                    upmixer: rgb(0x9b7fd9),
                    compressor: rgb(0xd96c6c),
                    limiter: rgb(0xdb8f4f),
                    gate: rgb(0xe0c062),
                    loudness: rgb(0x6cb2d9),
                    binaural: rgb(0xd96cb0),
                    convolution: rgb(0x9b7fd9),
                    monitor: rgb(0x6abf69),
                    spectrum: rgb(0x9b7fd9),
                    mute_solo: rgb(0x6cb2d9),
                },
                graph_colors: GraphLineColors {
                    input: rgb(0x6cb2d9),
                    target: rgb(0x6abf69),
                    filter_response: rgb(0xe0c062),
                    corrected: rgb(0x6abf69),
                    error: rgb(0xd96c6c),
                    deviation: rgb(0x9b7fd9),
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
                    }, // rgb(0x7a9a73)
                    directivity_er: Rgba {
                        r: 0.851,
                        g: 0.424,
                        b: 0.690,
                        a: 1.0,
                    }, // rgb(0xd96cb0)
                    directivity_sp: Rgba {
                        r: 0.608,
                        g: 0.498,
                        b: 0.851,
                        a: 1.0,
                    }, // rgb(0x9b7fd9)
                },
                band_colors: vec![
                    rgb(0xd96c6c), // Red
                    rgb(0xdb8f4f), // Orange
                    rgb(0xe0c062), // Yellow
                    rgb(0x6abf69), // Green
                    rgb(0x6cb2d9), // Teal
                    rgb(0x6abf69), // Blue
                    rgb(0x9b7fd9), // Violet
                    rgb(0xd96cb0), // Pink
                    rgb(0x6cb2d9), // Indigo
                    rgb(0x7dd07c), // Cyan
                    rgb(0x7a9a73), // Gray
                ],
                channel_colors: vec![
                    rgb(0x6cb2d9), // Blue
                    rgb(0xd96c6c), // Red
                    rgb(0x7dd07c), // Green
                    rgb(0xd9b96c), // Yellow
                    rgb(0xb07aa1), // Purple
                    rgb(0x5fb5b0), // Teal
                ],
                eq_curve_colors: EQCurveColors {
                    background: rgb(0x1a2418),
                    grid: Rgba {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.083,
                    }, // rgba(0xffffff15) = 21/255 ≈ 8.3%
                    curve_boost: rgb(0x6abf69),
                    curve_cut: rgb(0xd96c6c),
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
                    background: rgb(0x1a2418),
                    bass: rgb(0x6abf69),
                    mids: rgb(0xe0c062),
                    treble: rgb(0xd96c6c),
                },
                meter_colors: MeterColors {
                    background: rgb(0x1a2418),
                    normal: rgb(0x6abf69),
                    warning: rgb(0xe0c062),
                    clip: rgb(0xd96c6c),
                    peak: rgb(0xffffff),
                    text: rgb(0xa8c4a2),
                },
            },
            feedback: ThemeFeedback {
                meter_normal: rgb(0x6abf69),
                meter_warning: rgb(0xe0c062),
                meter_clip: rgb(0xd96c6c),
                progress_bar_bg: rgb(0x3a4a35),
                progress_bar_fill: rgb(0x6abf69),
                toast_success_bg: rgb(0x1e3a1e),
                toast_error_bg: rgb(0x3a1e1e),
                toast_info_bg: rgb(0x1e2a3a),
                toast_warning_bg: rgb(0x3a321e),
                peak_indicator: rgb(0xffffff),
                drag_over_highlight: Rgba {
                    r: 0.416,
                    g: 0.749,
                    b: 0.408,
                    a: 0.251,
                },
                drag_over_border: rgb(0x6abf69),
                neutral_indicator: rgb(0x6cb2d9),
                warning_background: Rgba {
                    r: 0.878,
                    g: 0.753,
                    b: 0.384,
                    a: 0.2,
                },
                knob_color: rgb(0xffffff),
                optimization_color: rgb(0x9b7fd9),
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
                    a: 0.5,
                },
            },
            layout: ThemeLayout {
                separator_size: 20.0,
                font_family: None,
                design_tokens: Default::default(),
            },
        }
    }
}
