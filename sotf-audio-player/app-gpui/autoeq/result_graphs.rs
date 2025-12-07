//! Result visualization graphs for headphone EQ optimization
//!
//! Displays 4 plots in a 2x2 grid showing the optimization results:
//! 1. Individual IIR filters and combined response
//! 2. Filter response vs deviation from target
//! 3. Error curve (deviation - filter response)
//! 4. Response with/without filter and target

use crate::autoeq::speaker_eq::SpeakerOptimizationResult;
use crate::autoeq::HeadphoneOptimizationResult;
use crate::components::graphs::{band_color, format_frequency};
use crate::theme::Theme;
use crate::ui::PlayerView;
use d3rs::color::D3Color;
use d3rs::scale::{LinearScale, LogScale, Scale};
use d3rs::shape::{LineConfig, LinePoint, render_line};
use gpui::prelude::*;
use gpui::*;

/// Color palette for the plots
mod colors {
    use gpui::rgb;

    pub fn input() -> gpui::Rgba {
        rgb(0x6366f1) // Indigo - input/original
    }
    pub fn target() -> gpui::Rgba {
        rgb(0x22c55e) // Green - target
    }
    pub fn filter() -> gpui::Rgba {
        rgb(0xf59e0b) // Amber - filter response
    }
    pub fn corrected() -> gpui::Rgba {
        rgb(0x3b82f6) // Blue - corrected
    }
    pub fn error() -> gpui::Rgba {
        rgb(0xef4444) // Red - error
    }
    pub fn deviation() -> gpui::Rgba {
        rgb(0x8b5cf6) // Violet - deviation
    }
    pub fn grid() -> gpui::Rgba {
        gpui::rgba(0xffffff15) // Subtle white for grid lines
    }
}

/// Width reserved for Y-axis labels
const Y_AXIS_WIDTH: f32 = 32.0;
/// Height reserved for X-axis labels
const X_AXIS_HEIGHT: f32 = 16.0;
/// Minimum frequency for all plots
const MIN_FREQ: f64 = 20.0;
/// Maximum frequency for all plots
const MAX_FREQ: f64 = 20000.0;

impl PlayerView {
    /// Render the optimization result graphs in a 2x2 grid
    pub fn render_optimization_result_graphs(
        &self,
        result: &HeadphoneOptimizationResult,
        theme: &Theme,
        available_width: f32,
    ) -> impl IntoElement {
        // Each graph takes half the width minus gap
        let gap = 8.0;
        let graph_width = (available_width - gap) / 2.0;
        let graph_height = 200.0;

        div()
            .flex()
            .flex_col()
            .gap_2()
            .w_full()
            // Row 1: Filter Response + Filter vs Deviation
            .child(
                div()
                    .flex()
                    .gap_2()
                    // Plot 1: Individual IIR filters and sum
                    .child(render_plot_with_title(
                        "Filter Response",
                        render_filter_response_plot(result, theme, graph_width, graph_height),
                        theme,
                    ))
                    // Plot 2: Filter vs Deviation
                    .child(render_plot_with_title(
                        "Filter vs Deviation",
                        render_filter_vs_deviation_plot(result, theme, graph_width, graph_height),
                        theme,
                    )),
            )
            // Row 2: Error + Response Comparison
            .child(
                div()
                    .flex()
                    .gap_2()
                    // Plot 3: Error curve
                    .child(render_plot_with_title(
                        "Residual Error",
                        render_error_plot(result, theme, graph_width, graph_height),
                        theme,
                    ))
                    // Plot 4: Response with/without filter and target
                    .child(render_plot_with_title(
                        "Response Comparison",
                        render_response_comparison_plot(result, theme, graph_width, graph_height),
                        theme,
                    )),
            )
            // Row 3: Optimization Process
            .child(
                div()
                    .flex()
                    .gap_2()
                    // Plot 5: Loss vs Iteration
                    .child(render_plot_with_title(
                        &format!(
                            "Optimization Process (Before: {:.2}, After: {:.2})",
                            result.initial_loss, result.final_loss
                        ),
                        render_optimization_loss_plot(result, theme, available_width - 16.0, graph_height),
                        theme,
                    )),
            )
    }
}

/// Wrap a plot with a title
fn render_plot_with_title(title: &str, plot: Div, theme: &Theme) -> Div {
    let title = SharedString::from(title.to_string());
    div()
        .flex()
        .flex_col()
        .flex_1()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_primary)
                .child(title),
        )
        .child(plot)
}

/// Render grid lines for the graph area
fn render_grid_lines(
    graph_width: f32,
    graph_height: f32,
    min_db: f64,
    max_db: f64,
    theme: &Theme,
) -> Div {
    let freq_scale = LogScale::new()
        .domain(MIN_FREQ, MAX_FREQ)
        .range(0.0, graph_width as f64);

    let db_scale = LinearScale::new()
        .domain(min_db, max_db)
        .range(graph_height as f64, 0.0);

    // Frequency grid lines (vertical)
    let freq_ticks: Vec<f64> = vec![50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0];

    // dB grid lines (horizontal)
    let range = max_db - min_db;
    let step = if range <= 12.0 {
        3.0
    } else if range <= 24.0 {
        6.0
    } else {
        12.0
    };
    let mut db_ticks: Vec<f64> = Vec::new();
    let mut tick = (min_db / step).ceil() * step;
    while tick <= max_db {
        db_ticks.push(tick);
        tick += step;
    }

    let grid_color = colors::grid();

    div()
        .absolute()
        .inset_0()
        // Vertical grid lines (frequency)
        .children(freq_ticks.iter().map(|&freq| {
            let x_pos = freq_scale.scale(freq) as f32;
            div()
                .absolute()
                .left(px(x_pos))
                .top_0()
                .bottom_0()
                .w(px(1.0))
                .bg(grid_color)
        }))
        // Horizontal grid lines (dB)
        .children(db_ticks.iter().map(|&db| {
            let y_pos = db_scale.scale(db) as f32;
            let is_zero = db.abs() < 0.01;
            div()
                .absolute()
                .top(px(y_pos))
                .left_0()
                .right_0()
                .h(px(1.0))
                .when(is_zero, |el| el.bg(theme.text_muted).opacity(0.5))
                .when(!is_zero, |el| el.bg(grid_color))
        }))
}

/// Render the Y-axis labels (SPL in dB)
fn render_y_axis(min_db: f64, max_db: f64, height: f32, theme: &Theme) -> Div {
    let range = max_db - min_db;
    let step = if range <= 12.0 {
        3.0
    } else if range <= 24.0 {
        6.0
    } else {
        12.0
    };

    let mut ticks: Vec<f64> = Vec::new();
    let mut tick = (min_db / step).ceil() * step;
    while tick <= max_db {
        ticks.push(tick);
        tick += step;
    }

    let db_scale = LinearScale::new()
        .domain(min_db, max_db)
        .range(height as f64, 0.0);

    div()
        .w(px(Y_AXIS_WIDTH))
        .h(px(height))
        .relative()
        .children(ticks.iter().map(|&db| {
            let y_pos = db_scale.scale(db) as f32;
            div()
                .absolute()
                .right(px(2.0))
                .top(px(y_pos - 5.0))
                .text_size(px(9.0))
                .text_color(theme.text_muted)
                .child(format!("{:.0}", db))
        }))
}

/// Render the X-axis labels (Frequency, logarithmic)
fn render_x_axis(width: f32, theme: &Theme) -> Div {
    let ticks: Vec<(f64, &str)> = vec![
        (20.0, "20"),
        (100.0, "100"),
        (1000.0, "1k"),
        (10000.0, "10k"),
    ];

    let freq_scale = LogScale::new()
        .domain(MIN_FREQ, MAX_FREQ)
        .range(0.0, width as f64);

    div()
        .w(px(width))
        .h(px(X_AXIS_HEIGHT))
        .relative()
        .children(ticks.iter().map(|&(freq, label)| {
            let x_pos = freq_scale.scale(freq) as f32;
            div()
                .absolute()
                .left(px(x_pos - 8.0))
                .top(px(1.0))
                .text_size(px(9.0))
                .text_color(theme.text_muted)
                .child(label)
        }))
}

/// Render a compact horizontal legend
fn render_compact_legend(items: &[(String, Rgba)], theme: &Theme) -> Div {
    div()
        .h(px(16.0))
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .children(items.iter().map(|(label, color)| {
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().w(px(10.0)).h(px(2.0)).rounded_sm().bg(*color))
                .child(
                    div()
                        .text_size(px(9.0))
                        .text_color(theme.text_muted)
                        .child(label.clone()),
                )
        }))
}

/// Calculate a nice dB range for the given values
fn calculate_db_range(values: &[f64]) -> (f64, f64) {
    let min_val = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max_val = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    let range = (max_val - min_val).max(6.0);
    let padding = range * 0.1;

    let min_db = ((min_val - padding) / 6.0).floor() * 6.0;
    let max_db = ((max_val + padding) / 6.0).ceil() * 6.0;

    (min_db.max(-48.0), max_db.min(48.0))
}

/// Render Plot 1: Individual IIR filters and combined response
fn render_filter_response_plot(
    result: &HeadphoneOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let all_values: Vec<f64> = result
        .individual_filter_responses
        .iter()
        .flatten()
        .copied()
        .chain(result.filter_response.iter().copied())
        .collect();
    let (min_db, max_db) = calculate_db_range(&all_values);

    let graph_width = width - Y_AXIS_WIDTH;
    let graph_height = height;

    let freq_scale = LogScale::new()
        .domain(MIN_FREQ, MAX_FREQ)
        .range(0.0, graph_width as f64);
    let db_scale = LinearScale::new()
        .domain(min_db, max_db)
        .range(graph_height as f64, 0.0);

    let legend_items: Vec<(String, Rgba)> = result
        .biquads
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let label = format!("F{} {}", i + 1, format_frequency(b.freq));
            (label, band_color(i, theme))
        })
        .collect();

    let mut curve_elements: Vec<AnyElement> = Vec::new();

    for (i, curve) in result.individual_filter_responses.iter().enumerate() {
        let points: Vec<LinePoint> = result
            .frequencies
            .iter()
            .zip(curve.iter())
            .map(|(&f, &db)| LinePoint::new(f, db))
            .collect();
        let color = band_color(i, theme);
        let config = LineConfig::new()
            .stroke_width(1.5)
            .stroke_color(D3Color::from_rgba(color));
        curve_elements.push(render_line(&freq_scale, &db_scale, &points, &config).into_any_element());
    }

    let sum_points: Vec<LinePoint> = result
        .frequencies
        .iter()
        .zip(result.filter_response.iter())
        .map(|(&f, &db)| LinePoint::new(f, db))
        .collect();
    let sum_config = LineConfig::new()
        .stroke_width(2.0)
        .stroke_color(D3Color::from_rgba(colors::filter()));
    curve_elements.push(render_line(&freq_scale, &db_scale, &sum_points, &sum_config).into_any_element());

    let theme = theme.clone();
    let mut legend_with_sum = legend_items;
    legend_with_sum.push(("Sum".to_string(), colors::filter()));

    div()
        .w(px(width))
        .h(px(height + X_AXIS_HEIGHT + 16.0))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .child(render_y_axis(min_db, max_db, graph_height, &theme))
                .child(
                    div()
                        .flex_col()
                        .child(
                            div()
                                .w(px(graph_width))
                                .h(px(graph_height))
                                .bg(theme.surface)
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .relative()
                                .overflow_hidden()
                                .child(render_grid_lines(graph_width, graph_height, min_db, max_db, &theme))
                                .children(curve_elements),
                        )
                        .child(render_x_axis(graph_width, &theme)),
                ),
        )
        .child(render_compact_legend(&legend_with_sum, &theme))
}

/// Render Plot 2: Filter response vs deviation from target
fn render_filter_vs_deviation_plot(
    result: &HeadphoneOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let all_values: Vec<f64> = result
        .filter_response
        .iter()
        .chain(result.deviation_curve.iter())
        .copied()
        .collect();
    let (min_db, max_db) = calculate_db_range(&all_values);

    let graph_width = width - Y_AXIS_WIDTH;
    let graph_height = height;

    let freq_scale = LogScale::new()
        .domain(MIN_FREQ, MAX_FREQ)
        .range(0.0, graph_width as f64);
    let db_scale = LinearScale::new()
        .domain(min_db, max_db)
        .range(graph_height as f64, 0.0);

    let points1: Vec<LinePoint> = result
        .frequencies
        .iter()
        .zip(result.deviation_curve.iter())
        .map(|(&f, &db)| LinePoint::new(f, db))
        .collect();
    let points2: Vec<LinePoint> = result
        .frequencies
        .iter()
        .zip(result.filter_response.iter())
        .map(|(&f, &db)| LinePoint::new(f, db))
        .collect();

    let config1 = LineConfig::new()
        .stroke_width(2.0)
        .stroke_color(D3Color::from_rgba(colors::deviation()));
    let config2 = LineConfig::new()
        .stroke_width(2.0)
        .stroke_color(D3Color::from_rgba(colors::filter()));

    let curve1 = render_line(&freq_scale, &db_scale, &points1, &config1);
    let curve2 = render_line(&freq_scale, &db_scale, &points2, &config2);

    let legend_items = vec![
        ("Deviation".to_string(), colors::deviation()),
        ("Filter".to_string(), colors::filter()),
    ];

    let theme = theme.clone();

    div()
        .w(px(width))
        .h(px(height + X_AXIS_HEIGHT + 16.0))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .child(render_y_axis(min_db, max_db, graph_height, &theme))
                .child(
                    div()
                        .flex_col()
                        .child(
                            div()
                                .w(px(graph_width))
                                .h(px(graph_height))
                                .bg(theme.surface)
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .relative()
                                .overflow_hidden()
                                .child(render_grid_lines(graph_width, graph_height, min_db, max_db, &theme))
                                .child(curve1)
                                .child(curve2),
                        )
                        .child(render_x_axis(graph_width, &theme)),
                ),
        )
        .child(render_compact_legend(&legend_items, &theme))
}

/// Render Plot 3: Error curve (deviation - filter)
fn render_error_plot(
    result: &HeadphoneOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let (min_db, max_db) = calculate_db_range(&result.error_curve);

    let graph_width = width - Y_AXIS_WIDTH;
    let graph_height = height;

    let freq_scale = LogScale::new()
        .domain(MIN_FREQ, MAX_FREQ)
        .range(0.0, graph_width as f64);
    let db_scale = LinearScale::new()
        .domain(min_db, max_db)
        .range(graph_height as f64, 0.0);

    let points: Vec<LinePoint> = result
        .frequencies
        .iter()
        .zip(result.error_curve.iter())
        .map(|(&f, &db)| LinePoint::new(f, db))
        .collect();

    let config = LineConfig::new()
        .stroke_width(2.0)
        .stroke_color(D3Color::from_rgba(colors::error()));

    let curve = render_line(&freq_scale, &db_scale, &points, &config);

    let legend_items = vec![("Error".to_string(), colors::error())];

    let theme = theme.clone();

    div()
        .w(px(width))
        .h(px(height + X_AXIS_HEIGHT + 16.0))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .child(render_y_axis(min_db, max_db, graph_height, &theme))
                .child(
                    div()
                        .flex_col()
                        .child(
                            div()
                                .w(px(graph_width))
                                .h(px(graph_height))
                                .bg(theme.surface)
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .relative()
                                .overflow_hidden()
                                .child(render_grid_lines(graph_width, graph_height, min_db, max_db, &theme))
                                .child(curve),
                        )
                        .child(render_x_axis(graph_width, &theme)),
                ),
        )
        .child(render_compact_legend(&legend_items, &theme))
}

/// Render Plot 4: Response comparison (input, corrected, target)
fn render_response_comparison_plot(
    result: &HeadphoneOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let all_values: Vec<f64> = result
        .input_curve
        .iter()
        .chain(result.corrected_curve.iter())
        .chain(result.target_curve.iter())
        .copied()
        .collect();
    let (min_db, max_db) = calculate_db_range(&all_values);

    let graph_width = width - Y_AXIS_WIDTH;
    let graph_height = height;

    let freq_scale = LogScale::new()
        .domain(MIN_FREQ, MAX_FREQ)
        .range(0.0, graph_width as f64);
    let db_scale = LinearScale::new()
        .domain(min_db, max_db)
        .range(graph_height as f64, 0.0);

    let points1: Vec<LinePoint> = result
        .frequencies
        .iter()
        .zip(result.input_curve.iter())
        .map(|(&f, &db)| LinePoint::new(f, db))
        .collect();
    let points2: Vec<LinePoint> = result
        .frequencies
        .iter()
        .zip(result.corrected_curve.iter())
        .map(|(&f, &db)| LinePoint::new(f, db))
        .collect();
    let points3: Vec<LinePoint> = result
        .frequencies
        .iter()
        .zip(result.target_curve.iter())
        .map(|(&f, &db)| LinePoint::new(f, db))
        .collect();

    let config1 = LineConfig::new()
        .stroke_width(1.5)
        .stroke_color(D3Color::from_rgba(colors::input()));
    let config2 = LineConfig::new()
        .stroke_width(2.0)
        .stroke_color(D3Color::from_rgba(colors::corrected()));
    let config3 = LineConfig::new()
        .stroke_width(2.0)
        .stroke_color(D3Color::from_rgba(colors::target()));

    let curve1 = render_line(&freq_scale, &db_scale, &points1, &config1);
    let curve2 = render_line(&freq_scale, &db_scale, &points2, &config2);
    let curve3 = render_line(&freq_scale, &db_scale, &points3, &config3);

    let legend_items = vec![
        ("Original".to_string(), colors::input()),
        ("Corrected".to_string(), colors::corrected()),
        ("Target".to_string(), colors::target()),
    ];

    let theme = theme.clone();

    div()
        .w(px(width))
        .h(px(height + X_AXIS_HEIGHT + 16.0))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .child(render_y_axis(min_db, max_db, graph_height, &theme))
                .child(
                    div()
                        .flex_col()
                        .child(
                            div()
                                .w(px(graph_width))
                                .h(px(graph_height))
                                .bg(theme.surface)
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .relative()
                                .overflow_hidden()
                                .child(render_grid_lines(graph_width, graph_height, min_db, max_db, &theme))
                                .child(curve1)
                                .child(curve3)
                                .child(curve2),
                        )
                        .child(render_x_axis(graph_width, &theme)),
                ),
        )
        .child(render_compact_legend(&legend_items, &theme))
}

/// Render Iteration X-axis
fn render_iteration_x_axis(width: f32, max_iter: f64, theme: &Theme) -> Div {
    let ticks: Vec<f64> = vec![
        0.0,
        max_iter * 0.25,
        max_iter * 0.5,
        max_iter * 0.75,
        max_iter,
    ];

    let iter_scale = LinearScale::new()
        .domain(0.0, max_iter)
        .range(0.0, width as f64);

    div()
        .w(px(width))
        .h(px(X_AXIS_HEIGHT))
        .relative()
        .children(ticks.iter().map(|&iter| {
            let x_pos = iter_scale.scale(iter) as f32;
            div()
                .absolute()
                .left(px(x_pos - 8.0))
                .top(px(1.0))
                .text_size(px(9.0))
                .text_color(theme.text_muted)
                .child(format!("{:.0}", iter))
        }))
}

/// Render Plot 5: Optimization Loss vs Iteration
fn render_optimization_loss_plot(
    result: &HeadphoneOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    if result.optimization_history.is_empty() {
        return div().child("No history available");
    }

    let max_iter = result.optimization_history.last().map(|x| x.0).unwrap_or(0) as f64;
    let losses: Vec<f64> = result.optimization_history.iter().map(|x| x.1).collect();
    let (min_loss, max_loss) = calculate_db_range(&losses);

    let graph_width = width - Y_AXIS_WIDTH;
    let graph_height = height;

    let iter_scale = LinearScale::new()
        .domain(0.0, max_iter)
        .range(0.0, graph_width as f64);
    let loss_scale = LinearScale::new()
        .domain(min_loss, max_loss)
        .range(graph_height as f64, 0.0);

    let points: Vec<LinePoint> = result
        .optimization_history
        .iter()
        .map(|&(i, loss)| LinePoint::new(i as f64, loss))
        .collect();

    let config = LineConfig::new()
        .stroke_width(2.0)
        .stroke_color(D3Color::from_rgba(colors::deviation()));

    let curve = render_line(&iter_scale, &loss_scale, &points, &config);

    let legend_items = vec![("Loss".to_string(), colors::deviation())];

    let theme = theme.clone();

    // Custom grid lines
    let grid = {
        let grid_color = colors::grid();
        let x_ticks = vec![
            0.0,
            max_iter * 0.25,
            max_iter * 0.5,
            max_iter * 0.75,
            max_iter,
        ];
        
        div()
            .absolute()
            .inset_0()
            .children(x_ticks.iter().map(|&x| {
                let x_pos = iter_scale.scale(x) as f32;
                div()
                    .absolute()
                    .left(px(x_pos))
                    .top_0()
                    .bottom_0()
                    .w(px(1.0))
                    .bg(grid_color)
            }))
             .child(
                 div().absolute().top(px(loss_scale.scale(min_loss) as f32)).left_0().right_0().h(px(1.0)).bg(grid_color)
            )
             .child(
                 div().absolute().top(px(loss_scale.scale(max_loss) as f32)).left_0().right_0().h(px(1.0)).bg(grid_color)
            )
    };

    div()
        .w(px(width))
        .h(px(height + X_AXIS_HEIGHT + 16.0))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .child(render_y_axis(min_loss, max_loss, graph_height, &theme))
                .child(
                    div()
                        .flex_col()
                        .child(
                            div()
                                .w(px(graph_width))
                                .h(px(graph_height))
                                .bg(theme.surface)
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .relative()
                                .overflow_hidden()
                                .child(grid)
                                .child(curve),
                        )
                        .child(render_iteration_x_axis(graph_width, max_iter, &theme)),
                ),
        )
        .child(render_compact_legend(&legend_items, &theme))
}

impl PlayerView {
    /// Render Spinorama speaker optimization result graphs
    pub fn render_speaker_optimization_result_graphs(
        &self,
        result: &SpeakerOptimizationResult,
        theme: &Theme,
        available_width: f32,
    ) -> impl IntoElement {
        let gap = 8.0;
        let graph_width = (available_width - gap) / 2.0;
        let graph_height = 200.0;

        div()
            .flex()
            .flex_col()
            .gap_2()
            .w_full()
            // Row 1: Frequency Response (Input/Target/Corrected) and Filter Response
            .child(
                div()
                    .flex()
                    .gap_2()
                    // Plot 1: Main Response (Input, Target, Corrected)
                    .child(render_plot_with_title(
                        "On-Axis / Listening Window Response",
                        render_spinorama_main_response_plot(result, theme, graph_width, graph_height),
                        theme,
                    ))
                    // Plot 2: Filter Response
                     .child(render_plot_with_title(
                         "Filter Response",
                         render_speaker_filter_response_plot(result, theme, graph_width, graph_height),
                         theme,
                     ))
            )
            // Row 2: Early Reflections and Sound Power
            .child(
                div()
                    .flex()
                    .gap_2()
                    // Plot 3: Early Reflections (Original vs Corrected)
                    .child(render_plot_with_title(
                        "Early Reflections",
                        render_spinorama_er_plot(result, theme, graph_width, graph_height),
                        theme,
                    ))
                    // Plot 4: Sound Power (Original vs Corrected)
                    .child(render_plot_with_title(
                        "Sound Power",
                        render_spinorama_sp_plot(result, theme, graph_width, graph_height),
                        theme,
                    ))
            )
            // Row 3: Directivity Indexes
             .child(
                div()
                    .flex()
                    .gap_2()
                     // Plot 5: Directivity Index (ER & SP)
                    .child(render_plot_with_title(
                        "Directivity Index",
                        render_spinorama_di_plot(result, theme, available_width - 16.0, graph_height),
                        theme,
                    ))
            )
             // Row 4: Optimization Process
            .child(
                div()
                    .flex()
                    .gap_2()
                    // Plot 6: Loss vs Iteration
                    .child(render_plot_with_title(
                        &format!(
                            "Optimization Process (Before: {:.2}, After: {:.2})",
                            result.initial_loss, result.final_loss
                        ),
                        render_speaker_optimization_loss_plot(result, theme, available_width - 16.0, graph_height),
                        theme,
                    )),
            )

    }
}

/// Render Main Response Plot (Input, Target, Corrected)
fn render_spinorama_main_response_plot(
    result: &SpeakerOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let all_values: Vec<f64> = result
        .input_curve
        .iter()
        .chain(result.corrected_curve.iter())
        .chain(result.target_curve.iter())
        .copied()
        .collect();
    let (min_db, max_db) = calculate_db_range(&all_values);
    let graph_width = width - Y_AXIS_WIDTH;
    let graph_height = height;

    let freq_scale = LogScale::new().domain(MIN_FREQ, MAX_FREQ).range(0.0, graph_width as f64);
    let db_scale = LinearScale::new().domain(min_db, max_db).range(graph_height as f64, 0.0);

    let points_input: Vec<LinePoint> = result.frequencies.iter().zip(result.input_curve.iter()).map(|(&f, &db)| LinePoint::new(f, db)).collect();
    let points_corrected: Vec<LinePoint> = result.frequencies.iter().zip(result.corrected_curve.iter()).map(|(&f, &db)| LinePoint::new(f, db)).collect();
    let points_target: Vec<LinePoint> = result.frequencies.iter().zip(result.target_curve.iter()).map(|(&f, &db)| LinePoint::new(f, db)).collect();

    let config_input = LineConfig::new().stroke_width(1.5).stroke_color(D3Color::from_rgba(colors::input()));
    let config_corrected = LineConfig::new().stroke_width(2.0).stroke_color(D3Color::from_rgba(colors::corrected()));
    let config_target = LineConfig::new().stroke_width(2.0).stroke_color(D3Color::from_rgba(colors::target()));

    let curve_input = render_line(&freq_scale, &db_scale, &points_input, &config_input);
    let curve_corrected = render_line(&freq_scale, &db_scale, &points_corrected, &config_corrected);
    let curve_target = render_line(&freq_scale, &db_scale, &points_target, &config_target);
   
    let legend_items = vec![
        ("Original".to_string(), colors::input()),
        ("Corrected".to_string(), colors::corrected()),
        ("Target".to_string(), colors::target()),
    ];

    let theme = theme.clone();

    div()
        .w(px(width))
        .h(px(height + X_AXIS_HEIGHT + 16.0))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .child(render_y_axis(min_db, max_db, graph_height, &theme))
                .child(
                     div()
                        .flex_col()
                        .child(
                             div()
                                .w(px(graph_width))
                                .h(px(graph_height))
                                .bg(theme.surface)
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .relative()
                                .overflow_hidden()
                                .child(render_grid_lines(graph_width, graph_height, min_db, max_db, &theme))
                                .child(curve_target)
                                .child(curve_input)
                                .child(curve_corrected),
                        )
                        .child(render_x_axis(graph_width, &theme)),
                )
        )
        .child(render_compact_legend(&legend_items, &theme))
}

/// Render Filter Response Plot
fn render_speaker_filter_response_plot(
    result: &SpeakerOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
     let all_values: Vec<f64> = result
        .filter_response.iter().copied().collect();
     let (min_db, max_db) = calculate_db_range(&all_values);
     let graph_width = width - Y_AXIS_WIDTH;
     let graph_height = height;

    let freq_scale = LogScale::new().domain(MIN_FREQ, MAX_FREQ).range(0.0, graph_width as f64);
    let db_scale = LinearScale::new().domain(min_db, max_db).range(graph_height as f64, 0.0);

     let points: Vec<LinePoint> = result.frequencies.iter().zip(result.filter_response.iter()).map(|(&f, &db)| LinePoint::new(f, db)).collect();
     let config = LineConfig::new().stroke_width(2.0).stroke_color(D3Color::from_rgba(colors::filter()));
     let curve = render_line(&freq_scale, &db_scale, &points, &config);

     let legend_items = vec![("Filter Response".to_string(), colors::filter())];
     let theme = theme.clone();

     div()
        .w(px(width))
        .h(px(height + X_AXIS_HEIGHT + 16.0))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .child(render_y_axis(min_db, max_db, graph_height, &theme))
                .child(
                     div()
                        .flex_col()
                        .child(
                             div()
                                .w(px(graph_width))
                                .h(px(graph_height))
                                .bg(theme.surface)
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .relative()
                                .overflow_hidden()
                                .child(render_grid_lines(graph_width, graph_height, min_db, max_db, &theme))
                                .child(curve),
                        )
                        .child(render_x_axis(graph_width, &theme)),
                )
        )
        .child(render_compact_legend(&legend_items, &theme))

}

/// Render Early Reflections Plot
fn render_spinorama_er_plot(
    result: &SpeakerOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let er_corrected: Vec<f64> = result.er_curve.iter().zip(result.filter_response.iter()).map(|(er, f)| er + f).collect();
    
    let all_values: Vec<f64> = result.er_curve.iter().chain(er_corrected.iter()).copied().collect();
    let (min_db, max_db) = calculate_db_range(&all_values);
    let graph_width = width - Y_AXIS_WIDTH;
    let graph_height = height;

    let freq_scale = LogScale::new().domain(MIN_FREQ, MAX_FREQ).range(0.0, graph_width as f64);
    let db_scale = LinearScale::new().domain(min_db, max_db).range(graph_height as f64, 0.0);

    let points_orig: Vec<LinePoint> = result.frequencies.iter().zip(result.er_curve.iter()).map(|(&f, &db)| LinePoint::new(f, db)).collect();
    let points_corr: Vec<LinePoint> = result.frequencies.iter().zip(er_corrected.iter()).map(|(&f, &db)| LinePoint::new(f, db)).collect();

    let config_orig = LineConfig::new().stroke_width(1.5).stroke_color(D3Color::from_rgba(gpui::rgba(0xaaaaaaff))); // Grey
    let config_corr = LineConfig::new().stroke_width(2.0).stroke_color(D3Color::from_rgba(colors::corrected()));

    let curve_orig = render_line(&freq_scale, &db_scale, &points_orig, &config_orig);
    let curve_corr = render_line(&freq_scale, &db_scale, &points_corr, &config_corr);

    let legend_items = vec![
        ("Original ER".to_string(), gpui::rgba(0xaaaaaaff)),
        ("Corrected ER".to_string(), colors::corrected()),
    ];
     let theme = theme.clone();

     div()
        .w(px(width))
        .h(px(height + X_AXIS_HEIGHT + 16.0))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .child(render_y_axis(min_db, max_db, graph_height, &theme))
                .child(
                     div()
                        .flex_col()
                        .child(
                             div()
                                .w(px(graph_width))
                                .h(px(graph_height))
                                .bg(theme.surface)
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .relative()
                                .overflow_hidden()
                                .child(render_grid_lines(graph_width, graph_height, min_db, max_db, &theme))
                                .child(curve_orig)
                                .child(curve_corr),
                        )
                        .child(render_x_axis(graph_width, &theme)),
                )
        )
        .child(render_compact_legend(&legend_items, &theme))
}

/// Render Sound Power Plot
fn render_spinorama_sp_plot(
    result: &SpeakerOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
     // SP Original vs SP Corrected
    let sp_corrected: Vec<f64> = result.sp_curve.iter().zip(result.filter_response.iter()).map(|(sp, f)| sp + f).collect();
    
    let all_values: Vec<f64> = result.sp_curve.iter().chain(sp_corrected.iter()).copied().collect();
    let (min_db, max_db) = calculate_db_range(&all_values);
    let graph_width = width - Y_AXIS_WIDTH;
    let graph_height = height;

    let freq_scale = LogScale::new().domain(MIN_FREQ, MAX_FREQ).range(0.0, graph_width as f64);
    let db_scale = LinearScale::new().domain(min_db, max_db).range(graph_height as f64, 0.0);

    let points_orig: Vec<LinePoint> = result.frequencies.iter().zip(result.sp_curve.iter()).map(|(&f, &db)| LinePoint::new(f, db)).collect();
    let points_corr: Vec<LinePoint> = result.frequencies.iter().zip(sp_corrected.iter()).map(|(&f, &db)| LinePoint::new(f, db)).collect();

    let config_orig = LineConfig::new().stroke_width(1.5).stroke_color(D3Color::from_rgba(gpui::rgba(0xaaaaaaff))); // Grey
    let config_corr = LineConfig::new().stroke_width(2.0).stroke_color(D3Color::from_rgba(colors::corrected()));

    let curve_orig = render_line(&freq_scale, &db_scale, &points_orig, &config_orig);
    let curve_corr = render_line(&freq_scale, &db_scale, &points_corr, &config_corr);

    let legend_items = vec![
        ("Original SP".to_string(), gpui::rgba(0xaaaaaaff)),
        ("Corrected SP".to_string(), colors::corrected()),
    ];
     let theme = theme.clone();

     div()
        .w(px(width))
        .h(px(height + X_AXIS_HEIGHT + 16.0))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .child(render_y_axis(min_db, max_db, graph_height, &theme))
                .child(
                     div()
                        .flex_col()
                        .child(
                             div()
                                .w(px(graph_width))
                                .h(px(graph_height))
                                .bg(theme.surface)
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .relative()
                                .overflow_hidden()
                                .child(render_grid_lines(graph_width, graph_height, min_db, max_db, &theme))
                                .child(curve_orig)
                                .child(curve_corr),
                        )
                        .child(render_x_axis(graph_width, &theme)),
                )
        )
        .child(render_compact_legend(&legend_items, &theme))
}

/// Render Directivity Index Plot
fn render_spinorama_di_plot(
    result: &SpeakerOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let all_values: Vec<f64> = result.er_di_curve.iter().chain(result.sp_di_curve.iter()).copied().collect();
    let (min_db, max_db) = calculate_db_range(&all_values);
    let graph_width = width - Y_AXIS_WIDTH;
    let graph_height = height;

    let freq_scale = LogScale::new().domain(MIN_FREQ, MAX_FREQ).range(0.0, graph_width as f64);
    let db_scale = LinearScale::new().domain(min_db, max_db).range(graph_height as f64, 0.0);

    let points_er: Vec<LinePoint> = result.frequencies.iter().zip(result.er_di_curve.iter()).map(|(&f, &db)| LinePoint::new(f, db)).collect();
    let points_sp: Vec<LinePoint> = result.frequencies.iter().zip(result.sp_di_curve.iter()).map(|(&f, &db)| LinePoint::new(f, db)).collect();

    let config_er = LineConfig::new().stroke_width(2.0).stroke_color(D3Color::from_rgba(gpui::rgba(0xf472b6ff))); // Pink
    let config_sp = LineConfig::new().stroke_width(2.0).stroke_color(D3Color::from_rgba(gpui::rgba(0xc084fcff))); // Purple

    let curve_er = render_line(&freq_scale, &db_scale, &points_er, &config_er);
    let curve_sp = render_line(&freq_scale, &db_scale, &points_sp, &config_sp);

    let legend_items = vec![
        ("ER Directivity Index".to_string(), gpui::rgba(0xf472b6ff)),
        ("SP Directivity Index".to_string(), gpui::rgba(0xc084fcff)),
    ];
     let theme = theme.clone();

     div()
        .w(px(width))
        .h(px(height + X_AXIS_HEIGHT + 16.0))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .child(render_y_axis(min_db, max_db, graph_height, &theme))
                .child(
                     div()
                        .flex_col()
                        .child(
                             div()
                                .w(px(graph_width))
                                .h(px(graph_height))
                                .bg(theme.surface)
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .relative()
                                .overflow_hidden()
                                .child(render_grid_lines(graph_width, graph_height, min_db, max_db, &theme))
                                .child(curve_er)
                                .child(curve_sp),
                        )
                        .child(render_x_axis(graph_width, &theme)),
                )
        )
        .child(render_compact_legend(&legend_items, &theme))
}

/// Plot 6: Speaker Optimization Loss vs Iteration
fn render_speaker_optimization_loss_plot(
    result: &SpeakerOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    if result.optimization_history.is_empty() {
        return div().child("No history available");
    }

    let max_iter = result.optimization_history.last().map(|x| x.0).unwrap_or(0) as f64;
    let losses: Vec<f64> = result.optimization_history.iter().map(|x| x.1).collect();
    let (min_loss, max_loss) = calculate_db_range(&losses);

    let graph_width = width - Y_AXIS_WIDTH;
    let graph_height = height;

    let iter_scale = LinearScale::new()
        .domain(0.0, max_iter)
        .range(0.0, graph_width as f64);
    let loss_scale = LinearScale::new()
        .domain(min_loss, max_loss)
        .range(graph_height as f64, 0.0);

    let points: Vec<LinePoint> = result
        .optimization_history
        .iter()
        .map(|&(i, loss)| LinePoint::new(i as f64, loss))
        .collect();

    let config = LineConfig::new()
        .stroke_width(2.0)
        .stroke_color(D3Color::from_rgba(colors::deviation()));

    let curve = render_line(&iter_scale, &loss_scale, &points, &config);

    let legend_items = vec![("Loss".to_string(), colors::deviation())];

    let theme = theme.clone();

    // Custom grid lines
    let grid = {
        let grid_color = colors::grid();
        let x_ticks = vec![
            0.0,
            max_iter * 0.25,
            max_iter * 0.5,
            max_iter * 0.75,
            max_iter,
        ];
        
        div()
            .absolute()
            .inset_0()
            .children(x_ticks.iter().map(|&x| {
                let x_pos = iter_scale.scale(x) as f32;
                div()
                    .absolute()
                    .left(px(x_pos))
                    .top_0()
                    .bottom_0()
                    .w(px(1.0))
                    .bg(grid_color)
            }))
             .child(
                 div().absolute().top(px(loss_scale.scale(min_loss) as f32)).left_0().right_0().h(px(1.0)).bg(grid_color)
            )
             .child(
                 div().absolute().top(px(loss_scale.scale(max_loss) as f32)).left_0().right_0().h(px(1.0)).bg(grid_color)
            )
    };

    div()
        .w(px(width))
        .h(px(height + X_AXIS_HEIGHT + 16.0))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .child(render_y_axis(min_loss, max_loss, graph_height, &theme))
                .child(
                    div()
                        .flex_col()
                        .child(
                            div()
                                .w(px(graph_width))
                                .h(px(graph_height))
                                .bg(theme.surface)
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .relative()
                                .overflow_hidden()
                                .child(grid)
                                .child(curve),
                        )
                        .child(render_iteration_x_axis(graph_width, max_iter, &theme)),
                ),
        )
        .child(render_compact_legend(&legend_items, &theme))
}
