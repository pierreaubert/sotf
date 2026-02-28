use serde_json::{Map, Value, json};
use sotf_audio_player_gpui::theme::{Theme, ThemeId};

fn rgba_to_hex(r: f32, g: f32, b: f32, a: f32) -> String {
    let ri = (r * 255.0).round() as u8;
    let gi = (g * 255.0).round() as u8;
    let bi = (b * 255.0).round() as u8;
    let ai = (a * 255.0).round() as u8;
    if ai == 255 {
        format!("#{ri:02x}{gi:02x}{bi:02x}")
    } else {
        format!("#{ri:02x}{gi:02x}{bi:02x}{ai:02x}")
    }
}

fn color_token(r: f32, g: f32, b: f32, a: f32) -> Value {
    json!({ "$type": "color", "$value": rgba_to_hex(r, g, b, a) })
}

fn c(color: gpui::Rgba) -> Value {
    color_token(color.r, color.g, color.b, color.a)
}

fn theme_set_name(id: ThemeId) -> &'static str {
    match id {
        ThemeId::Dark => "theme/dark",
        ThemeId::Light => "theme/light",
        ThemeId::Midnight => "theme/midnight",
        ThemeId::Forest => "theme/forest",
        ThemeId::BlackAndWhite => "theme/black-and-white",
    }
}

fn export_theme_colors(theme: &Theme) -> Value {
    let band_map: Map<String, Value> = theme
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

fn main() {
    let global = json!({
        "typography": {
            "fontFamily": { "$type": "fontFamily", "$value": "B612" },
            "fontSize": {
                "xs": { "$type": "dimension", "$value": "12px" },
                "sm": { "$type": "dimension", "$value": "14px" },
                "md": { "$type": "dimension", "$value": "14px" },
                "lg": { "$type": "dimension", "$value": "18px" },
                "xl": { "$type": "dimension", "$value": "20px" },
                "xxl": { "$type": "dimension", "$value": "24px" }
            },
            "fontWeight": {
                "light": { "$type": "fontWeight", "$value": 300 },
                "normal": { "$type": "fontWeight", "$value": 400 },
                "medium": { "$type": "fontWeight", "$value": 500 },
                "semibold": { "$type": "fontWeight", "$value": 600 },
                "bold": { "$type": "fontWeight", "$value": 700 }
            }
        },
        "spacing": {
            "none": { "$type": "dimension", "$value": "0px" },
            "xs": { "$type": "dimension", "$value": "2px" },
            "sm": { "$type": "dimension", "$value": "4px" },
            "md": { "$type": "dimension", "$value": "8px" },
            "lg": { "$type": "dimension", "$value": "16px" },
            "xl": { "$type": "dimension", "$value": "24px" },
            "xxl": { "$type": "dimension", "$value": "32px" }
        },
        "sizing": {
            "separatorSize": { "$type": "dimension", "$value": "20px" },
            "borderRadius": { "$type": "dimension", "$value": "4px" }
        }
    });

    let mut root = Map::new();
    root.insert("global".to_string(), global);

    for id in ThemeId::all() {
        let theme = Theme::from_id(*id);
        let set_name = theme_set_name(*id);
        root.insert(set_name.to_string(), export_theme_colors(&theme));
    }

    let output = serde_json::to_string_pretty(&Value::Object(root)).expect("JSON serialization");

    let out_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of crates/export-design-tokens")
        .parent()
        .expect("workspace root")
        .join("design-tokens")
        .join("tokens.json");

    std::fs::write(&out_path, output.as_bytes()).expect("write tokens.json");
    println!("Wrote {}", out_path.display());
}
