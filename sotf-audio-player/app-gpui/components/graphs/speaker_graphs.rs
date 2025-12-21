use crate::components::graphs::common::{colors, rgba_to_u32, theme_to_chart_theme};
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
        let graph_width = ((available_width - gap) / 2.0).max(800.0);
        let graph_height = 300.0;

        div()
            .flex()
            .flex_col()
            .w_full()
            .child(
                // "On-Axis / Listening Window Response",
                render_spinorama_main_response_plot(
                    result,
                    theme,
                    graph_width,
                    graph_height,
                ),
	    )
            .gap_8()
	    .child(
		// "Filter Response",
                render_speaker_filter_response_plot(
                    result,
                    theme,
                    graph_width,
                    graph_height,
                ),
            )
            .gap_8()
            .child(
		// "Early Reflections",
		render_spinorama_er_plot(result, theme, graph_width, graph_height),
            )
            .gap_8()
	    .child(
                // "Sound Power",
                render_spinorama_sp_plot(result, theme, graph_width, graph_height),
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
}

/// Render Filter Response Plot using gpui-px
fn render_speaker_filter_response_plot(
    result: &SpeakerOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);

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
}

/// Plot 6: Speaker Optimization Loss vs Iteration using gpui-px
#[allow(dead_code)]
fn render_speaker_optimization_loss_plot(
    result: &SpeakerOptimizationResult,
    theme: &Theme,
    width: f32,
    height: f32,
    title: &str,
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

    // Linear scale for iterations (default, no x_scale specified)
    let chart = line(&iterations, &losses)
        .title(title)
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
}

/// Render measurement preview graph showing Input, Target, and Deviation curves
/// Used in spinorama_eq Step 1 (Select Speaker) for previewing the measurement before optimization
pub fn render_speaker_preview_graph(
    frequencies: &[f64],
    input_curve: &[f64],
    target_curve: &[f64],
    deviation_curve: &[f64],
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    let chart_theme = theme_to_chart_theme(theme);

    let chart = line(frequencies, input_curve)
        .x_scale(ScaleType::Log)
        .label("Input")
        .color(0x3498db) // Blue
        .stroke_width(1.5)
        .theme(chart_theme)
        .size(width, height)
        .add_series(target_curve, Some("Target"), 0x27ae60, 1.5, 1.0) // Green
        .add_series(deviation_curve, Some("Deviation"), 0xe74c3c, 1.0, 1.0) // Red
        .build();

    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
}

use crate::app::types::SpinoramaCurves;

/// Render CEA2034 Spinorama plot with dual y-axis
/// Left axis: ON (On Axis), LW (Listening Window), ER (Early Reflections), SP (Sound Power)
/// Right axis: ERDI (Early Reflections DI), SPDI (Sound Power DI)
pub fn render_spinorama_cea2034_graph(
    curves: &SpinoramaCurves,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Div {
    if !curves.is_valid() {
        return div().child("No data available");
    }

    let chart_theme = theme_to_chart_theme(theme);

    // CEA2034 standard colors (matching spinorama.org)
    const ON_AXIS_COLOR: u32 = 0x1f77b4; // Blue
    const LISTENING_WINDOW_COLOR: u32 = 0xff7f0e; // Orange
    const EARLY_REFLECTIONS_COLOR: u32 = 0x2ca02c; // Green
    const SOUND_POWER_COLOR: u32 = 0xd62728; // Red
    const ERDI_COLOR: u32 = 0x9467bd; // Purple (dashed style would be nice)
    const SPDI_COLOR: u32 = 0x8c564b; // Brown (dashed style would be nice)

    let chart = line(&curves.frequencies, &curves.on_axis)
        .x_scale(ScaleType::Log)
        .x_range(20.0, 20000.0)
        .y_label("SPL (dB)")
        .y_range(-40.0, 10.0)
        .y2range(0.0, 50.0)
        .y2_label("DI (dB)")
        .label("On Axis")
        .color(ON_AXIS_COLOR)
        .stroke_width(2.0)
        .theme(chart_theme)
        .size(width, height)
        // Primary axis series (SPL curves)
        .add_series(
            &curves.listening_window,
            Some("Listening Window"),
            LISTENING_WINDOW_COLOR,
            2.0,
            1.0,
        )
        .add_series(
            &curves.early_reflections,
            Some("Early Reflections"),
            EARLY_REFLECTIONS_COLOR,
            1.5,
            0.8,
        )
        .add_series(
            &curves.sound_power,
            Some("Sound Power"),
            SOUND_POWER_COLOR,
            1.5,
            0.8,
        )
        // Secondary axis series (DI curves)
        .add_series_y2(
            &curves.early_reflections_di,
            Some("ER DI"),
            ERDI_COLOR,
            1.5,
            0.9,
        )
        .add_series_y2(
            &curves.sound_power_di,
            Some("SP DI"),
            SPDI_COLOR,
            1.5,
            0.9,
        )
        .build();

    div()
        .w(px(width))
        .flex()
        .flex_col()
        .when_some(chart.ok(), |el, c| el.child(c))
}
