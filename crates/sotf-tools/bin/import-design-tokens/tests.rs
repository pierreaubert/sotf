use super::generate::generate_theme_file;
use super::generate::generate_theme_file_group;
use super::get::get_color;
use super::parse::parse_hex;
use super::theme::theme_config_for;
use super::theme::theme_configs;
use super::types::ThemeConfig;
use serde_json::Value;
use sotf_audio_player_gpui::theme::ThemeId;

use serde_json::json;
use sotf_audio_player_gpui::theme::Theme;

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
        .plugin_palette
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
                "normal": c(theme.feedback.meter_normal),
                "warning": c(theme.feedback.meter_warning),
                "clip": c(theme.feedback.meter_clip)
            },
            "button": {
                "muteActive": c(theme.button_mute_active),
                "soloActive": c(theme.button_solo_active),
                "dimActive": c(theme.button_dim_active)
            },
            "playback": {
                "progressBarBg": c(theme.feedback.progress_bar_bg),
                "progressBarFill": c(theme.feedback.progress_bar_fill)
            },
            "toast": {
                "successBg": c(theme.feedback.toast_success_bg),
                "errorBg": c(theme.feedback.toast_error_bg),
                "infoBg": c(theme.feedback.toast_info_bg),
                "warningBg": c(theme.feedback.toast_warning_bg)
            },
            "plugin": {
                "eq": c(theme.plugin_palette.plugin_colors.eq),
                "gain": c(theme.plugin_palette.plugin_colors.gain),
                "upmixer": c(theme.plugin_palette.plugin_colors.upmixer),
                "compressor": c(theme.plugin_palette.plugin_colors.compressor),
                "limiter": c(theme.plugin_palette.plugin_colors.limiter),
                "gate": c(theme.plugin_palette.plugin_colors.gate),
                "loudness": c(theme.plugin_palette.plugin_colors.loudness),
                "binaural": c(theme.plugin_palette.plugin_colors.binaural),
                "convolution": c(theme.plugin_palette.plugin_colors.convolution),
                "monitor": c(theme.plugin_palette.plugin_colors.monitor),
                "spectrum": c(theme.plugin_palette.plugin_colors.spectrum),
                "muteSolo": c(theme.plugin_palette.plugin_colors.mute_solo)
            },
            "graph": {
                "input": c(theme.plugin_palette.graph_colors.input),
                "target": c(theme.plugin_palette.graph_colors.target),
                "filterResponse": c(theme.plugin_palette.graph_colors.filter_response),
                "corrected": c(theme.plugin_palette.graph_colors.corrected),
                "error": c(theme.plugin_palette.graph_colors.error),
                "deviation": c(theme.plugin_palette.graph_colors.deviation),
                "grid": c(theme.plugin_palette.graph_colors.grid),
                "secondaryLine": c(theme.plugin_palette.graph_colors.secondary_line),
                "directivityEr": c(theme.plugin_palette.graph_colors.directivity_er),
                "directivitySp": c(theme.plugin_palette.graph_colors.directivity_sp)
            },
            "band": Value::Object(band_map),
            "eqCurve": {
                "background": c(theme.plugin_palette.eq_curve_colors.background),
                "grid": c(theme.plugin_palette.eq_curve_colors.grid),
                "curveBoost": c(theme.plugin_palette.eq_curve_colors.curve_boost),
                "curveCut": c(theme.plugin_palette.eq_curve_colors.curve_cut),
                "fillBoost": c(theme.plugin_palette.eq_curve_colors.fill_boost),
                "fillCut": c(theme.plugin_palette.eq_curve_colors.fill_cut),
                "zeroLine": c(theme.plugin_palette.eq_curve_colors.zero_line)
            },
            "spectrum": {
                "background": c(theme.plugin_palette.spectrum_colors.background),
                "bass": c(theme.plugin_palette.spectrum_colors.bass),
                "mids": c(theme.plugin_palette.spectrum_colors.mids),
                "treble": c(theme.plugin_palette.spectrum_colors.treble)
            },
            "meterColors": {
                "background": c(theme.plugin_palette.meter_colors.background),
                "normal": c(theme.plugin_palette.meter_colors.normal),
                "warning": c(theme.plugin_palette.meter_colors.warning),
                "clip": c(theme.plugin_palette.meter_colors.clip),
                "peak": c(theme.plugin_palette.meter_colors.peak),
                "text": c(theme.plugin_palette.meter_colors.text)
            },
            "additional": {
                "peakIndicator": c(theme.feedback.peak_indicator),
                "dragOverHighlight": c(theme.feedback.drag_over_highlight),
                "dragOverBorder": c(theme.feedback.drag_over_border),
                "neutralIndicator": c(theme.feedback.neutral_indicator),
                "warningBackground": c(theme.feedback.warning_background),
                "knobColor": c(theme.feedback.knob_color),
                "optimizationColor": c(theme.feedback.optimization_color),
                "gridColor": c(theme.feedback.grid_color),
                "overlayBg": c(theme.feedback.overlay_bg)
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
