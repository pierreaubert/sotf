//! EQ Plugin UI Component
//!
//! Provides a professional parametric EQ visualization with:
//! - Frequency response graph
//! - Band controls with color coding
//! - Interactive editing

use super::common::render_knob_sized;
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_px::{ChartTheme, ScaleType, line};
use gpui_ui_kit::PotentiometerSize;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
// Tabs are now custom-rendered to avoid context issues
use sotf_audio_player::EQFilter;
use sotf_audio_player::param_specs::eq::*;

/// Sample rate for filter calculations
const SAMPLE_RATE: f64 = 48000.0;

/// Q handle bar constants
const Q_BAR_MIN_WIDTH: f32 = 40.0;
const Q_BAR_MAX_WIDTH: f32 = 100.0;
const Q_HANDLE_RADIUS: f32 = 5.0;
const Q_BAR_HEIGHT: f32 = 3.0;

/// Convert Q value to bar width (inverse: higher Q = narrower bar)
fn q_to_bar_width(q: f64) -> f32 {
    let t = ((q - Q_MIN) / (Q_MAX - Q_MIN)).clamp(0.0, 1.0) as f32;
    // Inverse mapping: Q_MIN -> max width, Q_MAX -> min width
    Q_BAR_MAX_WIDTH - t * (Q_BAR_MAX_WIDTH - Q_BAR_MIN_WIDTH)
}

/// Convert horizontal drag delta to Q change
/// Positive delta (dragging right handle right) = increase Q
/// Negative delta (dragging left handle left) = decrease Q
fn drag_delta_to_q_change(delta_px: f32) -> f64 {
    // Scale factor: moving 30px should roughly change Q by the full range
    let scale = (Q_MAX - Q_MIN) / 60.0;
    delta_px as f64 * scale
}

/// Drag data for EQ control point manipulation (frequency/gain)
#[derive(Clone)]
struct EqControlPointDrag {
    band_idx: usize,
    plugin_idx: usize,
    color: u32,
}

impl Render for EqControlPointDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let rgba_color = gpui::rgba(self.color * 256 + 0xFF);
        div()
            .w(px(CONTROL_POINT_RADIUS * 3.0))
            .h(px(CONTROL_POINT_RADIUS * 3.0))
            .rounded_full()
            .bg(rgba_color)
            .border(px(2.0))
            .border_color(gpui::white())
            .shadow_lg()
    }
}

/// Drag data for Q handle manipulation
#[derive(Clone)]
struct EqQHandleDrag {
    band_idx: usize,
    plugin_idx: usize,
    is_right_handle: bool, // true = right handle (increase Q), false = left handle (decrease Q)
    start_x: f32,
    start_q: f64,
    color: u32,
}

impl Render for EqQHandleDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let rgba_color = gpui::rgba(self.color * 256 + 0xFF);
        div()
            .w(px(Q_HANDLE_RADIUS * 2.0))
            .h(px(Q_HANDLE_RADIUS * 2.0))
            .rounded_full()
            .bg(rgba_color)
            .border(px(1.0))
            .border_color(gpui::white())
            .shadow_md()
    }
}

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
    let any_soloed = filters.iter().any(|f| f.solo);
    filters
        .iter()
        .filter(|f| {
            if f.muted {
                return false;
            }
            if any_soloed && !f.solo {
                return false;
            }
            true
        })
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
    if filter.muted {
        return 0.0;
    }
    let biquad = Biquad::new(
        filter.filter_type,
        filter.frequency,
        SAMPLE_RATE,
        filter.q,
        filter.gain_db,
    );
    biquad.log_result(freq)
}

/// Chart layout constants for control point positioning
const CHART_LEFT_MARGIN: f32 = 60.0;
const CHART_RIGHT_MARGIN: f32 = 180.0; // Space for legend
const CHART_TOP_MARGIN: f32 = 20.0;
const CHART_BOTTOM_MARGIN: f32 = 40.0;
const CHART_HEIGHT: f32 = 300.0;
const MIN_FREQ: f64 = 20.0;
const MAX_FREQ: f64 = 20000.0;
const MIN_GAIN_DB: f64 = -24.0;
const MAX_GAIN_DB: f64 = 24.0;
const CONTROL_POINT_RADIUS: f32 = 8.0;

/// Convert frequency (Hz) to x pixel position
fn freq_to_x(freq: f64, plot_width: f32) -> f32 {
    let log_min = MIN_FREQ.ln();
    let log_max = MAX_FREQ.ln();
    let t = (freq.ln() - log_min) / (log_max - log_min);
    CHART_LEFT_MARGIN + (t as f32) * plot_width
}

/// Convert x pixel position to frequency (Hz)
fn x_to_freq(x: f32, plot_width: f32) -> f64 {
    let t = ((x - CHART_LEFT_MARGIN) / plot_width).clamp(0.0, 1.0) as f64;
    let log_min = MIN_FREQ.ln();
    let log_max = MAX_FREQ.ln();
    (log_min + t * (log_max - log_min)).exp()
}

/// Convert gain (dB) to y pixel position
fn gain_to_y(gain_db: f64) -> f32 {
    let plot_height = CHART_HEIGHT - CHART_TOP_MARGIN - CHART_BOTTOM_MARGIN;
    let t = (MAX_GAIN_DB - gain_db) / (MAX_GAIN_DB - MIN_GAIN_DB);
    CHART_TOP_MARGIN + (t as f32) * plot_height
}

/// Convert y pixel position to gain (dB)
fn y_to_gain(y: f32) -> f64 {
    let plot_height = CHART_HEIGHT - CHART_TOP_MARGIN - CHART_BOTTOM_MARGIN;
    let t = ((y - CHART_TOP_MARGIN) / plot_height).clamp(0.0, 1.0) as f64;
    MAX_GAIN_DB - t * (MAX_GAIN_DB - MIN_GAIN_DB)
}

/// Render EQ frequency response using gpui-px with draggable control points
///
/// Shows all filter bands overlaid on a single plot with log frequency axis
fn render_eq_visualization(
    entity: Entity<AppState>,
    plugin_idx: usize,
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
        .y_label("dB")
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
        let is_muted = filter.muted;
        let is_soloed = filter.solo;
        let any_soloed = filters.iter().any(|f| f.solo);
        let effective_muted = is_muted || (any_soloed && !is_soloed);
        let opacity = if is_selected { 1.0 } else { 0.5 };
        let stroke = if is_selected { 2.0 } else { 1.5 };
        let opacity = if effective_muted { 0.2 } else { opacity };

        let status = if is_muted && is_soloed {
            " (muted+solo)"
        } else if is_muted {
            " (muted)"
        } else if is_soloed {
            " (solo)"
        } else if any_soloed {
            " (silent)"
        } else {
            ""
        };

        let label = format!(
            "#{} - {} @ {}Hz{}",
            i + 1,
            filter.filter_type.short_name(),
            filter.frequency as i32,
            status
        );

        chart_builder =
            chart_builder.add_series(&band_response, Some(label), color, stroke, opacity);
    }

    // Calculate plot width for control point positioning
    let plot_width = width - CHART_LEFT_MARGIN - CHART_RIGHT_MARGIN;

    // Build the chart element
    let chart_element = match chart_builder.build() {
        Ok(chart) => chart.into_any_element(),
        Err(_) => div()
            .w(px(width))
            .h(px(CHART_HEIGHT))
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.eq_curve_colors.background)
            .text_color(theme.text_secondary)
            .child("Unable to render chart")
            .into_any_element(),
    };

    // Create control points for each filter
    let mut control_points: Vec<AnyElement> = Vec::new();
    for (i, filter) in filters.iter().enumerate() {
        let is_selected = selected_band == Some(i);
        let color = BAND_COLORS.get(i).copied().unwrap_or(0x9ca3af);
        let rgba_color = gpui::rgba(color as u32 * 256 + 0xFF);

        // Calculate position
        let x = freq_to_x(filter.frequency, plot_width);
        let y = gain_to_y(filter.gain_db);

        let entity_clone = entity.clone();
        let band_idx = i;

        // Control point circle
        let border_color = if is_selected {
            gpui::white()
        } else {
            gpui::hsla(0.0, 0.0, 1.0, 0.5) // semi-transparent white
        };

        // Calculate Q bar width
        let bar_width = q_to_bar_width(filter.q);
        let bar_half_width = bar_width / 2.0;

        // Q bar (horizontal line through control point)
        let q_bar = div()
            .absolute()
            .left(px(x - bar_half_width))
            .top(px(y - Q_BAR_HEIGHT / 2.0))
            .w(px(bar_width))
            .h(px(Q_BAR_HEIGHT))
            .bg(rgba_color)
            .rounded(px(Q_BAR_HEIGHT / 2.0))
            .opacity(if is_selected { 0.8 } else { 0.5 })
            .into_any_element();

        control_points.push(q_bar);

        // Left Q handle (decrease Q when dragged left)
        let left_handle = {
            let entity_left = entity.clone();
            let current_q = filter.q;
            div()
                .id(("eq-q-left", i))
                .absolute()
                .left(px(x - bar_half_width - Q_HANDLE_RADIUS))
                .top(px(y - Q_HANDLE_RADIUS))
                .w(px(Q_HANDLE_RADIUS * 2.0))
                .h(px(Q_HANDLE_RADIUS * 2.0))
                .rounded_full()
                .bg(rgba_color)
                .border(px(1.0))
                .border_color(if is_selected {
                    gpui::white()
                } else {
                    gpui::hsla(0.0, 0.0, 1.0, 0.4)
                })
                .cursor(gpui::CursorStyle::ResizeLeftRight)
                .hover(|s| s.size(px(Q_HANDLE_RADIUS * 2.5)))
                .on_drag(
                    EqQHandleDrag {
                        band_idx,
                        plugin_idx,
                        is_right_handle: false,
                        start_x: x - bar_half_width,
                        start_q: current_q,
                        color,
                    },
                    |drag, _, _, cx| {
                        cx.stop_propagation();
                        cx.new(|_| drag.clone())
                    },
                )
                .on_drag_move::<EqQHandleDrag>({
                    move |event, window, cx| {
                        let drag_data = event.drag(cx);
                        let position = event.event.position;
                        let x_px: f32 = position.x.into();

                        // For left handle: moving left decreases Q, moving right increases Q
                        let delta = drag_data.start_x - x_px;
                        let q_change = drag_delta_to_q_change(delta);
                        let new_q = (drag_data.start_q + q_change).clamp(Q_MIN, Q_MAX);

                        let plugin_idx = drag_data.plugin_idx;
                        let band_idx = drag_data.band_idx;

                        entity_left.update(cx, |state, cx| {
                            state.app.editing_plugin_index = Some(plugin_idx);
                            // Update Q (param index = band_idx * 4 + 1)
                            state
                                .app
                                .set_plugin_param(plugin_idx, band_idx * 4 + 1, new_q);
                            cx.notify();
                        });
                        window.refresh();
                    }
                })
                .into_any_element()
        };

        control_points.push(left_handle);

        // Right Q handle (increase Q when dragged right)
        let right_handle = {
            let entity_right = entity.clone();
            let current_q = filter.q;
            div()
                .id(("eq-q-right", i))
                .absolute()
                .left(px(x + bar_half_width - Q_HANDLE_RADIUS))
                .top(px(y - Q_HANDLE_RADIUS))
                .w(px(Q_HANDLE_RADIUS * 2.0))
                .h(px(Q_HANDLE_RADIUS * 2.0))
                .rounded_full()
                .bg(rgba_color)
                .border(px(1.0))
                .border_color(if is_selected {
                    gpui::white()
                } else {
                    gpui::hsla(0.0, 0.0, 1.0, 0.4)
                })
                .cursor(gpui::CursorStyle::ResizeLeftRight)
                .hover(|s| s.size(px(Q_HANDLE_RADIUS * 2.5)))
                .on_drag(
                    EqQHandleDrag {
                        band_idx,
                        plugin_idx,
                        is_right_handle: true,
                        start_x: x + bar_half_width,
                        start_q: current_q,
                        color,
                    },
                    |drag, _, _, cx| {
                        cx.stop_propagation();
                        cx.new(|_| drag.clone())
                    },
                )
                .on_drag_move::<EqQHandleDrag>({
                    move |event, window, cx| {
                        let drag_data = event.drag(cx);
                        let position = event.event.position;
                        let x_px: f32 = position.x.into();

                        // For right handle: moving right increases Q, moving left decreases Q
                        let delta = x_px - drag_data.start_x;
                        let q_change = drag_delta_to_q_change(delta);
                        let new_q = (drag_data.start_q + q_change).clamp(Q_MIN, Q_MAX);

                        let plugin_idx = drag_data.plugin_idx;
                        let band_idx = drag_data.band_idx;

                        entity_right.update(cx, |state, cx| {
                            state.app.editing_plugin_index = Some(plugin_idx);
                            // Update Q (param index = band_idx * 4 + 1)
                            state
                                .app
                                .set_plugin_param(plugin_idx, band_idx * 4 + 1, new_q);
                            cx.notify();
                        });
                        window.refresh();
                    }
                })
                .into_any_element()
        };

        control_points.push(right_handle);

        // Main control point circle (rendered on top)
        let control_point = div()
            .id(("eq-control-point", i))
            .absolute()
            .left(px(x - CONTROL_POINT_RADIUS))
            .top(px(y - CONTROL_POINT_RADIUS))
            .w(px(CONTROL_POINT_RADIUS * 2.0))
            .h(px(CONTROL_POINT_RADIUS * 2.0))
            .rounded_full()
            .bg(rgba_color)
            .border(px(2.0))
            .border_color(border_color)
            .shadow_md()
            .cursor(gpui::CursorStyle::PointingHand)
            .hover(|s| s.size(px(CONTROL_POINT_RADIUS * 2.5)))
            .on_mouse_down(MouseButton::Left, {
                let entity_click = entity.clone();
                move |_event, _window, cx| {
                    // Select this band when clicking on it
                    entity_click.update(cx, |state, _| {
                        state.app.selected_eq_band = band_idx;
                    });
                }
            })
            .on_drag(
                EqControlPointDrag {
                    band_idx,
                    plugin_idx,
                    color,
                },
                |drag, _, _, cx| {
                    cx.stop_propagation();
                    cx.new(|_| drag.clone())
                },
            )
            .on_drag_move::<EqControlPointDrag>({
                let plot_width = plot_width;
                move |event, window, cx| {
                    let drag_data = event.drag(cx);
                    let position = event.event.position;

                    // Convert position to freq/gain using Into<f32> for Pixels
                    let x_px: f32 = position.x.into();
                    let y_px: f32 = position.y.into();
                    let new_freq = x_to_freq(x_px, plot_width).clamp(MIN_FREQ, MAX_FREQ);
                    let new_gain = y_to_gain(y_px).clamp(MIN_GAIN_DB, MAX_GAIN_DB);

                    let plugin_idx = drag_data.plugin_idx;
                    let band_idx = drag_data.band_idx;

                    entity_clone.update(cx, |state, cx| {
                        state.app.editing_plugin_index = Some(plugin_idx);
                        // Update frequency (param index = band_idx * 4 + 0)
                        state
                            .app
                            .set_plugin_param(plugin_idx, band_idx * 4, new_freq);
                        // Update gain (param index = band_idx * 4 + 2)
                        state
                            .app
                            .set_plugin_param(plugin_idx, band_idx * 4 + 2, new_gain);
                        cx.notify();
                    });
                    window.refresh();
                }
            })
            .into_any_element();

        control_points.push(control_point);
    }

    // Wrap chart and control points in a relative container
    div()
        .relative()
        .w(px(width))
        .h(px(CHART_HEIGHT))
        .child(chart_element)
        .children(control_points)
        .into_any_element()
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
    // For now, we'll default to vertical layout
    let use_horizontal_layout = false;

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

    // Calculate graph width dynamically based on estimated legend space
    // Worst case legend label: "#10 - HS @ 20000Hz (muted+solo)" ≈ 35 chars
    const CHAR_WIDTH_PX: f32 = 7.5;
    const LEGEND_LABEL_CHARS: f32 = 35.0;
    const LEGEND_PADDING_PX: f32 = 60.0; // margins, color swatch, etc.
    let estimated_legend_width = LEGEND_LABEL_CHARS * CHAR_WIDTH_PX + LEGEND_PADDING_PX;

    // Base available width (typical window sizes)
    let base_available_width = if use_horizontal_layout { 900.0 } else { 1500.0 };
    let graph_width = base_available_width - estimated_legend_width;

    // Build the UI - graph uses most of the horizontal space
    let graph_section = div()
        .flex()
        .flex_col()
        .flex_1()
        .child(render_eq_visualization(
            entity.clone(),
            plugin_idx,
            state.filters,
            highlight_band_idx,
            theme,
            graph_width,
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
                let filter = state.filters.get(band_idx);
                let is_muted = filter.map(|f| f.muted).unwrap_or(false);
                let is_soloed = filter.map(|f| f.solo).unwrap_or(false);
                let filter_short_name = filter.map(|f| f.filter_type.short_name()).unwrap_or("PK");
                let entity_clone = entity.clone();
                let accent = theme.accent;
                let text_on_accent = theme.text_on_accent;
                let text_secondary = theme.text_secondary;
                let text_muted_color = theme.text_muted;
                let text_primary = theme.text_primary;
                let bg_secondary = theme.background_secondary;
                let surface_hover = theme.surface_hover;
                let error = theme.error;
                let success = theme.success;
                let border = theme.border;

                let tab = div()
                    .id(("eq-band", band_idx))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .rounded_md()
                    .cursor_pointer()
                    .when(is_selected, |d| {
                        d.bg(accent)
                            .text_color(text_on_accent)
                            .font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(!is_selected, |d| {
                        d.bg(bg_secondary)
                            .text_color(text_secondary)
                            .hover(move |s| s.bg(surface_hover))
                    })
                    .when(is_muted, |d| d.opacity(0.5))
                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                        entity_clone.update(cx, |state, _| {
                            state.app.selected_eq_band = band_idx;
                        });
                    })
                    // Band number with filter type short code (e.g., "#1 PK")
                    .child(div().child(format!("#{} {}", band_idx + 1, filter_short_name)))
                    // Mute and Solo buttons row
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            // Mute button (small circle)
                            .child({
                                let entity_clone2 = entity.clone();
                                div()
                                    .w(px(18.0))
                                    .h(px(18.0))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(if is_muted { error } else { bg_secondary })
                                    .border(px(1.0))
                                    .border_color(if is_muted { error } else { border })
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .cursor_pointer()
                                    .when(is_muted, |d| d.text_color(text_primary))
                                    .when(!is_muted, |d| d.text_color(text_muted_color))
                                    .hover(move |s| {
                                        s.bg(if is_muted { error } else { surface_hover })
                                    })
                                    .on_mouse_down(MouseButton::Left, {
                                        let plugin_idx = plugin_idx;
                                        move |_event, _window, cx| {
                                            cx.stop_propagation();
                                            entity_clone2.update(cx, |state, cx| {
                                                state.app.editing_plugin_index = Some(plugin_idx);
                                                if let Err(e) =
                                                    state.app.toggle_eq_band_mute(band_idx)
                                                {
                                                    log::warn!(
                                                        "Failed to toggle EQ band mute: {}",
                                                        e
                                                    );
                                                }
                                                cx.notify();
                                            });
                                        }
                                    })
                                    .child("M")
                            })
                            // Solo button (small circle)
                            .child({
                                let entity_clone3 = entity.clone();
                                div()
                                    .w(px(18.0))
                                    .h(px(18.0))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(if is_soloed { success } else { bg_secondary })
                                    .border(px(1.0))
                                    .border_color(if is_soloed { success } else { border })
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .cursor_pointer()
                                    .when(is_soloed, |d| d.text_color(text_primary))
                                    .when(!is_soloed, |d| d.text_color(text_muted_color))
                                    .hover(move |s| {
                                        s.bg(if is_soloed { success } else { surface_hover })
                                    })
                                    .on_mouse_down(MouseButton::Left, {
                                        let plugin_idx = plugin_idx;
                                        move |_event, _window, cx| {
                                            cx.stop_propagation();
                                            entity_clone3.update(cx, |state, cx| {
                                                state.app.editing_plugin_index = Some(plugin_idx);
                                                if let Err(e) =
                                                    state.app.toggle_eq_band_solo(band_idx)
                                                {
                                                    log::warn!(
                                                        "Failed to toggle EQ band solo: {}",
                                                        e
                                                    );
                                                }
                                                cx.notify();
                                            });
                                        }
                                    })
                                    .child("S")
                            }),
                    );

                tabs_container = tabs_container.child(tab);
            }

            tabs_container
                // Add band button
                .child({
                    let entity_clone = entity.clone();
                    let plugin_idx = plugin_idx;
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
                            entity_clone.update(cx, |state, cx| {
                                state.app.editing_plugin_index = Some(plugin_idx);
                                if let Err(e) = state.app.add_eq_band() {
                                    log::warn!("Failed to add EQ band: {}", e);
                                }
                                cx.notify();
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
                            .child(render_knob_sized(
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
                                PotentiometerSize::Sm,
                                theme,
                            ))
                            .child(render_knob_sized(
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
                                PotentiometerSize::Sm,
                                theme,
                            ))
                            .child(render_knob_sized(
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
                                PotentiometerSize::Sm,
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
                    d.bg(theme.accent).text_color(theme.text_on_accent)
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
