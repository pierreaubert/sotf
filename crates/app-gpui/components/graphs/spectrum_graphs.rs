//! Spectrum Graphs
//!
//! Visualizations for spectrum data using Heatmaps / Surface plots.

use crate::theme::Theme;

use gpui::prelude::*;

use gpui::*;

use gpui_px::{ColorScale, ScaleType, heatmap};

/// Data grid for spectrum plotting (Frequency x Y-Axis)

/// Y-Axis can be Time (Spectrogram) or Angle (Directivity)
#[derive(Clone)]
pub struct SpectrumGrid {
    pub x_values: Vec<f64>, // Frequency bins
    pub y_values: Vec<f64>, // Time frames or Angles
    pub z_values: Vec<f64>, // Magnitude values (flattened row-major: y * width + x)
}

/// Configuration for spectrum heatmap
#[derive(Clone)]
pub struct SpectrumConfig {
    pub title: Option<String>,
    pub x_label: Option<String>,
    pub y_label: Option<String>,
    pub x_range: Option<(f64, f64)>, // Optional override
    pub y_range: Option<(f64, f64)>, // Optional override
    pub x_scale: ScaleType,
    pub width: f32,
    pub height: f32,
    pub color_scale: Option<ColorScale>,
}

impl Default for SpectrumConfig {
    fn default() -> Self {
        Self {
            title: None,
            x_label: Some("Frequency (Hz)".to_string()),
            y_label: None,
            x_range: None,
            y_range: None,
            x_scale: ScaleType::Log,
            width: 800.0,
            height: 400.0,
            color_scale: None, // Will use gpui-px default if None
        }
    }
}

/// Render a spectrum heatmap
pub fn render_spectrum_heatmap(
    data: SpectrumGrid,
    config: SpectrumConfig,
    _theme: &Theme,
    interactive_state: Option<&gpui_px::interaction::InteractiveChartState>,
) -> impl IntoElement {
    if data.x_values.is_empty() || data.y_values.is_empty() || data.z_values.is_empty() {
        return div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(config.width))
            .h(px(config.height))
            .child(
                div()
                    .text_base()
                    .text_color(gpui::rgb(0x666666))
                    .child("No spectrum data available."),
            )
            .into_any_element();
    }

    // let chart_theme = theme_to_chart_theme(theme);
    let width_samples = data.x_values.len();
    let height_samples = data.y_values.len();

    // Verify z_values size matches x * y
    if data.z_values.len() != width_samples * height_samples {
        return div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(config.width))
            .h(px(config.height))
            .text_color(gpui::rgb(0xFF0000))
            .child("Data dimension mismatch")
            .into_any_element();
    }

    let mut chart = heatmap(&data.z_values, width_samples, height_samples)
        .x(&data.x_values)
        .y(&data.y_values)
        .x_scale(config.x_scale)
        .chart_size(config.width, config.height);

    if let Some(title) = &config.title {
        chart = chart.title(title.clone());
    }

    // Axis labels are not directly supported by HeatmapChart builder in this version.
    // They should be rendered outside the chart if needed.

    if let Some(cs) = config.color_scale {
        chart = chart.color_scale(cs);
    }

    // Apply ranges if specified
    if let Some((min, max)) = config.x_range {
        chart = chart.x_range(min, max);
    }
    if let Some((min, max)) = config.y_range {
        chart = chart.y_range(min, max);
    }

    // Build the chart
    match chart.build() {
        Ok(element) => {
            if let Some(state) = interactive_state {
                gpui_px::interaction::interactive(
                    "spectrum-heatmap",
                    element.into_any_element(),
                    state.clone(),
                )
                .build()
                .into_any_element()
            } else {
                element.into_any_element()
            }
        }
        Err(e) => div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(config.width))
            .h(px(config.height))
            .text_color(gpui::rgb(0xFF0000))
            .child(format!("Error building chart: {}", e))
            .into_any_element(),
    }
}
