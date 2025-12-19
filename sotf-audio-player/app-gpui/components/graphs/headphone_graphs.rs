use crate::components::graphs::common::{
    band_color, colors, format_frequency, render_compact_legend, render_plot_with_title,
    rgba_to_u32, theme_to_chart_theme,
};
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_px::{line, ScaleType};

impl PlayerView {
    /// Render the optimization result graphs in a 2x2 grid
    pub fn render_optimization_result_graphs(
        &self,
        result: &crate::app::types::HeadphoneEqResult,
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
            // Row 1: Response Comparison + Filter Response
            .child(
                div()
                    .flex()
                    .gap_2()
                    // Plot 1: Response with/without filter and target
                    .child(render_plot_with_title(
                        "Response Comparison",
                        render_response_comparison_plot(result, theme, graph_width, graph_height),
                        theme,
                    ))
                    // Plot 2: Individual IIR filters and sum
                    .child(render_plot_with_title(
                        "Filter Response",
                        render_filter_response_plot(result, theme, graph_width, graph_height),
                        theme,
                    )),
            )
            // Row 2: Filter vs Deviation + Error
            .child(
                div()
                    .flex()
                    .gap_2()
                    // Plot 3: Filter vs Deviation
                    .child(render_plot_with_title(
                        "Filter vs Deviation",
                        render_filter_vs_deviation_plot(result, theme, graph_width, graph_height),
                        theme,
                    ))
                    // Plot 4: Error curve
                    .child(render_plot_with_title(
                        "Residual Error",
                        render_error_plot(result, theme, graph_width, graph_height),
                        theme,
                    )),
            )
    }
}

/// Helper to unzip response data
fn unzip_response(data: Option<&Vec<(f64, f64)>>) -> (Vec<f64>, Vec<f64>) {
    if let Some(data) = data {
        data.iter().cloned().unzip()
    } else {
        (Vec::new(), Vec::new())
    }
}

/// Render Plot 1: Individual IIR filters and combined response using gpui-px
fn render_filter_response_plot(
    result: &crate::app::types::HeadphoneEqResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);
    let (freqs, sum_response) = unzip_response(result.filter_response.as_ref());

    // Build legend items
    let mut legend_items: Vec<(String, Rgba)> = Vec::new();
    legend_items.push(("Sum".to_string(), colors::filter(theme)));

    // Start with the sum (combined filter response) as the primary series
    let mut chart_builder = line(&freqs, &sum_response)
        .x_scale(ScaleType::Log)
        .label("Sum")
        .color(rgba_to_u32(colors::filter(theme)))
        .stroke_width(2.0)
        .theme(chart_theme)
        .size(width, height);

    // Add individual filter responses as additional series
    if let Some(individual) = &result.individual_responses {
        for (i, curve_data) in individual.iter().enumerate() {
            let (_, curve): (Vec<f64>, Vec<f64>) = curve_data.iter().cloned().unzip();
            let freq = result.biquads.get(i).map(|b| b.freq).unwrap_or(0.0);
            let label = format!("F{} {}", i + 1, format_frequency(freq));
            let color = band_color(i, theme);
            legend_items.push((label.clone(), color));

            chart_builder =
                chart_builder.add_series(&curve, Some(label), rgba_to_u32(color), 1.5, 1.0);
        }
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
    result: &crate::app::types::HeadphoneEqResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);
    let (freqs, deviation) = unzip_response(result.deviation_response.as_ref());
    let (_, filter) = unzip_response(result.filter_response.as_ref());

    let legend_items = vec![
        ("Deviation".to_string(), colors::deviation(theme)),
        ("Filter".to_string(), colors::filter(theme)),
    ];

    let chart = line(&freqs, &deviation)
        .x_scale(ScaleType::Log)
        .label("Deviation")
        .color(rgba_to_u32(colors::deviation(theme)))
        .stroke_width(2.0)
        .theme(chart_theme)
        .size(width, height)
        .add_series(
            &filter,
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
    result: &crate::app::types::HeadphoneEqResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);
    let (freqs, error) = unzip_response(result.error_response.as_ref());

    let legend_items = vec![("Error".to_string(), colors::error(theme))];

    let chart = line(&freqs, &error)
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
    result: &crate::app::types::HeadphoneEqResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);
    let (freqs, original) = unzip_response(result.original_response.as_ref());
    let (_, target) = unzip_response(result.target_response.as_ref());
    let (_, corrected) = unzip_response(result.corrected_response.as_ref());

    let legend_items = vec![
        ("Original".to_string(), colors::input(theme)),
        ("Corrected".to_string(), colors::corrected(theme)),
        ("Target".to_string(), colors::target(theme)),
    ];

    let chart = line(&freqs, &original)
        .x_scale(ScaleType::Log)
        .label("Original")
        .color(rgba_to_u32(colors::input(theme)))
        .stroke_width(1.5)
        .theme(chart_theme)
        .size(width, height)
        .add_series(
            &target,
            Some("Target"),
            rgba_to_u32(colors::target(theme)),
            2.0,
            1.0,
        )
        .add_series(
            &corrected,
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