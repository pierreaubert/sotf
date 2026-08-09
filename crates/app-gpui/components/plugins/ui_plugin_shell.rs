//! Plugin Shell Component
//!
//! Standardized wrapper for all plugin UIs providing a consistent visual frame:
//! - Colored accent strip (from theme.plugin_palette.plugin_colors)
//! - Header bar with type icon and plugin name
//! - Bypass toggle
//! - Elevated background panel for plugin content

use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName, IconSize};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant};
use sotf_audio_player::PluginType;

/// Get the accent color for a plugin type from the theme
pub fn plugin_accent_color(plugin_type: &PluginType, theme: &Theme) -> Rgba {
    match plugin_type {
        PluginType::EQ => theme.plugin_palette.plugin_colors.eq,
        PluginType::Gain => theme.plugin_palette.plugin_colors.gain,
        PluginType::AAE => theme.plugin_palette.plugin_colors.upmixer,
        PluginType::Upmixer => theme.plugin_palette.plugin_colors.upmixer,
        PluginType::Compressor => theme.plugin_palette.plugin_colors.compressor,
        PluginType::Limiter => theme.plugin_palette.plugin_colors.limiter,
        PluginType::Gate => theme.plugin_palette.plugin_colors.gate,
        PluginType::Expander => theme.plugin_palette.plugin_colors.gate,
        PluginType::MultibandCompressor => theme.plugin_palette.plugin_colors.compressor,
        PluginType::MultibandExpander => theme.plugin_palette.plugin_colors.gate,
        PluginType::LoudnessCompensation => theme.plugin_palette.plugin_colors.loudness,
        PluginType::FletcherMunson => theme.plugin_palette.plugin_colors.loudness,
        PluginType::BinauralDecoder => theme.plugin_palette.plugin_colors.binaural,
        PluginType::Convolution => theme.plugin_palette.plugin_colors.convolution,
        PluginType::LoudnessMonitor => theme.plugin_palette.plugin_colors.monitor,
        PluginType::SpectrumAnalyzer => theme.plugin_palette.plugin_colors.spectrum,
        PluginType::ChannelMuteSolo => theme.plugin_palette.plugin_colors.mute_solo,
        PluginType::Matrix => theme.plugin_palette.plugin_colors.upmixer,
        PluginType::XTC => theme.plugin_palette.plugin_colors.binaural,
        PluginType::Denoiser => theme.plugin_palette.plugin_colors.eq,
        PluginType::Declick => theme.plugin_palette.plugin_colors.eq,
        PluginType::HissReducer => theme.plugin_palette.plugin_colors.eq,
        PluginType::SpeechDenoiser => theme.plugin_palette.plugin_colors.eq,
        PluginType::Pnd => theme.plugin_palette.plugin_colors.eq,
        PluginType::ABCompare => theme.plugin_palette.plugin_colors.compressor,
        PluginType::Crossover => theme.plugin_palette.plugin_colors.upmixer,
        PluginType::BandSplit => theme.plugin_palette.plugin_colors.upmixer,
        PluginType::BandMerge => theme.plugin_palette.plugin_colors.upmixer,
        PluginType::Downmix => theme.plugin_palette.plugin_colors.upmixer,
        PluginType::MonoToStereo => theme.plugin_palette.plugin_colors.binaural,
        PluginType::Crossfeed => theme.plugin_palette.plugin_colors.binaural,
        PluginType::Delay => theme.plugin_palette.plugin_colors.eq,
        PluginType::Aec => theme.plugin_palette.plugin_colors.eq,
        PluginType::Beamformer => theme.plugin_palette.plugin_colors.binaural,
        PluginType::AmbisonicsDecoder => theme.plugin_palette.plugin_colors.upmixer,
        PluginType::StereoImager => theme.plugin_palette.plugin_colors.upmixer,
        PluginType::DeEsser => theme.plugin_palette.plugin_colors.compressor,
        PluginType::TransientShaper => theme.plugin_palette.plugin_colors.compressor,
        PluginType::Saturation => theme.plugin_palette.plugin_colors.eq,
        PluginType::DynamicEq => theme.plugin_palette.plugin_colors.compressor,
        PluginType::LinearPhaseEq => theme.plugin_palette.plugin_colors.eq,
        PluginType::SpectralCompressor => theme.plugin_palette.plugin_colors.compressor,
        PluginType::External => theme.plugin_palette.plugin_colors.upmixer,
    }
}

/// Get the icon for a plugin type.
///
/// For LoudnessMonitor, pass `is_input_mon` / `is_output_mon` to pick a
/// directional arrow; otherwise the generic monitor icon is used.
pub fn plugin_icon(plugin_type: &PluginType, is_input_mon: bool, is_output_mon: bool) -> IconName {
    match plugin_type {
        PluginType::EQ | PluginType::DynamicEq | PluginType::LinearPhaseEq => {
            IconName::SlidersHorizontal
        }
        PluginType::Gain | PluginType::LoudnessCompensation => IconName::Volume2,
        PluginType::AAE | PluginType::Convolution | PluginType::Delay => IconName::Repeat,
        PluginType::Upmixer
        | PluginType::Matrix
        | PluginType::Crossover
        | PluginType::BandSplit
        | PluginType::BandMerge
        | PluginType::Downmix
        | PluginType::StereoImager
        | PluginType::AmbisonicsDecoder => IconName::Speaker,
        PluginType::Compressor
        | PluginType::Limiter
        | PluginType::Gate
        | PluginType::Expander
        | PluginType::MultibandCompressor
        | PluginType::MultibandExpander
        | PluginType::DeEsser
        | PluginType::TransientShaper
        | PluginType::SpectralCompressor => IconName::AudioWaveform,
        PluginType::FletcherMunson
        | PluginType::BinauralDecoder
        | PluginType::XTC
        | PluginType::Crossfeed => IconName::Headphones,
        PluginType::LoudnessMonitor => {
            if is_input_mon {
                IconName::ChevronLeft
            } else if is_output_mon {
                IconName::ChevronRight
            } else {
                IconName::Volume2
            }
        }
        PluginType::SpectrumAnalyzer => IconName::AudioWaveform,
        PluginType::ChannelMuteSolo | PluginType::MonoToStereo => IconName::SlidersHorizontal,
        PluginType::Denoiser
        | PluginType::Declick
        | PluginType::HissReducer
        | PluginType::SpeechDenoiser
        | PluginType::Pnd
        | PluginType::Aec
        | PluginType::Beamformer => IconName::Brain,
        PluginType::ABCompare => IconName::Repeat,
        PluginType::Saturation => IconName::AudioWaveform,
        PluginType::External => IconName::Plug,
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
        PluginType::AAE => "AAE Reverb",
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
        PluginType::Declick => "Declick",
        PluginType::HissReducer => "Hiss Red",
        PluginType::SpeechDenoiser => "Speech",
        PluginType::Pnd => "PND",
        PluginType::ABCompare => "A/B Comp",
        PluginType::Crossover => "Crossover",
        PluginType::BandSplit => "Split",
        PluginType::BandMerge => "Merge",
        PluginType::Downmix => "Downmix",
        PluginType::MonoToStereo => "Mono->2.0",
        PluginType::Crossfeed => "Crossfeed",
        PluginType::Delay => "Delay",
        PluginType::Aec => "AEC",
        PluginType::Beamformer => "Beamfmr",
        PluginType::AmbisonicsDecoder => "Ambi",
        PluginType::StereoImager => "Stereo",
        PluginType::DeEsser => "De-Ess",
        PluginType::TransientShaper => "Transient",
        PluginType::Saturation => "Saturate",
        PluginType::DynamicEq => "DynEQ",
        PluginType::LinearPhaseEq => "FIR EQ",
        PluginType::SpectralCompressor => "Spectral Compressor",
        PluginType::External => "External Plugin",
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
    d: &Ds,
    plugin_idx: usize,
    plugin_type: &PluginType,
    is_input_monitor: bool,
    is_output_monitor: bool,
    enabled: bool,
    text: PluginCommonTranslations,
    theme: &Theme,
    content: impl IntoElement,
    on_bypass: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
) -> impl IntoElement {
    let accent = plugin_accent_color(plugin_type, theme);
    let icon = plugin_icon(plugin_type, is_input_monitor, is_output_monitor);
    let name = plugin_type.name().to_uppercase();
    let description = super::ui_rack::plugin_description(plugin_type, text);

    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .rounded(d.r_xl)
        .bg(theme.background_secondary)
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        // Accent color strip at top
        // intentional: 3px accent strip — visual element, not spacing
        .child(div().h(px(3.0)).w_full().bg(accent))
        // Header bar
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .justify_between()
                .px(d.card)
                .py(d.pad_y)
                .border_b_1()
                .border_color(theme.border)
                // Left: icon + name
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_w_0()
                        .items_center()
                        .gap(d.gap)
                        .child(Icon::new(icon).size(IconSize::Sm).color(accent))
                        .child(
                            div()
                                .flex()
                                .flex_1()
                                .flex_col()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_size(d.text_sm)
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.text_primary)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .child(name),
                                )
                                .child(
                                    div()
                                        .text_size(d.text_xs)
                                        .text_color(theme.text_muted)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .child(description),
                                ),
                        ),
                )
                // Right: bypass toggle
                .children(on_bypass.map(|cb| {
                    let bypass = Button::new(
                        ("shell-bypass", plugin_idx),
                        if enabled { "Active" } else { "Bypassed" },
                    )
                    .variant(if enabled {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Secondary
                    })
                    .size(ButtonSize::Xs)
                    .theme(theme.to_button_theme())
                    .aria_label(if enabled {
                        "Bypass plugin"
                    } else {
                        "Activate plugin"
                    })
                    .on_click_event(move |_event, window, cx| cb(!enabled, window, cx));

                    div()
                        .flex_none()
                        .flex()
                        .flex_col()
                        .items_end()
                        .gap(d.grid)
                        .child(bypass)
                        .child(
                            div()
                                .text_size(d.text_xs)
                                .text_color(theme.text_muted)
                                .child(text.reset_hint),
                        )
                })),
        )
        // Content area with padding
        .child(div().w_full().min_w_0().p(d.card).child(content))
}
use crate::app::i18n::PluginCommonTranslations;
