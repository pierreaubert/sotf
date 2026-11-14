use super::{
    EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme,
    ThemeFeedback, ThemeLayout, ThemePluginPalette, rgb, rgba,
};
use gpui::Rgba;

impl Theme {
    /// Light theme
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the complete theme token mapping together"
    )]
    pub fn light() -> Self {
        Self {
            background: rgb(0x00f5_f5f5),
            background_secondary: rgb(0x00ee_eeee),
            background_tertiary: rgb(0x00e0_e0e0),
            surface: rgb(0x00ff_ffff),
            surface_hover: rgb(0x00f0_f0f0),
            surface_selected: rgb(0x00cc_e5ff),
            text_primary: rgb(0x0000_0000),
            text_secondary: rgb(0x0011_1111),
            text_muted: rgb(0x0044_4444),
            text_disabled: rgb(0x0076_7676),
            border: rgb(0x0099_9999),
            border_focused: rgb(0x0000_77ee),
            accent: rgb(0x0000_74e8),
            accent_hover: rgb(0x0026_87ff),
            accent_muted: rgb(0x00b3_d4fc),
            text_on_accent: rgb(0x00ff_ffff),
            text_on_accent_muted: rgba(0xffff_ffcc),
            icon_on_accent: rgb(0x00ff_ffff),
            success: rgb(0x0028_a745),
            warning: rgb(0x00ff_c107),
            error: rgb(0x00dc_3545),
            info: rgb(0x0017_a2b8),
            button_mute_active: rgb(0x00dc_3545),
            button_solo_active: rgb(0x00ff_c107),
            button_dim_active: rgb(0x006f_42c1),
            plugin_palette: ThemePluginPalette {
                plugin_colors: PluginColorMap {
                    eq: rgb(0x0000_66cc),
                    gain: rgb(0x0028_a745),
                    upmixer: rgb(0x006f_42c1),
                    compressor: rgb(0x00dc_3545),
                    limiter: rgb(0x00fd_7e14),
                    gate: rgb(0x00ff_c107),
                    loudness: rgb(0x0017_a2b8),
                    binaural: rgb(0x00e8_3e8c),
                    convolution: rgb(0x0066_10f2),
                    monitor: rgb(0x0020_c997),
                    spectrum: rgb(0x009b_59b6),
                    mute_solo: rgb(0x0000_7bff),
                },
                graph_colors: GraphLineColors {
                    input: rgb(0x0000_7bff),
                    target: rgb(0x0028_a745),
                    filter_response: rgb(0x00ff_c107),
                    corrected: rgb(0x0000_66cc),
                    error: rgb(0x00dc_3545),
                    deviation: rgb(0x009b_59b6),
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
                    rgb(0x00dc_3545), // Red
                    rgb(0x00fd_7e14), // Orange
                    rgb(0x00ff_c107), // Yellow
                    rgb(0x0028_a745), // Green
                    rgb(0x0020_c997), // Teal
                    rgb(0x0000_66cc), // Blue
                    rgb(0x006f_42c1), // Violet
                    rgb(0x00e8_3e8c), // Pink
                    rgb(0x0000_7bff), // Indigo
                    rgb(0x0000_bcd4), // Cyan
                    rgb(0x0099_9999), // Gray
                ],
                channel_colors: vec![
                    rgb(0x001a_73e8), // Blue
                    rgb(0x00d9_3025), // Red
                    rgb(0x0018_8038), // Green
                    rgb(0x00e3_7400), // Orange
                    rgb(0x007b_1fa2), // Purple
                    rgb(0x0000_838f), // Cyan
                ],
                eq_curve_colors: EQCurveColors {
                    background: rgb(0x00fa_fafa),
                    grid: Rgba {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.063,
                    }, // rgba(0x00000010) = 16/255 ≈ 6.3%
                    curve_boost: rgb(0x0028_a745),
                    curve_cut: rgb(0x00dc_3545),
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
                    background: rgb(0x00ff_ffff),
                    bass: rgb(0x0028_a745),
                    mids: rgb(0x00ff_c107),
                    treble: rgb(0x00dc_3545),
                },
                meter_colors: MeterColors {
                    background: rgb(0x00f5_f5f5),
                    normal: rgb(0x0028_a745),
                    warning: rgb(0x00ff_c107),
                    clip: rgb(0x00dc_3545),
                    peak: rgb(0x0000_0000),
                    text: rgb(0x0055_5555),
                },
            },
            feedback: ThemeFeedback {
                meter_normal: rgb(0x0028_a745),
                meter_warning: rgb(0x00ff_c107),
                meter_clip: rgb(0x00dc_3545),
                progress_bar_bg: rgb(0x00cc_cccc),
                progress_bar_fill: rgb(0x0000_66cc),
                toast_success_bg: rgb(0x00d4_edda),
                toast_error_bg: rgb(0x00f8_d7da),
                toast_info_bg: rgb(0x00d1_ecf1),
                toast_warning_bg: rgb(0x00ff_f3cd),
                peak_indicator: rgb(0x0000_0000),
                drag_over_highlight: Rgba {
                    r: 0.0,
                    g: 0.4,
                    b: 0.8,
                    a: 0.251,
                },
                drag_over_border: rgb(0x0000_66cc),
                neutral_indicator: rgb(0x0000_7bff),
                warning_background: Rgba {
                    r: 1.0,
                    g: 0.753,
                    b: 0.0,
                    a: 0.188,
                },
                knob_color: rgb(0x0000_0000),
                optimization_color: rgb(0x009b_59b6),
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
                design_tokens: gpui_audio_kit::AudioDesignTokens::default(),
            },
        }
    }
}
