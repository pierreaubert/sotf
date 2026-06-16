use super::{
    EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme,
    ThemeFeedback, ThemeLayout, ThemePluginPalette, rgb, rgba,
};
use gpui::Rgba;

impl Theme {
    /// Light theme
    pub fn light() -> Self {
        Self {
            background: rgb(0xf5f5f5),
            background_secondary: rgb(0xeeeeee),
            background_tertiary: rgb(0xe0e0e0),
            surface: rgb(0xffffff),
            surface_hover: rgb(0xf0f0f0),
            surface_selected: rgb(0xcce5ff),
            text_primary: rgb(0x000000),
            text_secondary: rgb(0x111111),
            text_muted: rgb(0x444444),
            text_disabled: rgb(0x767676),
            border: rgb(0x999999),
            border_focused: rgb(0x0077ee),
            accent: rgb(0x0074e8),
            accent_hover: rgb(0x2687ff),
            accent_muted: rgb(0xb3d4fc),
            text_on_accent: rgb(0xffffff),
            text_on_accent_muted: rgba(0xffffffcc),
            icon_on_accent: rgb(0xffffff),
            success: rgb(0x28a745),
            warning: rgb(0xffc107),
            error: rgb(0xdc3545),
            info: rgb(0x17a2b8),
            button_mute_active: rgb(0xdc3545),
            button_solo_active: rgb(0xffc107),
            button_dim_active: rgb(0x6f42c1),
            plugin_palette: ThemePluginPalette {
                plugin_colors: PluginColorMap {
                    eq: rgb(0x0066cc),
                    gain: rgb(0x28a745),
                    upmixer: rgb(0x6f42c1),
                    compressor: rgb(0xdc3545),
                    limiter: rgb(0xfd7e14),
                    gate: rgb(0xffc107),
                    loudness: rgb(0x17a2b8),
                    binaural: rgb(0xe83e8c),
                    convolution: rgb(0x6610f2),
                    monitor: rgb(0x20c997),
                    spectrum: rgb(0x9b59b6),
                    mute_solo: rgb(0x007bff),
                },
                graph_colors: GraphLineColors {
                    input: rgb(0x007bff),
                    target: rgb(0x28a745),
                    filter_response: rgb(0xffc107),
                    corrected: rgb(0x0066cc),
                    error: rgb(0xdc3545),
                    deviation: rgb(0x9b59b6),
                    grid: Rgba {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.063,
                    }, // rgba(0x00000010) = 16/255 ≈ 6.3%
                    secondary_line: Rgba {
                        r: 0.333,
                        g: 0.333,
                        b: 0.333,
                        a: 1.0,
                    }, // rgb(0x555555)
                    directivity_er: Rgba {
                        r: 0.910,
                        g: 0.118,
                        b: 0.388,
                        a: 1.0,
                    }, // rgb(0xe91e63)
                    directivity_sp: Rgba {
                        r: 0.612,
                        g: 0.153,
                        b: 0.690,
                        a: 1.0,
                    }, // rgb(0x9c27b0)
                },
                band_colors: vec![
                    rgb(0xdc3545), // Red
                    rgb(0xfd7e14), // Orange
                    rgb(0xffc107), // Yellow
                    rgb(0x28a745), // Green
                    rgb(0x20c997), // Teal
                    rgb(0x0066cc), // Blue
                    rgb(0x6f42c1), // Violet
                    rgb(0xe83e8c), // Pink
                    rgb(0x007bff), // Indigo
                    rgb(0x00bcd4), // Cyan
                    rgb(0x999999), // Gray
                ],
                channel_colors: vec![
                    rgb(0x1a73e8), // Blue
                    rgb(0xd93025), // Red
                    rgb(0x188038), // Green
                    rgb(0xe37400), // Orange
                    rgb(0x7b1fa2), // Purple
                    rgb(0x00838f), // Cyan
                ],
                eq_curve_colors: EQCurveColors {
                    background: rgb(0xfafafa),
                    grid: Rgba {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.063,
                    }, // rgba(0x00000010) = 16/255 ≈ 6.3%
                    curve_boost: rgb(0x28a745),
                    curve_cut: rgb(0xdc3545),
                    fill_boost: Rgba {
                        r: 0.157,
                        g: 0.655,
                        b: 0.267,
                        a: 0.188,
                    }, // rgba(0x28a74430) ≈ 19%
                    fill_cut: Rgba {
                        r: 0.863,
                        g: 0.208,
                        b: 0.208,
                        a: 0.188,
                    }, // rgba(0xdc354530) ≈ 19%
                    zero_line: Rgba {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.188,
                    }, // rgba(0x00000030) ≈ 19%
                },
                spectrum_colors: SpectrumColors {
                    background: rgb(0xffffff),
                    bass: rgb(0x28a745),
                    mids: rgb(0xffc107),
                    treble: rgb(0xdc3545),
                },
                meter_colors: MeterColors {
                    background: rgb(0xf5f5f5),
                    normal: rgb(0x28a745),
                    warning: rgb(0xffc107),
                    clip: rgb(0xdc3545),
                    peak: rgb(0x000000),
                    text: rgb(0x555555),
                },
            },
            feedback: ThemeFeedback {
                meter_normal: rgb(0x28a745),
                meter_warning: rgb(0xffc107),
                meter_clip: rgb(0xdc3545),
                progress_bar_bg: rgb(0xcccccc),
                progress_bar_fill: rgb(0x0066cc),
                toast_success_bg: rgb(0xd4edda),
                toast_error_bg: rgb(0xf8d7da),
                toast_info_bg: rgb(0xd1ecf1),
                toast_warning_bg: rgb(0xfff3cd),
                peak_indicator: rgb(0x000000),
                drag_over_highlight: Rgba {
                    r: 0.0,
                    g: 0.4,
                    b: 0.8,
                    a: 0.251,
                },
                drag_over_border: rgb(0x0066cc),
                neutral_indicator: rgb(0x007bff),
                warning_background: Rgba {
                    r: 1.0,
                    g: 0.753,
                    b: 0.0,
                    a: 0.188,
                },
                knob_color: rgb(0x000000),
                optimization_color: rgb(0x9b59b6),
                grid_color: Rgba {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.063,
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
