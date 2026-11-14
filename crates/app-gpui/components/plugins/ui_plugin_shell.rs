//! Plugin Shell Component
//!
//! Standardized wrapper for all plugin UIs providing a consistent visual frame:
//! - Colored accent strip (from `theme.plugin_palette.plugin_colors`)
//! - Header bar with type icon and plugin name
//! - Bypass toggle
//! - Elevated background panel for plugin content

use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName, IconSize};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::{AnyElement, App, FontWeight, Rgba, Window, div, px};
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant};
use sotf_audio_player::PluginType;

/// Get the category accent color from the active theme.
#[must_use]
pub fn plugin_category_color(category: &str, theme: &Theme) -> Rgba {
    let colors = &theme.plugin_palette.plugin_colors;
    match category {
        "Dynamics" => colors.compressor,
        "EQ & Tone" => colors.eq,
        "Denoising" => colors.gain,
        "Spatial" => colors.upmixer,
        "Analysis" => colors.loudness,
        "Routing" => colors.limiter,
        _ => theme.text_muted,
    }
}

/// Get the category accent color for a plugin.
#[must_use]
pub fn plugin_accent_color(plugin_type: &PluginType, theme: &Theme) -> Rgba {
    let category = sotf_audio_player::plugin_categories::category_of(plugin_type).unwrap_or(
        match plugin_type {
            // Legacy loudness EQ and external plugins are not in the picker.
            PluginType::FletcherMunson => "EQ & Tone",
            PluginType::External => "Spatial",
            _ => "",
        },
    );
    plugin_category_color(category, theme)
}

/// Get the supplied artwork for a plugin type.
///
/// Input and output monitors use the directional device artwork. Plugins
/// without dedicated artwork retain an appropriate generic icon.
#[must_use]
pub fn plugin_icon(plugin_type: &PluginType, is_input_mon: bool, is_output_mon: bool) -> IconName {
    match plugin_type {
        PluginType::EQ => IconName::PluginEq,
        PluginType::AnalogEq => IconName::PluginAnalogEq,
        PluginType::DynamicEq => IconName::PluginDynamicEq,
        PluginType::LinearPhaseEq => IconName::PluginLinearPhaseEq,
        PluginType::Gain => IconName::PluginGain,
        PluginType::Dither => IconName::PluginDither,
        PluginType::AAE => IconName::PluginAae,
        PluginType::Upmixer => IconName::PluginUpmixer,
        PluginType::Compressor | PluginType::MultibandCompressor => {
            IconName::PluginMultibandCompressor
        }
        PluginType::Expander | PluginType::MultibandExpander => IconName::PluginMultibandExpander,
        PluginType::Limiter => IconName::PluginLimiter,
        PluginType::Gate => IconName::PluginGate,
        PluginType::AnalogCompressor => IconName::PluginAnalogCompressor,
        PluginType::AnalogLimiter => IconName::PluginAnalogLimiter,
        PluginType::LoudnessCompensation | PluginType::FletcherMunson => {
            IconName::PluginLoudnessCompensation
        }
        PluginType::BinauralDecoder => IconName::PluginBinaural,
        PluginType::Convolution => IconName::PluginConvolution,
        PluginType::ChannelMuteSolo => IconName::PluginChannelMuteSolo,
        PluginType::Matrix => IconName::PluginMatrix,
        PluginType::XTC => IconName::PluginXtc,
        PluginType::Denoiser => IconName::PluginDenoiser,
        PluginType::Declick => IconName::PluginDeclick,
        PluginType::HissReducer => IconName::PluginHissReducer,
        PluginType::SpeechDenoiser => IconName::PluginSpeechDenoiser,
        PluginType::Pnd => IconName::PluginPnd,
        PluginType::ABCompare => IconName::PluginAbCompare,
        PluginType::Crossover => IconName::PluginCrossover,
        PluginType::BandSplit => IconName::PluginBandSplit,
        PluginType::BandMerge => IconName::PluginBandMerge,
        PluginType::Downmix => IconName::PluginDownmix,
        PluginType::MonoToStereo => IconName::PluginMonoToStereo,
        PluginType::Crossfeed => IconName::PluginCrossfeed,
        PluginType::Delay => IconName::PluginDelay,
        PluginType::Aec => IconName::PluginAec,
        PluginType::Beamformer => IconName::PluginBeamformer,
        PluginType::AmbisonicsDecoder => IconName::PluginAmbisonics,
        PluginType::StereoImager => IconName::PluginStereoImager,
        PluginType::DeEsser => IconName::PluginDeEsser,
        PluginType::TransientShaper => IconName::PluginTransientShaper,
        PluginType::Saturation => IconName::PluginSaturation,
        PluginType::SpectralCompressor => IconName::PluginSpectralCompressor,
        PluginType::LoudnessMonitor => {
            if is_input_mon {
                IconName::PluginHalInput
            } else if is_output_mon {
                IconName::PluginHalOutput
            } else {
                IconName::Volume2
            }
        }
        PluginType::SpectrumAnalyzer => IconName::AudioWaveform,
        PluginType::External => IconName::Plug,
    }
}

/// Get the display name for a plugin type (short form for rack cards).
#[must_use]
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
        PluginType::Dither => "Dither",
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
        PluginType::AnalogEq => "Analog EQ",
        PluginType::AnalogLimiter => "Analog Lim",
        PluginType::AnalogCompressor => "Analog Comp",
        PluginType::DynamicEq => "DynEQ",
        PluginType::LinearPhaseEq => "FIR EQ",
        PluginType::SpectralCompressor => "Spectral Compressor",
        PluginType::External => "External Plugin",
    }
}

/// Shared enable/bypass affordance copy (ui.md Phase 2).
///
/// The rack toggle and the shell toggle render at different sizes, but they
/// must agree on state meaning, tooltip, and accessible name.
#[must_use]
pub fn bypass_label(enabled: bool, text: PluginCommonTranslations) -> &'static str {
    text.bypass_labels()[usize::from(!enabled)]
}

/// Shared enable/bypass tooltip and accessible name (ui.md Phase 2).
#[must_use]
pub fn bypass_tooltip(enabled: bool, text: PluginCommonTranslations) -> &'static str {
    text.bypass_labels()[2 + usize::from(!enabled)]
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
#[allow(
    clippy::type_complexity,
    reason = "Widget callbacks retain their typed GPUI handler signatures"
)]
#[expect(
    clippy::too_many_lines,
    reason = "Keep the declarative layout and its interaction handlers together"
)]
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
    header_control: Option<AnyElement>,
    on_bypass: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
) -> impl IntoElement {
    let has_header_controls = header_control.is_some();
    let accent = plugin_accent_color(plugin_type, theme);
    let icon = plugin_icon(plugin_type, is_input_monitor, is_output_monitor);
    let name = plugin_type.name();
    let description = super::ui_rack::plugin_description(plugin_type, text);
    let description_theme = theme.clone();
    let tooltip_theme = theme.clone();
    #[cfg(feature = "dev-api")]
    let content = {
        use crate::app::dev_api::DevTrackExt;
        content.dev_track(format!("plugin.content.{plugin_idx}"))
    };

    let shell = div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        // The rack detail pane owns vertical scrolling. Preserve the shell's
        // intrinsic content height so long custom editors contribute a real
        // scroll extent instead of shrinking while descendants overflow.
        .flex_shrink_0()
        .rounded(d.r_md)
        .bg(theme.background_secondary)
        .border_1()
        .border_color(theme.border)
        .when(matches!(plugin_type, PluginType::EQ), |shell| {
            shell.border_l_0().border_r_0().rounded_none()
        })
        // EQ has no rounded side frame; let its endpoint tick labels extend
        // into the rack's padding while the rack scrollport still clips the editor.
        .when(!matches!(plugin_type, PluginType::EQ), |shell| {
            shell.overflow_hidden()
        })
        // Accent color strip at top
        // intentional: 3px accent strip — visual element, not spacing
        .child(div().h(px(3.0)).w_full().bg(accent))
        // Header bar
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_wrap()
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
                        .when(has_header_controls, |title| title.flex_none().mr(d.grid))
                        .items_center()
                        .gap(d.gap)
                        .child(Icon::new(icon).size(IconSize::Sm).color(accent))
                        .child(
                            div().flex().flex_1().flex_col().min_w_0().child(
                                div()
                                    .id(("shell-plugin-name", plugin_idx))
                                    .tooltip(move |_, cx| {
                                        crate::components::themed_tooltip(
                                            description,
                                            &description_theme,
                                            cx,
                                        )
                                    })
                                    .text_size(d.text_sm)
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.text_primary)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .child(name),
                            ),
                        ),
                )
                .children(header_control)
                // Right: bypass toggle (shared affordance copy with the
                // rack toggle: same label, tooltip, and accessible name).
                .children(on_bypass.map(|cb| {
                    let bypass_tooltip_text = bypass_tooltip(enabled, text);
                    let bypass =
                        Button::new(("shell-bypass", plugin_idx), bypass_label(enabled, text))
                            .variant(if enabled {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            })
                            .size(ButtonSize::Xs)
                            .theme(theme.to_button_theme())
                            .aria_label(bypass_tooltip_text)
                            .on_click_event(move |_event, window, cx| cb(!enabled, window, cx));
                    #[cfg(feature = "dev-api")]
                    let bypass = {
                        use crate::app::dev_api::{DevElementState, DevTrackExt};
                        bypass.dev_track_with_state(
                            format!("plugin.shell.{plugin_idx}.bypass"),
                            DevElementState::default().selected(enabled),
                        )
                    };

                    div()
                        .id(("shell-bypass-tooltip", plugin_idx))
                        .flex_none()
                        .flex()
                        .flex_col()
                        .items_end()
                        .gap(d.grid)
                        .tooltip(move |_window, cx| {
                            crate::components::themed_tooltip(
                                bypass_tooltip_text,
                                &tooltip_theme,
                                cx,
                            )
                        })
                        .child(bypass)
                })),
        )
        // The editor owns control help; an ancestor tooltip would obscure
        // parameter labels and compete with exact-entry interactions.
        .child(
            div()
                .id(("shell-content-hint", plugin_idx))
                .w_full()
                .min_w_0()
                .p(d.card)
                .when(matches!(plugin_type, PluginType::EQ), |body| {
                    body.px_0().py(d.grid)
                })
                .child(content),
        );
    #[cfg(feature = "dev-api")]
    let shell = {
        use crate::app::dev_api::DevTrackExt;
        shell.dev_track(format!("plugin.shell.{plugin_idx}"))
    };
    shell
}
use crate::app::i18n::PluginCommonTranslations;
