//! Spectrum Analyzer UI Components

// intentional-file: spectrum analyzer with chart-internal pixel dimensions

use std::sync::Arc;

use crate::app::state::ui::SpectrumViewState;
use gpui::prelude::*;
use gpui::*;
use gpui_audio_kit::{
    SpectrumAxisTheme, SpectrumColors, SpectrumElement, render_spectrum_frequency_axis,
};
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, NumberInput, NumberInputSize, Select, SelectOption,
    SelectSize, Text, Toggle, ToggleStyle,
};
use sotf_plugins::{SpectralTiltCorrection, TiltReferenceFreq};

use super::common::render_knob;
use crate::app::AppState;
use crate::app::i18n::SpectrumTranslations;
use crate::components::design::Ds;
use crate::components::graphs::common::render_empty_state;
use crate::components::icons::IconName;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use crate::ui::PlayerView;
use sotf_plugins::param_specs::{find_by_key as pk, spectrum::PARAMS as SP};

#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;

macro_rules! dev_track {
    ($element:expr, $selector:expr) => {{
        #[cfg(feature = "dev-api")]
        {
            $element.dev_track($selector)
        }
        #[cfg(not(feature = "dev-api"))]
        {
            $element
        }
    }};
}

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
        // The toolkit default is sized for 100% text. This app's combined
        // responsive and user scaling can reach 250%, so reserve enough room
        // for a signed two-digit dB label and rem-based padding.
        db_axis_width: 64.0,
        db_axis_padding_right: d.grid,
        ..Default::default()
    }
}

/// Render the app's zoom-safe dB scale.
///
/// The reusable toolkit scale includes both +3 dB and 0 dB at normalized
/// positions 0.000 and 0.029. Those labels are only a few pixels apart and
/// inevitably overlap once accessible text scaling is applied. Keep the true
/// +3 dB ceiling and the evenly spaced -20 dB ticks; the graph itself still
/// maps 0 dB at its exact value.
fn render_spectrum_db_axis(theme: SpectrumAxisTheme) -> impl IntoElement {
    const LABELS: [(&str, f32); 4] = [("+3", 0.0), ("-20", 0.223), ("-40", 0.417), ("-60", 0.612)];

    div()
        .w(px(theme.db_axis_width))
        .h_full()
        .flex()
        .flex_col()
        .relative()
        .children(LABELS.into_iter().map(move |(label, position)| {
            div()
                .absolute()
                .top(relative(position))
                .right_0()
                .pr(theme.db_axis_padding_right)
                .text_size(theme.text_size)
                .text_color(theme.text_color)
                .whitespace_nowrap()
                .child(div().mt(px(theme.db_label_offset_y)).child(label))
        }))
}

fn spectrum_db_axis_spacer(d: &Ds, theme: &Theme) -> impl IntoElement {
    div().w(px(spectrum_axis_theme(d, theme).db_axis_width))
}

fn corrected_spectrum_magnitudes(
    data: &SpectrumData,
    correction: SpectralTiltCorrection,
    reference: TiltReferenceFreq,
    min_freq: f32,
) -> Arc<[f32]> {
    let slope = match correction {
        SpectralTiltCorrection::None => 0.0,
        SpectralTiltCorrection::ThreeDbPerOctave | SpectralTiltCorrection::Pink => 3.0,
        SpectralTiltCorrection::SixDbPerOctave => 6.0,
        SpectralTiltCorrection::Custom(value) => value,
    };
    let reference_hz = match reference {
        TiltReferenceFreq::Standard | TiltReferenceFreq::OneKilohertz => 1_000.0,
        TiltReferenceFreq::TwoKilohertz => 2_000.0,
        TiltReferenceFreq::MinFreq => min_freq.max(f32::MIN_POSITIVE),
    };
    if slope == 0.0 {
        return data.magnitudes.clone();
    }
    data.magnitudes
        .iter()
        .zip(data.frequencies.iter())
        .map(|(&magnitude, &frequency)| {
            if !magnitude.is_finite() {
                magnitude
            } else {
                magnitude + slope * (frequency / reference_hz).log2()
            }
        })
        .collect::<Vec<_>>()
        .into()
}

fn spectral_tilt_option_label(correction: SpectralTiltCorrection, custom: &str) -> String {
    match correction {
        SpectralTiltCorrection::Custom(slope) if slope.is_finite() => {
            format!("{custom} ({slope:+.1} dB/oct)")
        }
        _ => custom.to_string(),
    }
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
    pub chart_height: f32,
    /// Definite pixel width of the plugin content area. The detail panel is
    /// shrink-to-fit, so flex/`w_full` cannot size the chart — it must be
    /// given explicit pixels (same approach as the EQ chart).
    pub available_width: f32,
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
    text: SpectrumTranslations,
    theme: &Theme,
) -> impl IntoElement {
    // === TOP: Spectrum display (full width) ===
    // The panel this renders into is shrink-to-fit, so relative/flex sizing
    // collapses the chart to its minimum. Derive explicit pixel dimensions
    // from the content-area width instead: the graph takes everything left
    // after the dB axis, and its height follows a fixed aspect ratio (with
    // the caller-provided height acting as floor and 3x as ceiling).
    let axis_theme = spectrum_axis_theme(d, theme);
    let chart_width = (state.available_width - axis_theme.db_axis_width).max(160.0);
    let chart_height = (chart_width * 0.38).clamp(state.chart_height, state.chart_height * 3.0);
    let spectrum_display = div()
        .flex()
        .flex_col()
        .gap(d.gap)
        // Main spectrum area with dB axis
        .child(
            div()
                .flex()
                .gap(d.grid)
                .child(render_spectrum_db_axis(spectrum_axis_theme(d, theme)))
                .child(
                    div()
                        .w(px(chart_width))
                        .h(px(chart_height))
                        .bg(theme.surface)
                        .rounded(d.r_lg)
                        .border_1()
                        .border_color(theme.border)
                        .flex()
                        .items_end()
                        .gap_px()
                        .p(d.pad_y)
                        .child(if let Some(data) = state.data {
                            let magnitudes = corrected_spectrum_magnitudes(
                                data,
                                state.tilt_correction,
                                state.tilt_reference,
                                state.min_freq,
                            );
                            SpectrumElement::new(magnitudes)
                                .height(px(chart_height))
                                .frequency_range(state.min_freq, state.max_freq)
                                .smoothing(state.smoothing)
                                .colors(spectrum_colors_from_theme(
                                    &theme.plugin_palette.spectrum_colors,
                                ))
                                .into_any_element()
                        } else {
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size_full()
                                .text_color(theme.text_muted)
                                .child(text.no_signal)
                                .into_any_element()
                        }),
                ),
        )
        // Frequency axis
        .child(
            div()
                .flex()
                .w(px(chart_width + axis_theme.db_axis_width))
                .child(spectrum_db_axis_spacer(d, theme))
                .child(div().flex_1().child(render_spectrum_frequency_axis(
                    state.min_freq,
                    state.max_freq,
                    spectrum_axis_theme(d, theme),
                ))),
        );

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
            text.bins,
            state.num_bins as f64,
            pk(SP, "num_bins").min_f64(),
            pk(SP, "num_bins").max_f64(),
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
            text.minimum_frequency,
            state.min_freq as f64,
            pk(SP, "min_freq").min_f64(),
            pk(SP, "min_freq").max_f64(),
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
            text.maximum_frequency,
            state.max_freq as f64,
            pk(SP, "max_freq").min_f64(),
            pk(SP, "max_freq").max_f64(),
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
            text.smoothing,
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
                        .child(text.tilt),
                )
                .child(
                    div().w(rems(8.5)).child(
                        Select::new("tilt-correction-select")
                            .options(vec![
                                SelectOption::new("none".to_string(), text.none),
                                SelectOption::new("3db".to_string(), "+3dB/oct"),
                                SelectOption::new("6db".to_string(), "+6dB/oct"),
                                SelectOption::new("pink".to_string(), "Pink (+3dB/oct)"),
                                SelectOption::new(
                                    "custom".to_string(),
                                    spectral_tilt_option_label(state.tilt_correction, text.custom),
                                )
                                .disabled(true),
                            ])
                            .selected(match state.tilt_correction {
                                SpectralTiltCorrection::None => "none".to_string(),
                                SpectralTiltCorrection::ThreeDbPerOctave => "3db".to_string(),
                                SpectralTiltCorrection::SixDbPerOctave => "6db".to_string(),
                                SpectralTiltCorrection::Pink => "pink".to_string(),
                                SpectralTiltCorrection::Custom(_) => "custom".to_string(),
                            })
                            .is_open(state.tilt_select_open)
                            .size(SelectSize::Xs)
                            .theme(theme.to_select_theme())
                            .aria_label(text.tilt)
                            .on_toggle({
                                let entity = entity.downgrade();
                                move |is_open, _window, cx| {
                                    let Some(entity) = entity.upgrade() else {
                                        return;
                                    };
                                    entity.update(cx, |state, cx| {
                                        state.app.plugin_ui.spectrum_tilt_select_open = is_open;
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
                        .child(text.reference),
                )
                .child(
                    div().w(rems(8.5)).child(
                        Select::new("tilt-reference-select")
                            .options(vec![
                                SelectOption::new("standard".to_string(), text.standard),
                                SelectOption::new("1khz".to_string(), "1 kHz"),
                                SelectOption::new("2khz".to_string(), "2 kHz"),
                                SelectOption::new("minfreq".to_string(), text.min_frequency_short),
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
                            .aria_label(text.reference)
                            .on_toggle({
                                let entity = entity.downgrade();
                                move |is_open, _window, cx| {
                                    let Some(entity) = entity.upgrade() else {
                                        return;
                                    };
                                    entity.update(cx, |state, cx| {
                                        state.app.plugin_ui.spectrum_reference_select_open =
                                            is_open;
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

#[cfg(test)]
mod spectrum_tilt_tests {
    use super::*;

    fn flat_data() -> SpectrumData {
        SpectrumData {
            frequencies: Arc::new(vec![500.0, 1_000.0, 2_000.0, 4_000.0]),
            magnitudes: Arc::from(vec![-20.0; 4]),
            peak_magnitude: -20.0,
        }
    }

    #[test]
    fn none_is_unchanged_and_slopes_are_reference_normalized() {
        let data = flat_data();
        let unchanged = corrected_spectrum_magnitudes(
            &data,
            SpectralTiltCorrection::None,
            TiltReferenceFreq::OneKilohertz,
            20.0,
        );
        assert!(Arc::ptr_eq(&unchanged, &data.magnitudes));
        let three = corrected_spectrum_magnitudes(
            &data,
            SpectralTiltCorrection::ThreeDbPerOctave,
            TiltReferenceFreq::OneKilohertz,
            20.0,
        );
        assert_eq!(three.as_ref(), &[-23.0, -20.0, -17.0, -14.0]);
        let six = corrected_spectrum_magnitudes(
            &data,
            SpectralTiltCorrection::SixDbPerOctave,
            TiltReferenceFreq::TwoKilohertz,
            20.0,
        );
        assert_eq!(six.as_ref(), &[-32.0, -26.0, -20.0, -14.0]);
    }

    #[test]
    fn pink_and_minimum_frequency_reference_are_applied() {
        let data = SpectrumData {
            frequencies: Arc::new(vec![20.0, 40.0, 80.0]),
            magnitudes: Arc::from(vec![-30.0; 3]),
            peak_magnitude: -30.0,
        };
        let corrected = corrected_spectrum_magnitudes(
            &data,
            SpectralTiltCorrection::Pink,
            TiltReferenceFreq::MinFreq,
            20.0,
        );
        assert_eq!(corrected.as_ref(), &[-30.0, -27.0, -24.0]);
    }

    #[test]
    fn custom_tilt_option_includes_the_actual_slope() {
        assert_eq!(
            spectral_tilt_option_label(SpectralTiltCorrection::Custom(4.25), "Custom"),
            "Custom (+4.3 dB/oct)"
        );
        assert_eq!(
            spectral_tilt_option_label(SpectralTiltCorrection::Custom(-2.0), "Custom"),
            "Custom (-2.0 dB/oct)"
        );
        assert_eq!(
            spectral_tilt_option_label(SpectralTiltCorrection::Custom(f32::NAN), "Custom"),
            "Custom"
        );
    }
}

include!("ui_spectrum_screen.rs");
