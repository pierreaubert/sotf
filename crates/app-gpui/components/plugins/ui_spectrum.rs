//! Spectrum Analyzer UI Components

// intentional-file: spectrum analyzer with chart-internal pixel dimensions

use std::sync::Arc;

use gpui::prelude::*;
use gpui::*;
use gpui_audio_kit::{
    SpectrumAxisTheme, SpectrumColors, SpectrumElement, render_spectrum_db_axis,
    render_spectrum_frequency_axis,
};
use gpui_ui_kit::{Select, SelectOption, SelectSize};
use sotf_plugins::{SpectralTiltCorrection, TiltReferenceFreq};

use super::common::render_knob;
use crate::app::AppState;
use crate::components::design::Ds;
use crate::components::graphs::common::render_empty_state;
use crate::components::icons::IconName;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use crate::ui::PlayerView;

// ============================================================================
// Spectrum color adapters
// ============================================================================

fn spectrum_colors_from_theme(theme_colors: &crate::theme::SpectrumColors) -> SpectrumColors {
    SpectrumColors {
        background: theme_colors.background,
        low: theme_colors.bass,
        mid: theme_colors.mids,
        high: theme_colors.treble,
    }
}

fn spectrum_axis_theme(d: &Ds, theme: &Theme) -> SpectrumAxisTheme {
    SpectrumAxisTheme {
        text_color: theme.text_muted,
        text_size: d.text_xs,
        db_axis_padding_right: d.grid,
        ..Default::default()
    }
}

fn spectrum_db_axis_spacer(d: &Ds, theme: &Theme) -> impl IntoElement {
    div().w(px(spectrum_axis_theme(d, theme).db_axis_width))
}

// ============================================================================
// Plugin UI
// ============================================================================

use sotf_plugins::SpectrumData;

/// State for rendering the Spectrum Analyzer plugin
pub struct SpectrumRenderState<'a> {
    pub num_bins: usize,
    pub min_freq: f32,
    pub max_freq: f32,
    pub smoothing: f32,
    pub tilt_correction: SpectralTiltCorrection,
    pub tilt_reference: TiltReferenceFreq,
    pub tilt_select_open: bool,
    pub reference_select_open: bool,
    pub is_editing: bool,
    pub selected_param: usize,
    pub data: Option<&'a SpectrumData>,
}

/// Render the Spectrum Analyzer plugin
///
/// Layout (vertical):
/// +--------------------------------------------------------------------+
/// | SPECTRUM DISPLAY (full width)                                      |
/// | ┌─ dB axis ─┬─ Spectrum Graph ──────────────────────────────────┐  |
/// | │            │                                                   │  |
/// | │            │                                                   │  |
/// | └────────────┴───────────────────────────────────────────────────┘  |
/// |              [ frequency axis ]                                     |
/// +--------------------------------------------------------------------+
/// | CONFIG (horizontal row, wrapping)                                   |
/// | [Bins] [Min Hz] [Max Hz] [Smooth] [Tilt] [Reference]               |
/// +--------------------------------------------------------------------+
pub fn render_spectrum_analyzer_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: SpectrumRenderState,
    theme: &Theme,
) -> impl IntoElement {
    // === TOP: Spectrum display (full width) ===
    let spectrum_display = div()
        .flex()
        .flex_col()
        .w_full()
        .gap(d.gap)
        // Main spectrum area with dB axis
        .child(
            div()
                .flex()
                .gap(d.grid)
                .child(render_spectrum_db_axis(spectrum_axis_theme(d, theme)))
                .child(
                    div()
                        .flex_1()
                        .h(px(200.0))
                        .bg(theme.surface)
                        .rounded(d.r_lg)
                        .border_1()
                        .border_color(theme.border)
                        .flex()
                        .items_end()
                        .gap_px()
                        .p(d.pad_y)
                        .child(if let Some(data) = state.data {
                            let magnitudes: Arc<[f32]> =
                                Arc::from(data.magnitudes.as_ref().as_slice());
                            SpectrumElement::new(magnitudes)
                                .height(px(200.0))
                                .frequency_range(state.min_freq, state.max_freq)
                                .smoothing(state.smoothing)
                                .colors(spectrum_colors_from_theme(&theme.spectrum_colors))
                                .into_any_element()
                        } else {
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size_full()
                                .text_color(theme.text_muted)
                                .child("No signal")
                                .into_any_element()
                        }),
                ),
        )
        // Frequency axis
        .child(div().flex().child(spectrum_db_axis_spacer(d, theme)).child(
            render_spectrum_frequency_axis(
                state.min_freq,
                state.max_freq,
                spectrum_axis_theme(d, theme),
            ),
        ));

    // === BOTTOM: Config params (horizontal row with wrapping) ===
    let config_row = div()
        .flex()
        .flex_wrap()
        .gap(d.gap_md)
        .items_end()
        .pt(d.pad_y)
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Bins",
            state.num_bins as f64,
            10.0,
            100.0,
            "",
            0,
            state.selected_param,
            state.is_editing,
            None,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Min Hz",
            state.min_freq as f64,
            10.0,
            1000.0,
            "Hz",
            1,
            state.selected_param,
            state.is_editing,
            None,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Max Hz",
            state.max_freq as f64,
            1000.0,
            24000.0,
            "Hz",
            2,
            state.selected_param,
            state.is_editing,
            None,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Smooth",
            state.smoothing as f64,
            0.0,
            1.0,
            "",
            3,
            state.selected_param,
            state.is_editing,
            None,
            theme,
        ))
        // Tilt correction selector
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.grid)
                .child(
                    div()
                        .text_size(d.text_xs)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_secondary)
                        .child("Tilt"),
                )
                .child(
                    div().w(px(100.0)).child(
                        Select::new("tilt-correction-select")
                            .options(vec![
                                SelectOption::new("none".to_string(), "None"),
                                SelectOption::new("3db".to_string(), "+3dB/oct"),
                                SelectOption::new("6db".to_string(), "+6dB/oct"),
                                SelectOption::new("pink".to_string(), "Pink (+3dB/oct)"),
                            ])
                            .selected(match state.tilt_correction {
                                SpectralTiltCorrection::None => "none".to_string(),
                                SpectralTiltCorrection::ThreeDbPerOctave => "3db".to_string(),
                                SpectralTiltCorrection::SixDbPerOctave => "6db".to_string(),
                                SpectralTiltCorrection::Pink => "pink".to_string(),
                                SpectralTiltCorrection::Custom(_) => "none".to_string(),
                            })
                            .is_open(state.tilt_select_open)
                            .size(SelectSize::Xs)
                            .theme(theme.to_select_theme())
                            .on_toggle({
                                let entity = entity.clone();
                                move |is_open, _window, cx| {
                                    entity.update(cx, |state, cx| {
                                        state.app.spectrum_tilt_select_open = is_open;
                                        cx.notify();
                                    });
                                }
                            })
                            .on_change({
                                let entity = entity.clone();
                                move |value, _, cx| {
                                    entity.update(cx, |state, _cx| {
                                        let tilt = match value.as_ref() {
                                            "3db" => SpectralTiltCorrection::ThreeDbPerOctave,
                                            "6db" => SpectralTiltCorrection::SixDbPerOctave,
                                            "pink" => SpectralTiltCorrection::Pink,
                                            _ => SpectralTiltCorrection::None,
                                        };
                                        state.app.set_spectrum_tilt_correction(plugin_idx, tilt);
                                    });
                                }
                            }),
                    ),
                ),
        )
        // Reference frequency selector
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.grid)
                .child(
                    div()
                        .text_size(d.text_xs)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_secondary)
                        .child("Reference"),
                )
                .child(
                    div().w(px(100.0)).child(
                        Select::new("tilt-reference-select")
                            .options(vec![
                                SelectOption::new("standard".to_string(), "Standard"),
                                SelectOption::new("1khz".to_string(), "1 kHz"),
                                SelectOption::new("2khz".to_string(), "2 kHz"),
                                SelectOption::new("minfreq".to_string(), "Min Freq"),
                            ])
                            .selected(match state.tilt_reference {
                                TiltReferenceFreq::Standard => "standard".to_string(),
                                TiltReferenceFreq::OneKilohertz => "1khz".to_string(),
                                TiltReferenceFreq::TwoKilohertz => "2khz".to_string(),
                                TiltReferenceFreq::MinFreq => "minfreq".to_string(),
                            })
                            .is_open(state.reference_select_open)
                            .size(SelectSize::Xs)
                            .theme(theme.to_select_theme())
                            .on_toggle({
                                let entity = entity.clone();
                                move |is_open, _window, cx| {
                                    entity.update(cx, |state, cx| {
                                        state.app.spectrum_reference_select_open = is_open;
                                        cx.notify();
                                    });
                                }
                            })
                            .on_change({
                                let entity = entity.clone();
                                move |value, _, cx| {
                                    entity.update(cx, |state, _cx| {
                                        let reference = match value.as_ref() {
                                            "1khz" => TiltReferenceFreq::OneKilohertz,
                                            "2khz" => TiltReferenceFreq::TwoKilohertz,
                                            "minfreq" => TiltReferenceFreq::MinFreq,
                                            _ => TiltReferenceFreq::Standard,
                                        };
                                        state
                                            .app
                                            .set_spectrum_tilt_reference(plugin_idx, reference);
                                    });
                                }
                            }),
                    ),
                ),
        );

    // === Main layout: vertical stack ===
    div()
        .flex()
        .flex_col()
        .gap(d.gap)
        .p(d.pad_x)
        .w_full()
        .child(spectrum_display)
        .child(config_row)
}

impl PlayerView {
    /// Render the full-screen spectrum analyzer display
    /// Uses GPU-accelerated SpectrumElement for high-performance rendering
    pub(crate) fn render_spectrum_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        let content = if let Some(info) = &state.app.playback.spectrum_info {
            // Convert magnitudes to Arc for the GPU element
            let magnitudes: Arc<[f32]> = Arc::from(info.magnitudes.as_ref().as_slice());

            div()
                .flex()
                .flex_col()
                .size_full()
                // Main spectrum area with axes
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .gap(d.grid)
                        // dB axis (vertical, left side)
                        .child(render_spectrum_db_axis(spectrum_axis_theme(&d, &theme)))
                        // GPU-accelerated spectrum visualization
                        .child(
                            div().flex_1().child(
                                SpectrumElement::new(magnitudes)
                                    .height(px(256.0))
                                    .frequency_range(20.0, 20000.0)
                                    .smoothing(0.3)
                                    .colors(spectrum_colors_from_theme(&theme.spectrum_colors)),
                            ),
                        ),
                )
                // Frequency axis (horizontal, below spectrum)
                .child(
                    div()
                        .flex()
                        .child(spectrum_db_axis_spacer(&d, &theme))
                        .child(render_spectrum_frequency_axis(
                            20.0,
                            20000.0,
                            spectrum_axis_theme(&d, &theme),
                        )),
                )
        } else {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .child(render_empty_state(
                    IconName::AudioWaveform,
                    "No spectrum data available. Play audio to see visualization.",
                    &theme,
                ))
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .p(d.card)
            .child(
                div()
                    .text_size(d.text_lg)
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb(d.section)
                    .child("Spectrum Analyzer"),
            )
            .child(content)
    }
}
