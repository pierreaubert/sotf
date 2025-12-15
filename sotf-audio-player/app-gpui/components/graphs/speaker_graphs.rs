use crate::components::graphs::common::{
    colors, render_compact_legend, render_plot_with_title, rgba_to_u32, theme_to_chart_theme,
};
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_px::{ScaleType, line};
use sotf_audio_player::autoeq::SpeakerOptimizationResult;

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
                        render_spinorama_main_response_plot(
                            result,
                            theme,
                            graph_width,
                            graph_height,
                        ),
                        theme,
                    ))
                    // Plot 2: Filter Response
                    .child(render_plot_with_title(
                        "Filter Response",
                        render_speaker_filter_response_plot(
                            result,
                            theme,
                            graph_width,
                            graph_height,
                        ),
                        theme,
                    )),
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
                    )),
            )
            // Row 3: Directivity Indexes
            .child(
                div()
                    .flex()
                    .gap_2()
                    // Plot 5: Directivity Index (ER & SP)
                    .child(render_plot_with_title(
                        "Directivity Index",
                        render_spinorama_di_plot(
                            result,
                            theme,
                            available_width - 16.0,
                            graph_height,
                        ),
                        theme,
                    )),
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
                        render_speaker_optimization_loss_plot(
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

/// Render Main Response Plot (Input, Target, Corrected) using gpui-px
fn render_spinorama_main_response_plot(
    result: &SpeakerOptimizationResult,
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

/// Render Filter Response Plot using gpui-px
fn render_speaker_filter_response_plot(
    result: &SpeakerOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);

    let legend_items = vec![("Filter Response".to_string(), colors::filter(theme))];

    let chart = line(&result.frequencies, &result.filter_response)
        .x_scale(ScaleType::Log)
        .label("Filter Response")
        .color(rgba_to_u32(colors::filter(theme)))
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

/// Render Early Reflections Plot using gpui-px
fn render_spinorama_er_plot(
    result: &SpeakerOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);

    // Calculate corrected ER curve
    let er_corrected: Vec<f64> = result
        .er_curve
        .iter()
        .zip(result.filter_response.iter())
        .map(|(er, f)| er + f)
        .collect();

    let legend_items = vec![
        ("Original ER".to_string(), colors::secondary_line(theme)),
        ("Corrected ER".to_string(), colors::corrected(theme)),
    ];

    let chart = line(&result.frequencies, &result.er_curve)
        .x_scale(ScaleType::Log)
        .label("Original ER")
        .color(rgba_to_u32(colors::secondary_line(theme)))
        .stroke_width(1.5)
        .theme(chart_theme)
        .size(width, height)
        .add_series(
            &er_corrected,
            Some("Corrected ER"),
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

/// Render Sound Power Plot using gpui-px
fn render_spinorama_sp_plot(
    result: &SpeakerOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);

    // Calculate corrected SP curve
    let sp_corrected: Vec<f64> = result
        .sp_curve
        .iter()
        .zip(result.filter_response.iter())
        .map(|(sp, f)| sp + f)
        .collect();

    let legend_items = vec![
        ("Original SP".to_string(), colors::secondary_line(theme)),
        ("Corrected SP".to_string(), colors::corrected(theme)),
    ];

    let chart = line(&result.frequencies, &result.sp_curve)
        .x_scale(ScaleType::Log)
        .label("Original SP")
        .color(rgba_to_u32(colors::secondary_line(theme)))
        .stroke_width(1.5)
        .theme(chart_theme)
        .size(width, height)
        .add_series(
            &sp_corrected,
            Some("Corrected SP"),
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

/// Render Directivity Index Plot using gpui-px
fn render_spinorama_di_plot(
    result: &SpeakerOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);

    let legend_items = vec![
        (
            "ER Directivity Index".to_string(),
            colors::directivity_er(theme),
        ),
        (
            "SP Directivity Index".to_string(),
            colors::directivity_sp(theme),
        ),
    ];

    let chart = line(&result.frequencies, &result.er_di_curve)
        .x_scale(ScaleType::Log)
        .label("ER DI")
        .color(rgba_to_u32(colors::directivity_er(theme)))
        .stroke_width(2.0)
        .theme(chart_theme)
        .size(width, height)
        .add_series(
            &result.sp_di_curve,
            Some("SP DI"),
            rgba_to_u32(colors::directivity_sp(theme)),
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

/// Plot 6: Speaker Optimization Loss vs Iteration using gpui-px
fn render_speaker_optimization_loss_plot(
    result: &SpeakerOptimizationResult,
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
    let losses: Vec<f64> = result
        .optimization_history
        .iter()
        .map(|&(_, loss)| loss)
        .collect();

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
