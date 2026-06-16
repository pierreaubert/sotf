use anyhow::{Context, Result};
use clap::Parser;
use gpui_design_tools::{DesignTokenFormat, export_design_tokens_to_path};
use serde_json::{Map, Value, json};
use sotf_audio_player_gpui::theme::{Theme, ThemeId};
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
struct Args {
    /// Export the generic gpui-toolkit DesignSystem token document.
    #[arg(long)]
    toolkit: bool,

    /// Output path. Defaults to the legacy SOTF app token file or toolkit token file.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Generic toolkit token format when --toolkit is used.
    #[arg(long, default_value = "style-dictionary-json")]
    format: String,
}

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
        ThemeId::Onyx => "theme/onyx",
        ThemeId::Protanopia => "theme/protanopia",
        ThemeId::Deuteranopia => "theme/deuteranopia",
        ThemeId::Tritanopia => "theme/tritanopia",
    }
}

fn export_theme_colors(theme: &Theme) -> Value {
    let band_map: Map<String, Value> = theme
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

fn main() -> Result<()> {
    let args = Args::parse();
    if args.toolkit {
        let format = DesignTokenFormat::parse(&args.format)?;
        let output = args.output.unwrap_or_else(default_toolkit_tokens_path);
        export_design_tokens_to_path(&output, format)?;
        println!("Wrote {}", output.display());
        return Ok(());
    }

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

    let out_path = args.output.unwrap_or_else(default_app_tokens_path);

    std::fs::write(&out_path, output.as_bytes())
        .with_context(|| format!("write {}", out_path.display()))?;
    println!("Wrote {}", out_path.display());
    Ok(())
}

fn workspace_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of crates/sotf-tools")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn default_app_tokens_path() -> PathBuf {
    workspace_root().join("design-tokens").join("tokens.json")
}

fn default_toolkit_tokens_path() -> PathBuf {
    workspace_root()
        .join("design-tokens")
        .join("gpui-tokens.json")
}
