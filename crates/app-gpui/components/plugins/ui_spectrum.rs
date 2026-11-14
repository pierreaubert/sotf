//! Spectrum Analyzer UI Components

// intentional-file: spectrum analyzer with chart-internal pixel dimensions

use std::sync::Arc;

use crate::app::state::ui::SpectrumViewState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Bounds, Entity, MouseMoveEvent, Pixels, canvas, div, px, relative, rems,
};
use gpui_audio_kit::{
    SpectrumAxisTheme, SpectrumColors, SpectrumElement, render_spectrum_frequency_axis,
};
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, NumberInput, NumberInputSize, Select, SelectOption,
    SelectSize, Text, Toggle, ToggleStyle,
};
use sotf_plugins::{SpectralTiltCorrection, TiltReferenceFreq};

use super::ui_layout_renderer::render_horizontal_param;
use crate::app::AppState;
use crate::app::i18n::SpectrumTranslations;
use crate::components::design::Ds;
use crate::components::graphs::common::render_empty_state;
use crate::components::icons::IconName;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use crate::ui::PlayerView;
use sotf_plugins::param_specs::spectrum::PARAMS as SP;

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
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
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
        return SpectrumViewState::display_magnitudes(&data.magnitudes);
    }
    SpectrumViewState::display_magnitudes(&data.magnitudes)
        .iter()
        .zip(data.frequencies.iter())
        .map(|(&magnitude, &frequency)| {
            if magnitude.is_finite() {
                magnitude + slope * (frequency / reference_hz).log2()
            } else {
                magnitude
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
#[derive(Debug)]
pub struct SpectrumRenderState<'a> {
    pub num_bins: usize,
    pub min_freq: f32,
    pub max_freq: f32,
    pub smoothing: f32,
    pub tilt_correction: SpectralTiltCorrection,
    pub tilt_reference: TiltReferenceFreq,
    pub is_editing: bool,
    pub selected_param: usize,
    pub data: Option<&'a SpectrumData>,
    pub chart_height: f32,
    /// Root rem size in pixels, matching the desktop's current font zoom.
    pub rem_size_px: f32,
    /// Definite pixel width of the plugin content area. The detail panel is
    /// shrink-to-fit, so flex/`w_full` cannot size the chart — it must be
    /// given explicit pixels (same approach as the EQ chart).
    pub available_width: f32,
}

/// Render display tilt and reference selectors beside the plugin title.
#[expect(
    clippy::too_many_lines,
    reason = "Keep the declarative layout and its interaction handlers together"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
pub(crate) fn render_header(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &sotf_audio_player::PluginSettings,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let sotf_audio_player::PluginSettings::SpectrumAnalyzer {
        tilt_correction,
        tilt_reference,
        ..
    } = settings
    else {
        return div().into_any_element();
    };
    let app = &entity.read(cx).app;
    let text = SpectrumTranslations::for_language(app.ui_state.language);
    let open_tilt = app.plugin_ui.spectrum_tilt_select_open;
    let open_reference = app.plugin_ui.spectrum_reference_select_open;
    let tilt = *tilt_correction;
    let reference = *tilt_reference;
    let mut row = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(d.grid);
    for is_reference in [false, true] {
        let label = if is_reference {
            text.reference
        } else {
            text.tilt
        };
        let (options, selected) = if is_reference {
            (
                vec![
                    SelectOption::new("standard", text.standard),
                    SelectOption::new("1khz", "1 kHz"),
                    SelectOption::new("2khz", "2 kHz"),
                    SelectOption::new("minfreq", text.min_frequency_short),
                ],
                match reference {
                    TiltReferenceFreq::Standard => "standard",
                    TiltReferenceFreq::OneKilohertz => "1khz",
                    TiltReferenceFreq::TwoKilohertz => "2khz",
                    TiltReferenceFreq::MinFreq => "minfreq",
                },
            )
        } else {
            (
                vec![
                    SelectOption::new("none", text.none),
                    SelectOption::new("3db", "+3dB/oct"),
                    SelectOption::new("6db", "+6dB/oct"),
                    SelectOption::new("pink", "Pink (+3dB/oct)"),
                    SelectOption::new("custom", spectral_tilt_option_label(tilt, text.custom))
                        .disabled(true),
                ],
                match tilt {
                    SpectralTiltCorrection::None => "none",
                    SpectralTiltCorrection::ThreeDbPerOctave => "3db",
                    SpectralTiltCorrection::SixDbPerOctave => "6db",
                    SpectralTiltCorrection::Pink => "pink",
                    SpectralTiltCorrection::Custom(_) => "custom",
                },
            )
        };
        let highlight = app.plugin_state.plugin_ui_state.spectrum_header_highlight;
        let selected_index = options
            .iter()
            .position(|option| option.value.as_ref() == selected);
        let highlight_entity = entity.downgrade();
        let toggle = entity.downgrade();
        let change = entity.clone();
        let select = Select::new(if is_reference {
            "tilt-reference-select"
        } else {
            "tilt-correction-select"
        })
        .options(options)
        .highlighted_index(highlight)
        .selected(selected)
        .size(SelectSize::Xs)
        .is_open(if is_reference {
            open_reference
        } else {
            open_tilt
        })
        .theme(theme.to_select_theme())
        .aria_label(label)
        .on_toggle(move |open, _, cx| {
            let _ = toggle.update(cx, |state, cx| {
                state
                    .app
                    .plugin_state
                    .plugin_ui_state
                    .spectrum_header_highlight = open.then_some(selected_index).flatten();
                if is_reference {
                    state.app.plugin_ui.spectrum_reference_select_open = open;
                    if open {
                        state.app.plugin_ui.spectrum_tilt_select_open = false;
                    }
                } else {
                    state.app.plugin_ui.spectrum_tilt_select_open = open;
                    if open {
                        state.app.plugin_ui.spectrum_reference_select_open = false;
                    }
                }
                cx.notify();
            });
        })
        .on_highlight(move |index, _, cx| {
            let _ = highlight_entity.update(cx, |state, cx| {
                state
                    .app
                    .plugin_state
                    .plugin_ui_state
                    .spectrum_header_highlight = index;
                cx.notify();
            });
        })
        .on_change(move |value, _, cx| {
            change.update(cx, |state, cx| {
                if is_reference {
                    let reference = match value.as_ref() {
                        "standard" => TiltReferenceFreq::Standard,
                        "1khz" => TiltReferenceFreq::OneKilohertz,
                        "2khz" => TiltReferenceFreq::TwoKilohertz,
                        "minfreq" => TiltReferenceFreq::MinFreq,
                        _ => return,
                    };
                    state.app.set_spectrum_tilt_reference(plugin_idx, reference);
                    state.app.plugin_ui.spectrum_reference_select_open = false;
                } else {
                    let tilt = match value.as_ref() {
                        "none" => SpectralTiltCorrection::None,
                        "3db" => SpectralTiltCorrection::ThreeDbPerOctave,
                        "6db" => SpectralTiltCorrection::SixDbPerOctave,
                        "pink" => SpectralTiltCorrection::Pink,
                        _ => return,
                    };
                    state.app.set_spectrum_tilt_correction(plugin_idx, tilt);
                    state.app.plugin_ui.spectrum_tilt_select_open = false;
                }
                cx.notify();
            });
        });
        let select = dev_track!(
            select,
            if is_reference {
                "spectrum.header.reference"
            } else {
                "spectrum.header.tilt"
            }
        );
        row = row.child(
            div()
                .key_context("PluginLayoutChoiceSelect")
                .flex()
                .items_center()
                .gap(d.gap)
                .child(Text::label(label))
                .child(select),
        );
    }
    row.into_any_element()
}

/// Render primary analyzer controls and a full-width spectrum graph.
///
/// Display choices belong to the shell header. The primary numeric fields
/// precede the graph so they remain reachable in a short rack viewport.
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "Keep the declarative layout and its interaction handlers together"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "UI controls and graph coordinates use approximate floating-point parameter values"
)]
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
    let graph_gap = d.grid.0 * state.rem_size_px;
    let chart_width = (state.available_width - axis_theme.db_axis_width - graph_gap).max(160.0);
    let compact = state.available_width < 720.0;
    let chart_height = if compact {
        96.0
    } else {
        (chart_width * 0.26).clamp(state.chart_height, state.chart_height * 1.5)
    };
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
                        .flex()
                        .items_end()
                        .gap_px()
                        .child(if let Some(data) = state.data {
                            let magnitudes = corrected_spectrum_magnitudes(
                                data,
                                state.tilt_correction,
                                state.tilt_reference,
                                state.min_freq,
                            );
                            SpectrumElement::new(magnitudes)
                                .peak_hold(true)
                                .peak_color(theme.text_primary)
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
                .w(px(chart_width + axis_theme.db_axis_width + graph_gap))
                .gap(d.grid)
                .child(spectrum_db_axis_spacer(d, theme))
                .child(div().flex_1().child(render_spectrum_frequency_axis(
                    state.min_freq,
                    state.max_freq,
                    spectrum_axis_theme(d, theme),
                ))),
        );

    #[cfg(feature = "dev-api")]
    let spectrum_display = { spectrum_display.dev_track("spectrum.chart") };

    let mut primary = div().w_full().min_w_0().flex().flex_wrap().gap(d.grid);
    for (index, value, label) in [
        (0, state.num_bins as f64, text.bins),
        (1, f64::from(state.min_freq), text.minimum_frequency),
        (2, f64::from(state.max_freq), text.maximum_frequency),
        (3, f64::from(state.smoothing), text.smoothing),
    ] {
        let mut param = SP[index];
        param.name = label;
        primary = primary.child(
            div()
                .flex_1()
                .min_w(rems(23.0))
                .child(render_horizontal_param(
                    d,
                    entity.clone(),
                    plugin_idx,
                    index,
                    &param,
                    value,
                    state.is_editing,
                    state.selected_param,
                    true,
                    theme,
                )),
        );
    }
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(primary)
        .child(spectrum_display)
        .child(Text::caption(text.frequency_axis))
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
