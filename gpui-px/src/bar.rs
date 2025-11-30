//! Bar chart - Plotly Express style API.

use crate::error::ChartError;
use crate::{
    DEFAULT_COLOR, DEFAULT_HEIGHT, DEFAULT_PADDING_FRACTION, DEFAULT_TITLE_FONT_SIZE,
    DEFAULT_WIDTH, TITLE_AREA_HEIGHT, ScaleType, extent_padded, validate_data_array,
    validate_data_length, validate_dimensions, validate_positive,
};
use d3rs::color::D3Color;
use d3rs::scale::{LinearScale, LogScale};
use d3rs::shape::{BarConfig, BarDatum, render_bars};
use d3rs::text::{VectorFontConfig, render_vector_text};
use gpui::prelude::*;
use gpui::*;

/// Bar chart builder.
#[derive(Debug, Clone)]
pub struct BarChart {
    categories: Vec<String>,
    values: Vec<f64>,
    title: Option<String>,
    color: u32,
    opacity: f32,
    bar_gap: f32,
    border_radius: f32,
    width: f32,
    height: f32,
    y_scale_type: ScaleType,
}

impl BarChart {
    /// Set chart title (rendered at top of chart).
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set bar color as 24-bit RGB hex value (format: 0xRRGGBB).
    ///
    /// # Example
    /// ```rust,no_run
    /// use gpui_px::bar;
    /// let chart = bar(&["A"], &[1.0])
    ///     .color(0x2ca02c)  // Plotly green
    ///     .build();
    /// ```
    pub fn color(mut self, hex: u32) -> Self {
        self.color = hex;
        self
    }

    /// Set bar opacity (0.0 - 1.0).
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Set gap between bars in pixels.
    pub fn bar_gap(mut self, gap: f32) -> Self {
        self.bar_gap = gap;
        self
    }

    /// Set bar corner radius.
    pub fn border_radius(mut self, radius: f32) -> Self {
        self.border_radius = radius;
        self
    }

    /// Set chart dimensions.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Set Y-axis scale type (linear or log).
    ///
    /// # Example
    /// ```rust,no_run
    /// use gpui_px::{bar, ScaleType};
    /// let chart = bar(&["A", "B", "C"], &[10.0, 100.0, 1000.0])
    ///     .y_scale(ScaleType::Log)
    ///     .build();
    /// ```
    pub fn y_scale(mut self, scale: ScaleType) -> Self {
        self.y_scale_type = scale;
        self
    }

    /// Build and validate the chart, returning renderable element.
    pub fn build(self) -> Result<impl IntoElement, ChartError> {
        // Validate inputs
        if self.categories.is_empty() {
            return Err(ChartError::EmptyData {
                field: "categories",
            });
        }
        validate_data_array(&self.values, "values")?;
        validate_data_length(
            self.categories.len(),
            self.values.len(),
            "categories",
            "values",
        )?;
        validate_dimensions(self.width, self.height)?;

        // Validate positive values for log scale
        if self.y_scale_type == ScaleType::Log {
            validate_positive(&self.values, "values")?;
        }

        // Calculate plot area (reserve space for title if present)
        let title_height = if self.title.is_some() {
            TITLE_AREA_HEIGHT
        } else {
            0.0
        };
        let plot_height = self.height - title_height;

        // Calculate y domain with padding
        let (mut y_min, mut y_max) = extent_padded(&self.values, DEFAULT_PADDING_FRACTION);

        // For linear scale, always include zero baseline for bar charts
        // For log scale, we can't include zero
        if self.y_scale_type == ScaleType::Linear {
            y_min = y_min.min(0.0);
            y_max = y_max.max(0.0);
        }

        // Create X scale (always linear for categories)
        let x_scale = LinearScale::new()
            .domain(0.0, self.categories.len() as f64)
            .range(0.0, self.width as f64);

        // Create data
        let data: Vec<BarDatum> = self
            .categories
            .iter()
            .zip(self.values.iter())
            .map(|(cat, &val)| BarDatum::new(cat.clone(), val))
            .collect();

        // Create config
        let config = BarConfig::new()
            .fill_color(D3Color::from_hex(self.color))
            .opacity(self.opacity)
            .bar_gap(self.bar_gap)
            .border_radius(self.border_radius);

        // Build the element based on Y scale type
        let bar_element: AnyElement = match self.y_scale_type {
            ScaleType::Linear => {
                let y_scale = LinearScale::new()
                    .domain(y_min, y_max)
                    .range(plot_height as f64, 0.0);
                render_bars(&x_scale, &y_scale, &data, self.width, plot_height, &config)
                    .into_any_element()
            }
            ScaleType::Log => {
                let y_scale = LogScale::new()
                    .domain(y_min.max(1e-10), y_max)
                    .range(plot_height as f64, 0.0);
                render_bars(&x_scale, &y_scale, &data, self.width, plot_height, &config)
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

        // Add plot area
        container = container.child(
            div()
                .w(px(self.width))
                .h(px(plot_height))
                .relative()
                .child(bar_element),
        );

        Ok(container)
    }
}

/// Create a bar chart from categories and values.
///
/// # Example
///
/// ```rust,no_run
/// use gpui_px::bar;
///
/// let categories = vec!["A", "B", "C", "D"];
/// let values = vec![10.0, 25.0, 15.0, 30.0];
///
/// let chart = bar(&categories, &values)
///     .title("My Bar Chart")
///     .color(0x2ca02c)
///     .build()?;
/// # Ok::<(), gpui_px::ChartError>(())
/// ```
pub fn bar<S: AsRef<str>>(categories: &[S], values: &[f64]) -> BarChart {
    BarChart {
        categories: categories.iter().map(|s| s.as_ref().to_string()).collect(),
        values: values.to_vec(),
        title: None,
        color: DEFAULT_COLOR,
        opacity: 0.8,
        bar_gap: 2.0,
        border_radius: 2.0,
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
        y_scale_type: ScaleType::Linear,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bar_empty_categories() {
        let empty_categories: Vec<&str> = vec![];
        let result = bar(&empty_categories, &[1.0, 2.0, 3.0]).build();
        assert!(matches!(
            result,
            Err(ChartError::EmptyData {
                field: "categories"
            })
        ));
    }

    #[test]
    fn test_bar_empty_values() {
        let result = bar(&["A", "B", "C"], &[]).build();
        assert!(matches!(
            result,
            Err(ChartError::EmptyData { field: "values" })
        ));
    }

    #[test]
    fn test_bar_data_length_mismatch() {
        let result = bar(&["A", "B"], &[1.0, 2.0, 3.0]).build();
        assert!(matches!(
            result,
            Err(ChartError::DataLengthMismatch {
                x_field: "categories",
                y_field: "values",
                x_len: 2,
                y_len: 3,
            })
        ));
    }

    #[test]
    fn test_bar_invalid_value_nan() {
        let result = bar(&["A", "B", "C"], &[1.0, f64::NAN, 3.0]).build();
        assert!(matches!(
            result,
            Err(ChartError::InvalidData {
                field: "values",
                reason: "contains NaN or Infinity"
            })
        ));
    }

    #[test]
    fn test_bar_successful_build() {
        let categories = vec!["A", "B", "C", "D"];
        let values = vec![10.0, 25.0, 15.0, 30.0];
        let result = bar(&categories, &values)
            .title("Test Bar Chart")
            .color(0x2ca02c)
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_bar_negative_values() {
        let categories = vec!["A", "B", "C"];
        let values = vec![-5.0, 10.0, -3.0];
        let result = bar(&categories, &values).build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_bar_builder_chain() {
        let result = bar(&["X", "Y", "Z"], &[1.0, 2.0, 3.0])
            .title("My Bar Chart")
            .color(0xff0000)
            .opacity(0.9)
            .bar_gap(5.0)
            .border_radius(4.0)
            .size(800.0, 600.0)
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_bar_log_y_scale() {
        let categories = vec!["A", "B", "C", "D"];
        let values = vec![10.0, 100.0, 1000.0, 10000.0];
        let result = bar(&categories, &values)
            .y_scale(ScaleType::Log)
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_bar_log_y_scale_zero_value() {
        let categories = vec!["A", "B", "C"];
        let values = vec![0.0, 10.0, 100.0];
        let result = bar(&categories, &values)
            .y_scale(ScaleType::Log)
            .build();
        assert!(matches!(
            result,
            Err(ChartError::InvalidData {
                field: "values",
                reason: "contains non-positive values for log scale"
            })
        ));
    }

    #[test]
    fn test_bar_log_y_scale_negative_value() {
        let categories = vec!["A", "B", "C"];
        let values = vec![-5.0, 10.0, 100.0];
        let result = bar(&categories, &values)
            .y_scale(ScaleType::Log)
            .build();
        assert!(matches!(
            result,
            Err(ChartError::InvalidData {
                field: "values",
                reason: "contains non-positive values for log scale"
            })
        ));
    }

    #[test]
    fn test_bar_log_scale_with_title() {
        let categories = vec!["Low", "Medium", "High"];
        let values = vec![10.0, 100.0, 1000.0];
        let result = bar(&categories, &values)
            .title("Log Scale Bar Chart")
            .y_scale(ScaleType::Log)
            .color(0x2ca02c)
            .build();
        assert!(result.is_ok());
    }
}
