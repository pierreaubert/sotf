use super::{
    EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme,
    ThemeFeedback, ThemeLayout, ThemePluginPalette, rgb,
};
use gpui::Rgba;

impl Theme {
    /// Black & White theme (monochrome high contrast)
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the complete theme token mapping together"
    )]
    pub fn black_and_white() -> Self {
        Self {
            background: rgb(0x0000_0000),
            background_secondary: rgb(0x000a_0a0a),
            background_tertiary: rgb(0x0014_1414),
            surface: rgb(0x0014_1414),
            surface_hover: rgb(0x0022_2222),
            surface_selected: rgb(0x0033_3333),
            text_primary: rgb(0x00ff_ffff),
            text_secondary: rgb(0x00ff_ffff),
            text_muted: rgb(0x00b0_b0b0),
            text_disabled: rgb(0x0059_5959),
            border: rgb(0x00dd_dddd),
            border_focused: rgb(0x00ff_ffff),
            accent: rgb(0x00ff_ffff),
            accent_hover: rgb(0x00ee_eeee),
            accent_muted: rgb(0x0088_8888),
            text_on_accent: rgb(0x0000_0000),
            text_on_accent_muted: rgb(0x0033_3333),
            icon_on_accent: rgb(0x0000_0000),
            success: rgb(0x00aa_aaaa),
            warning: rgb(0x0088_8888),
            error: rgb(0x0066_6666),
            info: rgb(0x0099_9999),
            button_mute_active: rgb(0x0066_6666),
            button_solo_active: rgb(0x00aa_aaaa),
            button_dim_active: rgb(0x0033_3333),
            plugin_palette: ThemePluginPalette {
                plugin_colors: PluginColorMap {
                    eq: rgb(0x00cc_cccc),
                    gain: rgb(0x00aa_aaaa),
                    upmixer: rgb(0x0088_8888),
                    compressor: rgb(0x0066_6666),
                    limiter: rgb(0x0077_7777),
                    gate: rgb(0x0099_9999),
                    loudness: rgb(0x00bb_bbbb),
                    binaural: rgb(0x0080_8080),
                    convolution: rgb(0x0090_9090),
                    monitor: rgb(0x00ab_abab),
                    spectrum: rgb(0x008a_8a8a),
                    mute_solo: rgb(0x0099_9999),
                },
                graph_colors: GraphLineColors {
                    input: rgb(0x005c_77a5),           // Blue
                    target: rgb(0x0071_a152),          // Green
                    filter_response: rgb(0x00dc_842a), // Orange
                    corrected: rgb(0x0076_b7b2),       // Teal-cyan
                    error: rgb(0x00c8_5857),           // Red
                    deviation: rgb(0x00b0_7aa1),       // Purple
                    grid: Rgba {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.083,
                    }, // rgba(0xffffff15) = 21/255 ≈ 8.3%
                    secondary_line: rgb(0x00ba_b0ac),  // Gray
                    directivity_er: rgb(0x00e1_5759),  // Red-pink
                    directivity_sp: rgb(0x0089_b5b1),  // Teal
                },
                band_colors: vec![
                    rgb(0x005c_77a5), // Blue
                    rgb(0x00dc_842a), // Orange
                    rgb(0x00c8_5857), // Red
                    rgb(0x0089_b5b1), // Teal
                    rgb(0x0071_a152), // Green
                    rgb(0x00ba_b0ac), // Gray
                    rgb(0x00e1_5759), // Red-pink
                    rgb(0x00b0_7aa1), // Purple
                    rgb(0x0076_b7b2), // Teal-cyan
                    rgb(0x00ff_9da7), // Pink
                ],
                channel_colors: vec![
                    rgb(0x005c_77a5), // Blue
                    rgb(0x00c4_4e52), // Red
                    rgb(0x0055_a868), // Green
                    rgb(0x00cc_b974), // Yellow
                    rgb(0x00b0_7aa1), // Purple
                    rgb(0x0076_b7b2), // Teal
                ],
                eq_curve_colors: EQCurveColors {
                    background: rgb(0x0000_0000),
                    grid: Rgba {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.063,
                    }, // rgba(0xffffff10) = 16/255 ≈ 6.3%
                    curve_boost: rgb(0x00bb_bbbb),
                    curve_cut: rgb(0x0066_6666),
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
                    background: rgb(0x0000_0000),
                    bass: rgb(0x00bb_bbbb),
                    mids: rgb(0x0088_8888),
                    treble: rgb(0x0044_4444),
                },
                meter_colors: MeterColors {
                    background: rgb(0x0000_0000),
                    normal: rgb(0x0088_8888),
                    warning: rgb(0x00aa_aaaa),
                    clip: rgb(0x00ff_ffff),
                    peak: rgb(0x00ff_ffff),
                    text: rgb(0x0088_8888),
                },
            },
            feedback: ThemeFeedback {
                meter_normal: rgb(0x0066_6666),
                meter_warning: rgb(0x00aa_aaaa),
                meter_clip: rgb(0x00ff_ffff),
                progress_bar_bg: rgb(0x0022_2222),
                progress_bar_fill: rgb(0x00ff_ffff),
                toast_success_bg: rgb(0x0011_1111),
                toast_error_bg: rgb(0x0011_1111),
                toast_info_bg: rgb(0x0011_1111),
                toast_warning_bg: rgb(0x0011_1111),
                peak_indicator: rgb(0x00ff_ffff),
                drag_over_highlight: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.251,
                },
                drag_over_border: rgb(0x00ff_ffff),
                neutral_indicator: rgb(0x00cc_cccc),
                warning_background: Rgba {
                    r: 0.667,
                    g: 0.667,
                    b: 0.667,
                    a: 0.2,
                },
                knob_color: rgb(0x00ff_ffff),
                optimization_color: rgb(0x00bb_bbbb),
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
                    a: 0.7,
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
