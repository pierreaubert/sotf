use super::{
    EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme,
    ThemeFeedback, ThemeLayout, ThemePluginPalette, rgb, rgba,
};
use gpui::Rgba;

impl Theme {
    /// Onyx theme (near-black with warm amber/gold accent)
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the complete theme token mapping together"
    )]
    pub fn onyx() -> Self {
        Self {
            background: rgb(0x000c_0c0e),
            background_secondary: rgb(0x0011_1114),
            background_tertiary: rgb(0x0014_1416),
            surface: rgb(0x001a_1a1e),
            surface_hover: rgb(0x0024_2428),
            surface_selected: rgb(0x003d_2e0a),
            text_primary: rgb(0x00fa_faf9),
            text_secondary: rgb(0x00d6_d3d1),
            text_muted: rgb(0x00a8_a29e),
            text_disabled: rgb(0x0078_716c),
            border: rgb(0x003a_3a3e),
            border_focused: rgb(0x00f5_9e0b),
            accent: rgb(0x00f5_9e0b),
            accent_hover: rgb(0x00fb_bf24),
            accent_muted: rgb(0x0078_350f),
            text_on_accent: rgb(0x000c_0c0e),
            text_on_accent_muted: rgba(0x0c0c_0ecc),
            icon_on_accent: rgb(0x000c_0c0e),
            success: rgb(0x004a_de80),
            warning: rgb(0x00fb_923c),
            error: rgb(0x00ef_4444),
            info: rgb(0x0038_bdf8),
            button_mute_active: rgb(0x00ef_4444),
            button_solo_active: rgb(0x00fb_923c),
            button_dim_active: rgb(0x00a7_8bfa),
            plugin_palette: ThemePluginPalette {
                plugin_colors: PluginColorMap {
                    eq: rgb(0x00fb_bf24),
                    gain: rgb(0x004a_de80),
                    upmixer: rgb(0x00a7_8bfa),
                    compressor: rgb(0x00ef_4444),
                    limiter: rgb(0x00fb_923c),
                    gate: rgb(0x00f5_9e0b),
                    loudness: rgb(0x0038_bdf8),
                    binaural: rgb(0x00f9_7316),
                    convolution: rgb(0x00c0_84fc),
                    monitor: rgb(0x004a_de80),
                    spectrum: rgb(0x00a7_8bfa),
                    mute_solo: rgb(0x00f5_9e0b),
                },
                graph_colors: GraphLineColors {
                    input: rgb(0x00fb_bf24),
                    target: rgb(0x004a_de80),
                    filter_response: rgb(0x00fb_923c),
                    corrected: rgb(0x00f5_9e0b),
                    error: rgb(0x00ef_4444),
                    deviation: rgb(0x00a7_8bfa),
                    grid: Rgba {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.083,
                    },
                    secondary_line: rgb(0x00a8_a29e),
                    directivity_er: rgb(0x00fb_923c),
                    directivity_sp: rgb(0x00c0_84fc),
                },
                band_colors: vec![
                    rgb(0x00ef_4444), // Red
                    rgb(0x00f9_7316), // Orange
                    rgb(0x00fb_923c), // Amber
                    rgb(0x004a_de80), // Green
                    rgb(0x0038_bdf8), // Sky
                    rgb(0x00fb_bf24), // Gold
                    rgb(0x00a7_8bfa), // Violet
                    rgb(0x00f5_9e0b), // Amber
                    rgb(0x0060_a5fa), // Blue
                    rgb(0x0034_d399), // Emerald
                    rgb(0x00a8_a29e), // Stone
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
                    background: rgb(0x000c_0c0e),
                    grid: Rgba {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.083,
                    },
                    curve_boost: rgb(0x004a_de80),
                    curve_cut: rgb(0x00ef_4444),
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
                    background: rgb(0x000a_0a0c),
                    bass: rgb(0x00f5_9e0b),
                    mids: rgb(0x00fb_bf24),
                    treble: rgb(0x00ef_4444),
                },
                meter_colors: MeterColors {
                    background: rgb(0x000c_0c0e),
                    normal: rgb(0x004a_de80),
                    warning: rgb(0x00fb_923c),
                    clip: rgb(0x00ef_4444),
                    peak: rgb(0x00fa_faf9),
                    text: rgb(0x00a8_a29e),
                },
            },
            feedback: ThemeFeedback {
                meter_normal: rgb(0x004a_de80),
                meter_warning: rgb(0x00fb_923c),
                meter_clip: rgb(0x00ef_4444),
                progress_bar_bg: rgb(0x0024_2428),
                progress_bar_fill: rgb(0x00f5_9e0b),
                toast_success_bg: rgb(0x0014_532d),
                toast_error_bg: rgb(0x0045_0a0a),
                toast_info_bg: rgb(0x000c_2d48),
                toast_warning_bg: rgb(0x0045_1a03),
                peak_indicator: rgb(0x00fa_faf9),
                drag_over_highlight: Rgba {
                    r: 0.961,
                    g: 0.620,
                    b: 0.043,
                    a: 0.251,
                },
                drag_over_border: rgb(0x00f5_9e0b),
                neutral_indicator: rgb(0x00f5_9e0b),
                warning_background: Rgba {
                    r: 0.984,
                    g: 0.573,
                    b: 0.235,
                    a: 0.2,
                },
                knob_color: rgb(0x00fa_faf9),
                optimization_color: rgb(0x00a7_8bfa),
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
            },
            layout: ThemeLayout {
                separator_size: 20.0,
                font_family: None,
                design_tokens: gpui_audio_kit::AudioDesignTokens::default(),
            },
        }
    }
}
