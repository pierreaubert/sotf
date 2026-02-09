//! EQ Plugin UI Component
//!
//! Provides a professional parametric EQ visualization with:
//! - Frequency response graph
//! - Band controls with color coding
//! - Interactive editing

use super::common::render_knob_sized;
use crate::app::AppState;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_px::{ChartTheme, ScaleType, line};
use gpui_ui_kit::PotentiometerSize;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
// Tabs are now custom-rendered to avoid context issues
use sotf_audio_player::EQFilter;
use sotf_audio_player::param_specs::eq::*;

use std::cell::RefCell;
use std::rc::Rc;

/// Sample rate for filter calculations
const SAMPLE_RATE: f64 = 48000.0;

/// Wrapper element to capture bounds for coordinate transformation
struct EqChartWrapper {
    child: AnyElement,
    bounds_ref: Rc<RefCell<Option<Bounds<Pixels>>>>,
}

impl EqChartWrapper {
    fn new(child: AnyElement, bounds_ref: Rc<RefCell<Option<Bounds<Pixels>>>>) -> Self {
        Self { child, bounds_ref }
    }
}

impl IntoElement for EqChartWrapper {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for EqChartWrapper {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        Some(std::panic::Location::caller())
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = self.child.request_layout(window, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.child.prepaint(window, cx);
        ()
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        *self.bounds_ref.borrow_mut() = Some(bounds);
        self.child.paint(window, cx);
    }
}

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
    start_freq: f64,
    start_gain: f64,
    start_x: f32,
    start_y: f32,
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
    /// Number of channels
    pub channels: usize,
    /// Global filters (used when per_channel_mode is false)
    pub filters: &'a [EQFilter],
    /// Per-channel filters (used when per_channel_mode is true)
    pub channel_filters: &'a Option<Vec<Vec<EQFilter>>>,
    /// Whether to use per-channel mode
    pub per_channel_mode: bool,
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
/// These MUST match gpui-px line chart margins (see gpui-px/src/line.rs)
const CHART_LEFT_MARGIN: f32 = 50.0; // gpui-px margin_left
const CHART_RIGHT_MARGIN: f32 = 20.0; // gpui-px margin_right (no secondary axis)
// Note: gpui-px subtracts margin_top from plot_height but doesn't render top padding
// so control points should use 0 for top margin offset
// UPDATE: gpui-px renders the plot area starting at margin_top offset.
// We must match this offset for control points to align with the rendered curve.
const CHART_TOP_MARGIN: f32 = 5.0; // Adjusted to 5.0 for better alignment
const CHART_BOTTOM_MARGIN: f32 = 40.0; // Increased to 40.0 to avoid legend overlap
const CHART_HEIGHT: f32 = 300.0;
// gpui-px uses 10.0 for margin_top in plot_height calculation by default
const GPUI_PX_MARGIN_TOP: f32 = 10.0;
const MIN_FREQ: f64 = 20.0;
const MAX_FREQ: f64 = 20000.0;
const CONTROL_POINT_RADIUS: f32 = 8.0;

/// Calculate dynamic y-axis range based on filter gains.
/// Returns (min_db, max_db) for the chart y-axis.
///
/// Logic:
/// - If max absolute gain <= 0.5 dB, use -1.0 to +1.0 dB
/// - Otherwise, multiply by 1.2 and round to next integer (separately for upper/lower)
fn calculate_dynamic_y_range(filters: &[EQFilter]) -> (f64, f64) {
    if filters.is_empty() {
        return (-1.0, 1.0);
    }

    // Find min and max gain across all filters
    let mut min_gain = 0.0_f64;
    let mut max_gain = 0.0_f64;

    for filter in filters {
        if filter.gain_db < min_gain {
            min_gain = filter.gain_db;
        }
        if filter.gain_db > max_gain {
            max_gain = filter.gain_db;
        }
    }

    // Calculate upper bound
    let upper_bound = if max_gain <= 0.5 {
        1.0
    } else {
        (max_gain * 1.2).ceil()
    };

    // Calculate lower bound
    let lower_bound = if min_gain.abs() <= 0.5 {
        -1.0
    } else {
        (min_gain * 1.2).floor()
    };

    (lower_bound, upper_bound)
}

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

/// Convert gain (dB) to y pixel position with dynamic range
fn gain_to_y(gain_db: f64, min_db: f64, max_db: f64) -> f32 {
    // gpui-px calculates plot_height = height - margin_top(10) - margin_bottom(30)
    // but renders the plot starting at y=0 (no actual top margin offset)
    let plot_height = CHART_HEIGHT - GPUI_PX_MARGIN_TOP - CHART_BOTTOM_MARGIN;
    let t = (max_db - gain_db) / (max_db - min_db);
    CHART_TOP_MARGIN + (t as f32) * plot_height
}

/// Convert y pixel position to gain (dB) with dynamic range
fn y_to_gain(y: f32, min_db: f64, max_db: f64) -> f64 {
    let plot_height = CHART_HEIGHT - GPUI_PX_MARGIN_TOP - CHART_BOTTOM_MARGIN;
    let t = ((y - CHART_TOP_MARGIN) / plot_height).clamp(0.0, 1.0) as f64;
    max_db - t * (max_db - min_db)
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
    // Calculate dynamic y-axis range based on filter gains
    let (min_db, max_db) = calculate_dynamic_y_range(filters);

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

    // Build labels first so we can calculate plot width accurately
    let mut labels: Vec<String> = vec!["Combined".to_string()];
    for (i, filter) in filters.iter().enumerate() {
        let is_muted = filter.muted;
        let is_soloed = filter.solo;
        let any_soloed = filters.iter().any(|f| f.solo);

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

        labels.push(format!(
            "#{} - {} @ {}Hz{}",
            i + 1,
            filter.filter_type.short_name(),
            filter.frequency as i32,
            status
        ));
    }

    // Calculate plot width using the same algorithm as gpui-px
    let plot_width = calculate_plot_width(width, labels.iter().map(|s| s.as_str()));

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
        .x_range(MIN_FREQ, MAX_FREQ)
        .y_range(min_db, max_db) // Dynamic Y range based on filter gains
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

        // Use pre-computed label
        let label = labels[i + 1].clone();

        chart_builder =
            chart_builder.add_series(&band_response, Some(label), color, stroke, opacity);
    }

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
    // Shared bounds reference for drag handlers
    let bounds_ref = Rc::new(RefCell::new(None::<Bounds<Pixels>>));

    for (i, filter) in filters.iter().enumerate() {
        let is_selected = selected_band == Some(i);
        let color = BAND_COLORS.get(i).copied().unwrap_or(0x9ca3af);
        let rgba_color = gpui::rgba(color as u32 * 256 + 0xFF);

        // Calculate position
        let x = freq_to_x(filter.frequency, plot_width);
        let y = gain_to_y(filter.gain_db, min_db, max_db);

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
            let bounds_ref = bounds_ref.clone();
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
                        let bounds = if let Some(b) = *bounds_ref.borrow() {
                            b
                        } else {
                            return;
                        };
                        let drag_data = event.drag(cx);
                        let position = event.event.position;
                        // Convert global mouse X to local chart coordinate
                        let x_px: f32 = (position.x - bounds.origin.x).into();

                        // For left handle: moving left decreases Q, moving right increases Q
                        // drag_data.start_x is in local coordinates
                        let delta = drag_data.start_x - x_px;
                        let q_change = drag_delta_to_q_change(delta);
                        let new_q = (drag_data.start_q + q_change).clamp(Q_MIN, Q_MAX);

                        let plugin_idx = drag_data.plugin_idx;
                        let band_idx = drag_data.band_idx;

                        entity_left.update(cx, |state, cx| {
                            state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                            // Update Q (param index = band_idx * 4 + 1)
                            state
                                .app
                                .set_plugin_param(plugin_idx, band_idx * 4 + 1, new_q);
                            cx.notify();
                        });
                        // window.refresh(); // Not needed with cx.notify()
                    }
                })
                .into_any_element()
        };

        control_points.push(left_handle);

        // Right Q handle (increase Q when dragged right)
        let right_handle = {
            let entity_right = entity.clone();
            let current_q = filter.q;
            let bounds_ref = bounds_ref.clone();
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
                        let bounds = if let Some(b) = *bounds_ref.borrow() {
                            b
                        } else {
                            return;
                        };
                        let drag_data = event.drag(cx);
                        let position = event.event.position;
                        // Convert global mouse X to local chart coordinate
                        let x_px: f32 = (position.x - bounds.origin.x).into();

                        // For right handle: moving right increases Q, moving left decreases Q
                        let delta = x_px - drag_data.start_x;
                        let q_change = drag_delta_to_q_change(delta);
                        let new_q = (drag_data.start_q + q_change).clamp(Q_MIN, Q_MAX);

                        let plugin_idx = drag_data.plugin_idx;
                        let band_idx = drag_data.band_idx;

                        entity_right.update(cx, |state, cx| {
                            state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                            // Update Q (param index = band_idx * 4 + 1)
                            state
                                .app
                                .set_plugin_param(plugin_idx, band_idx * 4 + 1, new_q);
                            cx.notify();
                        });
                        // window.refresh();
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
                move |event, _window, cx| {
                    if event.click_count >= 2 {
                        // Double-click: reset band to default values
                        entity_click.update(cx, |state, cx| {
                            state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                            state.app.plugin_state.selected_eq_band = band_idx;
                            // Reset frequency to 1000 Hz
                            state
                                .app
                                .set_plugin_param(plugin_idx, band_idx * 4, FREQUENCY_DEFAULT);
                            // Reset Q to 1.0
                            state
                                .app
                                .set_plugin_param(plugin_idx, band_idx * 4 + 1, Q_DEFAULT);
                            // Reset gain to 0.0 dB
                            state.app.set_plugin_param(
                                plugin_idx,
                                band_idx * 4 + 2,
                                GAIN_DB_DEFAULT,
                            );
                            cx.notify();
                        });
                    } else {
                        // Single click: select this band
                        entity_click.update(cx, |state, _| {
                            state.app.plugin_state.selected_eq_band = band_idx;
                        });
                    }
                }
            })
            .on_drag(
                EqControlPointDrag {
                    band_idx,
                    plugin_idx,
                    color,
                    start_freq: filter.frequency,
                    start_gain: filter.gain_db,
                    start_x: x,
                    start_y: y,
                },
                |drag, _, _, cx| {
                    cx.stop_propagation();
                    cx.new(|_| drag.clone())
                },
            )
            .into_any_element();

        control_points.push(control_point);
    }

    // Wrap chart and control points in a relative container
    // The on_drag_move handler is on the container so it receives events
    // even when the cursor moves away from the small control point circle
    let container = div()
        .id("eq-chart-container")
        .relative()
        .w(px(width))
        .h(px(CHART_HEIGHT))
        .child(chart_element)
        .children(control_points)
        .on_drag_move::<EqControlPointDrag>({
            let entity = entity.clone();
            let bounds_ref = bounds_ref.clone();
            move |event, window, cx| {
                let bounds = if let Some(b) = *bounds_ref.borrow() {
                    b
                } else {
                    return;
                };
                let drag_data = event.drag(cx);
                // Position is relative to this container div, which IS the chart area
                let position = event.event.position;

                // Convert global mouse coordinates to local chart coordinates
                let x_px: f32 = (position.x - bounds.origin.x).into();
                let y_px: f32 = (position.y - bounds.origin.y).into();

                // Convert directly to freq/gain (no delta calculation needed)
                // Use wider range for dragging to allow extending beyond current view
                let new_freq = x_to_freq(x_px, plot_width).clamp(MIN_FREQ, MAX_FREQ);
                let new_gain = y_to_gain(y_px, min_db, max_db).clamp(-24.0, 24.0);

                let plugin_idx = drag_data.plugin_idx;
                let band_idx = drag_data.band_idx;

                entity.update(cx, |state, cx| {
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
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
                // window.refresh();
            }
        });

    EqChartWrapper::new(container.into_any_element(), bounds_ref).into_any_element()
}

/// Render the EQ plugin with graphical visualization
pub fn render_eq_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: EqRenderState,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> impl IntoElement {
    // Read selected channel from AppState
    let app_state = entity.read(cx);
    let selected_eq_channel = app_state.app.plugin_state.selected_eq_channel;
    let _ = app_state;

    // Determine which filters to display based on mode
    let display_filters: &[EQFilter] = if state.per_channel_mode {
        // Per-channel mode: get filters for selected channel
        if let Some(ch_filters) = state.channel_filters {
            let ch_idx = selected_eq_channel.min(ch_filters.len().saturating_sub(1));
            if ch_idx < ch_filters.len() {
                &ch_filters[ch_idx]
            } else {
                // Fallback to global filters
                state.filters
            }
        } else {
            // No channel filters available, fall back to global
            state.filters
        }
    } else {
        // Global mode: use the global filters
        state.filters
    };

    // Clamp selected band to valid range
    let selected_band_idx = state
        .selected_band_idx
        .min(display_filters.len().saturating_sub(1));
    let num_bands = display_filters.len();

    // Determine layout mode based on available width
    // For now, we'll default to vertical layout
    let use_horizontal_layout = false;

    // Get the selected filter
    let selected_filter = if num_bands > 0 {
        Some(&display_filters[selected_band_idx])
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
            display_filters,
            highlight_band_idx,
            theme,
            graph_width,
        ));

    // Clone values needed for closures
    let channels = state.channels;
    let per_channel_mode = state.per_channel_mode;

    let controls_section = div()
        .flex()
        .flex_col()
        .items_center() // Center band selector and knob box
        .gap_4()
        .when(use_horizontal_layout, |d| d.min_w(px(300.0)))
        .when(!use_horizontal_layout, |d| d.w_full())
        // Channel Mode Toggle and Channel Selector
        .child({
            let entity_clone = entity.clone();
            let entity_clone2 = entity.clone();
            let accent = theme.accent;
            let text_on_accent = theme.text_on_accent;
            let text_secondary = theme.text_secondary;
            let bg_secondary = theme.background_secondary;
            let surface_hover = theme.surface_hover;
            let border = theme.border;

            div()
                .flex()
                .items_center()
                .justify_center()
                .gap_4()
                .p_2()
                .bg(theme.surface)
                .rounded_lg()
                // Mode toggle buttons
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        // All Channels button
                        .child({
                            let is_selected = !per_channel_mode;
                            div()
                                .id("eq-mode-all")
                                .px_3()
                                .py_1()
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
                                .on_mouse_down(MouseButton::Left, {
                                    let entity = entity_clone.clone();
                                    move |_event, _window, cx| {
                                        entity.update(cx, |state, cx| {
                                            state.app.set_eq_per_channel_mode(plugin_idx, false);
                                            cx.notify();
                                        });
                                    }
                                })
                                .child("All Channels")
                        })
                        // Per Channel button
                        .child({
                            let is_selected = per_channel_mode;
                            div()
                                .id("eq-mode-per-channel")
                                .px_3()
                                .py_1()
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
                                .on_mouse_down(MouseButton::Left, {
                                    let entity = entity_clone2.clone();
                                    move |_event, _window, cx| {
                                        entity.update(cx, |state, cx| {
                                            state.app.set_eq_per_channel_mode(plugin_idx, true);
                                            cx.notify();
                                        });
                                    }
                                })
                                .child("Per Channel")
                        }),
                )
                // Channel selector (only shown in per-channel mode)
                .when(per_channel_mode, |d| {
                    d.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .border(px(1.0))
                            .border_color(border)
                            .rounded_md()
                            .px_2()
                            .children((0..channels).map(|ch| {
                                let entity = entity.clone();
                                let is_selected = ch == selected_eq_channel;
                                let ch_name = get_channel_name(ch, channels);
                                div()
                                    .id(("eq-channel", ch))
                                    .px_2()
                                    .py_1()
                                    .text_sm()
                                    .rounded_sm()
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
                                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                        entity.update(cx, |state, _| {
                                            state.app.plugin_state.selected_eq_channel = ch;
                                        });
                                    })
                                    .child(ch_name)
                            })),
                    )
                })
        })
        // Band selector tabs (custom rendering to avoid context issues)
        .child({
            let mut tabs_container = div()
                .flex()
                .items_center()
                .justify_center() // Center tabs
                .gap_2()
                .p_1()
                .bg(theme.surface)
                .rounded_lg();

            // Build each band tab manually
            for band_idx in 0..num_bands {
                let is_selected = band_idx == selected_band_idx;
                let filter = display_filters.get(band_idx);
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

                let focus_handle = cx.focus_handle();

                let tab = div()
                    .id(("eq-band", band_idx))
                    .track_focus(&focus_handle)
                    .key_context("plugin-control")
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .rounded_md()
                    .cursor_pointer()
                    .when(is_selected, |d: Stateful<Div>| {
                        d.bg(accent)
                            .text_color(text_on_accent)
                            .font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(!is_selected, |d: Stateful<Div>| {
                        d.bg(bg_secondary)
                            .text_color(text_secondary)
                            .hover(move |s: StyleRefinement| s.bg(surface_hover))
                    })
                    .when(is_muted, |d: Stateful<Div>| d.opacity(0.5))
                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                        entity_clone.update(cx, |state, _| {
                            state.app.plugin_state.selected_eq_band = band_idx;
                            // Also set editing plugin index so keybindings work
                            state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
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
                                                state.app.plugin_state.editing_plugin_index =
                                                    Some(plugin_idx);
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
                                                state.app.plugin_state.editing_plugin_index =
                                                    Some(plugin_idx);
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
                        .id("eq-add-band")
                        .focusable()
                        .key_context("plugin-control")
                        .px_3()
                        .py_1p5()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .rounded_sm()
                        .cursor_pointer()
                        .bg(theme.success)
                        .text_color(theme.text_on_accent)
                        .hover(|s: StyleRefinement| s.opacity(0.8))
                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                            entity_clone.update(cx, |state, cx| {
                                state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
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
                            .gap_6()
                            .justify_center()
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
            .gap_8() // Increased gap between graph and controls
            .child(graph_section)
            .child(controls_section)
    };

    main_container
}

/// Calculate the actual plot width based on chart width and legend configuration.
/// This must match gpui-px line chart legend calculation exactly.
///
/// # Arguments
/// * `chart_width` - Total width of the chart container
/// * `labels` - Iterator of label strings to calculate legend width from
/// * `has_title` - Whether the chart has a title (affects vertical space, not width)
fn calculate_plot_width<'a>(chart_width: f32, labels: impl Iterator<Item = &'a str>) -> f32 {
    // gpui-px constants
    const LEGEND_GAP: f32 = 20.0;
    const COLOR_INDICATOR_WIDTH: f32 = 16.0;
    const GAP_AFTER_COLOR: f32 = 8.0;
    const PADDING: f32 = 16.0; // 8px on each side
    const CHAR_WIDTH: f32 = 7.0; // Approximate width per character for text_xs

    // Find max label length
    let max_label_len = labels.map(|l| l.len()).max().unwrap_or(0);

    // Calculate legend width (vertical legend for Right position)
    let estimated_text_width = (max_label_len as f32) * CHAR_WIDTH;
    let legend_width = COLOR_INDICATOR_WIDTH + GAP_AFTER_COLOR + estimated_text_width + PADDING;
    let width_for_legend = legend_width + LEGEND_GAP;

    // Final plot width
    (chart_width - CHART_LEFT_MARGIN - CHART_RIGHT_MARGIN - width_for_legend).max(0.0)
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
        BiquadFilterType::AllPass => 7,
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
        (7, "AP"), // All Pass
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

/// Get a human-readable channel name based on channel index and total count
fn get_channel_name(channel_idx: usize, total_channels: usize) -> String {
    match total_channels {
        1 => "Mono".to_string(),
        2 => match channel_idx {
            0 => "L".to_string(),
            1 => "R".to_string(),
            _ => format!("Ch {}", channel_idx + 1),
        },
        5 | 6 => match channel_idx {
            // 5.0 or 5.1
            0 => "L".to_string(),
            1 => "R".to_string(),
            2 => "C".to_string(),
            3 => "LFE".to_string(),
            4 => "Ls".to_string(),
            5 => "Rs".to_string(),
            _ => format!("Ch {}", channel_idx + 1),
        },
        7 | 8 => match channel_idx {
            // 7.0 or 7.1
            0 => "L".to_string(),
            1 => "R".to_string(),
            2 => "C".to_string(),
            3 => "LFE".to_string(),
            4 => "Ls".to_string(),
            5 => "Rs".to_string(),
            6 => "Lb".to_string(),
            7 => "Rb".to_string(),
            _ => format!("Ch {}", channel_idx + 1),
        },
        _ => format!("Ch {}", channel_idx + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test constants matching the module constants
    const TEST_CHART_HEIGHT: f32 = 300.0;
    // gpui-px uses GPUI_PX_MARGIN_TOP for height calculation, not CHART_TOP_MARGIN
    const TEST_PLOT_HEIGHT: f32 = TEST_CHART_HEIGHT - GPUI_PX_MARGIN_TOP - CHART_BOTTOM_MARGIN;
    // Fixed gain range for testing (used as reference range)
    const TEST_MIN_GAIN_DB: f64 = -24.0;
    const TEST_MAX_GAIN_DB: f64 = 24.0;

    /// Test that freq_to_x and x_to_freq are inverse operations
    #[test]
    fn test_freq_x_roundtrip() {
        let plot_width = 500.0;
        let test_freqs = [20.0, 100.0, 1000.0, 10000.0, 20000.0];

        for &freq in &test_freqs {
            let x = freq_to_x(freq, plot_width);
            let recovered_freq = x_to_freq(x, plot_width);
            let rel_error = (recovered_freq - freq).abs() / freq;
            assert!(
                rel_error < 0.001,
                "freq_to_x/x_to_freq roundtrip failed for freq={}: got {}, error={}",
                freq,
                recovered_freq,
                rel_error
            );
        }
    }

    /// Test that gain_to_y and y_to_gain are inverse operations
    #[test]
    fn test_gain_y_roundtrip() {
        let test_gains = [-24.0, -12.0, 0.0, 12.0, 24.0];

        for &gain in &test_gains {
            let y = gain_to_y(gain, TEST_MIN_GAIN_DB, TEST_MAX_GAIN_DB);
            let recovered_gain = y_to_gain(y, TEST_MIN_GAIN_DB, TEST_MAX_GAIN_DB);
            let abs_error = (recovered_gain - gain).abs();
            assert!(
                abs_error < 0.01,
                "gain_to_y/y_to_gain roundtrip failed for gain={}: got {}, error={}",
                gain,
                recovered_gain,
                abs_error
            );
        }
    }

    /// Test that freq_to_x maps boundary frequencies correctly
    #[test]
    fn test_freq_to_x_boundaries() {
        let plot_width = 500.0;

        // MIN_FREQ should map to left margin
        let x_min = freq_to_x(MIN_FREQ, plot_width);
        assert!(
            (x_min - CHART_LEFT_MARGIN).abs() < 0.01,
            "MIN_FREQ should map to left margin: got {} expected {}",
            x_min,
            CHART_LEFT_MARGIN
        );

        // MAX_FREQ should map to left margin + plot_width
        let x_max = freq_to_x(MAX_FREQ, plot_width);
        let expected_max = CHART_LEFT_MARGIN + plot_width;
        assert!(
            (x_max - expected_max).abs() < 0.01,
            "MAX_FREQ should map to right edge: got {} expected {}",
            x_max,
            expected_max
        );
    }

    /// Test that gain_to_y maps boundary gains correctly
    #[test]
    fn test_gain_to_y_boundaries() {
        // MAX_GAIN_DB should map to top margin
        let y_max = gain_to_y(TEST_MAX_GAIN_DB, TEST_MIN_GAIN_DB, TEST_MAX_GAIN_DB);
        assert!(
            (y_max - CHART_TOP_MARGIN).abs() < 0.01,
            "MAX_GAIN_DB should map to top margin: got {} expected {}",
            y_max,
            CHART_TOP_MARGIN
        );

        // MIN_GAIN_DB should map to top margin + plot_height
        let y_min = gain_to_y(TEST_MIN_GAIN_DB, TEST_MIN_GAIN_DB, TEST_MAX_GAIN_DB);
        let expected_min = CHART_TOP_MARGIN + TEST_PLOT_HEIGHT;
        assert!(
            (y_min - expected_min).abs() < 0.01,
            "MIN_GAIN_DB should map to bottom edge: got {} expected {}",
            y_min,
            expected_min
        );

        // 0 dB should be at vertical center
        let y_zero = gain_to_y(0.0, TEST_MIN_GAIN_DB, TEST_MAX_GAIN_DB);
        let expected_center = CHART_TOP_MARGIN + TEST_PLOT_HEIGHT / 2.0;
        assert!(
            (y_zero - expected_center).abs() < 0.01,
            "0 dB should map to vertical center: got {} expected {}",
            y_zero,
            expected_center
        );
    }

    /// Test x_to_freq clamping at boundaries
    #[test]
    fn test_x_to_freq_clamping() {
        let plot_width = 500.0;

        // X before left margin should clamp to MIN_FREQ
        let freq_before = x_to_freq(0.0, plot_width);
        assert!(
            (freq_before - MIN_FREQ).abs() < 0.01,
            "x before margin should clamp to MIN_FREQ: got {}",
            freq_before
        );

        // X after right edge should clamp to MAX_FREQ
        let freq_after = x_to_freq(CHART_LEFT_MARGIN + plot_width + 100.0, plot_width);
        assert!(
            (freq_after - MAX_FREQ).abs() < 0.01,
            "x after right edge should clamp to MAX_FREQ: got {}",
            freq_after
        );
    }

    /// Test y_to_gain clamping at boundaries
    #[test]
    fn test_y_to_gain_clamping() {
        // Y before top margin should clamp to MAX_GAIN_DB
        let gain_above = y_to_gain(0.0, TEST_MIN_GAIN_DB, TEST_MAX_GAIN_DB);
        assert!(
            (gain_above - TEST_MAX_GAIN_DB).abs() < 0.01,
            "y above margin should clamp to MAX_GAIN_DB: got {}",
            gain_above
        );

        // Y after bottom edge should clamp to MIN_GAIN_DB
        let gain_below = y_to_gain(
            TEST_CHART_HEIGHT + 100.0,
            TEST_MIN_GAIN_DB,
            TEST_MAX_GAIN_DB,
        );
        assert!(
            (gain_below - TEST_MIN_GAIN_DB).abs() < 0.01,
            "y below bottom should clamp to MIN_GAIN_DB: got {}",
            gain_below
        );
    }

    /// Test Q to bar width conversion
    #[test]
    fn test_q_to_bar_width() {
        // Q_MIN should give maximum width
        let width_at_min_q = q_to_bar_width(Q_MIN);
        assert!(
            (width_at_min_q - Q_BAR_MAX_WIDTH).abs() < 0.01,
            "Q_MIN should give max width: got {} expected {}",
            width_at_min_q,
            Q_BAR_MAX_WIDTH
        );

        // Q_MAX should give minimum width
        let width_at_max_q = q_to_bar_width(Q_MAX);
        assert!(
            (width_at_max_q - Q_BAR_MIN_WIDTH).abs() < 0.01,
            "Q_MAX should give min width: got {} expected {}",
            width_at_max_q,
            Q_BAR_MIN_WIDTH
        );

        // Mid-Q should give mid-width
        let mid_q = (Q_MIN + Q_MAX) / 2.0;
        let mid_width = (Q_BAR_MIN_WIDTH + Q_BAR_MAX_WIDTH) / 2.0;
        let width_at_mid_q = q_to_bar_width(mid_q);
        assert!(
            (width_at_mid_q - mid_width).abs() < 1.0,
            "Mid Q should give mid width: got {} expected ~{}",
            width_at_mid_q,
            mid_width
        );
    }

    /// Test that control points stay within plot bounds for valid filter values
    #[test]
    fn test_control_points_within_bounds() {
        let chart_width = 800.0;
        let labels = ["Combined", "#1 - PK @ 1000Hz", "#2 - LS @ 100Hz (muted)"];
        let plot_width = calculate_plot_width(chart_width, labels.iter().copied());

        // Test various filter configurations
        let test_cases = [
            (MIN_FREQ, TEST_MIN_GAIN_DB),
            (MIN_FREQ, TEST_MAX_GAIN_DB),
            (MAX_FREQ, TEST_MIN_GAIN_DB),
            (MAX_FREQ, TEST_MAX_GAIN_DB),
            (1000.0, 0.0),
            (100.0, -6.0),
            (10000.0, 6.0),
        ];

        for (freq, gain) in test_cases {
            let x = freq_to_x(freq, plot_width);
            let y = gain_to_y(gain, TEST_MIN_GAIN_DB, TEST_MAX_GAIN_DB);

            // X should be within chart area (left margin to left margin + plot width)
            assert!(
                x >= CHART_LEFT_MARGIN && x <= CHART_LEFT_MARGIN + plot_width,
                "X out of bounds for freq={}: x={}, bounds=[{}, {}]",
                freq,
                x,
                CHART_LEFT_MARGIN,
                CHART_LEFT_MARGIN + plot_width
            );

            // Y should be within chart area (top margin to top margin + plot height)
            assert!(
                y >= CHART_TOP_MARGIN && y <= CHART_TOP_MARGIN + TEST_PLOT_HEIGHT,
                "Y out of bounds for gain={}: y={}, bounds=[{}, {}]",
                gain,
                y,
                CHART_TOP_MARGIN,
                CHART_TOP_MARGIN + TEST_PLOT_HEIGHT
            );
        }
    }

    /// Test calculate_plot_width matches expected gpui-px behavior
    #[test]
    fn test_calculate_plot_width() {
        let chart_width = 800.0;

        // Short labels should give larger plot width
        let short_labels = ["A", "B"];
        let plot_width_short = calculate_plot_width(chart_width, short_labels.iter().copied());

        // Long labels should give smaller plot width
        let long_labels = ["#10 - PK @ 20000Hz (muted+solo)", "Combined response curve"];
        let plot_width_long = calculate_plot_width(chart_width, long_labels.iter().copied());

        assert!(
            plot_width_short > plot_width_long,
            "Short labels should give larger plot width: short={} long={}",
            plot_width_short,
            plot_width_long
        );

        // Plot width should always be positive for reasonable chart widths
        assert!(
            plot_width_short > 0.0,
            "Plot width should be positive: {}",
            plot_width_short
        );
        assert!(
            plot_width_long > 0.0,
            "Plot width should be positive: {}",
            plot_width_long
        );
    }

    /// Test logarithmic frequency scaling produces expected ratios
    #[test]
    fn test_freq_logarithmic_scaling() {
        let plot_width = 600.0;

        // Each octave should span equal distance on the plot
        let x_100 = freq_to_x(100.0, plot_width);
        let x_200 = freq_to_x(200.0, plot_width);
        let x_1000 = freq_to_x(1000.0, plot_width);
        let x_2000 = freq_to_x(2000.0, plot_width);

        let octave_width_low = x_200 - x_100;
        let octave_width_high = x_2000 - x_1000;

        // Octaves should be approximately equal width in log scale
        let rel_diff = (octave_width_high - octave_width_low).abs() / octave_width_low;
        assert!(
            rel_diff < 0.01,
            "Octave widths should be equal in log scale: low={} high={} diff={}",
            octave_width_low,
            octave_width_high,
            rel_diff
        );
    }

    /// Test drag delta to Q change conversion
    #[test]
    fn test_drag_delta_to_q_change() {
        // Dragging 60px should change Q by the full range
        let full_range_delta = 60.0;
        let q_change = drag_delta_to_q_change(full_range_delta);
        let expected_change = Q_MAX - Q_MIN;

        assert!(
            (q_change - expected_change).abs() < 0.01,
            "60px drag should change Q by full range: got {} expected {}",
            q_change,
            expected_change
        );

        // Negative delta should decrease Q
        let negative_change = drag_delta_to_q_change(-30.0);
        assert!(
            negative_change < 0.0,
            "Negative drag should decrease Q: got {}",
            negative_change
        );
    }

    /// Test filter response calculation
    #[test]
    fn test_calculate_response_at_freq() {
        // Empty filters should return 0 dB
        let empty: Vec<EQFilter> = vec![];
        assert!(
            (calculate_response_at_freq(&empty, 1000.0) - 0.0).abs() < 0.001,
            "Empty filters should give 0 dB response"
        );

        // Single flat filter (0 dB gain) should return ~0 dB
        let flat_filter = vec![EQFilter {
            frequency: 1000.0,
            q: 1.0,
            gain_db: 0.0,
            filter_type: BiquadFilterType::Peak,
            muted: false,
            solo: false,
        }];
        let response = calculate_response_at_freq(&flat_filter, 1000.0);
        assert!(
            response.abs() < 0.1,
            "0 dB gain filter should give ~0 dB response: got {}",
            response
        );

        // Muted filter should not contribute
        let muted_filter = vec![EQFilter {
            frequency: 1000.0,
            q: 1.0,
            gain_db: 12.0,
            filter_type: BiquadFilterType::Peak,
            muted: true,
            solo: false,
        }];
        let muted_response = calculate_response_at_freq(&muted_filter, 1000.0);
        assert!(
            muted_response.abs() < 0.1,
            "Muted filter should give ~0 dB response: got {}",
            muted_response
        );
    }

    /// Test solo behavior in filter response
    #[test]
    fn test_calculate_response_solo() {
        let filters = vec![
            EQFilter {
                frequency: 100.0,
                q: 1.0,
                gain_db: 6.0,
                filter_type: BiquadFilterType::Peak,
                muted: false,
                solo: false,
            },
            EQFilter {
                frequency: 1000.0,
                q: 1.0,
                gain_db: 12.0,
                filter_type: BiquadFilterType::Peak,
                muted: false,
                solo: true, // This one is soloed
            },
        ];

        // At 1000 Hz, only the soloed filter should contribute
        let response_at_solo = calculate_response_at_freq(&filters, 1000.0);
        let solo_filter_only = vec![filters[1].clone()];
        let expected_response = calculate_response_at_freq(&solo_filter_only, 1000.0);

        assert!(
            (response_at_solo - expected_response).abs() < 0.1,
            "Solo filter should be the only contributor: got {} expected {}",
            response_at_solo,
            expected_response
        );
    }

    /// Test band response calculation
    #[test]
    fn test_calculate_band_response() {
        let filter = EQFilter {
            frequency: 1000.0,
            q: 1.0,
            gain_db: 6.0,
            filter_type: BiquadFilterType::Peak,
            muted: false,
            solo: false,
        };

        // At center frequency, peak filter should show approximately the gain
        let response = calculate_band_response(&filter, 1000.0);
        assert!(
            (response - 6.0).abs() < 0.5,
            "Peak filter at center freq should show ~gain: got {} expected ~6.0",
            response
        );

        // Far from center, response should be near 0
        let far_response = calculate_band_response(&filter, 20.0);
        assert!(
            far_response.abs() < 1.0,
            "Peak filter far from center should be ~0: got {}",
            far_response
        );

        // Muted filter should return 0
        let muted_filter = EQFilter {
            muted: true,
            ..filter
        };
        let muted_response = calculate_band_response(&muted_filter, 1000.0);
        assert!(
            muted_response.abs() < 0.001,
            "Muted filter should return 0: got {}",
            muted_response
        );
    }

    /// Test filter type index mapping
    #[test]
    fn test_filter_type_index() {
        assert_eq!(get_filter_type_index(&BiquadFilterType::Peak), 0);
        assert_eq!(get_filter_type_index(&BiquadFilterType::Lowshelf), 1);
        assert_eq!(get_filter_type_index(&BiquadFilterType::Highshelf), 2);
        assert_eq!(get_filter_type_index(&BiquadFilterType::Lowpass), 3);
        assert_eq!(get_filter_type_index(&BiquadFilterType::Highpass), 4);
        assert_eq!(get_filter_type_index(&BiquadFilterType::Bandpass), 5);
        assert_eq!(get_filter_type_index(&BiquadFilterType::Notch), 6);
        // HighpassVariableQ maps to Highpass
        assert_eq!(
            get_filter_type_index(&BiquadFilterType::HighpassVariableQ),
            4
        );
    }
}
