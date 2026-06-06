//! Spectrum Analyzer UI Components

// intentional-file: spectrum analyzer with chart-internal pixel dimensions

use std::sync::Arc;

use gpui::prelude::*;
use gpui::*;
use gpui_audio_kit::{SpectrumColors, SpectrumElement};
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

// ============================================================================
// Axis Components
// ============================================================================

/// Format frequency value for axis label
fn format_freq_label(freq: f32) -> String {
    if freq >= 1000.0 {
        let khz = freq / 1000.0;
        if khz == khz.floor() {
            format!("{}k", khz as i32)
        } else {
            format!("{:.1}k", khz)
        }
    } else if freq == freq.floor() {
        format!("{}", freq as i32)
    } else {
        format!("{:.0}", freq)
    }
}

/// Calculate logarithmic position of a frequency within a range
fn freq_to_log_position(freq: f32, min_freq: f32, max_freq: f32) -> f32 {
    let log_min = min_freq.log10();
    let log_max = max_freq.log10();
    let log_freq = freq.log10();
    (log_freq - log_min) / (log_max - log_min)
}

/// Generate frequency labels for the given range
fn generate_freq_labels(min_freq: f32, max_freq: f32) -> Vec<(String, f32)> {
    // Standard frequency points to consider
    let all_freqs: [f32; 15] = [
        20.0, 30.0, 50.0, 100.0, 200.0, 300.0, 500.0, 1000.0, 2000.0, 3000.0, 5000.0, 10000.0,
        15000.0, 20000.0, 24000.0,
    ];

    let mut labels = Vec::new();

    // Always include min and max
    labels.push((format_freq_label(min_freq), 0.0));

    // Add intermediate labels that fall within range
    for &freq in &all_freqs {
        if freq > min_freq * 1.1 && freq < max_freq * 0.9 {
            let pos = freq_to_log_position(freq, min_freq, max_freq);
            labels.push((format_freq_label(freq), pos));
        }
    }

    labels.push((format_freq_label(max_freq), 1.0));

    // Filter to avoid overlapping labels (keep at least 0.08 apart)
    let mut filtered = Vec::new();
    for (label, pos) in labels {
        if filtered.is_empty()
            || filtered
                .last()
                .map(|(_, last_pos): &(String, f32)| pos - last_pos > 0.08)
                .unwrap_or(true)
        {
            filtered.push((label, pos));
        }
    }

    filtered
}

/// Render horizontal frequency axis (logarithmic scale)
fn render_frequency_axis(d: &Ds, min_freq: f32, max_freq: f32, theme: &Theme) -> impl IntoElement {
    let freq_labels = generate_freq_labels(min_freq, max_freq);

    div()
        .w_full()
        // intentional: axis label row height — chart-internal layout
        .h(px(20.0))
        .relative()
        .children(freq_labels.into_iter().map(|(label, pos)| {
            div()
                .absolute()
                .left(relative(pos))
                .top_0()
                .text_size(d.text_xs)
                .text_color(theme.text_muted)
                .child(
                    div()
                        // intentional: pixel-exact label centering offset
                        .ml(px(-12.0))
                        .child(label),
                )
        }))
}

/// Render vertical dB axis (-60dB to +3dB)
fn render_db_axis(d: &Ds, theme: &Theme) -> impl IntoElement {
    // Range: -100 dB to +3 dB (103 dB total)
    // Position = (3 - db) / 103
    let db_labels = [
        ("+3", 0.0),    // (3 - 3) / 103 = 0.0
        ("0", 0.029),   // (3 - 0) / 103 ≈ 0.029
        ("-20", 0.223), // (3 - (-20)) / 103 ≈ 0.223
        ("-40", 0.417), // (3 - (-40)) / 103 ≈ 0.417
        ("-60", 0.612), // (3 - (-60)) / 103 ≈ 0.612
    ];

    div()
        // intentional: dB axis column width — chart-internal layout
        .w(px(32.0))
        .h_full()
        .flex()
        .flex_col()
        .relative()
        .children(db_labels.iter().map(|(label, pos)| {
            div()
                .absolute()
                .top(relative(*pos as f32))
                .right_0()
                .text_size(d.text_xs)
                .text_color(theme.text_muted)
                .pr(d.grid)
                .child(
                    div()
                        // intentional: pixel-exact label centering offset
                        .mt(px(-6.0))
                        .child(*label),
                )
        }))
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
                .child(render_db_axis(d, theme))
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
        .child(
            div()
                .flex()
                .child(div().w(px(32.0)))
                .child(render_frequency_axis(
                    d,
                    state.min_freq,
                    state.max_freq,
                    theme,
                )),
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
                        .child(render_db_axis(&d, &theme))
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
                        .child(div().w(px(32.0))) // Spacer to align with dB axis
                        .child(render_frequency_axis(&d, 20.0, 20000.0, &theme)),
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
