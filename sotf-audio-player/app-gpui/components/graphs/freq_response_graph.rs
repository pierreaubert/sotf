use crate::components::graphs::common::{
    band_color, rgba_to_u32, theme_to_chart_theme,
};
use crate::theme::Theme;
use autoeq_iir::Biquad;
use d3rs::scale::{LogScale, Scale};
use gpui::prelude::*;
use gpui::*;
use gpui_px::{ScaleType, line};
use sotf_audio_player::EQFilter;

use super::GraphConfig;

/// Default sample rate for filter calculations
const SAMPLE_RATE: f64 = 48000.0;

/// Calculate the combined response in dB at a given frequency
fn calculate_response_at_freq(filters: &[EQFilter], freq: f64) -> f64 {
    if filters.is_empty() {
        return 0.0;
    }
    filters
        .iter()
        .map(|f| {
            let biquad = Biquad::new(
                f.filter_type.clone(),
                f.frequency,
                SAMPLE_RATE,
                f.q,
                f.gain_db,
            );
            biquad.log_result(freq)
        })
        .sum()
}

/// Render the main frequency response graph using gpui-px
pub fn render_freq_response_graph(
    filters: &[EQFilter],
    selected_band: Option<usize>,
    config: GraphConfig,
    theme: &Theme,
    available_width: f32,
) -> impl IntoElement {
    let theme = theme.clone();

    // Calculate dimensions based on legend position
    let graph_area_width = available_width;
    let graph_area_height = (graph_area_width / config.aspect_ratio).max(config.min_height);

    // Generate frequency points for smooth curve (logarithmically spaced)
    let num_points = 120;
    let freq_points: Vec<f64> = (0..num_points)
        .map(|i| {
            let t = i as f64 / (num_points - 1) as f64;
            let log_min = config.min_freq.ln();
            let log_max = config.max_freq.ln();
            (log_min + t * (log_max - log_min)).exp()
        })
        .collect();

    // Calculate response curve data points
    let response_db: Vec<f64> = freq_points
        .iter()
        .map(|&freq| calculate_response_at_freq(filters, freq))
        .collect();

    // Build chart using gpui-px::line()
    let chart_theme = theme_to_chart_theme(&theme);

    // Create the line chart with log X scale for frequency
    let chart_result = if config.show_response_curve && !filters.is_empty() {
        line(&freq_points, &response_db)
            .x_scale(ScaleType::Log)
            .color(rgba_to_u32(theme.accent))
            .stroke_width(2.0)
            .theme(chart_theme)
            .size(graph_area_width, graph_area_height)
            .build()
    } else {
        // Empty chart with just axes - create a flat line at 0dB
        let flat_response: Vec<f64> = freq_points.iter().map(|_| 0.0).collect();
        line(&freq_points, &flat_response)
            .x_scale(ScaleType::Log)
            .color(rgba_to_u32(theme.text_muted))
            .stroke_width(0.5)
            .opacity(0.3)
            .theme(chart_theme)
            .size(graph_area_width, graph_area_height)
            .build()
    };

    // Main container
    div()
        .w(px(available_width))
        .h(px(graph_area_height))
        .relative()
        // Chart from gpui-px
        .when_some(chart_result.ok(), |el, chart| el.child(chart))
        // Filter point indicators overlaid on top
        .child(render_filter_points_overlay(
            filters,
            selected_band,
            config.min_freq,
            config.max_freq,
            config.min_db,
            config.max_db,
            graph_area_width,
            graph_area_height,
            &theme,
        ))
}

/// Render filter point indicators as an overlay
fn render_filter_points_overlay(
    filters: &[EQFilter],
    selected_band: Option<usize>,
    min_freq: f64,
    max_freq: f64,
    min_db: f64,
    max_db: f64,
    width: f32,
    height: f32,
    theme: &Theme,
) -> impl IntoElement {
    // Account for chart margins (gpui-px uses 50px left, 30px bottom, 10px top, 20px right)
    let margin_left = 50.0;
    let margin_right = 20.0;
    let margin_top = 10.0;
    let margin_bottom = 30.0;

    let plot_width = width - margin_left - margin_right;
    let plot_height = height - margin_top - margin_bottom;

    // Create log scale for frequency positioning
    let freq_scale = LogScale::new()
        .domain(min_freq, max_freq)
        .range(0.0, plot_width as f64);

    div()
        .absolute()
        .top(px(margin_top))
        .left(px(margin_left))
        .w(px(plot_width))
        .h(px(plot_height))
        .children(filters.iter().enumerate().map(|(i, f)| {
            // Calculate X position using log scale
            let x_pos = freq_scale.scale(f.frequency) as f32;

            // Calculate Y position using linear scale (inverted - top is max dB)
            let db_range = max_db - min_db;
            let y_normalized = (max_db - f.gain_db) / db_range;
            let y_pos = (y_normalized * plot_height as f64) as f32;

            let is_selected = selected_band == Some(i);
            let color = band_color(i, theme);
            let size = if is_selected { 16.0 } else { 12.0 };

            div()
                .absolute()
                .left(px(x_pos))
                .top(px(y_pos))
                .w(px(size))
                .h(px(size))
                .ml(px(-size / 2.0))
                .mt(px(-size / 2.0))
                .rounded_full()
                .bg(color)
                .border_2()
                .border_color(if is_selected {
                    theme.text_primary
                } else {
                    color
                })
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(theme.text_on_accent)
                .font_weight(FontWeight::BOLD)
                .child(format!("{}", i + 1))
        }))
}
