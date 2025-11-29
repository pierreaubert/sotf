//! Bar chart rendering

use crate::color::D3Color;
use crate::scale::Scale;
use gpui::prelude::*;
use gpui::*;

/// Configuration for bar chart rendering
#[derive(Clone)]
pub struct BarConfig {
    /// Fill color for bars
    pub fill_color: D3Color,
    /// Opacity of bars (0.0 - 1.0)
    pub opacity: f32,
    /// Gap between bars in pixels
    pub bar_gap: f32,
    /// Corner radius for bars
    pub border_radius: f32,
    /// Optional stroke color
    pub stroke_color: Option<D3Color>,
    /// Stroke width in pixels
    pub stroke_width: f32,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            fill_color: D3Color::from_hex(0x4682b4), // Steel blue
            opacity: 0.8,
            bar_gap: 2.0,
            border_radius: 2.0,
            stroke_color: None,
            stroke_width: 1.0,
        }
    }
}

impl BarConfig {
    /// Create a new bar configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the fill color
    pub fn fill_color(mut self, color: D3Color) -> Self {
        self.fill_color = color;
        self
    }

    /// Set the opacity
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Set the gap between bars
    pub fn bar_gap(mut self, gap: f32) -> Self {
        self.bar_gap = gap;
        self
    }

    /// Set the border radius
    pub fn border_radius(mut self, radius: f32) -> Self {
        self.border_radius = radius;
        self
    }

    /// Set the stroke color
    pub fn stroke_color(mut self, color: D3Color) -> Self {
        self.stroke_color = Some(color);
        self
    }

    /// Set the stroke width
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width;
        self
    }
}

/// Data point for a bar chart
#[derive(Debug, Clone)]
pub struct BarDatum {
    /// Category or x-axis value
    pub category: String,
    /// Value (height) of the bar
    pub value: f64,
}

impl BarDatum {
    /// Create a new bar datum
    pub fn new(category: impl Into<String>, value: f64) -> Self {
        Self {
            category: category.into(),
            value,
        }
    }
}

/// Render a bar chart
///
/// # Example
///
/// ```rust,no_run
/// use d3rs::prelude::*;
/// use d3rs::shape::{render_bars, BarConfig, BarDatum};
///
/// let x_scale = LinearScale::new().domain(0.0, 5.0).range(0.0, 400.0);
/// let y_scale = LinearScale::new().domain(0.0, 100.0).range(300.0, 0.0);
///
/// let data = vec![
///     BarDatum::new("A", 50.0),
///     BarDatum::new("B", 80.0),
///     BarDatum::new("C", 30.0),
/// ];
///
/// let config = BarConfig::new().fill_color(D3Color::from_hex(0x4682b4));
/// // render_bars(&x_scale, &y_scale, &data, 400.0, 300.0, &config)
/// ```
pub fn render_bars<XS, YS>(
    x_scale: &XS,
    y_scale: &YS,
    data: &[BarDatum],
    width: f32,
    _height: f32,
    config: &BarConfig,
) -> impl IntoElement
where
    XS: Scale<f64, f64>,
    YS: Scale<f64, f64>,
{
    let (x_min, x_max) = x_scale.range();
    let (y_min, y_max) = y_scale.range();
    let x_range_span = x_max - x_min;
    let y_range_span = y_max - y_min;

    // Calculate bar width based on number of bars
    let bar_count = data.len() as f32;
    let available_width = width - (config.bar_gap * (bar_count - 1.0));
    let bar_width = if bar_count > 0.0 {
        available_width / bar_count
    } else {
        0.0
    };

    // Get baseline (zero point in y scale)
    let (y_domain_min, y_domain_max) = y_scale.domain();
    let baseline = if y_domain_min <= 0.0 && y_domain_max >= 0.0 {
        y_scale.scale(0.0)
    } else {
        y_scale.scale(y_domain_min)
    };
    let baseline_pos = 1.0 - ((baseline - y_min) / y_range_span) as f32;

    div()
        .absolute()
        .inset_0()
        .children(data.iter().enumerate().map(|(i, datum)| {
            let x_value = i as f64 + 0.5; // Center bars at integer positions
            let x_range = x_scale.scale(x_value);
            let x_pos = ((x_range - x_min) / x_range_span) as f32;

            let y_range = y_scale.scale(datum.value);
            // Invert Y for screen coordinates (bottom-to-top becomes top-to-bottom)
            let y_pos = 1.0 - ((y_range - y_min) / y_range_span) as f32;

            // Calculate bar height (from baseline to value)
            let bar_height = (baseline_pos - y_pos).abs();
            let bar_top = if datum.value >= 0.0 {
                y_pos
            } else {
                baseline_pos
            };

            let fill = config.fill_color.to_rgba();

            let mut bar = div()
                .absolute()
                .left(relative(x_pos))
                .top(relative(bar_top))
                .w(px(bar_width))
                .h(px(bar_height))
                .ml(px(-bar_width / 2.0)) // Center the bar
                .bg(fill)
                .opacity(config.opacity);

            if config.border_radius > 0.0 {
                bar = bar.rounded(px(config.border_radius));
            }

            if let Some(stroke) = &config.stroke_color {
                bar = bar
                    .border_color(stroke.to_rgba())
                    .border(px(config.stroke_width));
            }

            bar
        }))
}
