//! Bar chart - Plotly Express style API.

use crate::error::ChartError;
use crate::{
    validate_data_array, validate_data_length, validate_dimensions, DEFAULT_COLOR, DEFAULT_HEIGHT,
    DEFAULT_TITLE_FONT_SIZE, DEFAULT_WIDTH, TITLE_AREA_HEIGHT,
};
use d3rs::color::D3Color;
use d3rs::scale::LinearScale;
use d3rs::shape::{render_bars, BarConfig, BarDatum};
use d3rs::text::{render_vector_text, VectorFontConfig};
use gpui::prelude::*;
use gpui::*;

/// Bar chart builder.
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
}

impl BarChart {
    /// Set chart title (rendered at top of chart).
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set bar color (hex value).
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

    /// Build and validate the chart, returning renderable element.
    pub fn build(self) -> Result<impl IntoElement, ChartError> {
        // Validate inputs
        if self.categories.is_empty() {
            return Err(ChartError::EmptyData { field: "categories" });
        }
        validate_data_array(&self.values, "values")?;
        validate_data_length(
            self.categories.len(),
            self.values.len(),
            "categories",
            "values",
        )?;
        validate_dimensions(self.width, self.height)?;

        // Calculate plot area (reserve space for title if present)
        let title_height = if self.title.is_some() {
            TITLE_AREA_HEIGHT
        } else {
            0.0
        };
        let plot_height = self.height - title_height;

        // Calculate y domain with padding (always include 0 for bar charts)
        let y_min = self.values.iter().copied().fold(f64::INFINITY, f64::min);
        let y_max = self.values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let y_domain_min = if y_min > 0.0 { 0.0 } else { y_min * 1.05 };
        let y_domain_max = if y_max < 0.0 { 0.0 } else { y_max * 1.05 };

        // Create scales
        let x_scale = LinearScale::new()
            .domain(0.0, self.categories.len() as f64)
            .range(0.0, self.width as f64);
        let y_scale = LinearScale::new()
            .domain(y_domain_min, y_domain_max)
            .range(plot_height as f64, 0.0);

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

        // Build the element
        let bar_element = render_bars(&x_scale, &y_scale, &data, self.width, plot_height, &config);

        // Build container with optional title
        let mut container = div()
            .w(px(self.width))
            .h(px(self.height))
            .relative()
            .flex()
            .flex_col();

        // Add title if present
        if let Some(title) = &self.title {
            let font_config = VectorFontConfig::horizontal(
                DEFAULT_TITLE_FONT_SIZE,
                hsla(0.0, 0.0, 0.2, 1.0),
            );
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
    }
}
