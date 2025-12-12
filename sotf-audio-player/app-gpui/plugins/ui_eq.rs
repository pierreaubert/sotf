//! EQ Plugin UI Component
//!
//! Provides a professional parametric EQ visualization with:
//! - Frequency response graph
//! - Band controls with color coding
//! - Interactive editing

use super::common::render_knob;
use crate::app::AppState;
use crate::theme::Theme;
use autoeq_iir::{Biquad, BiquadFilterType};
use gpui::prelude::*;
use gpui::*;
use gpui_px::{ScaleType, line};
use sotf_audio_player::EQFilter;

/// Sample rate for filter calculations
const SAMPLE_RATE: f64 = 48000.0;

/// State for rendering the EQ plugin
pub struct EqRenderState<'a> {
    pub filters: &'a [EQFilter],
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Calculate the combined response in dB at a given frequency
fn calculate_response_at_freq(filters: &[EQFilter], freq: f64) -> f64 {
    if filters.is_empty() {
        return 0.0;
    }
    filters
        .iter()
        .map(|f| {
            let biquad = Biquad::new(
                f.filter_type.clone(),
                f.frequency,
                SAMPLE_RATE,
                f.q,
                f.gain_db,
            );
            biquad.log_result(freq)
        })
        .sum()
}

/// Band colors for EQ visualization
const BAND_COLORS: [u32; 10] = [
    0xef4444, // Red
    0xf97316, // Orange
    0xeab308, // Yellow
    0x22c55e, // Green
    0x14b8a6, // Teal
    0x3b82f6, // Blue
    0x8b5cf6, // Violet
    0xec4899, // Pink
    0x6366f1, // Indigo
    0x06b6d4, // Cyan
];

/// Calculate single band response at a frequency
fn calculate_band_response(filter: &EQFilter, freq: f64) -> f64 {
    let biquad = Biquad::new(
        filter.filter_type.clone(),
        filter.frequency,
        SAMPLE_RATE,
        filter.q,
        filter.gain_db,
    );
    biquad.log_result(freq)
}

/// Render EQ frequency response using gpui-px
///
/// Shows all filter bands overlaid on a single plot with log frequency axis
fn render_eq_visualization(
    filters: &[EQFilter],
    selected_band: Option<usize>,
    _theme: &Theme,
    width: f32,
) -> impl IntoElement {
    // Generate frequency points (logarithmically spaced from 20Hz to 20kHz)
    let num_points = 120;
    let min_freq = 20.0_f64;
    let max_freq = 20000.0_f64;

    let freq_points: Vec<f64> = (0..num_points)
        .map(|i| {
            let t = i as f64 / (num_points - 1) as f64;
            let log_min = min_freq.ln();
            let log_max = max_freq.ln();
            (log_min + t * (log_max - log_min)).exp()
        })
        .collect();

    // Calculate combined response as primary series
    let combined_response: Vec<f64> = freq_points
        .iter()
        .map(|&freq| calculate_response_at_freq(filters, freq))
        .collect();

    // Start building the chart with the combined response
    let text_muted_u32 = {
        let c = _theme.text_muted;
        ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32)
    };
    let mut chart_builder = line(&freq_points, &combined_response)
        .x_scale(ScaleType::Log)
        .y_scale(ScaleType::Linear)
        .x_label("Frequency")
        .y_label("SPL")
        .size(width, 300.0)
        .color(text_muted_u32) // Dark gray for combined from theme.text_muted
        .stroke_width(2.5)
        .label("Combined");

    // Add each filter band as an additional series
    for (i, filter) in filters.iter().enumerate() {
        let band_response: Vec<f64> = freq_points
            .iter()
            .map(|&freq| calculate_band_response(filter, freq))
            .collect();

        let color = BAND_COLORS.get(i).copied().unwrap_or(0x9ca3af);
        let is_selected = selected_band == Some(i);
        let opacity = if is_selected { 1.0 } else { 0.5 };
        let stroke = if is_selected { 2.0 } else { 1.5 };

        let label = format!(
            "#{} - {} @ {}Hz",
            i + 1,
            filter.filter_type.short_name(),
            filter.frequency as i32
        );

        chart_builder =
            chart_builder.add_series(&band_response, Some(label), color, stroke, opacity);
    }

    // Build and return the chart
    match chart_builder.build() {
        Ok(chart) => chart.into_any_element(),
        Err(_) => div()
            .w(px(width))
            .h(px(300.0))
            .flex()
            .items_center()
            .justify_center()
            .child("Unable to render chart")
            .into_any_element(),
    }
}

/// Render the EQ plugin with graphical visualization
pub fn render_eq_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: EqRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let selected_band_idx = if state.is_editing {
        // Determine which band is selected based on the selected_param
        // Each band has 4 parameters (Freq, Q, Gain, Type - though Type isn't a knob)
        // So, param_idx / 4 gives the band index.
        Some(state.selected_param / 4)
    } else {
        None
    };

    div()
        .flex()
        .flex_col()
        .gap_2()
        // EQ Graph section
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                // Title
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .mb_1()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.text_primary)
                                .child("FREQUENCY RESPONSE"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child(format!("{} bands", state.filters.len())),
                        ),
                )
                // Graph (self-contained with axes) - full width
                .child(div().w_full().child(render_eq_visualization(
                    state.filters,
                    selected_band_idx,
                    theme,
                    700.0, // Increased width for better visibility
                ))),
        )
        // Band controls section
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                // Title
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text_primary)
                        .mb_1()
                        .child("FILTER BANDS"),
                )
                // Band controls in a grid
                .child(div().flex().flex_wrap().gap_2().children(
                    state.filters.iter().enumerate().map(|(i, filter)| {
                        // Each filter has 4 params: Type (idx 3), Freq (idx 0), Q (idx 1), Gain (idx 2)
                        let base_param_idx = i * 4;

                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .p_2()
                            .bg(theme.background_secondary)
                            .min_w(px(220.0))
                            // Band header with selector in one row
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .mb_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme.text_secondary)
                                            .child(format!("#{}", i + 1)),
                                    )
                                    .child(render_filter_type_selector(
                                        entity.clone(),
                                        plugin_idx,
                                        &filter.filter_type,
                                        i,
                                        base_param_idx + 3,
                                        None,
                                        theme,
                                    )),
                            )
                            // Parameters row
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(render_knob(
                                        entity.clone(),
                                        plugin_idx,
                                        "Freq",
                                        filter.frequency,
                                        20.0,
                                        20000.0,
                                        "Hz",
                                        base_param_idx,
                                        state.selected_param,
                                        state.is_editing,
                                        None,
                                        theme,
                                    ))
                                    .child(render_knob(
                                        entity.clone(),
                                        plugin_idx,
                                        "Q",
                                        filter.q,
                                        0.1,
                                        10.0,
                                        "",
                                        base_param_idx + 1,
                                        state.selected_param,
                                        state.is_editing,
                                        None,
                                        theme,
                                    ))
                                    .child(render_knob(
                                        entity.clone(),
                                        plugin_idx,
                                        "Gain",
                                        filter.gain_db,
                                        -24.0,
                                        24.0,
                                        "dB",
                                        base_param_idx + 2,
                                        state.selected_param,
                                        state.is_editing,
                                        None,
                                        theme,
                                    )),
                            )
                    }),
                )),
        )
        // Edit mode hint
        .when(state.is_editing, |d| {
            d.child(
                div()
                    .p_2()
                    .bg(theme.accent_muted)
                    .flex()
                    .gap_3()
                    .text_xs()
                    .text_color(theme.text_secondary)
                    .child("↑/↓: Select")
                    .child("←/→: Adjust")
                    .child("[/]: Step")
                    .child("Enter: Done"),
            )
        })
}

/// Get the index of a filter type in the standard ordering
fn get_filter_type_index(filter_type: &BiquadFilterType) -> usize {
    match filter_type {
        BiquadFilterType::Peak => 0,
        BiquadFilterType::Lowshelf => 1,
        BiquadFilterType::Highshelf => 2,
        BiquadFilterType::Lowpass => 3,
        BiquadFilterType::Highpass => 4,
        BiquadFilterType::Bandpass => 5,
        BiquadFilterType::Notch => 6,
        BiquadFilterType::HighpassVariableQ => 4, // Map to Highpass
    }
}

/// Render a filter type selector using exclusive buttons
fn render_filter_type_selector(
    entity: Entity<AppState>,
    plugin_idx: usize,
    current_type: &BiquadFilterType,
    _band_idx: usize,
    param_idx: usize,
    _select_open: Option<(usize, usize)>,
    theme: &Theme,
) -> impl IntoElement {
    // Define all filter types with 2-letter abbreviations
    let filter_types: Vec<(usize, &'static str)> = vec![
        (0, "PK"), // Peak
        (1, "LS"), // Low Shelf
        (2, "HS"), // High Shelf
        (3, "LP"), // Low Pass
        (4, "HP"), // High Pass
        (5, "BP"), // Band Pass
        (6, "NO"), // Notch
    ];

    let current_index = get_filter_type_index(current_type);

    div()
        .flex()
        .flex_wrap()
        .gap_1()
        .children(filter_types.into_iter().map(move |(idx, abbrev)| {
            let is_active = idx == current_index;
            let entity_clone = entity.clone();

            div()
                .px_2()
                .py_1()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .rounded_sm()
                .cursor_pointer()
                .when(is_active, |d| {
                    d.bg(theme.accent).text_color(theme.text_primary)
                })
                .when(!is_active, |d| {
                    d.bg(theme.background_secondary)
                        .text_color(theme.text_secondary)
                        .hover(|s| s.bg(theme.surface_hover))
                })
                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                    entity_clone.update(cx, |state, _| {
                        state
                            .app
                            .set_plugin_param(plugin_idx, param_idx, idx as f64);
                    });
                })
                .child(abbrev)
        }))
}
