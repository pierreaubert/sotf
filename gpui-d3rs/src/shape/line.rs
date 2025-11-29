//! Line chart rendering

use crate::color::D3Color;
use crate::scale::Scale;
use gpui::prelude::*;
use gpui::*;

/// Curve interpolation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveType {
    /// Linear interpolation (straight lines between points)
    Linear,
    /// Step function (horizontal then vertical)
    Step,
    /// Step before (vertical then horizontal)
    StepBefore,
    /// Step after (horizontal then vertical)
    StepAfter,
}

/// Configuration for line chart rendering
#[derive(Clone)]
pub struct LineConfig {
    /// Stroke color for the line
    pub stroke_color: D3Color,
    /// Line width in pixels
    pub stroke_width: f32,
    /// Opacity of the line (0.0 - 1.0)
    pub opacity: f32,
    /// Curve interpolation type
    pub curve: CurveType,
    /// Whether to show points at data locations
    pub show_points: bool,
    /// Point radius if show_points is true
    pub point_radius: f32,
    /// Fill color for points
    pub point_fill_color: Option<D3Color>,
}

impl Default for LineConfig {
    fn default() -> Self {
        Self {
            stroke_color: D3Color::from_hex(0x4682b4), // Steel blue
            stroke_width: 2.0,
            opacity: 1.0,
            curve: CurveType::Linear,
            show_points: false,
            point_radius: 3.0,
            point_fill_color: None,
        }
    }
}

impl LineConfig {
    /// Create a new line configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the stroke color
    pub fn stroke_color(mut self, color: D3Color) -> Self {
        self.stroke_color = color;
        self
    }

    /// Set the stroke width
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width;
        self
    }

    /// Set the opacity
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Set the curve type
    pub fn curve(mut self, curve: CurveType) -> Self {
        self.curve = curve;
        self
    }

    /// Enable point rendering
    pub fn show_points(mut self, show: bool) -> Self {
        self.show_points = show;
        self
    }

    /// Set point radius
    pub fn point_radius(mut self, radius: f32) -> Self {
        self.point_radius = radius;
        self
    }

    /// Set point fill color
    pub fn point_fill_color(mut self, color: D3Color) -> Self {
        self.point_fill_color = Some(color);
        self
    }
}

/// Data point for a line chart
#[derive(Debug, Clone, Copy)]
pub struct LinePoint {
    /// X coordinate
    pub x: f64,
    /// Y coordinate
    pub y: f64,
}

impl LinePoint {
    /// Create a new line point
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Render a line chart
///
/// # Example
///
/// ```rust,no_run
/// use d3rs::prelude::*;
/// use d3rs::shape::{render_line, LineConfig, LinePoint, CurveType};
///
/// let x_scale = LinearScale::new().domain(0.0, 100.0).range(0.0, 400.0);
/// let y_scale = LinearScale::new().domain(0.0, 100.0).range(300.0, 0.0);
///
/// let data = vec![
///     LinePoint::new(0.0, 20.0),
///     LinePoint::new(25.0, 50.0),
///     LinePoint::new(50.0, 30.0),
///     LinePoint::new(75.0, 80.0),
///     LinePoint::new(100.0, 60.0),
/// ];
///
/// let config = LineConfig::new()
///     .stroke_color(D3Color::from_hex(0x4682b4))
///     .curve(CurveType::Linear)
///     .show_points(true);
/// // render_line(&x_scale, &y_scale, &data, &config)
/// ```
pub fn render_line<XS, YS>(
    x_scale: &XS,
    y_scale: &YS,
    data: &[LinePoint],
    config: &LineConfig,
) -> impl IntoElement
where
    XS: Scale<f64, f64>,
    YS: Scale<f64, f64>,
{
    let (x_min, x_max) = x_scale.range();
    let (y_min, y_max) = y_scale.range();
    let x_range_span = x_max - x_min;
    let y_range_span = y_max - y_min;

    let stroke = config.stroke_color.to_rgba();

    // Container for line segments and points
    let mut container = div().absolute().inset_0();

    // Render line segments
    if data.len() >= 2 {
        for i in 0..data.len() - 1 {
            let p1 = &data[i];
            let p2 = &data[i + 1];

            let x1_range = x_scale.scale(p1.x);
            let x1_pos = ((x1_range - x_min) / x_range_span) as f32;
            let y1_range = y_scale.scale(p1.y);
            // Invert Y for screen coordinates (bottom-to-top becomes top-to-bottom)
            let y1_pos = 1.0 - ((y1_range - y_min) / y_range_span) as f32;

            let x2_range = x_scale.scale(p2.x);
            let x2_pos = ((x2_range - x_min) / x_range_span) as f32;
            let y2_range = y_scale.scale(p2.y);
            // Invert Y for screen coordinates (bottom-to-top becomes top-to-bottom)
            let y2_pos = 1.0 - ((y2_range - y_min) / y_range_span) as f32;

            // Render segment based on curve type
            match config.curve {
                CurveType::Linear => {
                    container = container.child(render_line_segment(
                        x1_pos,
                        y1_pos,
                        x2_pos,
                        y2_pos,
                        stroke,
                        config.stroke_width,
                        config.opacity,
                    ));
                }
                CurveType::Step | CurveType::StepAfter => {
                    // Horizontal then vertical
                    let mid_x = x2_pos;
                    let mid_y = y1_pos;
                    container = container
                        .child(render_line_segment(
                            x1_pos,
                            y1_pos,
                            mid_x,
                            mid_y,
                            stroke,
                            config.stroke_width,
                            config.opacity,
                        ))
                        .child(render_line_segment(
                            mid_x,
                            mid_y,
                            x2_pos,
                            y2_pos,
                            stroke,
                            config.stroke_width,
                            config.opacity,
                        ));
                }
                CurveType::StepBefore => {
                    // Vertical then horizontal
                    let mid_x = x1_pos;
                    let mid_y = y2_pos;
                    container = container
                        .child(render_line_segment(
                            x1_pos,
                            y1_pos,
                            mid_x,
                            mid_y,
                            stroke,
                            config.stroke_width,
                            config.opacity,
                        ))
                        .child(render_line_segment(
                            mid_x,
                            mid_y,
                            x2_pos,
                            y2_pos,
                            stroke,
                            config.stroke_width,
                            config.opacity,
                        ));
                }
            }
        }
    }

    // Render points if enabled
    if config.show_points {
        let point_fill = config
            .point_fill_color
            .as_ref()
            .unwrap_or(&config.stroke_color)
            .to_rgba();

        for point in data {
            let x_range = x_scale.scale(point.x);
            let x_pos = ((x_range - x_min) / x_range_span) as f32;
            let y_range = y_scale.scale(point.y);
            // Invert Y for screen coordinates (bottom-to-top becomes top-to-bottom)
            let y_pos = 1.0 - ((y_range - y_min) / y_range_span) as f32;

            let diameter = config.point_radius * 2.0;

            container = container.child(
                div()
                    .absolute()
                    .left(relative(x_pos))
                    .top(relative(y_pos))
                    .w(px(diameter))
                    .h(px(diameter))
                    .ml(px(-config.point_radius))
                    .mt(px(-config.point_radius))
                    .rounded_full()
                    .bg(point_fill)
                    .opacity(config.opacity),
            );
        }
    }

    container
}

/// Render a single line segment using divs
fn render_line_segment(
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    color: Rgba,
    width: f32,
    opacity: f32,
) -> Div {
    // Calculate line parameters
    let dx = x2 - x1;
    let dy = y2 - y1;
    let length = (dx * dx + dy * dy).sqrt();
    let angle = dy.atan2(dx);

    // Position at start point, rotate and extend
    div()
        .absolute()
        .left(relative(x1))
        .top(relative(y1))
        .w(relative(length))
        .h(px(width))
        .mt(px(-width / 2.0)) // Center vertically
        .bg(color)
        .opacity(opacity)
        // Note: GPUI doesn't have a direct rotate method for divs
        // This is a limitation - in a real implementation we'd use a transform
        // For now, this works for horizontal and vertical lines
        // A full implementation would need custom rendering or SVG
        .when(angle.abs() < 0.01 || (angle - std::f32::consts::PI).abs() < 0.01, |el| el)
        .when((angle - std::f32::consts::PI / 2.0).abs() < 0.01 || (angle + std::f32::consts::PI / 2.0).abs() < 0.01, |el| {
            // Vertical line - swap width and height
            el.w(px(width))
                .h(relative(length))
                .ml(px(-width / 2.0))
                .mt(px(0.0))
        })
}
