//! Line chart - Plotly Express style API.

use crate::error::ChartError;
use crate::{
    DEFAULT_COLOR, DEFAULT_HEIGHT, DEFAULT_PADDING_FRACTION, DEFAULT_TITLE_FONT_SIZE,
    DEFAULT_WIDTH, TITLE_AREA_HEIGHT, extent_padded, validate_data_array, validate_data_length,
    validate_dimensions,
};
use d3rs::color::D3Color;
use d3rs::scale::LinearScale;
use d3rs::shape::{CurveType, LineConfig, LinePoint, render_line};
use d3rs::text::{VectorFontConfig, render_vector_text};
use gpui::prelude::*;
use gpui::*;

/// Line chart builder.
#[derive(Debug, Clone)]
pub struct LineChart {
    x: Vec<f64>,
    y: Vec<f64>,
    title: Option<String>,
    color: u32,
    stroke_width: f32,
    opacity: f32,
    curve: CurveType,
    show_points: bool,
    width: f32,
    height: f32,
}

impl LineChart {
    /// Set chart title (rendered at top of chart).
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set line color as 24-bit RGB hex value (format: 0xRRGGBB).
    ///
    /// # Example
    /// ```rust,no_run
    /// use gpui_px::line;
    /// let chart = line(&[1.0], &[1.0])
    ///     .color(0xff7f0e)  // Plotly orange
    ///     .build();
    /// ```
    pub fn color(mut self, hex: u32) -> Self {
        self.color = hex;
        self
    }

    /// Set line stroke width in pixels.
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width;
        self
    }

    /// Set line opacity (0.0 - 1.0).
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Set curve interpolation type.
    pub fn curve(mut self, curve: CurveType) -> Self {
        self.curve = curve;
        self
    }

    /// Show data points on the line.
    pub fn show_points(mut self, show: bool) -> Self {
        self.show_points = show;
        self
    }

    /// Set chart dimensions.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Build and validate the chart, returning renderable element.
    pub fn build(self) -> Result<impl IntoElement, ChartError> {
        // Validate inputs
        validate_data_array(&self.x, "x")?;
        validate_data_array(&self.y, "y")?;
        validate_data_length(self.x.len(), self.y.len(), "x", "y")?;
        validate_dimensions(self.width, self.height)?;

        // Calculate plot area (reserve space for title if present)
        let title_height = if self.title.is_some() {
            TITLE_AREA_HEIGHT
        } else {
            0.0
        };
        let plot_height = self.height - title_height;

        // Calculate domains with padding
        let (x_min, x_max) = extent_padded(&self.x, DEFAULT_PADDING_FRACTION);
        let (y_min, y_max) = extent_padded(&self.y, DEFAULT_PADDING_FRACTION);

        // Create scales
        let x_scale = LinearScale::new()
            .domain(x_min, x_max)
            .range(0.0, self.width as f64);
        let y_scale = LinearScale::new()
            .domain(y_min, y_max)
            .range(plot_height as f64, 0.0);

        // Create data points
        let data: Vec<LinePoint> = self
            .x
            .iter()
            .zip(self.y.iter())
            .map(|(&x, &y)| LinePoint::new(x, y))
            .collect();

        // Create config
        let config = LineConfig::new()
            .stroke_color(D3Color::from_hex(self.color))
            .stroke_width(self.stroke_width)
            .opacity(self.opacity)
            .curve(self.curve)
            .show_points(self.show_points);

        // Build the element
        let line_element = render_line(&x_scale, &y_scale, &data, &config);

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

        // Add plot area
        container = container.child(
            div()
                .w(px(self.width))
                .h(px(plot_height))
                .relative()
                .child(line_element),
        );

        Ok(container)
    }
}

/// Create a line chart from x and y data.
///
/// # Example
///
/// ```rust,no_run
/// use gpui_px::{line, CurveType};
///
/// let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
/// let y = vec![2.0, 4.0, 3.0, 5.0, 4.5];
///
/// let chart = line(&x, &y)
///     .title("My Line Chart")
///     .color(0xff7f0e)
///     .curve(CurveType::Linear)
///     .show_points(true)
///     .build()?;
/// # Ok::<(), gpui_px::ChartError>(())
/// ```
pub fn line(x: &[f64], y: &[f64]) -> LineChart {
    LineChart {
        x: x.to_vec(),
        y: y.to_vec(),
        title: None,
        color: DEFAULT_COLOR,
        stroke_width: 2.0,
        opacity: 1.0,
        curve: CurveType::Linear,
        show_points: false,
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_empty_x_data() {
        let result = line(&[], &[1.0, 2.0, 3.0]).build();
        assert!(matches!(result, Err(ChartError::EmptyData { field: "x" })));
    }

    #[test]
    fn test_line_empty_y_data() {
        let result = line(&[1.0, 2.0, 3.0], &[]).build();
        assert!(matches!(result, Err(ChartError::EmptyData { field: "y" })));
    }

    #[test]
    fn test_line_data_length_mismatch() {
        let result = line(&[1.0, 2.0, 3.0, 4.0], &[1.0, 2.0]).build();
        assert!(matches!(
            result,
            Err(ChartError::DataLengthMismatch {
                x_field: "x",
                y_field: "y",
                x_len: 4,
                y_len: 2,
            })
        ));
    }

    #[test]
    fn test_line_infinity_in_x() {
        let result = line(&[1.0, 2.0, f64::NEG_INFINITY], &[1.0, 2.0, 3.0]).build();
        assert!(matches!(
            result,
            Err(ChartError::InvalidData {
                field: "x",
                reason: "contains NaN or Infinity"
            })
        ));
    }

    #[test]
    fn test_line_nan_in_y() {
        let result = line(&[1.0, 2.0, 3.0], &[1.0, f64::NAN, 3.0]).build();
        assert!(matches!(
            result,
            Err(ChartError::InvalidData {
                field: "y",
                reason: "contains NaN or Infinity"
            })
        ));
    }

    #[test]
    fn test_line_successful_build() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 3.0, 5.0, 4.5];
        let result = line(&x, &y).title("Test Line").color(0xff7f0e).build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_line_builder_chain() {
        let result = line(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0])
            .title("My Line")
            .color(0x00ff00)
            .stroke_width(3.0)
            .opacity(0.8)
            .curve(CurveType::Linear)
            .show_points(true)
            .size(800.0, 600.0)
            .build();
        assert!(result.is_ok());
    }
}
