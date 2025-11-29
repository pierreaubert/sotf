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

/// Render a line chart using GPUI's PathBuilder for proper vector line rendering
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
    let y_range_span = (y_max - y_min).abs();

    // Pre-calculate pixel positions for the line
    let mut pixel_points: Vec<Point<Pixels>> = Vec::with_capacity(data.len());
    for point in data {
        let x_range = x_scale.scale(point.x);
        let x_px = ((x_range - x_min) / x_range_span) as f32;
        let y_range = y_scale.scale(point.y);
        // Invert Y for screen coordinates
        let y_px = 1.0 - ((y_range - y_min) / y_range_span) as f32;
        pixel_points.push(gpui::point(px(x_px), px(y_px)));
    }

    let stroke_color = config.stroke_color.to_rgba();
    let stroke_width = config.stroke_width;
    let opacity = config.opacity;
    let curve_type = config.curve;
    let show_points = config.show_points;
    let point_radius = config.point_radius;
    let point_fill = config
        .point_fill_color
        .as_ref()
        .unwrap_or(&config.stroke_color)
        .to_rgba();

    canvas(
        // Prepaint: calculate actual pixel positions based on bounds
        move |bounds, _window, _cx| {
            let width: f32 = bounds.size.width.into();
            let height: f32 = bounds.size.height.into();
            let origin_x: f32 = bounds.origin.x.into();
            let origin_y: f32 = bounds.origin.y.into();

            // Convert relative positions to absolute pixel positions
            let absolute_points: Vec<Point<Pixels>> = pixel_points
                .iter()
                .map(|p| {
                    let rel_x: f32 = p.x.into();
                    let rel_y: f32 = p.y.into();
                    gpui::point(
                        px(origin_x + rel_x * width),
                        px(origin_y + rel_y * height),
                    )
                })
                .collect();

            absolute_points
        },
        // Paint: draw the line path and optionally points
        move |_bounds, absolute_points: Vec<Point<Pixels>>, window, _cx| {
            if absolute_points.len() < 2 {
                return;
            }

            // Build the path based on curve type
            let mut path_builder = PathBuilder::stroke(px(stroke_width));

            match curve_type {
                CurveType::Linear => {
                    path_builder.move_to(absolute_points[0]);
                    for point in &absolute_points[1..] {
                        path_builder.line_to(*point);
                    }
                }
                CurveType::Step | CurveType::StepAfter => {
                    path_builder.move_to(absolute_points[0]);
                    for i in 1..absolute_points.len() {
                        let prev = absolute_points[i - 1];
                        let curr = absolute_points[i];
                        // Horizontal then vertical
                        path_builder.line_to(gpui::point(curr.x, prev.y));
                        path_builder.line_to(curr);
                    }
                }
                CurveType::StepBefore => {
                    path_builder.move_to(absolute_points[0]);
                    for i in 1..absolute_points.len() {
                        let prev = absolute_points[i - 1];
                        let curr = absolute_points[i];
                        // Vertical then horizontal
                        path_builder.line_to(gpui::point(prev.x, curr.y));
                        path_builder.line_to(curr);
                    }
                }
            }

            // Build and paint the path
            if let Ok(path) = path_builder.build() {
                let color_with_opacity = Rgba {
                    r: stroke_color.r,
                    g: stroke_color.g,
                    b: stroke_color.b,
                    a: stroke_color.a * opacity,
                };
                window.paint_path(path, color_with_opacity);
            }

            // Paint points if enabled
            if show_points {
                for point in &absolute_points {
                    let point_bounds = Bounds {
                        origin: gpui::point(
                            point.x - px(point_radius),
                            point.y - px(point_radius),
                        ),
                        size: gpui::size(px(point_radius * 2.0), px(point_radius * 2.0)),
                    };
                    let color_with_opacity = Rgba {
                        r: point_fill.r,
                        g: point_fill.g,
                        b: point_fill.b,
                        a: point_fill.a * opacity,
                    };
                    window.paint_quad(PaintQuad {
                        bounds: point_bounds,
                        corner_radii: Corners::all(px(point_radius)),
                        background: color_with_opacity.into(),
                        border_widths: Edges::default(),
                        border_color: transparent_black(),
                        border_style: BorderStyle::default(),
                    });
                }
            }
        },
    )
    .size_full()
    .absolute()
    .inset_0()
}
