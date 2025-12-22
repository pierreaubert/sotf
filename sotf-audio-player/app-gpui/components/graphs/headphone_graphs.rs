use crate::components::graphs::common::{colors, rgba_to_u32, theme_to_chart_theme};
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_px::{ScaleType, line};

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
        let graph_width = ((available_width - gap) / 2.0).max(800.0);
        let graph_height = 300.0;

        div()
            .flex()
            .flex_row()
            .flex_col()
            .items_center()
            .justify_between()
            .w_full()
            .child(render_response_comparison_plot(
                result,
                theme,
                graph_width,
                graph_height,
            ))
            .gap_8()
            .child(render_filter_response_plot(
                result,
                theme,
                graph_width,
                graph_height,
            ))
            .gap_8()
            .child(render_filter_vs_deviation_plot(
                result,
                theme,
                graph_width,
                graph_height,
            ))
            .gap_8()
            .child(render_error_plot(result, theme, graph_width, graph_height))
            .gap_8()
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

    // Start with the sum (combined filter response) as the primary series
    let mut chart_builder = line(&freqs, &sum_response)
        .x_scale(ScaleType::Log)
        .y_range(-10.0, 10.0)
        .title("Filter Response")
        .label("Sum")
        .stroke_width(2.0)
        .theme(chart_theme)
        .size(width, height);

    // Add individual filter responses as additional series
    if let Some(individual) = &result.individual_responses {
        for (i, curve_data) in individual.iter().enumerate() {
            let (_, curve): (Vec<f64>, Vec<f64>) = curve_data.iter().cloned().unzip();
            let freq = result.biquads.get(i).map(|b| b.freq).unwrap_or(0.0);
            let label = format!("F{} {}", i + 1, freq.floor());
            chart_builder = chart_builder.add_series(
                &curve,
                Some(label),
                rgba_to_u32(colors::filter(theme)),
                1.5,
                1.0,
            );
        }
    }

    let chart = chart_builder.build();

    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
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

    let chart = line(&freqs, &deviation)
        .x_scale(ScaleType::Log)
        .x_label("Frequency (Hz)")
        .y_label("Amplitude (dB SPL)")
        .y_range(-10.0, 10.0)
        .label("Deviation")
        .title("Filter Response v.s. Deviation")
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

    // Create reference line points from 50Hz to 5kHz
    let ref_freqs: Vec<f64> = freqs
        .iter()
        .copied()
        .filter(|&f| f >= 50.0 && f <= 5000.0)
        .collect();
    let plus_one: Vec<f64> = vec![1.0; ref_freqs.len()];
    let minus_one: Vec<f64> = vec![-1.0; ref_freqs.len()];
    let error_color = rgba_to_u32(colors::error(theme));

    let chart = line(&freqs, &error)
        .x_scale(ScaleType::Log)
        .y_range(-2.0, 2.0)
        .x_label("Frequency (Hz)")
        .y_label("Amplitude (dB SPL)")
        .title("Error details")
        .label("Error")
        .color(error_color)
        .stroke_width(2.0)
        .theme(chart_theme)
        .size(width, height)
        // Add +1 dB reference line (50Hz - 5kHz)
        .add_series_with_x(&ref_freqs, &plus_one, Some("+1 dB"), error_color, 2.0, 0.5)
        // Add -1 dB reference line (50Hz - 5kHz)
        .add_series_with_x(&ref_freqs, &minus_one, Some("-1 dB"), error_color, 2.0, 0.5)
        .build();

    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
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
    let chart = line(&freqs, &original)
        .x_scale(ScaleType::Log)
        .title("Original v.s. Corrected v.s. Target")
        .label("Original")
        .stroke_width(1.5)
        .theme(chart_theme)
        .size(width, height)
        .x_label("Frequency (Hz)")
        .y_label("Amplitude (dB SPL)")
        .y_range(-10.0, 10.0)
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
}
