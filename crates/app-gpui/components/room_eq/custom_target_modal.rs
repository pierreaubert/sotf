//! Custom Target Curve Editor Modal
//!
//! Provides a modal dialog for defining custom target curves via draggable control points.
//! The curve is defined by control points connected by lines, displayed on a log-frequency graph.

use crate::app::AppState;
use crate::app::types::{CustomTargetCurve, TargetCurveControlPoint};
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_px::{ChartTheme, ScaleType, line};
use gpui_ui_kit::theme::ThemeExt;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Dialog, DialogSize, Text, TextSize, TextWeight,
    theme::ThemeExt,
};
use std::cell::RefCell;
use std::rc::Rc;

// Chart layout constants
const CHART_WIDTH: f32 = 800.0;
const CHART_HEIGHT: f32 = 400.0;
const CHART_LEFT_MARGIN: f32 = 50.0;
const CHART_RIGHT_MARGIN: f32 = 20.0;
const CHART_TOP_MARGIN: f32 = 10.0;
const CHART_BOTTOM_MARGIN: f32 = 40.0;
const GPUI_PX_MARGIN_TOP: f32 = 10.0;

const MIN_FREQ: f64 = sotf_plugins::AUDIBLE_MIN_FREQ;
const MAX_FREQ: f64 = sotf_plugins::AUDIBLE_MAX_FREQ;
const MIN_DB: f64 = -24.0;
const MAX_DB: f64 = 24.0;

const CONTROL_POINT_RADIUS: f32 = 10.0;

/// Drag data for control point manipulation
#[derive(Clone)]
struct TargetControlPointDrag {
    point_idx: usize,
    #[allow(dead_code)]
    start_freq: f64,
    #[allow(dead_code)]
    start_level: f64,
}

impl Render for TargetControlPointDrag {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .w(px(CONTROL_POINT_RADIUS * 2.0))
            .h(px(CONTROL_POINT_RADIUS * 2.0))
            .rounded_full()
            .bg(theme.accent)
            .border(px(2.0))
            .border_color(theme.text_on_accent)
            .shadow_lg()
    }
}

/// Wrapper element to capture bounds for coordinate transformation
struct ChartBoundsWrapper {
    child: AnyElement,
    bounds_ref: Rc<RefCell<Option<Bounds<Pixels>>>>,
}

impl ChartBoundsWrapper {
    fn new(child: AnyElement, bounds_ref: Rc<RefCell<Option<Bounds<Pixels>>>>) -> Self {
        Self { child, bounds_ref }
    }
}

impl IntoElement for ChartBoundsWrapper {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ChartBoundsWrapper {
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

/// Calculate plot width (accounting for legend space)
fn calculate_plot_width() -> f32 {
    // Simple calculation - no legend for this chart
    CHART_WIDTH - CHART_LEFT_MARGIN - CHART_RIGHT_MARGIN - 150.0 // Legend space
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

/// Convert level (dB) to y pixel position
fn level_to_y(level_db: f64) -> f32 {
    let plot_height = CHART_HEIGHT - GPUI_PX_MARGIN_TOP - CHART_BOTTOM_MARGIN;
    let t = (MAX_DB - level_db) / (MAX_DB - MIN_DB);
    CHART_TOP_MARGIN + (t as f32) * plot_height
}

/// Convert y pixel position to level (dB)
fn y_to_level(y: f32) -> f64 {
    let plot_height = CHART_HEIGHT - GPUI_PX_MARGIN_TOP - CHART_BOTTOM_MARGIN;
    let t = ((y - CHART_TOP_MARGIN) / plot_height).clamp(0.0, 1.0) as f64;
    MAX_DB - t * (MAX_DB - MIN_DB)
}

impl PlayerView {
    /// Load a custom target curve from a JSON file
    fn load_custom_target_curve(&mut self, cx: &mut Context<Self>) {
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            let state_entity = self.state.clone();
            cx.spawn(async move |_, cx| {
                let file = rfd::AsyncFileDialog::new()
                    .add_filter("JSON", &["json"])
                    .set_title("Load Target Curve")
                    .pick_file()
                    .await;

                if let Some(file) = file {
                    let path = file.path().to_path_buf();
                    match std::fs::read_to_string(&path) {
                        Ok(json) => match serde_json::from_str::<CustomTargetCurve>(&json) {
                            Ok(curve) => {
                                state_entity.update(cx, |state, cx| {
                                    state
                                        .app
                                        .measurement_state
                                        .room_eq_state
                                        .custom_target_curve = curve;
                                    cx.notify();
                                });
                            }
                            Err(e) => {
                                log::error!("Failed to parse target curve: {}", e);
                            }
                        },
                        Err(e) => {
                            log::error!("Failed to read file: {}", e);
                        }
                    }
                }
            })
            .detach();
        }
    }

    /// Save the current custom target curve to a JSON file
    fn save_custom_target_curve(&mut self, cx: &mut Context<Self>) {
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            let state = self.state.read(cx);
            let curve = state
                .app
                .measurement_state
                .room_eq_state
                .custom_target_curve
                .clone();

            cx.spawn(async move |_, _cx| {
                let file = rfd::AsyncFileDialog::new()
                    .add_filter("JSON", &["json"])
                    .set_title("Save Target Curve")
                    .set_file_name("target_curve.json")
                    .save_file()
                    .await;

                if let Some(file) = file {
                    let path = file.path().to_path_buf();
                    match serde_json::to_string_pretty(&curve) {
                        Ok(json) => {
                            if let Err(e) = std::fs::write(&path, json) {
                                log::error!("Failed to write file: {}", e);
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to serialize target curve: {}", e);
                        }
                    }
                }
            })
            .detach();
        }
    }

    /// Render the custom target curve editor modal
    pub(crate) fn render_custom_target_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let is_open = state
            .app
            .measurement_state
            .room_eq_state
            .dropdowns
            .custom_target_modal_open;

        if !is_open {
            return div().into_any_element();
        }

        let custom_curve = state
            .app
            .measurement_state
            .room_eq_state
            .custom_target_curve
            .clone();

        let is_presets_open = state
            .app
            .measurement_state
            .room_eq_state
            .dropdowns
            .custom_target_presets_open;

        let state_entity = self.state.clone();
        let view_entity = cx.entity().clone();

        Dialog::new("custom-target-modal")
            .title("Custom Target Curve Editor")
            .size(DialogSize::Full)
            .on_close({
                let state = state_entity.clone();
                move |_window, cx| {
                    let state = state.clone();
                    cx.defer(move |cx| {
                        state.update(cx, |state, _| {
                            state
                                .app
                                .measurement_state
                                .room_eq_state
                                .dropdowns
                                .custom_target_modal_open = false;
                            state
                                .app
                                .measurement_state
                                .room_eq_state
                                .dropdowns
                                .custom_target_presets_open = false;
                        });
                    });
                }
            })
            .content(
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .flex()
                    .flex_col()
                    .gap(d.section)
                    // Header row with title and presets on right
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(
                                Text::new("Custom Target Curve Editor")
                                    .size(TextSize::Md)
                                    .weight(TextWeight::Semibold)
                                    .color(theme.text_primary),
                            )
                            .child(
                                div()
                                    .relative()
                                    .child(
                                        Button::new("presets-curve", "Presets ▾")
                                            .variant(ButtonVariant::Secondary)
                                            .size(ButtonSize::Sm)
                                            .theme(theme.to_button_theme())
                                            .on_click({
                                                let state = state_entity.clone();
                                                move |_event, cx| {
                                                    state.update(cx, |state, cx| {
                                                        let open = !state
                                                            .app
                                                            .measurement_state
                                                            .room_eq_state
                                                            .dropdowns
                                                            .custom_target_presets_open;
                                                        state
                                                            .app
                                                            .measurement_state
                                                            .room_eq_state
                                                            .dropdowns
                                                            .custom_target_presets_open = open;
                                                        cx.notify();
                                                    });
                                                }
                                            }),
                                    )
                                    .when(is_presets_open, |parent| {
                                        parent.child(
                                            div()
                                                .absolute()
                                                .top_full()
                                                .right_0()
                                                .mt(d.grid)
                                                .w_40()
                                                .bg(theme.surface)
                                                .border_1()
                                                .border_color(theme.border)
                                                .shadow_lg()
                                                .rounded(d.r_md)
                                                .p(d.grid)
                                                .flex()
                                                .flex_col()
                                                .gap(d.grid)
                                                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                    cx.stop_propagation();
                                                })
                                                .child(render_preset_option(
                                                    "Flat",
                                                    CustomTargetCurve::new_flat(),
                                                    &state_entity,
                                                    &theme,
                                                    d,
                                                ))
                                                .child(render_preset_option(
                                                    "Near-field",
                                                    CustomTargetCurve::new_near_field(),
                                                    &state_entity,
                                                    &theme,
                                                    d,
                                                ))
                                                .child(render_preset_option(
                                                    "Mid-field",
                                                    CustomTargetCurve::new_mid_field(),
                                                    &state_entity,
                                                    &theme,
                                                    d,
                                                ))
                                                .child(render_preset_option(
                                                    "Far-field",
                                                    CustomTargetCurve::new_far_field(),
                                                    &state_entity,
                                                    &theme,
                                                    d,
                                                )),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        Text::new("Click on the graph to add control points. Drag points to adjust. Double-click a point to remove it.")
                            .size(TextSize::Xs)
                            .color(theme.text_secondary),
                    )
                    .child(render_target_curve_graph(
                        state_entity.clone(),
                        &custom_curve,
                        &theme,
                    ))
                    .child(
                        div()
                            .flex()
                            .gap(d.gap)
                            .justify_end()
                            .child(
                                Button::new("load-curve", "Import...")
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Sm)
                                    .theme(theme.to_button_theme())
                                    .on_click({
                                        let view = view_entity.clone();
                                        move |_, cx| {
                                            view.update(cx, |view, cx| {
                                                view.load_custom_target_curve(cx);
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Button::new("save-curve", "Export...")
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Sm)
                                    .theme(theme.to_button_theme())
                                    .on_click({
                                        let view = view_entity.clone();
                                        move |_, cx| {
                                            view.update(cx, |view, cx| {
                                                view.save_custom_target_curve(cx);
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Button::new("done-curve", "Done")
                                    .variant(ButtonVariant::Primary)
                                    .size(ButtonSize::Sm)
                                    .theme(theme.to_button_theme())
                                    .on_click({
                                        let state = state_entity.clone();
                                        move |_event, cx| {
                                            state.update(cx, |state, cx| {
                                                state
                                                    .app
                                                    .measurement_state
                                                    .room_eq_state
                                                    .dropdowns
                                                    .custom_target_modal_open = false;
                                                cx.notify();
                                            });
                                        }
                                    }),
                            ),
                    ),
            )
            .into_any_element()
    }
}

fn render_preset_option(
    name: &str,
    curve: CustomTargetCurve,
    state_entity: &Entity<AppState>,
    theme: &crate::theme::Theme,
    d: Ds,
) -> impl IntoElement {
    let state = state_entity.clone();
    div()
        .p(d.pad_y)
        .rounded(d.r_sm)
        .hover(|s| s.bg(theme.surface_hover))
        .cursor_pointer()
        .child(
            Text::new(name.to_string())
                .size(TextSize::Xs)
                .color(theme.text_primary),
        )
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            state.update(cx, |state, cx| {
                state
                    .app
                    .measurement_state
                    .room_eq_state
                    .custom_target_curve = curve.clone();
                state
                    .app
                    .measurement_state
                    .room_eq_state
                    .dropdowns
                    .custom_target_presets_open = false;
                cx.notify();
            });
        })
}

/// Render the target curve graph with interactive control points
fn render_target_curve_graph(
    entity: Entity<AppState>,
    curve: &CustomTargetCurve,
    theme: &crate::theme::Theme,
) -> impl IntoElement {
    let plot_width = calculate_plot_width();

    // Generate the interpolated curve for display
    let curve_data = curve.generate_curve();
    let frequencies: Vec<f64> = curve_data.iter().map(|(f, _)| *f).collect();
    let levels: Vec<f64> = curve_data.iter().map(|(_, l)| *l).collect();

    // Create chart theme
    let chart_theme = ChartTheme {
        plot_background: theme.eq_curve_colors.background,
        grid_color: theme.eq_curve_colors.grid,
        axis_line_color: theme.graph_colors.grid,
        axis_label_color: theme.text_secondary,
        title_color: theme.text_primary,
        legend_text_color: theme.text_secondary,
    };

    // Build the chart
    let chart_builder = line(&frequencies, &levels)
        .x_scale(ScaleType::Log)
        .y_scale(ScaleType::Linear)
        .x_label("Frequency (Hz)")
        .y_label("Level (dB)")
        .x_range(MIN_FREQ, MAX_FREQ)
        .y_range(MIN_DB, MAX_DB)
        .size(CHART_WIDTH, CHART_HEIGHT)
        .color(0x3b82f6) // Blue
        .stroke_width(2.5)
        .label("Target Curve")
        .theme(chart_theme);

    let chart_element = match chart_builder.build() {
        Ok(chart) => chart.into_any_element(),
        Err(_) => div()
            .w(px(CHART_WIDTH))
            .h(px(CHART_HEIGHT))
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.eq_curve_colors.background)
            .text_color(theme.text_secondary)
            .child("Unable to render chart")
            .into_any_element(),
    };

    // Create control points
    let bounds_ref = Rc::new(RefCell::new(None::<Bounds<Pixels>>));
    let mut control_points: Vec<AnyElement> = Vec::new();

    for (i, point) in curve.control_points.iter().enumerate() {
        let x = freq_to_x(point.frequency, plot_width);
        let y = level_to_y(point.level_db);

        let entity_click = entity.clone();
        let entity_drag = entity.clone();
        let bounds_ref_drag = bounds_ref.clone();

        let control_point = div()
            .id(("target-control-point", i))
            .absolute()
            .left(px(x - CONTROL_POINT_RADIUS))
            .top(px(y - CONTROL_POINT_RADIUS))
            .w(px(CONTROL_POINT_RADIUS * 2.0))
            .h(px(CONTROL_POINT_RADIUS * 2.0))
            .rounded_full()
            .bg(theme.accent)
            .border(px(2.0))
            .border_color(theme.text_on_accent)
            .shadow_md()
            .cursor(gpui::CursorStyle::PointingHand)
            .hover(|s| s.size(px(CONTROL_POINT_RADIUS * 2.5)))
            .on_mouse_down(MouseButton::Left, {
                let point_idx = i;
                move |event, _window, cx| {
                    cx.stop_propagation();
                    if event.click_count >= 2 {
                        // Double-click: remove point (if more than 2 points)
                        entity_click.update(cx, |state, cx| {
                            state
                                .app
                                .measurement_state
                                .room_eq_state
                                .custom_target_curve
                                .remove_point(point_idx);
                            cx.notify();
                        });
                    }
                }
            })
            .on_drag(
                TargetControlPointDrag {
                    point_idx: i,
                    start_freq: point.frequency,
                    start_level: point.level_db,
                },
                |drag, _, _, cx| {
                    cx.stop_propagation();
                    cx.new(|_| drag.clone())
                },
            )
            .on_drag_move::<TargetControlPointDrag>({
                move |event, _window, cx| {
                    let bounds = if let Some(b) = *bounds_ref_drag.borrow() {
                        b
                    } else {
                        return;
                    };
                    let drag_data = event.drag(cx);
                    let position = event.event.position;

                    let x_px: f32 = (position.x - bounds.origin.x).into();
                    let y_px: f32 = (position.y - bounds.origin.y).into();

                    let new_freq = x_to_freq(x_px, plot_width).clamp(MIN_FREQ, MAX_FREQ);
                    let new_level = y_to_level(y_px).clamp(MIN_DB, MAX_DB);

                    let point_idx = drag_data.point_idx;

                    entity_drag.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .custom_target_curve
                            .update_point(point_idx, new_freq, new_level);
                        cx.notify();
                    });
                }
            })
            .into_any_element();

        control_points.push(control_point);
    }

    // Create the container with click-to-add functionality
    let entity_add = entity.clone();
    let bounds_ref_add = bounds_ref.clone();

    let container = div()
        .id("target-curve-chart-container")
        .relative()
        .w(px(CHART_WIDTH))
        .h(px(CHART_HEIGHT))
        .child(chart_element)
        .children(control_points)
        .on_mouse_down(MouseButton::Left, move |event, _window, cx| {
            // Only add on single click
            if event.click_count == 1 {
                cx.stop_propagation();

                let bounds = if let Some(b) = *bounds_ref_add.borrow() {
                    b
                } else {
                    return;
                };

                let x_px: f32 = (event.position.x - bounds.origin.x).into();
                let y_px: f32 = (event.position.y - bounds.origin.y).into();

                // Check if click is within the plot area
                let x_in_range =
                    (CHART_LEFT_MARGIN..=CHART_LEFT_MARGIN + plot_width).contains(&x_px);
                let y_in_range =
                    (CHART_TOP_MARGIN..=CHART_HEIGHT - CHART_BOTTOM_MARGIN).contains(&y_px);
                if x_in_range && y_in_range {
                    let new_freq = x_to_freq(x_px, plot_width).clamp(MIN_FREQ, MAX_FREQ);
                    let new_level = y_to_level(y_px).clamp(MIN_DB, MAX_DB);

                    entity_add.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .custom_target_curve
                            .add_point(TargetCurveControlPoint::new(new_freq, new_level));
                        cx.notify();
                    });
                }
            }
        });

    ChartBoundsWrapper::new(container.into_any_element(), bounds_ref).into_any_element()
}
