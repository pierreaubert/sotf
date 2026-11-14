use super::{
    EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme,
    ThemeFeedback, ThemeLayout, ThemePluginPalette, rgb, rgba,
};
use gpui::Rgba;

impl Theme {
    /// Dark theme (default)
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the complete theme token mapping together"
    )]
    pub fn dark() -> Self {
        Self {
            background: rgb(0x001e_1e1e),
            background_secondary: rgb(0x0025_2525),
            background_tertiary: rgb(0x002d_2d2d),
            surface: rgb(0x002d_2d2d),
            surface_hover: rgb(0x003e_3e3e),
            surface_selected: rgb(0x0026_4f78),
            text_primary: rgb(0x00ff_ffff),
            text_secondary: rgb(0x00ee_eeee),
            text_muted: rgb(0x00bb_bbbb),
            text_disabled: rgb(0x0088_8888),
            border: rgb(0x0055_5555),
            border_focused: rgb(0x001c_8cd9),
            accent: rgb(0x000a_84ff),
            accent_hover: rgb(0x003d_9cff),
            accent_muted: rgb(0x0026_4f78),
            text_on_accent: rgb(0x0000_0000),
            text_on_accent_muted: rgba(0x0000_00cc),
            icon_on_accent: rgb(0x0000_0000),
            success: rgb(0x004e_c9b0),
            warning: rgb(0x00dc_dcaa),
            error: rgb(0x00f4_8771),
            info: rgb(0x0056_9cd6),
            button_mute_active: rgb(0x00dc_2626),
            button_solo_active: rgb(0x00f5_9e0b),
            button_dim_active: rgb(0x0063_66f1),
            plugin_palette: ThemePluginPalette {
                plugin_colors: PluginColorMap {
                    eq: rgb(0x0025_63eb),
                    gain: rgb(0x0005_9669),
                    upmixer: rgb(0x007c_3aed),
                    compressor: rgb(0x00dc_2626),
                    limiter: rgb(0x00ea_580c),
                    gate: rgb(0x00ca_8a04),
                    loudness: rgb(0x0008_91b2),
                    binaural: rgb(0x00db_2777),
                    convolution: rgb(0x004f_46e5),
                    monitor: rgb(0x0014_b8a6),
                    spectrum: rgb(0x008b_5cf6),
                    mute_solo: rgb(0x0063_66f1),
                },
                graph_colors: GraphLineColors {
                    input: rgb(0x0063_66f1),
                    target: rgb(0x0022_c55e),
                    filter_response: rgb(0x00f5_9e0b),
                    corrected: rgb(0x003b_82f6),
                    error: rgb(0x00ef_4444),
                    deviation: rgb(0x008b_5cf6),
                    grid: Rgba {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.083,
                    }, // rgba(0xffffff15) = 21/255 ≈ 8.3%
                    secondary_line: Rgba {
                        r: 0.667,
                        g: 0.667,
                        b: 0.667,
                        a: 1.0,
                    }, // rgb(0xaaaaaa)
                    directivity_er: Rgba {
                        r: 0.961,
                        g: 0.443,
                        b: 0.714,
                        a: 1.0,
                    }, // rgb(0xf472b6)
                    directivity_sp: Rgba {
                        r: 0.753,
                        g: 0.518,
                        b: 0.992,
                        a: 1.0,
                    }, // rgb(0xc084fc)
                },
                band_colors: vec![
                    rgb(0x00ef_4444), // Red
                    rgb(0x00f9_7316), // Orange
                    rgb(0x00ea_b308), // Yellow
                    rgb(0x0022_c55e), // Green
                    rgb(0x0014_b8a6), // Teal
                    rgb(0x003b_82f6), // Blue
                    rgb(0x008b_5cf6), // Violet
                    rgb(0x00ec_4899), // Pink
                    rgb(0x0063_66f1), // Indigo
                    rgb(0x0006_b6d4), // Cyan
                    rgb(0x009c_a3af), // Gray
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
                    background: rgb(0x001a_1a1a),
                    grid: Rgba {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.125,
                    }, // rgba(0xffffff20) = 32/255 ≈ 12.5%
                    curve_boost: rgb(0x0022_c55e),
                    curve_cut: rgb(0x00ef_4444),
                    fill_boost: Rgba {
                        r: 0.133,
                        g: 0.773,
                        b: 0.369,
                        a: 0.251,
                    }, // rgba(0x22c55e40) = 64/255 ≈ 25%
                    fill_cut: Rgba {
                        r: 0.939,
                        g: 0.267,
                        b: 0.267,
                        a: 0.251,
                    }, // rgba(0xef444440) = 64/255 ≈ 25%
                    zero_line: Rgba {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.251,
                    }, // rgba(0xffffff40) = 64/255 ≈ 25%
                },
                spectrum_colors: SpectrumColors {
                    background: rgb(0x0000_0000),
                    bass: rgb(0x0022_c55e),
                    mids: rgb(0x00ea_b308),
                    treble: rgb(0x00ef_4444),
                },
                meter_colors: MeterColors {
                    background: rgb(0x001e_1e1e),
                    normal: rgb(0x0022_c55e),
                    warning: rgb(0x00f5_9e0b),
                    clip: rgb(0x00dc_2626),
                    peak: rgb(0x00ff_ffff),
                    text: rgb(0x0099_9999),
                },
            },
            feedback: ThemeFeedback {
                meter_normal: rgb(0x0022_c55e),
                meter_warning: rgb(0x00f5_9e0b),
                meter_clip: rgb(0x00dc_2626),
                progress_bar_bg: rgb(0x003e_3e3e),
                progress_bar_fill: rgb(0x0000_7acc),
                toast_success_bg: rgb(0x001e_3a1e),
                toast_error_bg: rgb(0x003a_1e1e),
                toast_info_bg: rgb(0x001e_2a3a),
                toast_warning_bg: rgb(0x003a_2e1e),
                peak_indicator: rgb(0x00ff_ffff),
                drag_over_highlight: Rgba {
                    r: 0.231,
                    g: 0.510,
                    b: 0.961,
                    a: 0.251,
                },
                drag_over_border: rgb(0x003b_82f6),
                neutral_indicator: rgb(0x0063_66f1),
                warning_background: Rgba {
                    r: 0.961,
                    g: 0.616,
                    b: 0.067,
                    a: 0.2,
                },
                knob_color: rgb(0x00ff_ffff),
                optimization_color: rgb(0x008b_5cf6),
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
                design_tokens: gpui_audio_kit::AudioDesignTokens::default(),
            },
        }
    }
}
