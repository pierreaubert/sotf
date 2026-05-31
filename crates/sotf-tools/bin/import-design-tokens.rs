use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use sotf_audio_player_gpui::theme::ThemeId;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

fn hex_nibble(ch: char, component: &str, display: &str) -> Result<u8> {
    ch.to_digit(16)
        .map(|n| n as u8)
        .ok_or_else(|| anyhow!("invalid hex ({component}) in {display}: '{ch}'"))
}

fn expand_hex_nibble(ch: char, component: &str, display: &str) -> Result<u8> {
    let n = hex_nibble(ch, component, display)?;
    Ok((n << 4) | n)
}

fn hex_byte(chars: &[char], index: usize, component: &str, display: &str) -> Result<u8> {
    let hi = hex_nibble(chars[index], component, display)?;
    let lo = hex_nibble(chars[index + 1], component, display)?;
    Ok((hi << 4) | lo)
}

/// Parse a hex color string into RGBA bytes.
/// Supports CSS hex forms: `#rgb`, `#rgba`, `#rrggbb`, and `#rrggbbaa`
/// with or without the leading `#`.
fn parse_hex_bytes(hex: &str) -> Result<(u8, u8, u8, u8)> {
    let raw = hex.trim().trim_start_matches('#');
    if raw.is_empty() {
        bail!("empty hex color string");
    }
    let display = format!("#{raw}");
    let chars: Vec<char> = raw.chars().collect();
    match chars.len() {
        3 => Ok((
            expand_hex_nibble(chars[0], "r", &display)?,
            expand_hex_nibble(chars[1], "g", &display)?,
            expand_hex_nibble(chars[2], "b", &display)?,
            255,
        )),
        4 => Ok((
            expand_hex_nibble(chars[0], "r", &display)?,
            expand_hex_nibble(chars[1], "g", &display)?,
            expand_hex_nibble(chars[2], "b", &display)?,
            expand_hex_nibble(chars[3], "a", &display)?,
        )),
        6 => {
            let r = hex_byte(&chars, 0, "r", &display)?;
            let g = hex_byte(&chars, 2, "g", &display)?;
            let b = hex_byte(&chars, 4, "b", &display)?;
            Ok((r, g, b, 255))
        }
        8 => {
            let r = hex_byte(&chars, 0, "r", &display)?;
            let g = hex_byte(&chars, 2, "g", &display)?;
            let b = hex_byte(&chars, 4, "b", &display)?;
            let a = hex_byte(&chars, 6, "a", &display)?;
            Ok((r, g, b, a))
        }
        n => bail!("unsupported hex length {n} in {display} (expected 3, 4, 6, or 8)"),
    }
}

/// Parse a hex color string into (r, g, b, a) as f32 components.
///
/// Returns a structured error rather than panicking so a malformed
/// `tokens.json` produces an actionable build-time message.
fn parse_hex(hex: &str) -> Result<(f32, f32, f32, f32)> {
    let (r, g, b, a) = parse_hex_bytes(hex)?;
    Ok((
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        f32::from(a) / 255.0,
    ))
}

/// Read a color token value from a JSON path like `color.base.background`
fn get_color(theme_obj: &Value, path: &str) -> Result<String> {
    let mut current = theme_obj;
    for part in path.split('.') {
        current = current
            .get(part)
            .ok_or_else(|| anyhow!("missing token path {path} at component '{part}'"))?;
    }
    let hex = current["$value"]
        .as_str()
        .ok_or_else(|| anyhow!("missing $value at {path}"))?;
    let (r, g, b, a) = parse_hex(hex).with_context(|| format!("parsing color at {path}"))?;
    let (r8, g8, b8, a8) = (
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
        (a * 255.0).round() as u8,
    );

    if a8 == 255 {
        Ok(format!("rgb(0x{r8:02x}{g8:02x}{b8:02x})"))
    } else {
        Ok(format!(
            "Rgba {{ r: {r:.3}, g: {g:.3}, b: {b:.3}, a: {a:.3} }}"
        ))
    }
}

/// Read band colors as a Vec of color expressions
fn get_band_colors(theme_obj: &Value) -> Result<Vec<String>> {
    let band_obj = &theme_obj["color"]["band"];
    let map = band_obj
        .as_object()
        .ok_or_else(|| anyhow!("color.band should be an object"))?;
    let mut entries: Vec<(usize, String)> = Vec::with_capacity(map.len());
    for (k, _) in map.iter() {
        let idx: usize = k
            .parse()
            .with_context(|| format!("band index '{k}' is not numeric"))?;
        let expr = get_color(theme_obj, &format!("color.band.{k}"))?;
        entries.push((idx, expr));
    }
    entries.sort_by_key(|(idx, _)| *idx);
    Ok(entries.into_iter().map(|(_, expr)| expr).collect())
}

#[derive(Clone, Copy)]
struct ThemeConfig {
    set_name: &'static str,
    fn_name: &'static str,
    file_name: &'static str,
    doc_comment: &'static str,
}

/// Derive the per-theme import config from a `ThemeId`.
///
/// This is the single source of truth for the round-trip mapping between
/// the JSON `set_name` produced by `export-design-tokens.rs` and the
/// `Theme::*` Rust function emitted into `app-gpui/app/theme/*.rs`.
fn theme_config_for(id: ThemeId) -> ThemeConfig {
    match id {
        ThemeId::Dark => ThemeConfig {
            set_name: "theme/dark",
            fn_name: "dark",
            file_name: "black.rs",
            doc_comment: "Dark theme (default)",
        },
        ThemeId::Light => ThemeConfig {
            set_name: "theme/light",
            fn_name: "light",
            file_name: "light.rs",
            doc_comment: "Light theme",
        },
        ThemeId::Midnight => ThemeConfig {
            set_name: "theme/midnight",
            fn_name: "midnight",
            file_name: "midnight.rs",
            doc_comment: "Midnight theme (deep blue)",
        },
        ThemeId::Forest => ThemeConfig {
            set_name: "theme/forest",
            fn_name: "forest",
            file_name: "forest.rs",
            doc_comment: "Forest theme (green tones)",
        },
        ThemeId::BlackAndWhite => ThemeConfig {
            set_name: "theme/black-and-white",
            fn_name: "black_and_white",
            file_name: "black_and_white.rs",
            doc_comment: "Black & White theme (monochrome high contrast)",
        },
        ThemeId::Onyx => ThemeConfig {
            set_name: "theme/onyx",
            fn_name: "onyx",
            file_name: "onyx.rs",
            doc_comment: "Onyx theme",
        },
        ThemeId::Protanopia => ThemeConfig {
            set_name: "theme/protanopia",
            fn_name: "protanopia",
            file_name: "accessible.rs",
            doc_comment: "Protanopia-safe dark theme",
        },
        ThemeId::Deuteranopia => ThemeConfig {
            set_name: "theme/deuteranopia",
            fn_name: "deuteranopia",
            file_name: "accessible.rs",
            doc_comment: "Deuteranopia-safe dark theme",
        },
        ThemeId::Tritanopia => ThemeConfig {
            set_name: "theme/tritanopia",
            fn_name: "tritanopia",
            file_name: "accessible.rs",
            doc_comment: "Tritanopia-safe dark theme",
        },
    }
}

fn theme_configs() -> Vec<ThemeConfig> {
    ThemeId::all()
        .iter()
        .copied()
        .map(theme_config_for)
        .collect()
}

fn generate_theme_file(tokens: &Value, config: &ThemeConfig) -> Result<String> {
    let t = &tokens[config.set_name];
    if t.is_null() {
        bail!(
            "theme set '{}' not found in tokens.json (export must include it)",
            config.set_name
        );
    }

    // Check if any color value uses Rgba struct (has alpha != 1.0)
    let has_rgba = needs_rgba_import(t);

    let rgba_use = if has_rgba { "\nuse gpui::Rgba;\n" } else { "" };

    let rgba_import = if has_rgba { ", rgba" } else { "" };

    let band_colors = get_band_colors(t)?;
    let band_lines: String = band_colors
        .iter()
        .map(|expr| format!("                {expr},\n"))
        .collect();

    Ok(format!(
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
        background = get_color(t, "color.base.background")?,
        background_secondary = get_color(t, "color.base.backgroundSecondary")?,
        background_tertiary = get_color(t, "color.base.backgroundTertiary")?,
        surface = get_color(t, "color.base.surface")?,
        surface_hover = get_color(t, "color.base.surfaceHover")?,
        surface_selected = get_color(t, "color.base.surfaceSelected")?,
        // Text
        text_primary = get_color(t, "color.text.primary")?,
        text_secondary = get_color(t, "color.text.secondary")?,
        text_muted = get_color(t, "color.text.muted")?,
        text_disabled = get_color(t, "color.text.disabled")?,
        text_on_accent = get_color(t, "color.text.onAccent")?,
        text_on_accent_muted = get_color(t, "color.text.onAccentMuted")?,
        icon_on_accent = get_color(t, "color.text.iconOnAccent")?,
        // Border
        border = get_color(t, "color.border.default")?,
        border_focused = get_color(t, "color.border.focused")?,
        // Accent
        accent = get_color(t, "color.accent.default")?,
        accent_hover = get_color(t, "color.accent.hover")?,
        accent_muted = get_color(t, "color.accent.muted")?,
        // Semantic
        success = get_color(t, "color.semantic.success")?,
        warning = get_color(t, "color.semantic.warning")?,
        error = get_color(t, "color.semantic.error")?,
        info = get_color(t, "color.semantic.info")?,
        // Meter
        meter_normal = get_color(t, "color.meter.normal")?,
        meter_warning = get_color(t, "color.meter.warning")?,
        meter_clip = get_color(t, "color.meter.clip")?,
        // Button
        button_mute_active = get_color(t, "color.button.muteActive")?,
        button_solo_active = get_color(t, "color.button.soloActive")?,
        button_dim_active = get_color(t, "color.button.dimActive")?,
        // Playback
        progress_bar_bg = get_color(t, "color.playback.progressBarBg")?,
        progress_bar_fill = get_color(t, "color.playback.progressBarFill")?,
        // Toast
        toast_success_bg = get_color(t, "color.toast.successBg")?,
        toast_error_bg = get_color(t, "color.toast.errorBg")?,
        toast_info_bg = get_color(t, "color.toast.infoBg")?,
        toast_warning_bg = get_color(t, "color.toast.warningBg")?,
        // Plugin
        plugin_eq = get_color(t, "color.plugin.eq")?,
        plugin_gain = get_color(t, "color.plugin.gain")?,
        plugin_upmixer = get_color(t, "color.plugin.upmixer")?,
        plugin_compressor = get_color(t, "color.plugin.compressor")?,
        plugin_limiter = get_color(t, "color.plugin.limiter")?,
        plugin_gate = get_color(t, "color.plugin.gate")?,
        plugin_loudness = get_color(t, "color.plugin.loudness")?,
        plugin_binaural = get_color(t, "color.plugin.binaural")?,
        plugin_convolution = get_color(t, "color.plugin.convolution")?,
        plugin_monitor = get_color(t, "color.plugin.monitor")?,
        plugin_spectrum = get_color(t, "color.plugin.spectrum")?,
        plugin_mute_solo = get_color(t, "color.plugin.muteSolo")?,
        // Graph
        graph_input = get_color(t, "color.graph.input")?,
        graph_target = get_color(t, "color.graph.target")?,
        graph_filter_response = get_color(t, "color.graph.filterResponse")?,
        graph_corrected = get_color(t, "color.graph.corrected")?,
        graph_error = get_color(t, "color.graph.error")?,
        graph_deviation = get_color(t, "color.graph.deviation")?,
        graph_grid = get_color(t, "color.graph.grid")?,
        graph_secondary_line = get_color(t, "color.graph.secondaryLine")?,
        graph_directivity_er = get_color(t, "color.graph.directivityEr")?,
        graph_directivity_sp = get_color(t, "color.graph.directivitySp")?,
        // Band
        band_lines = band_lines,
        // EQ Curve
        eq_background = get_color(t, "color.eqCurve.background")?,
        eq_grid = get_color(t, "color.eqCurve.grid")?,
        eq_curve_boost = get_color(t, "color.eqCurve.curveBoost")?,
        eq_curve_cut = get_color(t, "color.eqCurve.curveCut")?,
        eq_fill_boost = get_color(t, "color.eqCurve.fillBoost")?,
        eq_fill_cut = get_color(t, "color.eqCurve.fillCut")?,
        eq_zero_line = get_color(t, "color.eqCurve.zeroLine")?,
        // Spectrum
        spectrum_background = get_color(t, "color.spectrum.background")?,
        spectrum_bass = get_color(t, "color.spectrum.bass")?,
        spectrum_mids = get_color(t, "color.spectrum.mids")?,
        spectrum_treble = get_color(t, "color.spectrum.treble")?,
        // Meter colors
        meter_background = get_color(t, "color.meterColors.background")?,
        meter_normal_c = get_color(t, "color.meterColors.normal")?,
        meter_warning_c = get_color(t, "color.meterColors.warning")?,
        meter_clip_c = get_color(t, "color.meterColors.clip")?,
        meter_peak = get_color(t, "color.meterColors.peak")?,
        meter_text = get_color(t, "color.meterColors.text")?,
        // Additional
        peak_indicator = get_color(t, "color.additional.peakIndicator")?,
        drag_over_highlight = get_color(t, "color.additional.dragOverHighlight")?,
        drag_over_border = get_color(t, "color.additional.dragOverBorder")?,
        neutral_indicator = get_color(t, "color.additional.neutralIndicator")?,
        warning_background = get_color(t, "color.additional.warningBackground")?,
        knob_color = get_color(t, "color.additional.knobColor")?,
        optimization_color = get_color(t, "color.additional.optimizationColor")?,
        grid_color = get_color(t, "color.additional.gridColor")?,
        overlay_bg = get_color(t, "color.additional.overlayBg")?,
    ))
}

fn generate_theme_file_group(tokens: &Value, configs: &[ThemeConfig]) -> Result<String> {
    if let [config] = configs {
        return generate_theme_file(tokens, config);
    }

    let has_rgba = configs.iter().try_fold(false, |has_rgba, config| {
        let t = &tokens[config.set_name];
        if t.is_null() {
            bail!(
                "theme set '{}' not found in tokens.json (export must include it)",
                config.set_name
            );
        }
        Ok::<_, anyhow::Error>(has_rgba || needs_rgba_import(t))
    })?;

    let rgba_use = if has_rgba { "\nuse gpui::Rgba;\n" } else { "" };
    let rgba_import = if has_rgba { ", rgba" } else { "" };
    let mut methods = String::new();

    for config in configs {
        let generated = generate_theme_file(tokens, config)
            .with_context(|| format!("generating {}", config.fn_name))?;
        methods.push_str(
            generated_theme_methods(&generated)
                .with_context(|| format!("extracting {}", config.fn_name))?,
        );
        if !methods.ends_with('\n') {
            methods.push('\n');
        }
    }

    Ok(format!(
        r#"use super::{{
    EQCurveColors, GraphLineColors, MeterColors, PluginColorMap, SpectrumColors, Theme, rgb{rgba_import}
}};{rgba_use}

impl Theme {{
{methods}}}
"#,
        rgba_import = rgba_import,
        rgba_use = rgba_use,
        methods = methods,
    ))
}

fn generated_theme_methods(generated: &str) -> Result<&str> {
    let marker = "impl Theme {\n";
    let start = generated
        .find(marker)
        .ok_or_else(|| anyhow!("generated theme file missing impl block"))?
        + marker.len();
    let end = generated
        .rfind("\n}\n")
        .ok_or_else(|| anyhow!("generated theme file missing impl terminator"))?;
    Ok(&generated[start..end])
}

/// Check if a theme set has any colors with alpha != 1.0 (needs Rgba import)
fn needs_rgba_import(theme_obj: &Value) -> bool {
    fn check_value(v: &Value) -> bool {
        if let Some(hex) = v.get("$value").and_then(Value::as_str) {
            return parse_hex_bytes(hex)
                .map(|(_, _, _, a)| a != 255)
                .unwrap_or(false);
        }
        if let Some(obj) = v.as_object() {
            return obj.values().any(check_value);
        }
        false
    }
    check_value(theme_obj)
}

fn main() -> Result<()> {
    let tokens_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| anyhow!("parent of crates/sotf-tools"))?
        .parent()
        .ok_or_else(|| anyhow!("workspace root"))?
        .join("design-tokens")
        .join("tokens.json");

    let tokens_str = std::fs::read_to_string(&tokens_path)
        .with_context(|| format!("read {}", tokens_path.display()))?;
    let tokens: Value = serde_json::from_str(&tokens_str)
        .with_context(|| format!("parse {}", tokens_path.display()))?;

    let theme_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| anyhow!("parent of crates/sotf-tools"))?
        .join("app-gpui")
        .join("app")
        .join("theme");

    let mut generated = HashMap::new();
    let mut configs_by_file: BTreeMap<&'static str, Vec<ThemeConfig>> = BTreeMap::new();

    for config in theme_configs() {
        configs_by_file
            .entry(config.file_name)
            .or_default()
            .push(config);
    }

    for (file_name, configs) in configs_by_file {
        let content = generate_theme_file_group(&tokens, &configs)
            .with_context(|| format!("generating {file_name}"))?;
        let out_path = theme_dir.join(file_name);
        generated.insert(file_name, out_path.clone());
        std::fs::write(&out_path, content.as_bytes())
            .with_context(|| format!("write {}", out_path.display()))?;
        println!("Wrote {}", out_path.display());
    }

    println!(
        "\nGenerated {} theme files from {}",
        generated.len(),
        tokens_path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sotf_audio_player_gpui::theme::{Theme, ThemeId};

    #[test]
    fn parse_hex_rrggbb() {
        let (r, g, b, a) = parse_hex("#ff8000").unwrap();
        assert!((r - 1.0).abs() < 1e-6);
        assert!((g - 128.0 / 255.0).abs() < 1e-6);
        assert!((b - 0.0).abs() < 1e-6);
        assert!((a - 1.0).abs() < 1e-6);
    }

    #[test]
    fn parse_hex_rrggbbaa_leading_hash() {
        let (r, g, b, a) = parse_hex("#11223344").unwrap();
        assert!((r - 0x11 as f32 / 255.0).abs() < 1e-6);
        assert!((g - 0x22 as f32 / 255.0).abs() < 1e-6);
        assert!((b - 0x33 as f32 / 255.0).abs() < 1e-6);
        assert!((a - 0x44 as f32 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn parse_hex_rgb_shorthand() {
        let (r, g, b, a) = parse_hex("#f80").unwrap();
        assert!((r - 1.0).abs() < 1e-6);
        assert!((g - 0x88 as f32 / 255.0).abs() < 1e-6);
        assert!((b - 0.0).abs() < 1e-6);
        assert!((a - 1.0).abs() < 1e-6);
    }

    #[test]
    fn parse_hex_rgba_shorthand() {
        let (r, g, b, a) = parse_hex("#1234").unwrap();
        assert!((r - 0x11 as f32 / 255.0).abs() < 1e-6);
        assert!((g - 0x22 as f32 / 255.0).abs() < 1e-6);
        assert!((b - 0x33 as f32 / 255.0).abs() < 1e-6);
        assert!((a - 0x44 as f32 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn parse_hex_no_leading_hash() {
        // No leading '#': should still parse successfully.
        let (r, _g, _b, a) = parse_hex("ff0000").unwrap();
        assert!((r - 1.0).abs() < 1e-6);
        assert!((a - 1.0).abs() < 1e-6);
    }

    #[test]
    fn parse_hex_too_short() {
        let err = parse_hex("#ab").unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("unsupported hex length"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn parse_hex_non_hex() {
        // 6 chars but with invalid hex digits.
        let err = parse_hex("#zzzzzz").unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("invalid hex"), "unexpected error: {msg}");
    }

    #[test]
    fn parse_hex_empty() {
        assert!(parse_hex("").is_err());
        assert!(parse_hex("#").is_err());
    }

    #[test]
    fn get_color_expands_opaque_shorthand_to_rgb_macro() {
        let tokens = json!({
            "color": {
                "base": {
                    "background": { "$type": "color", "$value": "#f80" }
                }
            }
        });

        assert_eq!(
            get_color(&tokens, "color.base.background").unwrap(),
            "rgb(0xff8800)"
        );
    }

    #[test]
    fn get_color_reports_missing_path_component() {
        let tokens = json!({ "color": { "base": {} } });
        let err = get_color(&tokens, "color.base.background").unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("missing token path color.base.background at component 'background'"),
            "unexpected error: {msg}"
        );
    }

    /// Onyx round-trip regression: every theme exposed by `ThemeId::all()`
    /// must have a corresponding `theme_config_for` entry whose `set_name`
    /// matches what `export-design-tokens.rs` writes, and `generate_theme_file`
    /// must succeed for it.
    #[test]
    fn theme_configs_cover_all_themes() {
        let configs = theme_configs();
        assert_eq!(
            configs.len(),
            ThemeId::all().len(),
            "every ThemeId must produce a ThemeConfig"
        );

        // The export uses exactly these set names (kept in sync with
        // bin/export-design-tokens.rs::theme_set_name).
        let expected: Vec<&'static str> = ThemeId::all()
            .iter()
            .map(|id| match id {
                ThemeId::Dark => "theme/dark",
                ThemeId::Light => "theme/light",
                ThemeId::Midnight => "theme/midnight",
                ThemeId::Forest => "theme/forest",
                ThemeId::BlackAndWhite => "theme/black-and-white",
                ThemeId::Onyx => "theme/onyx",
                ThemeId::Protanopia => "theme/protanopia",
                ThemeId::Deuteranopia => "theme/deuteranopia",
                ThemeId::Tritanopia => "theme/tritanopia",
            })
            .collect();
        let got: Vec<&'static str> = configs.iter().map(|c| c.set_name).collect();
        assert_eq!(got, expected);

        // Onyx specifically — the bug we are guarding against.
        assert!(
            configs.iter().any(|c| c.set_name == "theme/onyx"),
            "Onyx theme must be present in theme_configs()"
        );
    }

    /// Build a minimal JSON object for a single theme by inlining the
    /// `Rgba` components of every field touched by `generate_theme_file`.
    /// We borrow the live Onyx theme so the test fails if the schema drifts.
    fn export_theme_minimal_to_json(theme: &Theme) -> Value {
        fn c(rgba: gpui::Rgba) -> Value {
            let ri = (rgba.r * 255.0).round() as u8;
            let gi = (rgba.g * 255.0).round() as u8;
            let bi = (rgba.b * 255.0).round() as u8;
            let ai = (rgba.a * 255.0).round() as u8;
            let hex = if ai == 255 {
                format!("#{ri:02x}{gi:02x}{bi:02x}")
            } else {
                format!("#{ri:02x}{gi:02x}{bi:02x}{ai:02x}")
            };
            json!({ "$type": "color", "$value": hex })
        }
        let band_map: serde_json::Map<String, Value> = theme
            .band_colors
            .iter()
            .enumerate()
            .map(|(i, color)| (i.to_string(), c(*color)))
            .collect();
        json!({
            "color": {
                "base": {
                    "background": c(theme.background),
                    "backgroundSecondary": c(theme.background_secondary),
                    "backgroundTertiary": c(theme.background_tertiary),
                    "surface": c(theme.surface),
                    "surfaceHover": c(theme.surface_hover),
                    "surfaceSelected": c(theme.surface_selected)
                },
                "text": {
                    "primary": c(theme.text_primary),
                    "secondary": c(theme.text_secondary),
                    "muted": c(theme.text_muted),
                    "disabled": c(theme.text_disabled),
                    "onAccent": c(theme.text_on_accent),
                    "onAccentMuted": c(theme.text_on_accent_muted),
                    "iconOnAccent": c(theme.icon_on_accent)
                },
                "border": {
                    "default": c(theme.border),
                    "focused": c(theme.border_focused)
                },
                "accent": {
                    "default": c(theme.accent),
                    "hover": c(theme.accent_hover),
                    "muted": c(theme.accent_muted)
                },
                "semantic": {
                    "success": c(theme.success),
                    "warning": c(theme.warning),
                    "error": c(theme.error),
                    "info": c(theme.info)
                },
                "meter": {
                    "normal": c(theme.meter_normal),
                    "warning": c(theme.meter_warning),
                    "clip": c(theme.meter_clip)
                },
                "button": {
                    "muteActive": c(theme.button_mute_active),
                    "soloActive": c(theme.button_solo_active),
                    "dimActive": c(theme.button_dim_active)
                },
                "playback": {
                    "progressBarBg": c(theme.progress_bar_bg),
                    "progressBarFill": c(theme.progress_bar_fill)
                },
                "toast": {
                    "successBg": c(theme.toast_success_bg),
                    "errorBg": c(theme.toast_error_bg),
                    "infoBg": c(theme.toast_info_bg),
                    "warningBg": c(theme.toast_warning_bg)
                },
                "plugin": {
                    "eq": c(theme.plugin_colors.eq),
                    "gain": c(theme.plugin_colors.gain),
                    "upmixer": c(theme.plugin_colors.upmixer),
                    "compressor": c(theme.plugin_colors.compressor),
                    "limiter": c(theme.plugin_colors.limiter),
                    "gate": c(theme.plugin_colors.gate),
                    "loudness": c(theme.plugin_colors.loudness),
                    "binaural": c(theme.plugin_colors.binaural),
                    "convolution": c(theme.plugin_colors.convolution),
                    "monitor": c(theme.plugin_colors.monitor),
                    "spectrum": c(theme.plugin_colors.spectrum),
                    "muteSolo": c(theme.plugin_colors.mute_solo)
                },
                "graph": {
                    "input": c(theme.graph_colors.input),
                    "target": c(theme.graph_colors.target),
                    "filterResponse": c(theme.graph_colors.filter_response),
                    "corrected": c(theme.graph_colors.corrected),
                    "error": c(theme.graph_colors.error),
                    "deviation": c(theme.graph_colors.deviation),
                    "grid": c(theme.graph_colors.grid),
                    "secondaryLine": c(theme.graph_colors.secondary_line),
                    "directivityEr": c(theme.graph_colors.directivity_er),
                    "directivitySp": c(theme.graph_colors.directivity_sp)
                },
                "band": Value::Object(band_map),
                "eqCurve": {
                    "background": c(theme.eq_curve_colors.background),
                    "grid": c(theme.eq_curve_colors.grid),
                    "curveBoost": c(theme.eq_curve_colors.curve_boost),
                    "curveCut": c(theme.eq_curve_colors.curve_cut),
                    "fillBoost": c(theme.eq_curve_colors.fill_boost),
                    "fillCut": c(theme.eq_curve_colors.fill_cut),
                    "zeroLine": c(theme.eq_curve_colors.zero_line)
                },
                "spectrum": {
                    "background": c(theme.spectrum_colors.background),
                    "bass": c(theme.spectrum_colors.bass),
                    "mids": c(theme.spectrum_colors.mids),
                    "treble": c(theme.spectrum_colors.treble)
                },
                "meterColors": {
                    "background": c(theme.meter_colors.background),
                    "normal": c(theme.meter_colors.normal),
                    "warning": c(theme.meter_colors.warning),
                    "clip": c(theme.meter_colors.clip),
                    "peak": c(theme.meter_colors.peak),
                    "text": c(theme.meter_colors.text)
                },
                "additional": {
                    "peakIndicator": c(theme.peak_indicator),
                    "dragOverHighlight": c(theme.drag_over_highlight),
                    "dragOverBorder": c(theme.drag_over_border),
                    "neutralIndicator": c(theme.neutral_indicator),
                    "warningBackground": c(theme.warning_background),
                    "knobColor": c(theme.knob_color),
                    "optimizationColor": c(theme.optimization_color),
                    "gridColor": c(theme.grid_color),
                    "overlayBg": c(theme.overlay_bg)
                }
            }
        })
    }

    /// Onyx import/export round-trip regression test.
    ///
    /// Exports the Onyx theme to the tokens.json JSON shape, then runs the
    /// importer's `generate_theme_file` over it. The previous bug silently
    /// dropped the Onyx theme on import; we assert here that:
    /// 1. `theme_configs()` produces an Onyx entry, and
    /// 2. `generate_theme_file` succeeds and emits a non-empty `Theme::onyx()`
    ///    function definition.
    #[test]
    fn onyx_round_trip() {
        let onyx_theme = Theme::from_id(ThemeId::Onyx);
        let onyx_json = export_theme_minimal_to_json(&onyx_theme);

        // Wrap under the same `set_name` the exporter uses.
        let tokens = json!({ "theme/onyx": onyx_json });

        let cfg = theme_config_for(ThemeId::Onyx);
        assert_eq!(cfg.set_name, "theme/onyx");
        assert_eq!(cfg.fn_name, "onyx");
        assert_eq!(cfg.file_name, "onyx.rs");

        let generated = generate_theme_file(&tokens, &cfg).expect("Onyx must round-trip on import");
        assert!(
            generated.contains("pub fn onyx() -> Self"),
            "generated Onyx file must declare `pub fn onyx`"
        );
        // Sanity check that band colors made it through (structural check).
        assert!(
            generated.contains("band_colors: vec!["),
            "generated Onyx file must contain band_colors vec"
        );
    }

    #[test]
    fn accessible_themes_generate_one_shared_file() {
        let ids = [
            ThemeId::Protanopia,
            ThemeId::Deuteranopia,
            ThemeId::Tritanopia,
        ];
        let configs: Vec<ThemeConfig> = ids.iter().copied().map(theme_config_for).collect();
        let mut token_sets = serde_json::Map::new();

        for id in ids {
            let config = theme_config_for(id);
            let theme_json = export_theme_minimal_to_json(&Theme::from_id(id));
            token_sets.insert(config.set_name.to_string(), theme_json);
        }

        let generated = generate_theme_file_group(&Value::Object(token_sets), &configs)
            .expect("accessibility themes must generate into one shared file");

        assert_eq!(generated.matches("impl Theme {").count(), 1);
        assert!(generated.contains("pub fn protanopia() -> Self"));
        assert!(generated.contains("pub fn deuteranopia() -> Self"));
        assert!(generated.contains("pub fn tritanopia() -> Self"));
    }
}
