use super::{
    EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme,
    ThemeFeedback, ThemeLayout, ThemePluginPalette, rgb, rgba,
};
use gpui::Rgba;

impl Theme {
    /// Midnight theme (deep blue)
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the complete theme token mapping together"
    )]
    pub fn midnight() -> Self {
        Self {
            background: rgb(0x000d_1117),
            background_secondary: rgb(0x0016_1b22),
            background_tertiary: rgb(0x0021_262d),
            surface: rgb(0x0021_262d),
            surface_hover: rgb(0x0030_363d),
            surface_selected: rgba(0x1f6f_eb33),
            text_primary: rgb(0x00ff_ffff),
            text_secondary: rgb(0x00e0_e6ed),
            text_muted: rgb(0x00b1_bac4),
            text_disabled: rgb(0x008b_949e),
            border: rgb(0x0044_4c56),
            border_focused: rgb(0x0079_b8ff),
            accent: rgb(0x0044_93f8),
            accent_hover: rgb(0x006c_b6ff),
            accent_muted: rgb(0x001f_6feb),
            text_on_accent: rgb(0x0000_0000),
            text_on_accent_muted: rgba(0x0000_00cc),
            icon_on_accent: rgb(0x0000_0000),
            success: rgb(0x003f_b950),
            warning: rgb(0x00d2_9922),
            error: rgb(0x00f8_5149),
            info: rgb(0x0058_a6ff),
            button_mute_active: rgb(0x00f8_5149),
            button_solo_active: rgb(0x00d2_9922),
            button_dim_active: rgb(0x0089_57e5),
            plugin_palette: ThemePluginPalette {
                plugin_colors: PluginColorMap {
                    eq: rgb(0x0079_c0ff),
                    gain: rgb(0x003f_b950),
                    upmixer: rgb(0x00a3_71f7),
                    compressor: rgb(0x00f8_5149),
                    limiter: rgb(0x00fb_8500),
                    gate: rgb(0x00d2_9922),
                    loudness: rgb(0x0056_d4dd),
                    binaural: rgb(0x00f0_883e),
                    convolution: rgb(0x00bc_8ef1),
                    monitor: rgb(0x003f_b950),
                    spectrum: rgb(0x00a3_71f7),
                    mute_solo: rgb(0x0058_a6ff),
                },
                graph_colors: GraphLineColors {
                    input: rgb(0x0079_c0ff),
                    target: rgb(0x003f_b950),
                    filter_response: rgb(0x00d2_9922),
                    corrected: rgb(0x0058_a6ff),
                    error: rgb(0x00f8_5149),
                    deviation: rgb(0x00a3_71f7),
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
                    rgb(0x00f8_5149), // Red
                    rgb(0x00fb_8500), // Orange
                    rgb(0x00d2_9922), // Yellow
                    rgb(0x003f_b950), // Green
                    rgb(0x0056_d4dd), // Teal
                    rgb(0x0079_c0ff), // Blue
                    rgb(0x00a3_71f7), // Violet
                    rgb(0x00f0_883e), // Pink
                    rgb(0x0058_a6ff), // Indigo
                    rgb(0x007e_e787), // Cyan
                    rgb(0x008b_949e), // Gray
                ],
                channel_colors: vec![
                    rgb(0x0042_85f4), // Blue
                    rgb(0x00ea_4335), // Red
                    rgb(0x0034_a853), // Green
                    rgb(0x00fb_bc04), // Yellow
                    rgb(0x009c_27b0), // Purple
                    rgb(0x0000_bcd4), // Cyan
                ],
                eq_curve_colors: EQCurveColors {
                    background: rgb(0x000d_1117),
                    grid: Rgba {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.083,
                    }, // rgba(0xffffff15) = 21/255 ≈ 8.3%
                    curve_boost: rgb(0x003f_b950),
                    curve_cut: rgb(0x00f8_5149),
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
                    background: rgb(0x000a_0e27),
                    bass: rgb(0x003f_b950),
                    mids: rgb(0x00d2_9922),
                    treble: rgb(0x00f8_5149),
                },
                meter_colors: MeterColors {
                    background: rgb(0x000d_1117),
                    normal: rgb(0x003f_b950),
                    warning: rgb(0x00d2_9922),
                    clip: rgb(0x00f8_5149),
                    peak: rgb(0x00ff_ffff),
                    text: rgb(0x008b_949e),
                },
            },
            feedback: ThemeFeedback {
                meter_normal: rgb(0x003f_b950),
                meter_warning: rgb(0x00d2_9922),
                meter_clip: rgb(0x00f8_5149),
                progress_bar_bg: rgb(0x0030_363d),
                progress_bar_fill: rgb(0x0058_a6ff),
                toast_success_bg: rgb(0x001b_4721),
                toast_error_bg: rgb(0x0049_0202),
                toast_info_bg: rgb(0x000d_2140),
                toast_warning_bg: rgb(0x004a_3219),
                peak_indicator: rgb(0x00ff_ffff),
                drag_over_highlight: Rgba {
                    r: 0.345,
                    g: 0.651,
                    b: 1.0,
                    a: 0.251,
                },
                drag_over_border: rgb(0x0058_a6ff),
                neutral_indicator: rgb(0x0058_a6ff),
                warning_background: Rgba {
                    r: 0.824,
                    g: 0.6,
                    b: 0.133,
                    a: 0.2,
                },
                knob_color: rgb(0x00ff_ffff),
                optimization_color: rgb(0x00a3_71f7),
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
                    a: 0.6,
                },
            },
            layout: ThemeLayout {
                separator_size: 20.0,
                font_family: None,
                design_tokens: gpui_audio_kit::AudioDesignTokens::default(),
            },
        }
    }
}
