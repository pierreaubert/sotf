//! Scatter chart - Plotly Express style API.

use crate::error::ChartError;
use crate::{
    DEFAULT_COLOR, DEFAULT_HEIGHT, DEFAULT_PADDING_FRACTION, DEFAULT_TITLE_FONT_SIZE,
    DEFAULT_WIDTH, ScaleType, TITLE_AREA_HEIGHT, extent_padded, validate_data_array,
    validate_data_length, validate_dimensions, validate_positive,
};
use d3rs::axis::{AxisConfig, DefaultAxisTheme, render_axis};
use d3rs::color::D3Color;
use d3rs::grid::{GridConfig, render_grid};
use d3rs::scale::{LinearScale, LogScale};
use d3rs::shape::{ScatterConfig, ScatterPoint, render_scatter};
use d3rs::text::{VectorFontConfig, render_vector_text};
use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, div, hsla, px, rgb};

/// Scatter chart builder.
#[derive(Debug, Clone)]
pub struct ScatterChart {
    x: Vec<f64>,
    y: Vec<f64>,
    title: Option<String>,
    color: u32,
    point_radius: f32,
    opacity: f32,
    width: f32,
    height: f32,
    x_scale_type: ScaleType,
    y_scale_type: ScaleType,
}

impl ScatterChart {
    /// Set chart title (rendered at top of chart).
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set point color as 24-bit RGB hex value (format: 0xRRGGBB).
    ///
    /// # Example
    /// ```rust,no_run
    /// use gpui_px::scatter;
    /// let chart = scatter(&[1.0], &[1.0])
    ///     .color(0x1f77b4)  // Plotly blue
    ///     .build();
    /// ```
    pub fn color(mut self, hex: u32) -> Self {
        self.color = hex;
        self
    }

    /// Set point radius in pixels.
    pub fn point_radius(mut self, radius: f32) -> Self {
        self.point_radius = radius;
        self
    }

    /// Set point opacity (0.0 - 1.0).
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Set chart dimensions.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Set X-axis scale type (linear or log).
    ///
    /// # Example
    /// ```rust,no_run
    /// use gpui_px::{scatter, ScaleType};
    /// let chart = scatter(&[10.0, 100.0, 1000.0], &[1.0, 2.0, 3.0])
    ///     .x_scale(ScaleType::Log)
    ///     .build();
    /// ```
    pub fn x_scale(mut self, scale: ScaleType) -> Self {
        self.x_scale_type = scale;
        self
    }

    /// Set Y-axis scale type (linear or log).
    pub fn y_scale(mut self, scale: ScaleType) -> Self {
        self.y_scale_type = scale;
        self
    }

    /// Build and validate the chart, returning renderable element.
    pub fn build(self) -> Result<impl IntoElement, ChartError> {
        // Validate inputs
        validate_data_array(&self.x, "x")?;
        validate_data_array(&self.y, "y")?;
        validate_data_length(self.x.len(), self.y.len(), "x", "y")?;
        validate_dimensions(self.width, self.height)?;

        // Validate positive values for log scales
        if self.x_scale_type == ScaleType::Log {
            validate_positive(&self.x, "x")?;
        }
        if self.y_scale_type == ScaleType::Log {
            validate_positive(&self.y, "y")?;
        }

        // Define margins (TODO: Make configurable?)
        let margin_left = 50.0;
        let margin_bottom = 30.0;
        let margin_top = 10.0;
        let margin_right = 20.0;

        // Calculate plot area (reserve space for title if present)
        let title_height = if self.title.is_some() {
            TITLE_AREA_HEIGHT
        } else {
            0.0
        };

        let plot_width = (self.width as f64 - margin_left - margin_right).max(0.0);
        let plot_height =
            (self.height as f64 - title_height as f64 - margin_top - margin_bottom).max(0.0);

        // Calculate domains with padding
        let (x_min, x_max) = extent_padded(&self.x, DEFAULT_PADDING_FRACTION);
        let (y_min, y_max) = extent_padded(&self.y, DEFAULT_PADDING_FRACTION);

        // Create data points
        let data: Vec<ScatterPoint> = self
            .x
            .iter()
            .zip(self.y.iter())
            .map(|(&x, &y)| ScatterPoint::new(x, y))
            .collect();

        // Create config
        let config = ScatterConfig::new()
            .fill_color(D3Color::from_hex(self.color))
            .point_radius(self.point_radius)
            .opacity(self.opacity);

        let theme = DefaultAxisTheme;

        // Build the element based on scale types
        let chart_content: AnyElement = match (self.x_scale_type, self.y_scale_type) {
            (ScaleType::Linear, ScaleType::Linear) => {
                let x_scale = LinearScale::new()
                    .domain(x_min, x_max)
                    .range(0.0, plot_width);
                let y_scale = LinearScale::new()
                    .domain(y_min, y_max)
                    .range(plot_height, 0.0);

                div()
                    .flex()
                    .child(render_axis(
                        &y_scale,
                        &AxisConfig::left(),
                        plot_height as f32,
                        &theme,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .w(px(plot_width as f32))
                                    .h(px(plot_height as f32))
                                    .relative()
                                    .bg(rgb(0xf8f8f8)) // Light gray background
                                    .child(render_grid(
                                        &x_scale,
                                        &y_scale,
                                        &GridConfig::default(),
                                        plot_width as f32,
                                        plot_height as f32,
                                        &theme,
                                    ))
                                    .child(render_scatter(&x_scale, &y_scale, &data, &config)),
                            )
                            .child(render_axis(
                                &x_scale,
                                &AxisConfig::bottom(),
                                plot_width as f32,
                                &theme,
                            )),
                    )
                    .into_any_element()
            }
            (ScaleType::Log, ScaleType::Linear) => {
                let x_scale = LogScale::new()
                    .domain(x_min.max(1e-10), x_max)
                    .range(0.0, plot_width);
                let y_scale = LinearScale::new()
                    .domain(y_min, y_max)
                    .range(plot_height, 0.0);

                div()
                    .flex()
                    .child(render_axis(
                        &y_scale,
                        &AxisConfig::left(),
                        plot_height as f32,
                        &theme,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .w(px(plot_width as f32))
                                    .h(px(plot_height as f32))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .child(render_grid(
                                        &x_scale,
                                        &y_scale,
                                        &GridConfig::default(),
                                        plot_width as f32,
                                        plot_height as f32,
                                        &theme,
                                    ))
                                    .child(render_scatter(&x_scale, &y_scale, &data, &config)),
                            )
                            .child(render_axis(
                                &x_scale,
                                &AxisConfig::bottom(),
                                plot_width as f32,
                                &theme,
                            )),
                    )
                    .into_any_element()
            }
            (ScaleType::Linear, ScaleType::Log) => {
                let x_scale = LinearScale::new()
                    .domain(x_min, x_max)
                    .range(0.0, plot_width);
                let y_scale = LogScale::new()
                    .domain(y_min.max(1e-10), y_max)
                    .range(plot_height, 0.0);

                div()
                    .flex()
                    .child(render_axis(
                        &y_scale,
                        &AxisConfig::left(),
                        plot_height as f32,
                        &theme,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .w(px(plot_width as f32))
                                    .h(px(plot_height as f32))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .child(render_grid(
                                        &x_scale,
                                        &y_scale,
                                        &GridConfig::default(),
                                        plot_width as f32,
                                        plot_height as f32,
                                        &theme,
                                    ))
                                    .child(render_scatter(&x_scale, &y_scale, &data, &config)),
                            )
                            .child(render_axis(
                                &x_scale,
                                &AxisConfig::bottom(),
                                plot_width as f32,
                                &theme,
                            )),
                    )
                    .into_any_element()
            }
            (ScaleType::Log, ScaleType::Log) => {
                let x_scale = LogScale::new()
                    .domain(x_min.max(1e-10), x_max)
                    .range(0.0, plot_width);
                let y_scale = LogScale::new()
                    .domain(y_min.max(1e-10), y_max)
                    .range(plot_height, 0.0);

                div()
                    .flex()
                    .child(render_axis(
                        &y_scale,
                        &AxisConfig::left(),
                        plot_height as f32,
                        &theme,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .w(px(plot_width as f32))
                                    .h(px(plot_height as f32))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .child(render_grid(
                                        &x_scale,
                                        &y_scale,
                                        &GridConfig::default(),
                                        plot_width as f32,
                                        plot_height as f32,
                                        &theme,
                                    ))
                                    .child(render_scatter(&x_scale, &y_scale, &data, &config)),
                            )
                            .child(render_axis(
                                &x_scale,
                                &AxisConfig::bottom(),
                                plot_width as f32,
                                &theme,
                            )),
                    )
                    .into_any_element()
            }
        };

        // Build container with optional title
        let mut container = div()
            .w(px(self.width))
            .h(px(self.height))
            .relative()
            .flex()
            .flex_col();

        // Add title if present
        if let Some(title) = &self.title {
            let font_config =
                VectorFontConfig::horizontal(DEFAULT_TITLE_FONT_SIZE, hsla(0.0, 0.0, 0.2, 1.0));
            container = container.child(
                div()
                    .w_full()
                    .h(px(title_height))
                    .flex()
                    .justify_center()
                    .items_center()
                    .child(render_vector_text(title, &font_config)),
            );
        }

        // Add chart content
        container = container.child(div().relative().child(chart_content));

        Ok(container)
    }
}

/// Create a scatter chart from x and y data.
///
/// # Example
///
/// ```rust,no_run
/// use gpui_px::scatter;
///
/// let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
/// let y = vec![2.0, 4.0, 3.0, 5.0, 4.5];
///
/// let chart = scatter(&x, &y)
///     .title("My Scatter Plot")
///     .color(0x1f77b4)
///     .build()?;
/// # Ok::<(), gpui_px::ChartError>(())
/// ```
pub fn scatter(x: &[f64], y: &[f64]) -> ScatterChart {
    ScatterChart {
        x: x.to_vec(),
        y: y.to_vec(),
        title: None,
        color: DEFAULT_COLOR,
        point_radius: 5.0,
        opacity: 0.7,
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
        x_scale_type: ScaleType::Linear,
        y_scale_type: ScaleType::Linear,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scatter_empty_x_data() {
        let result = scatter(&[], &[1.0, 2.0, 3.0]).build();
        assert!(matches!(result, Err(ChartError::EmptyData { field: "x" })));
    }

    #[test]
    fn test_scatter_empty_y_data() {
        let result = scatter(&[1.0, 2.0, 3.0], &[]).build();
        assert!(matches!(result, Err(ChartError::EmptyData { field: "y" })));
    }

    #[test]
    fn test_scatter_data_length_mismatch() {
        let result = scatter(&[1.0, 2.0], &[1.0, 2.0, 3.0]).build();
        assert!(matches!(
            result,
            Err(ChartError::DataLengthMismatch {
                x_field: "x",
                y_field: "y",
                x_len: 2,
                y_len: 3,
            })
        ));
    }

    #[test]
    fn test_scatter_nan_in_x() {
        let result = scatter(&[1.0, f64::NAN, 3.0], &[1.0, 2.0, 3.0]).build();
        assert!(matches!(
            result,
            Err(ChartError::InvalidData {
                field: "x",
                reason: "contains NaN or Infinity"
            })
        ));
    }

    #[test]
    fn test_scatter_infinity_in_y() {
        let result = scatter(&[1.0, 2.0, 3.0], &[1.0, f64::INFINITY, 3.0]).build();
        assert!(matches!(
            result,
            Err(ChartError::InvalidData {
                field: "y",
                reason: "contains NaN or Infinity"
            })
        ));
    }

    #[test]
    fn test_scatter_zero_width() {
        let result = scatter(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0])
            .size(0.0, 400.0)
            .build();
        assert!(matches!(
            result,
            Err(ChartError::InvalidDimension {
                field: "width",
                value: 0.0
            })
        ));
    }

    #[test]
    fn test_scatter_negative_height() {
        let result = scatter(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0])
            .size(600.0, -100.0)
            .build();
        assert!(matches!(
            result,
            Err(ChartError::InvalidDimension {
                field: "height",
                value: -100.0
            })
        ));
    }

    #[test]
    fn test_scatter_successful_build() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 3.0, 5.0, 4.5];
        let result = scatter(&x, &y).title("Test Chart").color(0x1f77b4).build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_scatter_builder_chain() {
        let result = scatter(&[1.0, 2.0], &[3.0, 4.0])
            .title("My Plot")
            .color(0xff0000)
            .point_radius(10.0)
            .opacity(0.5)
            .size(800.0, 600.0)
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_scatter_log_x_scale() {
        let x = vec![10.0, 100.0, 1000.0, 10000.0];
        let y = vec![1.0, 2.0, 3.0, 4.0];
        let result = scatter(&x, &y).x_scale(ScaleType::Log).build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_scatter_log_y_scale() {
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let y = vec![10.0, 100.0, 1000.0, 10000.0];
        let result = scatter(&x, &y).y_scale(ScaleType::Log).build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_scatter_log_xy_scale() {
        let x = vec![10.0, 100.0, 1000.0];
        let y = vec![20.0, 200.0, 2000.0];
        let result = scatter(&x, &y)
            .x_scale(ScaleType::Log)
            .y_scale(ScaleType::Log)
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_scatter_log_x_negative_values() {
        let x = vec![-10.0, -5.0, 5.0, 10.0];
        let y = vec![1.0, 2.0, 3.0, 4.0];
        let result = scatter(&x, &y).x_scale(ScaleType::Log).build();
        assert!(matches!(
            result,
            Err(ChartError::InvalidData {
                field: "x",
                reason: "contains non-positive values for log scale"
            })
        ));
    }

    #[test]
    fn test_scatter_log_y_zero_value() {
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let y = vec![0.0, 1.0, 2.0, 3.0];
        let result = scatter(&x, &y).y_scale(ScaleType::Log).build();
        assert!(matches!(
            result,
            Err(ChartError::InvalidData {
                field: "y",
                reason: "contains non-positive values for log scale"
            })
        ));
    }

    #[test]
    fn test_scatter_log_scale_with_title() {
        let x = vec![10.0, 100.0, 1000.0];
        let y = vec![1.0, 2.0, 3.0];
        let result = scatter(&x, &y)
            .title("Log Scale Plot")
            .x_scale(ScaleType::Log)
            .color(0x1f77b4)
            .build();
        assert!(result.is_ok());
    }
}
