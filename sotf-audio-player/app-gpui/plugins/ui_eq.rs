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
use gpui_px::{ChartTheme, ScaleType, line};
// Tabs are now custom-rendered to avoid context issues
use sotf_audio_player::EQFilter;
use sotf_audio_player::param_specs::eq::*;

/// Sample rate for filter calculations
const SAMPLE_RATE: f64 = 48000.0;

/// State for rendering the EQ plugin
pub struct EqRenderState<'a> {
    pub filters: &'a [EQFilter],
    pub is_editing: bool,
    pub selected_param: usize,
    pub selected_band_idx: usize,
}

/// Calculate the combined response in dB at a given frequency
fn calculate_response_at_freq(filters: &[EQFilter], freq: f64) -> f64 {
    if filters.is_empty() {
        return 0.0;
    }
    filters
        .iter()
        .map(|f| {
            let biquad = Biquad::new(f.filter_type, f.frequency, SAMPLE_RATE, f.q, f.gain_db);
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
        filter.filter_type,
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
    theme: &Theme,
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

    // Create chart theme from app theme
    let chart_theme = ChartTheme {
        plot_background: theme.eq_curve_colors.background,
        grid_color: theme.eq_curve_colors.grid,
        axis_line_color: theme.graph_colors.grid,
        axis_label_color: theme.text_secondary,
        title_color: theme.text_primary,
        legend_text_color: theme.text_secondary,
    };

    // Convert combined line color to u32
    let text_muted_u32 = {
        let c = theme.text_muted;
        ((c.r * 255.0) as u32) << 16 | ((c.g * 255.0) as u32) << 8 | (c.b * 255.0) as u32
    };
    let mut chart_builder = line(&freq_points, &combined_response)
        .x_scale(ScaleType::Log)
        .y_scale(ScaleType::Linear)
        .x_label("Frequency")
        .y_label("SPL")
        .size(width, 300.0)
        .color(text_muted_u32) // Combined response line
        .stroke_width(2.5)
        .label("Combined")
        .theme(chart_theme);

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
            .bg(theme.eq_curve_colors.background)
            .text_color(theme.text_secondary)
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
    // Clamp selected band to valid range
    let selected_band_idx = state
        .selected_band_idx
        .min(state.filters.len().saturating_sub(1));
    let num_bands = state.filters.len();

    // Determine layout mode based on available width
    // For now, we'll default to horizontal layout
    let use_horizontal_layout = true;

    // Get the selected filter
    let selected_filter = if num_bands > 0 {
        Some(&state.filters[selected_band_idx])
    } else {
        None
    };

    // Compute selected param for editing mode
    let highlight_band_idx = if state.is_editing {
        Some(state.selected_param / 4)
    } else {
        Some(selected_band_idx)
    };

    // Build the UI
    let graph_section = div()
        .flex()
        .flex_col()
        .flex_1()
        .child(render_eq_visualization(
            state.filters,
            highlight_band_idx,
            theme,
            if use_horizontal_layout { 500.0 } else { 700.0 },
        ));

    let controls_section = div()
        .flex()
        .flex_col()
        .gap_2()
        .when(use_horizontal_layout, |d| d.min_w(px(300.0)))
        .when(!use_horizontal_layout, |d| d.w_full())
        // Band selector tabs (custom rendering to avoid context issues)
        .child({
            let mut tabs_container = div()
                .flex()
                .items_center()
                .gap_2()
                .p_1()
                .bg(theme.surface)
                .rounded_lg();

            // Build each band tab manually
            for band_idx in 0..num_bands {
                let is_selected = band_idx == selected_band_idx;
                let entity_clone = entity.clone();
                let accent = theme.accent;
                let text_primary = theme.text_primary;
                let text_secondary = theme.text_secondary;
                let bg_secondary = theme.background_secondary;
                let surface_hover = theme.surface_hover;

                let tab = div()
                    .id(SharedString::from(format!("eq-band-{}", band_idx)))
                    .px_4()
                    .py_2()
                    .text_sm()
                    .rounded_md()
                    .cursor_pointer()
                    .when(is_selected, |d| {
                        d.bg(accent)
                            .text_color(text_primary)
                            .font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(!is_selected, |d| {
                        d.bg(bg_secondary)
                            .text_color(text_secondary)
                            .hover(move |s| s.bg(surface_hover))
                    })
                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                        entity_clone.update(cx, |state, _| {
                            state.app.selected_eq_band = band_idx;
                        });
                    })
                    .child(format!("{}", band_idx + 1));

                tabs_container = tabs_container.child(tab);
            }

            tabs_container
                // Add band button
                .child({
                    let entity_clone = entity.clone();
                    div()
                        .px_3()
                        .py_1p5()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .rounded_sm()
                        .cursor_pointer()
                        .bg(theme.success)
                        .text_color(theme.text_on_accent)
                        .hover(|s| s.opacity(0.8))
                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                            entity_clone.update(cx, |_state, _| {
                                // TODO: Implement add band functionality
                                // This would need to call a method on the player to add a new EQ filter
                            });
                        })
                        .child("+")
                })
        })
        // Selected band controls
        .when(selected_filter.is_some(), |d| {
            let filter = selected_filter.unwrap();
            let base_param_idx = selected_band_idx * 4;

            d.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .bg(theme.background_secondary)
                    .rounded_md()
                    // Filter type selector
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_xs().text_color(theme.text_muted).child("Type"))
                            .child(render_filter_type_selector(
                                entity.clone(),
                                plugin_idx,
                                &filter.filter_type,
                                selected_band_idx,
                                base_param_idx + 3,
                                None,
                                theme,
                            )),
                    )
                    // Knobs row
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .justify_around()
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Freq",
                                filter.frequency,
                                FREQUENCY_MIN,
                                FREQUENCY_MAX,
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
                                Q_MIN,
                                Q_MAX,
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
                                GAIN_DB_MIN,
                                GAIN_DB_MAX,
                                "dB",
                                base_param_idx + 2,
                                state.selected_param,
                                state.is_editing,
                                None,
                                theme,
                            )),
                    ),
            )
        });

    // Combine sections based on layout mode
    let main_container = if use_horizontal_layout {
        // Horizontal: graph on left, controls on right
        div()
            .flex()
            .flex_row()
            .gap_3()
            .child(graph_section)
            .child(controls_section)
    } else {
        // Vertical: graph on top, controls below
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(graph_section)
            .child(controls_section)
    };

    main_container
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
