use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Parse a hex color string into (r, g, b, a) as f32 components.
/// Supports: #rrggbb, #rrggbbaa
fn parse_hex(hex: &str) -> (f32, f32, f32, f32) {
    let hex = hex.trim_start_matches('#');
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).expect("valid hex r");
            let g = u8::from_str_radix(&hex[2..4], 16).expect("valid hex g");
            let b = u8::from_str_radix(&hex[4..6], 16).expect("valid hex b");
            (
                f32::from(r) / 255.0,
                f32::from(g) / 255.0,
                f32::from(b) / 255.0,
                1.0,
            )
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).expect("valid hex r");
            let g = u8::from_str_radix(&hex[2..4], 16).expect("valid hex g");
            let b = u8::from_str_radix(&hex[4..6], 16).expect("valid hex b");
            let a = u8::from_str_radix(&hex[6..8], 16).expect("valid hex a");
            (
                f32::from(r) / 255.0,
                f32::from(g) / 255.0,
                f32::from(b) / 255.0,
                f32::from(a) / 255.0,
            )
        }
        _ => panic!("unsupported hex format: #{hex}"),
    }
}

/// Read a color token value from a JSON path like `color.base.background`
fn get_color(theme_obj: &Value, path: &str) -> String {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = theme_obj;
    for part in &parts {
        current = &current[*part];
    }
    let hex = current["$value"]
        .as_str()
        .unwrap_or_else(|| panic!("missing $value at {path}"));
    let (r, g, b, a) = parse_hex(hex);

    // If fully opaque and the hex is 6 chars, use rgb() macro
    if a == 1.0 && hex.trim_start_matches('#').len() == 6 {
        let hex_str = hex.trim_start_matches('#');
        format!("rgb(0x{hex_str})")
    } else if hex.trim_start_matches('#').len() == 8 {
        // 8-char hex: check if we can use the rgba() macro (exact byte values)
        let hex_str = hex.trim_start_matches('#');
        let r_byte = u8::from_str_radix(&hex_str[0..2], 16).unwrap();
        let g_byte = u8::from_str_radix(&hex_str[2..4], 16).unwrap();
        let b_byte = u8::from_str_radix(&hex_str[4..6], 16).unwrap();
        let a_byte = u8::from_str_radix(&hex_str[6..8], 16).unwrap();

        // Check if all components round-trip cleanly through the rgba() macro
        let rf = f32::from(r_byte) / 255.0;
        let gf = f32::from(g_byte) / 255.0;
        let bf = f32::from(b_byte) / 255.0;
        let af = f32::from(a_byte) / 255.0;
        let _ = (rf, gf, bf, af);

        // Use explicit Rgba struct for precision
        format!(
            "Rgba {{ r: {r:.3}, g: {g:.3}, b: {b:.3}, a: {a:.3} }}",
            r = r,
            g = g,
            b = b,
            a = a
        )
    } else {
        format!("Rgba {{ r: {r:.3}, g: {g:.3}, b: {b:.3}, a: {a:.3} }}")
    }
}

/// Read band colors as a Vec of color expressions
fn get_band_colors(theme_obj: &Value) -> Vec<String> {
    let band_obj = &theme_obj["color"]["band"];
    let map = band_obj.as_object().expect("band should be an object");
    let mut entries: Vec<(usize, String)> = map
        .iter()
        .map(|(k, _)| {
            let idx: usize = k.parse().expect("band index should be numeric");
            let expr = get_color(theme_obj, &format!("color.band.{k}"));
            (idx, expr)
        })
        .collect();
    entries.sort_by_key(|(idx, _)| *idx);
    entries.into_iter().map(|(_, expr)| expr).collect()
}

struct ThemeConfig {
    set_name: &'static str,
    fn_name: &'static str,
    file_name: &'static str,
    doc_comment: &'static str,
}

fn theme_configs() -> Vec<ThemeConfig> {
    vec![
        ThemeConfig {
            set_name: "theme/dark",
            fn_name: "dark",
            file_name: "black.rs",
            doc_comment: "Dark theme (default)",
        },
        ThemeConfig {
            set_name: "theme/light",
            fn_name: "light",
            file_name: "light.rs",
            doc_comment: "Light theme",
        },
        ThemeConfig {
            set_name: "theme/midnight",
            fn_name: "midnight",
            file_name: "midnight.rs",
            doc_comment: "Midnight theme (deep blue)",
        },
        ThemeConfig {
            set_name: "theme/forest",
            fn_name: "forest",
            file_name: "forest.rs",
            doc_comment: "Forest theme (green tones)",
        },
        ThemeConfig {
            set_name: "theme/black-and-white",
            fn_name: "black_and_white",
            file_name: "black_and_white.rs",
            doc_comment: "Black & White theme (monochrome high contrast)",
        },
    ]
}

fn generate_theme_file(tokens: &Value, config: &ThemeConfig) -> String {
    let t = &tokens[config.set_name];

    // Check if any color value uses Rgba struct (has alpha != 1.0)
    let has_rgba = needs_rgba_import(t);

    let rgba_use = if has_rgba { "\nuse gpui::Rgba;\n" } else { "" };

    let rgba_import = if has_rgba { ", rgba" } else { "" };

    let band_colors = get_band_colors(t);
    let band_lines: String = band_colors
        .iter()
        .map(|expr| format!("                {expr},\n"))
        .collect();

    format!(
        r#"use super::{{
    EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme, rgb{rgba_import}
}};{rgba_use}

impl Theme {{
    /// {doc}
    pub fn {fn_name}() -> Self {{
        Self {{
            // Base colors
            background: {background},
            background_secondary: {background_secondary},
            background_tertiary: {background_tertiary},
            surface: {surface},
            surface_hover: {surface_hover},
            surface_selected: {surface_selected},

            // Text colors
            text_primary: {text_primary},
            text_secondary: {text_secondary},
            text_muted: {text_muted},
            text_disabled: {text_disabled},

            // Border colors
            border: {border},
            border_focused: {border_focused},

            // Accent colors
            accent: {accent},
            accent_hover: {accent_hover},
            accent_muted: {accent_muted},

            // Text on accent
            text_on_accent: {text_on_accent},
            text_on_accent_muted: {text_on_accent_muted},
            icon_on_accent: {icon_on_accent},

            // Semantic colors
            success: {success},
            warning: {warning},
            error: {error},
            info: {info},

            // Level meter colors
            meter_normal: {meter_normal},
            meter_warning: {meter_warning},
            meter_clip: {meter_clip},

            // Button colors
            button_mute_active: {button_mute_active},
            button_solo_active: {button_solo_active},
            button_dim_active: {button_dim_active},

            // Playback bar
            progress_bar_bg: {progress_bar_bg},
            progress_bar_fill: {progress_bar_fill},

            // Toast backgrounds
            toast_success_bg: {toast_success_bg},
            toast_error_bg: {toast_error_bg},
            toast_info_bg: {toast_info_bg},
            toast_warning_bg: {toast_warning_bg},

            // Plugin colors
            plugin_colors: PluginColorMap {{
                eq: {plugin_eq},
                gain: {plugin_gain},
                upmixer: {plugin_upmixer},
                compressor: {plugin_compressor},
                limiter: {plugin_limiter},
                gate: {plugin_gate},
                loudness: {plugin_loudness},
                binaural: {plugin_binaural},
                convolution: {plugin_convolution},
                monitor: {plugin_monitor},
                spectrum: {plugin_spectrum},
                mute_solo: {plugin_mute_solo},
            }},
            graph_colors: GraphLineColors {{
                input: {graph_input},
                target: {graph_target},
                filter_response: {graph_filter_response},
                corrected: {graph_corrected},
                error: {graph_error},
                deviation: {graph_deviation},
                grid: {graph_grid},
                secondary_line: {graph_secondary_line},
                directivity_er: {graph_directivity_er},
                directivity_sp: {graph_directivity_sp},
            }},
            band_colors: vec![
{band_lines}            ],
            eq_curve_colors: EQCurveColors {{
                background: {eq_background},
                grid: {eq_grid},
                curve_boost: {eq_curve_boost},
                curve_cut: {eq_curve_cut},
                fill_boost: {eq_fill_boost},
                fill_cut: {eq_fill_cut},
                zero_line: {eq_zero_line},
            }},
            spectrum_colors: SpectrumColors {{
                background: {spectrum_background},
                bass: {spectrum_bass},
                mids: {spectrum_mids},
                treble: {spectrum_treble},
            }},
            meter_colors: MeterColors {{
                background: {meter_background},
                normal: {meter_normal_c},
                warning: {meter_warning_c},
                clip: {meter_clip_c},
                peak: {meter_peak},
                text: {meter_text},
            }},

            // Additional semantic colors
            peak_indicator: {peak_indicator},
            drag_over_highlight: {drag_over_highlight},
            drag_over_border: {drag_over_border},
            neutral_indicator: {neutral_indicator},
            warning_background: {warning_background},
            knob_color: {knob_color},
            optimization_color: {optimization_color},
            grid_color: {grid_color},
            overlay_bg: {overlay_bg},

            // Layout sizes
            separator_size: 20.0,

            // Font family
            font_family: Some("B612".into()),
        }}
    }}
}}
"#,
        doc = config.doc_comment,
        fn_name = config.fn_name,
        rgba_import = rgba_import,
        rgba_use = rgba_use,
        // Base
        background = get_color(t, "color.base.background"),
        background_secondary = get_color(t, "color.base.backgroundSecondary"),
        background_tertiary = get_color(t, "color.base.backgroundTertiary"),
        surface = get_color(t, "color.base.surface"),
        surface_hover = get_color(t, "color.base.surfaceHover"),
        surface_selected = get_color(t, "color.base.surfaceSelected"),
        // Text
        text_primary = get_color(t, "color.text.primary"),
        text_secondary = get_color(t, "color.text.secondary"),
        text_muted = get_color(t, "color.text.muted"),
        text_disabled = get_color(t, "color.text.disabled"),
        text_on_accent = get_color(t, "color.text.onAccent"),
        text_on_accent_muted = get_color(t, "color.text.onAccentMuted"),
        icon_on_accent = get_color(t, "color.text.iconOnAccent"),
        // Border
        border = get_color(t, "color.border.default"),
        border_focused = get_color(t, "color.border.focused"),
        // Accent
        accent = get_color(t, "color.accent.default"),
        accent_hover = get_color(t, "color.accent.hover"),
        accent_muted = get_color(t, "color.accent.muted"),
        // Semantic
        success = get_color(t, "color.semantic.success"),
        warning = get_color(t, "color.semantic.warning"),
        error = get_color(t, "color.semantic.error"),
        info = get_color(t, "color.semantic.info"),
        // Meter
        meter_normal = get_color(t, "color.meter.normal"),
        meter_warning = get_color(t, "color.meter.warning"),
        meter_clip = get_color(t, "color.meter.clip"),
        // Button
        button_mute_active = get_color(t, "color.button.muteActive"),
        button_solo_active = get_color(t, "color.button.soloActive"),
        button_dim_active = get_color(t, "color.button.dimActive"),
        // Playback
        progress_bar_bg = get_color(t, "color.playback.progressBarBg"),
        progress_bar_fill = get_color(t, "color.playback.progressBarFill"),
        // Toast
        toast_success_bg = get_color(t, "color.toast.successBg"),
        toast_error_bg = get_color(t, "color.toast.errorBg"),
        toast_info_bg = get_color(t, "color.toast.infoBg"),
        toast_warning_bg = get_color(t, "color.toast.warningBg"),
        // Plugin
        plugin_eq = get_color(t, "color.plugin.eq"),
        plugin_gain = get_color(t, "color.plugin.gain"),
        plugin_upmixer = get_color(t, "color.plugin.upmixer"),
        plugin_compressor = get_color(t, "color.plugin.compressor"),
        plugin_limiter = get_color(t, "color.plugin.limiter"),
        plugin_gate = get_color(t, "color.plugin.gate"),
        plugin_loudness = get_color(t, "color.plugin.loudness"),
        plugin_binaural = get_color(t, "color.plugin.binaural"),
        plugin_convolution = get_color(t, "color.plugin.convolution"),
        plugin_monitor = get_color(t, "color.plugin.monitor"),
        plugin_spectrum = get_color(t, "color.plugin.spectrum"),
        plugin_mute_solo = get_color(t, "color.plugin.muteSolo"),
        // Graph
        graph_input = get_color(t, "color.graph.input"),
        graph_target = get_color(t, "color.graph.target"),
        graph_filter_response = get_color(t, "color.graph.filterResponse"),
        graph_corrected = get_color(t, "color.graph.corrected"),
        graph_error = get_color(t, "color.graph.error"),
        graph_deviation = get_color(t, "color.graph.deviation"),
        graph_grid = get_color(t, "color.graph.grid"),
        graph_secondary_line = get_color(t, "color.graph.secondaryLine"),
        graph_directivity_er = get_color(t, "color.graph.directivityEr"),
        graph_directivity_sp = get_color(t, "color.graph.directivitySp"),
        // Band
        band_lines = band_lines,
        // EQ Curve
        eq_background = get_color(t, "color.eqCurve.background"),
        eq_grid = get_color(t, "color.eqCurve.grid"),
        eq_curve_boost = get_color(t, "color.eqCurve.curveBoost"),
        eq_curve_cut = get_color(t, "color.eqCurve.curveCut"),
        eq_fill_boost = get_color(t, "color.eqCurve.fillBoost"),
        eq_fill_cut = get_color(t, "color.eqCurve.fillCut"),
        eq_zero_line = get_color(t, "color.eqCurve.zeroLine"),
        // Spectrum
        spectrum_background = get_color(t, "color.spectrum.background"),
        spectrum_bass = get_color(t, "color.spectrum.bass"),
        spectrum_mids = get_color(t, "color.spectrum.mids"),
        spectrum_treble = get_color(t, "color.spectrum.treble"),
        // Meter colors
        meter_background = get_color(t, "color.meterColors.background"),
        meter_normal_c = get_color(t, "color.meterColors.normal"),
        meter_warning_c = get_color(t, "color.meterColors.warning"),
        meter_clip_c = get_color(t, "color.meterColors.clip"),
        meter_peak = get_color(t, "color.meterColors.peak"),
        meter_text = get_color(t, "color.meterColors.text"),
        // Additional
        peak_indicator = get_color(t, "color.additional.peakIndicator"),
        drag_over_highlight = get_color(t, "color.additional.dragOverHighlight"),
        drag_over_border = get_color(t, "color.additional.dragOverBorder"),
        neutral_indicator = get_color(t, "color.additional.neutralIndicator"),
        warning_background = get_color(t, "color.additional.warningBackground"),
        knob_color = get_color(t, "color.additional.knobColor"),
        optimization_color = get_color(t, "color.additional.optimizationColor"),
        grid_color = get_color(t, "color.additional.gridColor"),
        overlay_bg = get_color(t, "color.additional.overlayBg"),
    )
}

/// Check if a theme set has any colors with alpha != 1.0 (needs Rgba import)
fn needs_rgba_import(theme_obj: &Value) -> bool {
    fn check_value(v: &Value) -> bool {
        if let Some(hex) = v.get("$value").and_then(Value::as_str) {
            let hex = hex.trim_start_matches('#');
            return hex.len() == 8;
        }
        if let Some(obj) = v.as_object() {
            return obj.values().any(check_value);
        }
        false
    }
    check_value(theme_obj)
}

fn main() {
    let tokens_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of crates/import-design-tokens")
        .parent()
        .expect("workspace root")
        .join("design-tokens")
        .join("tokens.json");

    let tokens_str = std::fs::read_to_string(&tokens_path).expect("read tokens.json");
    let tokens: Value = serde_json::from_str(&tokens_str).expect("parse tokens.json");

    let theme_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of crates/import-design-tokens")
        .join("app-gpui")
        .join("app")
        .join("theme");

    let mut generated = HashMap::new();

    for config in theme_configs() {
        let content = generate_theme_file(&tokens, &config);
        let out_path = theme_dir.join(config.file_name);
        generated.insert(config.file_name, out_path.clone());
        std::fs::write(&out_path, content.as_bytes()).expect("write theme file");
        println!("Wrote {}", out_path.display());
    }

    println!(
        "\nGenerated {} theme files from {}",
        generated.len(),
        tokens_path.display()
    );
}
