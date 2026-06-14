use super::super::common::{render_knob_sized, render_midi_badge, render_midi_page_indicator};
use super::calculate::calculate_band_response;
use super::calculate::calculate_dynamic_y_range;
use super::calculate::calculate_plot_width;
use super::calculate::calculate_response_at_freq;
use super::consts::BAND_COLOR_FALLBACK;
use super::consts::CHART_HEIGHT;
use super::consts::CONTROL_POINT_RADIUS;
use super::consts::MAX_FREQ;
use super::consts::MIN_FREQ;
use super::consts::Q_BAR_HEIGHT;
use super::consts::Q_HANDLE_RADIUS;
use super::consts::freq_to_x;
use super::consts::gain_to_y;
use super::consts::q_to_bar_width;
use super::consts::x_to_freq;
use super::consts::y_to_gain;
use super::eq_chart_wrapper::EqChartWrapper;
use super::eq_control_point_drag::EqControlPointDrag;
use super::eq_qhandle_drag::EqQHandleDrag;
use super::get::get_channel_name;
use super::get::get_filter_type_index;
use super::misc::drag_delta_to_q_change;
use super::types::EqRenderState;
use super::types::EqViewMode;
use crate::app::AppState;
use crate::components::design::Ds;
use crate::components::graphs::common::rgba_to_u32;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_audio_kit::PotentiometerSize;
use gpui_px::{ChartTheme, ScaleType, line};
use math_audio_iir_fir::BiquadFilterType;
use sotf_audio_player::EQFilter;
use sotf_audio_player_midi::mapping::MidiOverlay;
use sotf_plugins::param_specs::{eq::BAND_TEMPLATE as EQ, find_by_key as pk};
use std::cell::RefCell;
use std::rc::Rc;

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
    let is_lp_mode = matches!(
        state.mode,
        EqViewMode::LinearPhase { .. } | EqViewMode::FirDesigner { .. }
    );
    // Linear-phase EQ is global-only; force the toggle off so the renderer's
    // downstream logic doesn't try to surface per-channel data we don't have.
    let per_channel_mode = if is_lp_mode {
        false
    } else {
        state.per_channel_mode
    };

    let controls_section = div()
        .flex()
        .flex_col()
        .items_center() // Center band selector and knob box
        .gap(ds.section)
        .w_full()
        // Channel Mode Toggle and Channel Selector — hidden in linear-phase mode
        .when(!is_lp_mode, |container| {
            container.child({
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
                                                state
                                                    .app
                                                    .set_eq_per_channel_mode(plugin_idx, false);
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
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            move |_event, _window, cx| {
                                                entity.update(cx, |state, _| {
                                                    state.app.plugin_state.selected_eq_channel = ch;
                                                });
                                            },
                                        )
                                        .child(ch_name)
                                })),
                        )
                    })
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
                    // Filter type selector + topology controls
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(ds.grid)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(ds.grid)
                                    .child(
                                        div()
                                            .text_size(ds.text_xs)
                                            .text_color(theme.text_muted)
                                            .child("Type"),
                                    )
                                    .child(render_topology_controls(
                                        &ds,
                                        entity.clone(),
                                        plugin_idx,
                                        selected_band_idx,
                                        filter,
                                        theme,
                                    )),
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

    // Optional linear-phase info header — shown only for the LP variant.
    let fir_summary = match state.mode {
        EqViewMode::LinearPhase {
            latency_samples,
            latency_ms,
            fir_length,
            auto_gain,
            mix,
        } => Some((
            latency_samples,
            latency_ms,
            fir_length,
            "Linear",
            auto_gain,
            mix,
        )),
        EqViewMode::FirDesigner {
            latency_samples,
            latency_ms,
            fir_length,
            phase_mode,
            auto_gain,
            mix,
        } => Some((
            latency_samples,
            latency_ms,
            fir_length,
            phase_mode,
            auto_gain,
            mix,
        )),
        EqViewMode::Standard => None,
    };
    let lp_header = fir_summary.map(
        |(latency_samples, latency_ms, fir_length, phase_mode, auto_gain, mix)| {
            div()
                .flex()
                .items_center()
                .justify_center()
                .gap(ds.section)
                .px(ds.pad_x)
                .py(ds.pad_y_half)
                .bg(theme.surface)
                .rounded(ds.r_md)
                .text_size(ds.text_sm)
                .text_color(theme.text_secondary)
                .child(format!("Filters: {}", state.num_filters))
                .child(format!("FIR length: {fir_length}"))
                .child(format!("Phase: {phase_mode}"))
                .child(format!(
                    "Latency: {latency_samples} samples ({latency_ms:.2} ms)"
                ))
                .child(format!(
                    "Auto-gain: {}",
                    if auto_gain { "on" } else { "off" }
                ))
                .child(format!("Mix: {:.0}%", mix * 100.0))
        },
    );
    let lp_analysis = fir_summary.map(
        |(latency_samples, latency_ms, fir_length, phase_mode, _, _)| {
            render_linear_phase_analysis(
                &ds,
                latency_samples,
                latency_ms,
                fir_length,
                phase_mode,
                theme,
            )
        },
    );

    // Algorithm info bar for Standard EQ
    let eq_header = if matches!(state.mode, EqViewMode::Standard) {
        let topo_label = if state.topology > 0.5 {
            "SVF"
        } else {
            "Biquad"
        };
        Some(
            div()
                .flex()
                .items_center()
                .justify_center()
                .gap(ds.section)
                .px(ds.pad_x)
                .py(ds.pad_y_half)
                .bg(theme.surface)
                .rounded(ds.r_md)
                .text_size(ds.text_sm)
                .text_color(theme.text_secondary)
                .child(format!("Filters: {}", state.num_filters))
                .child(format!("Topology: {topo_label}"))
                .child(format!("TDF-II: {}", if state.tdf2 { "on" } else { "off" })),
        )
    } else {
        None
    };

    // Combine sections based on layout mode

    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(ds.section_xl)
        .children(eq_header)
        .children(lp_header)
        .child(graph_section)
        .children(lp_analysis)
        .child(controls_section)
}

fn render_linear_phase_analysis(
    d: &Ds,
    latency_samples: usize,
    latency_ms: f32,
    fir_length: usize,
    phase_mode: &str,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .justify_center()
        .gap(d.gap)
        .w_full()
        .child(render_lp_analysis_card(
            d,
            "Magnitude",
            "Editable paragraphic target".to_string(),
            theme.accent,
            theme,
        ))
        .child(render_lp_analysis_card(
            d,
            "Phase",
            if phase_mode == "Linear" {
                "Linear after latency compensation".to_string()
            } else {
                "Minimum phase, energy near start".to_string()
            },
            theme.success,
            theme,
        ))
        .child(render_lp_analysis_card(
            d,
            "Group Delay",
            format!("{latency_samples} samples / {latency_ms:.2} ms"),
            theme.warning,
            theme,
        ))
        .child(render_lp_analysis_card(
            d,
            "Impulse",
            if phase_mode == "Linear" {
                format!("{fir_length} taps, symmetric FIR")
            } else {
                format!("{fir_length} taps, minimum-phase FIR")
            },
            theme.text_secondary,
            theme,
        ))
}

fn render_lp_analysis_card(
    d: &Ds,
    label: &'static str,
    value: String,
    accent: Rgba,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .min_w(rems(10.0))
        .px(d.pad_x)
        .py(d.pad_y)
        .rounded(d.r_md)
        .bg(theme.surface)
        .border_l_4()
        .border_color(accent)
        .child(
            div()
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(accent)
                .child(label),
        )
        .child(
            div()
                .text_size(d.text_sm)
                .text_color(theme.text_secondary)
                .child(value),
        )
}

/// Render topology controls for an EQ band: a cycling label that switches
/// Biquad → Warped → Kautz, plus a contextual secondary control that only
/// makes sense for the current topology (lambda preset for Warped, +/-
/// section buttons for Kautz). Biquads still render the topology pill so the
/// user can opt into a different runtime when authoring by hand.
fn render_topology_controls(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    band_idx: usize,
    filter: &EQFilter,
    theme: &Theme,
) -> AnyElement {
    use sotf_audio::plugins::EqFilterTopology;

    let label = match filter.topology {
        EqFilterTopology::Biquad => "IIR",
        EqFilterTopology::WarpedBiquad => "Warp",
        EqFilterTopology::KautzFilter => "Kautz",
    };

    let pill = {
        let entity_topology = entity.clone();
        div()
            .px(d.pad_y)
            .py(d.pad_y_half)
            .text_size(d.text_xs)
            .font_weight(FontWeight::SEMIBOLD)
            .rounded(d.r_sm)
            .cursor_pointer()
            .bg(theme.background_secondary)
            .text_color(theme.text_primary)
            .hover(|s| s.bg(theme.surface_hover))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                entity_topology.update(cx, |state, _| {
                    state.app.cycle_eq_filter_topology(plugin_idx, band_idx);
                });
            })
            .child(label)
    };

    let secondary: AnyElement = match filter.topology {
        EqFilterTopology::Biquad => div().into_any_element(),
        EqFilterTopology::WarpedBiquad => {
            let lambda_text = filter
                .lambda
                .map(|v| format!("λ={v:.2}"))
                .unwrap_or_else(|| "λ=auto".to_string());
            let entity_lambda = entity.clone();
            div()
                .px(d.pad_y)
                .py(d.pad_y_half)
                .text_size(d.text_xs)
                .rounded(d.r_sm)
                .cursor_pointer()
                .bg(theme.background_secondary)
                .text_color(theme.text_secondary)
                .hover(|s| s.bg(theme.surface_hover))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    entity_lambda.update(cx, |state, _| {
                        state.app.cycle_eq_filter_lambda(plugin_idx, band_idx);
                    });
                })
                .child(lambda_text)
                .into_any_element()
        }
        EqFilterTopology::KautzFilter => {
            let count = filter.kautz_sections.len();
            let pole_freq = filter.frequency;
            let q = filter.q;
            let gain = filter.gain_db;
            let entity_add = entity.clone();
            let entity_remove = entity.clone();
            div()
                .flex()
                .items_center()
                .gap(d.grid)
                .child(
                    div()
                        .text_size(d.text_xs)
                        .text_color(theme.text_muted)
                        .child(format!(
                            "{count} section{}",
                            if count == 1 { "" } else { "s" }
                        )),
                )
                .child(
                    div()
                        .px(d.pad_y)
                        .py(d.pad_y_half)
                        .text_size(d.text_xs)
                        .rounded(d.r_sm)
                        .cursor_pointer()
                        .bg(theme.background_secondary)
                        .text_color(theme.text_primary)
                        .hover(|s| s.bg(theme.surface_hover))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            entity_add.update(cx, |state, _| {
                                state
                                    .app
                                    .add_eq_kautz_section(plugin_idx, band_idx, pole_freq, q, gain);
                            });
                        })
                        .child("+"),
                )
                .child(
                    div()
                        .px(d.pad_y)
                        .py(d.pad_y_half)
                        .text_size(d.text_xs)
                        .rounded(d.r_sm)
                        .cursor_pointer()
                        .bg(theme.background_secondary)
                        .text_color(theme.text_primary)
                        .hover(|s| s.bg(theme.surface_hover))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            entity_remove.update(cx, |state, _| {
                                state.app.pop_eq_kautz_section(plugin_idx, band_idx);
                            });
                        })
                        .child("-"),
                )
                .into_any_element()
        }
    };

    div()
        .flex()
        .items_center()
        .gap(d.grid)
        .child(pill)
        .child(secondary)
        .into_any_element()
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
