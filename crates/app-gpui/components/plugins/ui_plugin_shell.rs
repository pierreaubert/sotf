//! Plugin Shell Component
//!
//! Standardized wrapper for all plugin UIs providing a consistent visual frame:
//! - Colored accent strip (from theme.plugin_colors)
//! - Header bar with type icon and plugin name
//! - Bypass toggle
//! - Elevated background panel for plugin content

use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::PluginType;

/// Get the accent color for a plugin type from the theme
pub fn plugin_accent_color(plugin_type: &PluginType, theme: &Theme) -> Rgba {
    match plugin_type {
        PluginType::EQ => theme.plugin_colors.eq,
        PluginType::Gain => theme.plugin_colors.gain,
        PluginType::Upmixer => theme.plugin_colors.upmixer,
        PluginType::Compressor => theme.plugin_colors.compressor,
        PluginType::Limiter => theme.plugin_colors.limiter,
        PluginType::Gate => theme.plugin_colors.gate,
        PluginType::Expander => theme.plugin_colors.gate,
        PluginType::MultibandCompressor => theme.plugin_colors.compressor,
        PluginType::MultibandExpander => theme.plugin_colors.gate,
        PluginType::LoudnessCompensation => theme.plugin_colors.loudness,
        PluginType::FletcherMunson => theme.plugin_colors.loudness,
        PluginType::BinauralDecoder => theme.plugin_colors.binaural,
        PluginType::Convolution => theme.plugin_colors.convolution,
        PluginType::LoudnessMonitor => theme.plugin_colors.monitor,
        PluginType::SpectrumAnalyzer => theme.plugin_colors.spectrum,
        PluginType::ChannelMuteSolo => theme.plugin_colors.mute_solo,
        PluginType::Matrix => theme.plugin_colors.upmixer,
        PluginType::XTC => theme.plugin_colors.binaural,
        PluginType::Denoiser => theme.plugin_colors.eq,
        PluginType::Pnd => theme.plugin_colors.eq,
        PluginType::ABCompare => theme.plugin_colors.compressor,
        PluginType::BandSplit => theme.plugin_colors.upmixer,
        PluginType::BandMerge => theme.plugin_colors.upmixer,
        PluginType::Downmix => theme.plugin_colors.upmixer,
        PluginType::MonoToStereo => theme.plugin_colors.binaural,
        PluginType::Crossfeed => theme.plugin_colors.binaural,
        PluginType::Delay => theme.plugin_colors.eq,
        PluginType::Aec => theme.plugin_colors.eq,
        PluginType::Beamformer => theme.plugin_colors.binaural,
    }
}

/// Get the icon string for a plugin type.
///
/// For LoudnessMonitor, pass `is_input_mon` / `is_output_mon` to pick a
/// directional arrow; otherwise the generic monitor icon is used.
pub fn plugin_icon(
    plugin_type: &PluginType,
    is_input_mon: bool,
    is_output_mon: bool,
) -> &'static str {
    match plugin_type {
        PluginType::EQ => "≈",
        PluginType::Gain => "▲",
        PluginType::Upmixer => "◈",
        PluginType::Compressor => "◉",
        PluginType::Limiter => "█",
        PluginType::Gate => "⊡",
        PluginType::Expander => "⊟",
        PluginType::MultibandCompressor => "◎",
        PluginType::MultibandExpander => "◇",
        PluginType::LoudnessCompensation => "♫",
        PluginType::FletcherMunson => "\u{1f3a7}", // headphones emoji
        PluginType::BinauralDecoder => "◎",
        PluginType::Convolution => "∿",
        PluginType::LoudnessMonitor => {
            if is_input_mon {
                "◁"
            } else if is_output_mon {
                "▷"
            } else {
                "◐"
            }
        }
        PluginType::SpectrumAnalyzer => "▓",
        PluginType::ChannelMuteSolo => "◧",
        PluginType::Matrix => "⊞",
        PluginType::XTC => "⊗",
        PluginType::Denoiser => "◌",
        PluginType::Pnd => "♪",
        PluginType::ABCompare => "⇄",
        PluginType::BandSplit => "⊥",
        PluginType::BandMerge => "⊤",
        PluginType::Downmix => "▼",
        PluginType::MonoToStereo => "⊕",
        PluginType::Crossfeed => "⊞",
        PluginType::Delay => "⏱",
        PluginType::Aec => "⊘",
        PluginType::Beamformer => "⊙",
    }
}

/// Get the display name for a plugin type (short form for rack cards).
pub fn plugin_short_name(
    plugin_type: &PluginType,
    is_input_mon: bool,
    is_output_mon: bool,
    is_permanent: bool,
) -> &'static str {
    match plugin_type {
        PluginType::EQ => "Equalizer",
        PluginType::Gain => {
            if is_permanent {
                "Replay Gain"
            } else {
                "Gain"
            }
        }
        PluginType::Upmixer => "Upmixer",
        PluginType::Compressor => "Compressor",
        PluginType::Limiter => "Limiter",
        PluginType::Gate => "Gate",
        PluginType::Expander => "Expander",
        PluginType::MultibandCompressor => "MB Comp",
        PluginType::MultibandExpander => "MB Expand",
        PluginType::LoudnessCompensation => "Loudness",
        PluginType::FletcherMunson => "F-M EQ",
        PluginType::BinauralDecoder => "Binaural",
        PluginType::Convolution => "Convolution",
        PluginType::LoudnessMonitor => {
            if is_input_mon {
                "In Monitor"
            } else if is_output_mon {
                "Out Monitor"
            } else {
                "Monitor"
            }
        }
        PluginType::SpectrumAnalyzer => "Spectrum",
        PluginType::ChannelMuteSolo => "Mixer",
        PluginType::Matrix => "Matrix",
        PluginType::XTC => "XTC",
        PluginType::Denoiser => "Denoiser",
        PluginType::Pnd => "PND",
        PluginType::ABCompare => "A/B Comp",
        PluginType::BandSplit => "Split",
        PluginType::BandMerge => "Merge",
        PluginType::Downmix => "Downmix",
        PluginType::MonoToStereo => "Mono->2.0",
        PluginType::Crossfeed => "Crossfeed",
        PluginType::Delay => "Delay",
    }
}

/// Wrap plugin content in a standardized shell with accent strip, header, and elevated panel.
///
/// ```text
/// ┌─ [accent color strip] ──────────────────────────────┐
/// │ [icon] PLUGIN NAME                   [bypass toggle] │
/// ├──────────────────────────────────────────────────────┤
/// │                                                      │
/// │         (plugin-specific content here)                │
/// │                                                      │
/// └──────────────────────────────────────────────────────┘
/// ```
#[allow(clippy::type_complexity)]
pub fn render_plugin_shell(
    plugin_type: &PluginType,
    enabled: bool,
    theme: &Theme,
    content: impl IntoElement,
    on_bypass: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
) -> impl IntoElement {
    let accent = plugin_accent_color(plugin_type, theme);
    let icon = plugin_icon(plugin_type, false, false);
    let name = plugin_type.name().to_uppercase();

    div()
        .flex()
        .flex_col()
        .rounded_xl()
        .bg(theme.background_secondary)
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        // Accent color strip at top
        .child(div().h(px(3.0)).w_full().bg(accent))
        // Header bar
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px_4()
                .py_2()
                .border_b_1()
                .border_color(theme.border)
                // Left: icon + name
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().text_base().text_color(accent).child(icon.to_string()))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.text_primary)
                                .child(name),
                        ),
                )
                // Right: bypass toggle
                .children(on_bypass.map(|cb| {
                    let bypass_color = if enabled {
                        theme.success
                    } else {
                        theme.text_muted
                    };
                    div()
                        .id("shell-bypass")
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap_1()
                        .px_2()
                        .py(px(2.0))
                        .rounded_md()
                        .hover(move |s| s.bg(Theme::opacity_20pct(bypass_color)))
                        .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(bypass_color))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(bypass_color)
                                .child(if enabled { "ON" } else { "OFF" }),
                        )
                        .on_click(move |_, window, cx| {
                            cb(!enabled, window, cx);
                        })
                })),
        )
        // Content area with padding
        .child(div().p_4().child(content))
}
