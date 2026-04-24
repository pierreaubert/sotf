//! EQ Plugin UI Component
//!
//! Provides a professional parametric EQ visualization with:
//! - Frequency response graph
//! - Band controls with color coding
//! - Interactive editing

// intentional-file: EQ chart with pixel-exact control-point geometry

use super::common::{render_knob_sized, render_midi_badge, render_midi_page_indicator};
use crate::components::graphs::common::rgba_to_u32;
use crate::app::AppState;
use crate::components::design::Ds;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_px::{ChartTheme, ScaleType, line};
use gpui_ui_kit::PotentiometerSize;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use sotf_audio_player_midi::mapping::MidiOverlay;
// Tabs are now custom-rendered to avoid context issues
use sotf_audio_player::EQFilter;
use sotf_plugins::param_specs::{eq::BAND_TEMPLATE as EQ, find_by_key as pk};

use std::cell::RefCell;
use std::rc::Rc;

/// Sample rate for filter calculations
pub const SAMPLE_RATE: f64 = sotf_plugins::DEFAULT_PREVIEW_SAMPLE_RATE;

/// Wrapper element to capture bounds for coordinate transformation
pub(crate) struct EqChartWrapper {
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
pub const Q_BAR_MIN_WIDTH: f32 = 40.0;
pub const Q_BAR_MAX_WIDTH: f32 = 100.0;
const Q_HANDLE_RADIUS: f32 = 5.0;
const Q_BAR_HEIGHT: f32 = 3.0;

/// Convert Q value to bar width (inverse: higher Q = narrower bar)
pub fn q_to_bar_width(q: f64) -> f32 {
    let t = ((q - pk(EQ, "q").min_f64()) / (pk(EQ, "q").max_f64() - pk(EQ, "q").min_f64()))
        .clamp(0.0, 1.0) as f32;
    // Inverse mapping: pk(EQ, "q").min_f64() -> max width, pk(EQ, "q").max_f64() -> min width
    Q_BAR_MAX_WIDTH - t * (Q_BAR_MAX_WIDTH - Q_BAR_MIN_WIDTH)
}

/// Convert horizontal drag delta to Q change
/// Positive delta (dragging right handle right) = increase Q
/// Negative delta (dragging left handle left) = decrease Q
pub fn drag_delta_to_q_change(delta_px: f32) -> f64 {
    // Scale factor: moving 30px should roughly change Q by the full range
    let scale = (pk(EQ, "q").max_f64() - pk(EQ, "q").min_f64()) / 60.0;
    delta_px as f64 * scale
}

/// Drag data for EQ control point manipulation (frequency/gain)
#[derive(Clone)]
pub(crate) struct EqControlPointDrag {
    band_idx: usize,
    plugin_idx: usize,
    color: u32,
    #[allow(dead_code)]
    start_freq: f64,
    #[allow(dead_code)]
    start_gain: f64,
    #[allow(dead_code)]
    start_x: f32,
    #[allow(dead_code)]
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
pub(crate) struct EqQHandleDrag {
    band_idx: usize,
    plugin_idx: usize,
    #[allow(dead_code)]
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
    /// MIDI overlay for displaying controller assignments on EQ bands
    pub midi_overlay: Option<&'a MidiOverlay>,
}

/// Calculate the combined response in dB at a given frequency
pub fn calculate_response_at_freq(filters: &[EQFilter], freq: f64) -> f64 {
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

/// Fallback band color (gray) when theme band_colors is exhausted.
pub(crate) const BAND_COLOR_FALLBACK: u32 = 0x9ca3af;

/// Calculate single band response at a frequency
pub fn calculate_band_response(filter: &EQFilter, freq: f64) -> f64 {
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
/// Left margin = Y-axis total_size() = 60 (base) + 20 (title: font_size 12 + padding 8)
pub const CHART_LEFT_MARGIN: f32 = 80.0; // Y-axis rendered width when y_label is set
pub const CHART_RIGHT_MARGIN: f32 = 20.0; // gpui-px margin_right (no secondary axis)
// gpui-px plot area starts at y=0 (no top padding), so no top margin offset needed.
pub const CHART_TOP_MARGIN: f32 = 0.0;
// Must match gpui-px margin_bottom (30.0) so plot_height matches the rendered curve.
pub const CHART_BOTTOM_MARGIN: f32 = 30.0;
pub const CHART_HEIGHT: f32 = 300.0;
// gpui-px uses 10.0 for margin_top in plot_height calculation by default
pub const GPUI_PX_MARGIN_TOP: f32 = 10.0;
pub const MIN_FREQ: f64 = sotf_plugins::AUDIBLE_MIN_FREQ;
pub const MAX_FREQ: f64 = sotf_plugins::AUDIBLE_MAX_FREQ;
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
pub fn freq_to_x(freq: f64, plot_width: f32) -> f32 {
    let log_min = MIN_FREQ.ln();
    let log_max = MAX_FREQ.ln();
    let t = (freq.ln() - log_min) / (log_max - log_min);
    CHART_LEFT_MARGIN + (t as f32) * plot_width
}

/// Convert x pixel position to frequency (Hz)
pub fn x_to_freq(x: f32, plot_width: f32) -> f64 {
    let t = ((x - CHART_LEFT_MARGIN) / plot_width).clamp(0.0, 1.0) as f64;
    let log_min = MIN_FREQ.ln();
    let log_max = MAX_FREQ.ln();
    (log_min + t * (log_max - log_min)).exp()
}

/// Convert gain (dB) to y pixel position with dynamic range
pub fn gain_to_y(gain_db: f64, min_db: f64, max_db: f64) -> f32 {
    // gpui-px calculates plot_height = height - margin_top(10) - margin_bottom(30)
    // but renders the plot starting at y=0 (no actual top margin offset)
    let plot_height = CHART_HEIGHT - GPUI_PX_MARGIN_TOP - CHART_BOTTOM_MARGIN;
    let t = (max_db - gain_db) / (max_db - min_db);
    CHART_TOP_MARGIN + (t as f32) * plot_height
}

/// Convert y pixel position to gain (dB) with dynamic range
pub fn y_to_gain(y: f32, min_db: f64, max_db: f64) -> f64 {
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
    let num_points = 240;
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
        .x_label("Frequency (Hz)")
        .y_label("dB (SPL)")
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

        let color = theme
            .band_colors
            .get(i)
            .map(|c| rgba_to_u32(*c))
            .unwrap_or(BAND_COLOR_FALLBACK);
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
        let rgba_color = theme
            .band_colors
            .get(i)
            .copied()
            .unwrap_or(gpui::rgba(BAND_COLOR_FALLBACK * 256 + 0xFF));
        let color = rgba_to_u32(rgba_color);

        // Calculate position
        let x = freq_to_x(filter.frequency, plot_width);
        let y = gain_to_y(filter.gain_db, min_db, max_db);

        let band_idx = i;

        // Control point circle
        let border_color = if is_selected {
            theme.text_primary
        } else {
            Rgba {
                a: 0.5,
                ..theme.text_primary
            }
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
                    theme.text_primary
                } else {
                    Rgba {
                        a: 0.4,
                        ..theme.text_primary
                    }
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
                    move |event, _window, cx| {
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
                        let new_q = (drag_data.start_q + q_change)
                            .clamp(pk(EQ, "q").min_f64(), pk(EQ, "q").max_f64());

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
                    theme.text_primary
                } else {
                    Rgba {
                        a: 0.4,
                        ..theme.text_primary
                    }
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
                    move |event, _window, cx| {
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
                        let new_q = (drag_data.start_q + q_change)
                            .clamp(pk(EQ, "q").min_f64(), pk(EQ, "q").max_f64());

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
                            state.app.set_plugin_param(
                                plugin_idx,
                                band_idx * 4,
                                pk(EQ, "freq").default_f64(),
                            );
                            // Reset Q to 1.0
                            state.app.set_plugin_param(
                                plugin_idx,
                                band_idx * 4 + 1,
                                pk(EQ, "q").default_f64(),
                            );
                            // Reset gain to 0.0 dB
                            state.app.set_plugin_param(
                                plugin_idx,
                                band_idx * 4 + 2,
                                pk(EQ, "gain").default_f64(),
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
            move |event, _window, cx| {
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

/// Render a knob with an optional MIDI badge underneath
#[allow(clippy::too_many_arguments)]
fn render_eq_knob_with_midi(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    label: &str,
    value: f64,
    min: f64,
    max: f64,
    unit: &str,
    param_idx: usize,
    selected_param: usize,
    is_editing: bool,
    midi_overlay: Option<&MidiOverlay>,
    theme: &Theme,
) -> impl IntoElement {
    let midi_assignment = midi_overlay.and_then(|o| o.assignments.get(&param_idx));

    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(d.grid)
        .child(render_knob_sized(
            entity,
            plugin_idx,
            label,
            value,
            min,
            max,
            unit,
            param_idx,
            selected_param,
            is_editing,
            None,
            PotentiometerSize::Xs,
            theme,
        ))
        .children(midi_assignment.map(|assignment| render_midi_badge(d, assignment, theme)))
}

/// Render the EQ plugin with graphical visualization
pub fn render_eq_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: EqRenderState,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> impl IntoElement {
    let ds = Ds::from_cx(cx);

    // Read selected channel and window width from AppState
    let app_state = entity.read(cx);
    let selected_eq_channel = app_state.app.plugin_state.selected_eq_channel;
    let window_width = app_state.app.ui_state.window_width;
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

    // Use window width as chart width upper bound (GPUI flex constrains to actual container)
    let base_available_width = window_width.max(800.0);
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
        .gap(ds.section)
        .w_full()
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
                .gap(ds.section)
                .p(ds.pad_y)
                .bg(theme.surface)
                .rounded(ds.r_lg)
                // Mode toggle buttons
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(ds.grid)
                        // All Channels button
                        .child({
                            let is_selected = !per_channel_mode;
                            div()
                                .id("eq-mode-all")
                                .px(ds.pad_x)
                                .py(ds.pad_y_half)
                                .text_size(ds.text_sm)
                                .rounded(ds.r_md)
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
                                .px(ds.pad_x)
                                .py(ds.pad_y_half)
                                .text_size(ds.text_sm)
                                .rounded(ds.r_md)
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
                            .gap(ds.grid)
                            .border(px(1.0))
                            .border_color(border)
                            .rounded(ds.r_md)
                            .px(ds.pad_y)
                            .children((0..channels).map(|ch| {
                                let entity = entity.clone();
                                let is_selected = ch == selected_eq_channel;
                                let ch_name = get_channel_name(ch, channels);
                                div()
                                    .id(("eq-channel", ch))
                                    .px(ds.pad_y)
                                    .py(ds.pad_y_half)
                                    .text_size(ds.text_sm)
                                    .rounded(ds.r_sm)
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
                .gap(ds.gap)
                .p(ds.grid)
                .bg(theme.surface)
                .rounded(ds.r_lg);

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
                    .gap(ds.grid)
                    .px(ds.pad_x)
                    .py(ds.pad_y)
                    .text_size(ds.text_sm)
                    .rounded(ds.r_md)
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
                            .gap(ds.grid)
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
                                    .text_size(ds.text_xs)
                                    .font_weight(FontWeight::BOLD)
                                    .cursor_pointer()
                                    .when(is_muted, |d| d.text_color(text_primary))
                                    .when(!is_muted, |d| d.text_color(text_muted_color))
                                    .hover(move |s| {
                                        s.bg(if is_muted { error } else { surface_hover })
                                    })
                                    .on_mouse_down(MouseButton::Left, {
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
                                    .text_size(ds.text_xs)
                                    .font_weight(FontWeight::BOLD)
                                    .cursor_pointer()
                                    .when(is_soloed, |d| d.text_color(text_primary))
                                    .when(!is_soloed, |d| d.text_color(text_muted_color))
                                    .hover(move |s| {
                                        s.bg(if is_soloed { success } else { surface_hover })
                                    })
                                    .on_mouse_down(MouseButton::Left, {
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
                    div()
                        .id("eq-add-band")
                        .focusable()
                        .key_context("plugin-control")
                        .px(ds.pad_x)
                        .py_1p5()
                        .text_size(ds.text_sm)
                        .font_weight(FontWeight::BOLD)
                        .rounded(ds.r_sm)
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
        // MIDI page indicator (shown when controller connected)
        .when(
            state.midi_overlay.is_some_and(|o| o.has_controller()),
            |d| {
                let Some(overlay) = state.midi_overlay else {
                    return d;
                };
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(ds.gap)
                        .children(overlay.controller_name.as_ref().map(|name| {
                            div()
                                .text_size(ds.text_xs)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_secondary)
                                .child(name.clone())
                        }))
                        .child(render_midi_page_indicator(
                            &ds,
                            overlay.current_page,
                            overlay.total_pages,
                            theme,
                        )),
                )
            },
        )
        // Selected band controls
        .when(selected_filter.is_some(), |d| {
            let Some(filter) = selected_filter else {
                return d;
            };
            let base_param_idx = selected_band_idx * 4;
            let midi_overlay = state.midi_overlay;

            d.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(ds.gap)
                    .p(ds.pad_x)
                    .bg(theme.background_secondary)
                    .rounded(ds.r_md)
                    // Filter type selector
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(ds.grid)
                            .child(
                                div()
                                    .text_size(ds.text_xs)
                                    .text_color(theme.text_muted)
                                    .child("Type"),
                            )
                            .child(render_filter_type_selector(
                                &ds,
                                entity.clone(),
                                plugin_idx,
                                &filter.filter_type,
                                selected_band_idx,
                                base_param_idx + 3,
                                None,
                                theme,
                            )),
                    )
                    // Knobs row with MIDI badges
                    .child(
                        div()
                            .flex()
                            .gap(ds.section_lg)
                            .justify_center()
                            .child(render_eq_knob_with_midi(
                                &ds,
                                entity.clone(),
                                plugin_idx,
                                "Freq",
                                filter.frequency,
                                pk(EQ, "freq").min_f64(),
                                pk(EQ, "freq").max_f64(),
                                "Hz",
                                base_param_idx,
                                state.selected_param,
                                state.is_editing,
                                midi_overlay,
                                theme,
                            ))
                            .child(render_eq_knob_with_midi(
                                &ds,
                                entity.clone(),
                                plugin_idx,
                                "Q",
                                filter.q,
                                pk(EQ, "q").min_f64(),
                                pk(EQ, "q").max_f64(),
                                "",
                                base_param_idx + 1,
                                state.selected_param,
                                state.is_editing,
                                midi_overlay,
                                theme,
                            ))
                            .child(render_eq_knob_with_midi(
                                &ds,
                                entity.clone(),
                                plugin_idx,
                                "Gain",
                                filter.gain_db,
                                pk(EQ, "gain").min_f64(),
                                pk(EQ, "gain").max_f64(),
                                "dB",
                                base_param_idx + 2,
                                state.selected_param,
                                state.is_editing,
                                midi_overlay,
                                theme,
                            )),
                    ),
            )
        });

    // Combine sections based on layout mode

    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(ds.section_xl)
        .child(graph_section)
        .child(controls_section)
}

/// Calculate the actual plot width based on chart width and legend configuration.
/// This must match gpui-px line chart legend calculation exactly.
///
/// # Arguments
/// * `chart_width` - Total width of the chart container
/// * `labels` - Iterator of label strings to calculate legend width from
/// * `has_title` - Whether the chart has a title (affects vertical space, not width)
pub fn calculate_plot_width<'a>(chart_width: f32, labels: impl Iterator<Item = &'a str>) -> f32 {
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
pub fn get_filter_type_index(filter_type: &BiquadFilterType) -> usize {
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
        BiquadFilterType::LowshelfOrf => 1,       // Map to Lowshelf
        BiquadFilterType::HighshelfOrf => 2,      // Map to Highshelf
        BiquadFilterType::PeakMatched => 0,       // Map to Peak
    }
}

/// Render a filter type selector using exclusive buttons
fn render_filter_type_selector(
    d: &Ds,
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
    let d = *d;

    div()
        .flex()
        .flex_wrap()
        .gap(d.grid)
        .children(filter_types.into_iter().map(move |(idx, abbrev)| {
            let is_active = idx == current_index;
            let entity_clone = entity.clone();

            div()
                .px(d.pad_y)
                .py(d.pad_y_half)
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .rounded(d.r_sm)
                .cursor_pointer()
                .when(is_active, |el| {
                    el.bg(theme.accent).text_color(theme.text_on_accent)
                })
                .when(!is_active, |el| {
                    el.bg(theme.background_secondary)
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
