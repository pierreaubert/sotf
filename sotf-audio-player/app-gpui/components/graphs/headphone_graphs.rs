use crate::components::autoeq::HeadphoneOptimizationResult;
use crate::components::graphs::common::{
    band_color, colors, format_frequency, render_compact_legend, render_plot_with_title,
    rgba_to_u32, theme_to_chart_theme,
};
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_px::{ScaleType, line};

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
                        render_optimization_loss_plot(
                            result,
                            theme,
                            available_width - 16.0,
                            graph_height,
                        ),
                        theme,
                    )),
            )
    }
}

/// Render Plot 1: Individual IIR filters and combined response using gpui-px
fn render_filter_response_plot(
    result: &HeadphoneOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);

    // Build legend items
    let legend_items: Vec<(String, Rgba)> = result
        .biquads
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let label = format!("F{} {}", i + 1, format_frequency(b.freq));
            (label, band_color(i, theme))
        })
        .chain(std::iter::once(("Sum".to_string(), colors::filter(theme))))
        .collect();

    // Start with the sum (combined filter response) as the primary series
    let mut chart_builder = line(&result.frequencies, &result.filter_response)
        .x_scale(ScaleType::Log)
        .label("Sum")
        .color(rgba_to_u32(colors::filter(theme)))
        .stroke_width(2.0)
        .theme(chart_theme)
        .size(width, height);

    // Add individual filter responses as additional series
    for (i, curve) in result.individual_filter_responses.iter().enumerate() {
        let label = format!("F{}", i + 1);
        let color = band_color(i, theme);
        chart_builder = chart_builder.add_series(
            curve,
            Some(label),
            rgba_to_u32(color),
            1.5,
            1.0,
        );
    }

    let chart = chart_builder.build();

    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
        .child(render_compact_legend(&legend_items, theme))
}

/// Render Plot 2: Filter response vs deviation from target using gpui-px
fn render_filter_vs_deviation_plot(
    result: &HeadphoneOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);

    let legend_items = vec![
        ("Deviation".to_string(), colors::deviation(theme)),
        ("Filter".to_string(), colors::filter(theme)),
    ];

    let chart = line(&result.frequencies, &result.deviation_curve)
        .x_scale(ScaleType::Log)
        .label("Deviation")
        .color(rgba_to_u32(colors::deviation(theme)))
        .stroke_width(2.0)
        .theme(chart_theme)
        .size(width, height)
        .add_series(
            &result.filter_response,
            Some("Filter"),
            rgba_to_u32(colors::filter(theme)),
            2.0,
            1.0,
        )
        .build();

    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
        .child(render_compact_legend(&legend_items, theme))
}

/// Render Plot 3: Error curve (deviation - filter) using gpui-px
fn render_error_plot(
    result: &HeadphoneOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);

    let legend_items = vec![("Error".to_string(), colors::error(theme))];

    let chart = line(&result.frequencies, &result.error_curve)
        .x_scale(ScaleType::Log)
        .label("Error")
        .color(rgba_to_u32(colors::error(theme)))
        .stroke_width(2.0)
        .theme(chart_theme)
        .size(width, height)
        .build();

    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
        .child(render_compact_legend(&legend_items, theme))
}

/// Render Plot 4: Response comparison (input, corrected, target) using gpui-px
fn render_response_comparison_plot(
    result: &HeadphoneOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);

    let legend_items = vec![
        ("Original".to_string(), colors::input(theme)),
        ("Corrected".to_string(), colors::corrected(theme)),
        ("Target".to_string(), colors::target(theme)),
    ];

    let chart = line(&result.frequencies, &result.input_curve)
        .x_scale(ScaleType::Log)
        .label("Original")
        .color(rgba_to_u32(colors::input(theme)))
        .stroke_width(1.5)
        .theme(chart_theme)
        .size(width, height)
        .add_series(
            &result.target_curve,
            Some("Target"),
            rgba_to_u32(colors::target(theme)),
            2.0,
            1.0,
        )
        .add_series(
            &result.corrected_curve,
            Some("Corrected"),
            rgba_to_u32(colors::corrected(theme)),
            2.0,
            1.0,
        )
        .build();

    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
        .child(render_compact_legend(&legend_items, theme))
}

/// Render Plot 5: Optimization Loss vs Iteration using gpui-px
fn render_optimization_loss_plot(
    result: &HeadphoneOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    if result.optimization_history.is_empty() {
        return div().child("No history available");
    }

    let chart_theme = theme_to_chart_theme(theme);

    let iterations: Vec<f64> = result
        .optimization_history
        .iter()
        .map(|&(i, _)| i as f64)
        .collect();
    let losses: Vec<f64> = result.optimization_history.iter().map(|&(_, loss)| loss).collect();

    let legend_items = vec![("Loss".to_string(), colors::deviation(theme))];

    // Linear scale for iterations (default, no x_scale specified)
    let chart = line(&iterations, &losses)
        .label("Loss")
        .color(rgba_to_u32(colors::deviation(theme)))
        .stroke_width(2.0)
        .theme(chart_theme)
        .size(width, height)
        .build();

    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
        .child(render_compact_legend(&legend_items, theme))
}
