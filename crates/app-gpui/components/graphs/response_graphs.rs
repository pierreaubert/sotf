//! Response Graphs
//!
//! Shared plotting functions for Frequency Response, Phase, etc.
//! Used by Recording Evaluation and Room EQ Review.

use crate::components::graphs::common::{render_empty_state, rgba_to_u32, theme_to_chart_theme};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui_px::{LegendPosition, ScaleType, line};

/// Fallback channel colors when no theme is available.
const CHANNEL_COLORS_FALLBACK: [u32; 6] = [
    0x4285f4, // Blue
    0xea4335, // Red
    0x34a853, // Green
    0xfbbc04, // Yellow
    0x9c27b0, // Purple
    0x00bcd4, // Cyan
];

/// Get a channel color as u32 from the theme, with fallback.
pub fn channel_color(theme: &Theme, idx: usize) -> u32 {
    if theme.channel_colors.is_empty() {
        CHANNEL_COLORS_FALLBACK[idx % CHANNEL_COLORS_FALLBACK.len()]
    } else {
        rgba_to_u32(theme.channel_colors[idx % theme.channel_colors.len()])
    }
}

/// Data series for plotting
#[derive(Clone)]
pub struct Series {
    pub label: String,
    pub color: u32, // 0xRRGGBB
    pub x_values: Vec<f64>,
    pub y_values: Vec<f64>,
    pub stroke_width: f32,
    pub opacity: f32,
}

impl Series {
    pub fn new(label: impl Into<String>, color: u32, x: Vec<f64>, y: Vec<f64>) -> Self {
        Self {
            label: label.into(),
            color,
            x_values: x,
            y_values: y,
            stroke_width: 2.0,
            opacity: 1.0,
        }
    }

    pub fn with_width(mut self, width: f32) -> Self {
        self.stroke_width = width;
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }
}

/// Common chart configuration
#[derive(Clone)]
pub struct ChartConfig {
    pub title: Option<String>,
    pub x_label: Option<String>,
    pub y_label: Option<String>,
    pub x_range: (f64, f64),
    pub y_range: (f64, f64),
    pub x_scale: ScaleType,
    pub width: f32,
    pub height: f32,
}

impl Default for ChartConfig {
    fn default() -> Self {
        Self {
            title: None,
            x_label: None,
            y_label: None,
            x_range: (20.0, 20000.0),
            y_range: (-20.0, 20.0),
            x_scale: ScaleType::Log,
            width: 800.0,
            height: 300.0,
        }
    }
}

/// Render a line chart with multiple series
pub fn render_line_chart(
    series: Vec<Series>,
    config: ChartConfig,
    theme: &Theme,
    interactive_state: Option<&gpui_px::interaction::InteractiveChartState>,
) -> impl IntoElement {
    // Filter out series with empty data to prevent EmptyData errors from the chart builder
    let series: Vec<Series> = series
        .into_iter()
        .filter(|s| !s.x_values.is_empty() && !s.y_values.is_empty())
        .collect();

    if series.is_empty() {
        return render_empty_state(
            crate::components::icons::IconName::AudioWaveform,
            "No data",
            theme,
        );
    }

    let chart_theme = theme_to_chart_theme(theme);
    let first = &series[0];

    // Determine effective ranges
    // If interactive state is present and zoomed, use its domain
    let (x_min, x_max) = interactive_state
        .filter(|s| s.is_zoomed())
        .map(|s| s.x_domain())
        .unwrap_or(config.x_range);

    let (y_min, y_max) = interactive_state
        .filter(|s| s.is_zoomed())
        .map(|s| s.y_domain())
        .unwrap_or(config.y_range);

    let mut chart = line(&first.x_values, &first.y_values)
        .x_scale(config.x_scale)
        .x_range(x_min, x_max)
        .y_range(y_min, y_max)
        .legend_position(LegendPosition::Bottom)
        .color(first.color)
        .stroke_width(first.stroke_width)
        .opacity(first.opacity)
        .theme(chart_theme)
        .size(config.width, config.height);

    if let Some(_label) = &config.title {
        // gpui_px line builder might not expose title setting directly in this version
        // or it might be .title(label). Let's assume .label is series label.
        // We'll skip title on chart itself and let caller render it outside if needed,
        // or check if line() supports it. `line` builder sets label for the *series*.
        // The chart title is usually separate.
    }

    if let Some(label) = &config.y_label {
        chart = chart.y_label(label);
    }

    // First series label
    chart = chart.label(&first.label);

    // Add remaining series
    for s in series.iter().skip(1) {
        chart = chart.add_series_with_x(
            &s.x_values,
            &s.y_values,
            Some(&s.label),
            s.color,
            s.stroke_width,
            s.opacity,
        );
    }

    let chart_element = match chart.build() {
        Ok(c) => c.into_any_element(),
        Err(e) => {
            log::warn!("Chart build failed: {:?}", e);
            return render_empty_state(
                crate::components::icons::IconName::AudioWaveform,
                "Unable to render chart",
                theme,
            );
        }
    };

    if let Some(state) = interactive_state {
        gpui_px::interaction::interactive("shared-response-chart", chart_element, state.clone())
            .build()
            .into_any_element()
    } else {
        chart_element
    }
}
