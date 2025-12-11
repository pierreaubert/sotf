//! Line chart - Plotly Express style API.

use crate::error::ChartError;
use crate::{
    DEFAULT_COLOR, DEFAULT_HEIGHT, DEFAULT_PADDING_FRACTION, DEFAULT_TITLE_FONT_SIZE,
    DEFAULT_WIDTH, ScaleType, TITLE_AREA_HEIGHT, extent_padded, validate_data_array,
    validate_data_length, validate_dimensions, validate_positive,
};
use d3rs::axis::{AxisConfig, AxisTheme, render_axis};
use d3rs::color::D3Color;
use d3rs::grid::{GridConfig, render_grid};
use d3rs::scale::{LinearScale, LogScale};
use d3rs::shape::{CurveType, LineConfig, LinePoint, render_line};
use d3rs::text::{VectorFontConfig, render_vector_text};
use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, Rgba, div, px, rgb};

/// Theme for chart styling
#[derive(Debug, Clone)]
pub struct ChartTheme {
    /// Background color for plot area
    pub plot_background: Rgba,
    /// Grid line color
    pub grid_color: Rgba,
    /// Axis line color
    pub axis_line_color: Rgba,
    /// Axis label color
    pub axis_label_color: Rgba,
    /// Title text color
    pub title_color: Rgba,
    /// Legend text color
    pub legend_text_color: Rgba,
}

impl Default for ChartTheme {
    fn default() -> Self {
        Self {
            plot_background: rgb(0xf8f8f8),
            grid_color: rgba(0x000000, 0.1),
            axis_line_color: rgba(0x000000, 0.2),
            axis_label_color: rgba(0x000000, 0.6),
            title_color: rgba(0x000000, 0.8),
            legend_text_color: rgba(0x000000, 0.6),
        }
    }
}

/// Helper to create Rgba with alpha
fn rgba(hex: u32, alpha: f32) -> Rgba {
    Rgba {
        r: ((hex >> 16) & 0xFF) as f32 / 255.0,
        g: ((hex >> 8) & 0xFF) as f32 / 255.0,
        b: (hex & 0xFF) as f32 / 255.0,
        a: alpha,
    }
}

/// Axis theme adapter for d3rs
struct ChartAxisTheme {
    axis_line_color: Rgba,
    axis_label_color: Rgba,
}

impl AxisTheme for ChartAxisTheme {
    fn axis_line_color(&self) -> Rgba {
        self.axis_line_color
    }

    fn axis_label_color(&self) -> Rgba {
        self.axis_label_color
    }
}

/// Format tick labels for log scales with k/M suffixes
fn format_log_tick(value: f64) -> String {
    let abs_value = value.abs();

    // Handle zero
    if abs_value < 1e-10 {
        return "0".to_string();
    }

    // Format based on magnitude
    let formatted = if abs_value >= 1_000_000.0 {
        // Millions: 1M, 2M, etc.
        let millions = value / 1_000_000.0;
        if millions.fract().abs() < 1e-10 {
            format!("{:.0}M", millions)
        } else {
            format!("{:.1}M", millions)
        }
    } else if abs_value >= 1_000.0 {
        // Thousands: 1k, 10k, 100k, etc.
        let thousands = value / 1_000.0;
        if thousands.fract().abs() < 1e-10 {
            format!("{:.0}k", thousands)
        } else {
            format!("{:.1}k", thousands)
        }
    } else if abs_value >= 1.0 {
        // Regular values >= 1
        if value.fract().abs() < 1e-10 {
            format!("{:.0}", value)
        } else {
            format!("{:.1}", value)
        }
    } else {
        // Small values < 1
        format!("{:.2}", value)
    };

    formatted
}

/// Generate smart tick values for log scales to prevent label collision
/// Shows 1,2,3,4,5,10,20,30,40,50,100,... pattern
fn generate_log_ticks(min: f64, max: f64) -> Vec<f64> {
    let mut ticks = Vec::new();

    // Find the starting decade (power of 10)
    let start_exp = min.log10().floor() as i32;
    let end_exp = max.log10().ceil() as i32;

    for exp in start_exp..=end_exp {
        let base = 10_f64.powi(exp);

        // For each decade, show: 1, 2, 3, 4, 5, 10 (which becomes 1 of next decade)
        // This gives us: 1k, 2k, 3k, 4k, 5k, 10k, 20k, 30k, 40k, 50k, 100k, etc.
        for multiplier in [1.0, 2.0, 3.0, 4.0, 5.0] {
            let tick = base * multiplier;
            if tick >= min && tick <= max {
                ticks.push(tick);
            }
        }
    }

    // Add the final decade marker if we don't already have it
    let final_decade = 10_f64.powi(end_exp);
    if final_decade <= max && !ticks.contains(&final_decade) {
        ticks.push(final_decade);
    }

    ticks.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ticks.dedup();
    ticks
}

/// A single series in a line chart
#[derive(Debug, Clone)]
struct LineSeries {
    y: Vec<f64>,
    label: Option<String>,
    color: u32,
    stroke_width: f32,
    opacity: f32,
}

/// Line chart builder.
#[derive(Debug, Clone)]
pub struct LineChart {
    x: Vec<f64>,
    // Primary series (backwards compatible)
    y: Vec<f64>,
    label: Option<String>,
    color: u32,
    stroke_width: f32,
    opacity: f32,
    // Additional series
    series: Vec<LineSeries>,
    // Common settings
    title: Option<String>,
    x_label: Option<String>,
    y_label: Option<String>,
    curve: CurveType,
    show_points: bool,
    width: f32,
    height: f32,
    x_scale_type: ScaleType,
    y_scale_type: ScaleType,
    show_legend: bool,
    theme: ChartTheme,
}

impl LineChart {
    /// Set chart title (rendered at top of chart).
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set X-axis label.
    pub fn x_label(mut self, label: impl Into<String>) -> Self {
        self.x_label = Some(label.into());
        self
    }

    /// Set Y-axis label.
    pub fn y_label(mut self, label: impl Into<String>) -> Self {
        self.y_label = Some(label.into());
        self
    }

    /// Set label for legend entry.
    ///
    /// When a label is set, the legend will automatically be shown.
    ///
    /// # Example
    /// ```rust,no_run
    /// use gpui_px::line;
    /// let chart = line(&[1.0, 2.0], &[1.0, 2.0])
    ///     .label("Series A")
    ///     .build();
    /// ```
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self.show_legend = true;
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

    /// Set X-axis scale type (linear or log).
    ///
    /// # Example
    /// ```rust,no_run
    /// use gpui_px::{line, ScaleType};
    /// let chart = line(&[10.0, 100.0, 1000.0], &[1.0, 2.0, 3.0])
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

    /// Add an additional data series to the chart.
    ///
    /// All series share the same X-axis data. This allows overlaying multiple
    /// lines on a single chart.
    ///
    /// # Example
    /// ```rust,no_run
    /// use gpui_px::line;
    /// let x = vec![1.0, 2.0, 3.0];
    /// let y1 = vec![1.0, 2.0, 3.0];
    /// let y2 = vec![3.0, 2.0, 1.0];
    /// let chart = line(&x, &y1)
    ///     .label("Series 1")
    ///     .color(0x3b82f6)
    ///     .add_series(&y2, Some("Series 2"), 0xff7f0e, 2.0, 1.0)
    ///     .build();
    /// ```
    pub fn add_series(
        mut self,
        y: &[f64],
        label: Option<impl Into<String>>,
        color: u32,
        stroke_width: f32,
        opacity: f32,
    ) -> Self {
        self.series.push(LineSeries {
            y: y.to_vec(),
            label: label.map(|l| l.into()),
            color,
            stroke_width,
            opacity,
        });
        // Auto-enable legend if any series has a label
        if self.series.iter().any(|s| s.label.is_some()) {
            self.show_legend = true;
        }
        self
    }

    /// Set the chart theme.
    ///
    /// # Example
    /// ```rust,no_run
    /// use gpui_px::{line, ChartTheme};
    /// let chart = line(&[1.0, 2.0], &[1.0, 2.0])
    ///     .theme(ChartTheme::default())
    ///     .build();
    /// ```
    pub fn theme(mut self, theme: ChartTheme) -> Self {
        self.theme = theme;
        self
    }

    /// Build and validate the chart, returning renderable element.
    pub fn build(self) -> Result<impl IntoElement, ChartError> {
        // Validate inputs
        validate_data_array(&self.x, "x")?;
        validate_data_array(&self.y, "y")?;
        validate_data_length(self.x.len(), self.y.len(), "x", "y")?;
        validate_dimensions(self.width, self.height)?;

        // Validate all additional series
        for series in &self.series {
            validate_data_array(&series.y, "series.y")?;
            validate_data_length(self.x.len(), series.y.len(), "x", "series.y")?;
            if self.y_scale_type == ScaleType::Log {
                validate_positive(&series.y, "series.y")?;
            }
        }

        // Validate positive values for log scales
        if self.x_scale_type == ScaleType::Log {
            validate_positive(&self.x, "x")?;
        }
        if self.y_scale_type == ScaleType::Log {
            validate_positive(&self.y, "y")?;
        }

        // Define margins
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

        // Calculate domains with padding - include all series in Y-axis range
        let (x_min, x_max) = if self.x_scale_type == ScaleType::Log {
            // For log scale, use multiplicative padding to avoid going negative
            let min = self.x.iter().copied().fold(f64::INFINITY, f64::min);
            let max = self.x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let padding_factor = 1.0 + DEFAULT_PADDING_FRACTION;
            (min / padding_factor, max * padding_factor)
        } else {
            extent_padded(&self.x, DEFAULT_PADDING_FRACTION)
        };

        // Collect all Y values from primary series and additional series
        let mut all_y_values: Vec<f64> = self.y.clone();
        for series in &self.series {
            all_y_values.extend_from_slice(&series.y);
        }
        let (y_min, y_max) = if self.y_scale_type == ScaleType::Log {
            // For log scale, use multiplicative padding
            let min = all_y_values.iter().copied().fold(f64::INFINITY, f64::min);
            let max = all_y_values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let padding_factor = 1.0 + DEFAULT_PADDING_FRACTION;
            (min / padding_factor, max * padding_factor)
        } else {
            extent_padded(&all_y_values, DEFAULT_PADDING_FRACTION)
        };

        // Create data points for primary series
        let primary_data: Vec<LinePoint> = self
            .x
            .iter()
            .zip(self.y.iter())
            .map(|(&x, &y)| LinePoint::new(x, y))
            .collect();

        // Create configs for primary series
        let primary_config = LineConfig::new()
            .stroke_color(D3Color::from_hex(self.color))
            .stroke_width(self.stroke_width)
            .opacity(self.opacity)
            .curve(self.curve)
            .show_points(self.show_points);

        // Prepare additional series data and configs
        let mut series_data_configs: Vec<(Vec<LinePoint>, LineConfig)> = Vec::new();
        for series in &self.series {
            let series_points: Vec<LinePoint> = self
                .x
                .iter()
                .zip(series.y.iter())
                .map(|(&x, &y)| LinePoint::new(x, y))
                .collect();

            let series_config = LineConfig::new()
                .stroke_color(D3Color::from_hex(series.color))
                .stroke_width(series.stroke_width)
                .opacity(series.opacity)
                .curve(self.curve)
                .show_points(self.show_points);

            series_data_configs.push((series_points, series_config));
        }

        let axis_theme = ChartAxisTheme {
            axis_line_color: self.theme.axis_line_color,
            axis_label_color: self.theme.axis_label_color,
        };

        let grid_config = GridConfig::with_lines()
            .with_line_width(0.5)
            .with_line_opacity(0.3);

        // Build the element based on scale types
        let chart_content: AnyElement = match (self.x_scale_type, self.y_scale_type) {
            (ScaleType::Linear, ScaleType::Linear) => {
                let x_scale = LinearScale::new()
                    .domain(x_min, x_max)
                    .range(0.0, plot_width);
                let y_scale = LinearScale::new()
                    .domain(y_min, y_max)
                    .range(plot_height, 0.0);

                // Build plot area with grid and all lines
                let mut plot_area = div()
                    .w(px(plot_width as f32))
                    .h(px(plot_height as f32))
                    .relative()
                    .bg(self.theme.plot_background)
                    .child(render_grid(
                        &x_scale,
                        &y_scale,
                        &grid_config,
                        plot_width as f32,
                        plot_height as f32,
                        &axis_theme,
                    ));

                // Render all additional series first (so primary is on top)
                for (series_data, series_config) in &series_data_configs {
                    plot_area = plot_area.child(render_line(&x_scale, &y_scale, series_data, series_config));
                }

                // Render primary series on top
                plot_area = plot_area.child(render_line(&x_scale, &y_scale, &primary_data, &primary_config));

                // Create axis configs with labels
                let mut y_axis_config = AxisConfig::left().with_label_font_size(8.0);
                if let Some(ref label) = self.y_label {
                    y_axis_config = y_axis_config.with_title(label.clone());
                }

                let mut x_axis_config = AxisConfig::bottom()
                    .with_ticks(20)
                    .with_label_font_size(8.0);
                if let Some(ref label) = self.x_label {
                    x_axis_config = x_axis_config.with_title(label.clone());
                }

                div()
                    .flex()
                    .child(render_axis(
                        &y_scale,
                        &y_axis_config,
                        plot_height as f32,
                        &axis_theme,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(plot_area)
                            .child(render_axis(
                                &x_scale,
                                &x_axis_config,
                                plot_width as f32,
                                &axis_theme,
                            )),
                    )
                    .into_any_element()
            }
            (ScaleType::Log, ScaleType::Linear) => {
                let x_scale = LogScale::new()
                    .domain(x_min, x_max)
                    .range(0.0, plot_width);
                let y_scale = LinearScale::new()
                    .domain(y_min, y_max)
                    .range(plot_height, 0.0);

                // Build plot area with grid and all lines
                let mut plot_area = div()
                    .w(px(plot_width as f32))
                    .h(px(plot_height as f32))
                    .relative()
                    .bg(self.theme.plot_background)
                    .child(render_grid(
                        &x_scale,
                        &y_scale,
                        &grid_config,
                        plot_width as f32,
                        plot_height as f32,
                        &axis_theme,
                    ));

                // Render all additional series first
                for (series_data, series_config) in &series_data_configs {
                    plot_area = plot_area.child(render_line(&x_scale, &y_scale, series_data, series_config));
                }

                // Render primary series on top
                plot_area = plot_area.child(render_line(&x_scale, &y_scale, &primary_data, &primary_config));

                // Create axis configs with labels and angled X labels for log scale
                let mut y_axis_config = AxisConfig::left().with_label_font_size(8.0);
                if let Some(ref label) = self.y_label {
                    y_axis_config = y_axis_config.with_title(label.clone());
                }

                // Generate smart tick values for log X axis to prevent collision
                let x_ticks = generate_log_ticks(x_min, x_max);
                let mut x_axis_config = AxisConfig::bottom()
                    .with_tick_values(x_ticks)
                    .with_label_angle(-45.0)
                    .with_label_font_size(8.0)
                    .with_formatter(format_log_tick); // Use k/M formatting for log scale
                if let Some(ref label) = self.x_label {
                    x_axis_config = x_axis_config.with_title(label.clone());
                }

                div()
                    .flex()
                    .child(render_axis(
                        &y_scale,
                        &y_axis_config,
                        plot_height as f32,
                        &axis_theme,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(plot_area)
                            .child(render_axis(
                                &x_scale,
                                &x_axis_config,
                                plot_width as f32,
                                &axis_theme,
                            )),
                    )
                    .into_any_element()
            }
            (ScaleType::Linear, ScaleType::Log) => {
                let x_scale = LinearScale::new()
                    .domain(x_min, x_max)
                    .range(0.0, plot_width);
                let y_scale = LogScale::new()
                    .domain(y_min, y_max)
                    .range(plot_height, 0.0);

                // Build plot area with grid and all lines
                let mut plot_area = div()
                    .w(px(plot_width as f32))
                    .h(px(plot_height as f32))
                    .relative()
                    .bg(self.theme.plot_background)
                    .child(render_grid(
                        &x_scale,
                        &y_scale,
                        &grid_config,
                        plot_width as f32,
                        plot_height as f32,
                        &axis_theme,
                    ));

                // Render all additional series first
                for (series_data, series_config) in &series_data_configs {
                    plot_area = plot_area.child(render_line(&x_scale, &y_scale, series_data, series_config));
                }

                // Render primary series on top
                plot_area = plot_area.child(render_line(&x_scale, &y_scale, &primary_data, &primary_config));

                // Create axis configs with labels
                // Generate smart tick values for log Y axis to prevent collision
                let y_ticks = generate_log_ticks(y_min, y_max);
                let mut y_axis_config = AxisConfig::left()
                    .with_tick_values(y_ticks)
                    .with_label_font_size(8.0)
                    .with_formatter(format_log_tick); // Use k/M formatting for log scale
                if let Some(ref label) = self.y_label {
                    y_axis_config = y_axis_config.with_title(label.clone());
                }

                let mut x_axis_config = AxisConfig::bottom()
                    .with_ticks(20)
                    .with_label_font_size(8.0);
                if let Some(ref label) = self.x_label {
                    x_axis_config = x_axis_config.with_title(label.clone());
                }

                div()
                    .flex()
                    .child(render_axis(
                        &y_scale,
                        &y_axis_config,
                        plot_height as f32,
                        &axis_theme,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(plot_area)
                            .child(render_axis(
                                &x_scale,
                                &x_axis_config,
                                plot_width as f32,
                                &axis_theme,
                            )),
                    )
                    .into_any_element()
            }
            (ScaleType::Log, ScaleType::Log) => {
                let x_scale = LogScale::new()
                    .domain(x_min, x_max)
                    .range(0.0, plot_width);
                let y_scale = LogScale::new()
                    .domain(y_min, y_max)
                    .range(plot_height, 0.0);

                // Build plot area with grid and all lines
                let mut plot_area = div()
                    .w(px(plot_width as f32))
                    .h(px(plot_height as f32))
                    .relative()
                    .bg(self.theme.plot_background)
                    .child(render_grid(
                        &x_scale,
                        &y_scale,
                        &grid_config,
                        plot_width as f32,
                        plot_height as f32,
                        &axis_theme,
                    ));

                // Render all additional series first
                for (series_data, series_config) in &series_data_configs {
                    plot_area = plot_area.child(render_line(&x_scale, &y_scale, series_data, series_config));
                }

                // Render primary series on top
                plot_area = plot_area.child(render_line(&x_scale, &y_scale, &primary_data, &primary_config));

                // Create axis configs with labels and angled X labels for log scale
                // Generate smart tick values for both log axes to prevent collision
                let y_ticks = generate_log_ticks(y_min, y_max);
                let mut y_axis_config = AxisConfig::left()
                    .with_tick_values(y_ticks)
                    .with_label_font_size(8.0)
                    .with_formatter(format_log_tick); // Use k/M formatting for log scale
                if let Some(ref label) = self.y_label {
                    y_axis_config = y_axis_config.with_title(label.clone());
                }

                let x_ticks = generate_log_ticks(x_min, x_max);
                let mut x_axis_config = AxisConfig::bottom()
                    .with_tick_values(x_ticks)
                    .with_label_angle(-45.0)
                    .with_label_font_size(8.0)
                    .with_formatter(format_log_tick); // Use k/M formatting for log scale
                if let Some(ref label) = self.x_label {
                    x_axis_config = x_axis_config.with_title(label.clone());
                }

                div()
                    .flex()
                    .child(render_axis(
                        &y_scale,
                        &y_axis_config,
                        plot_height as f32,
                        &axis_theme,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(plot_area)
                            .child(render_axis(
                                &x_scale,
                                &x_axis_config,
                                plot_width as f32,
                                &axis_theme,
                            )),
                    )
                    .into_any_element()
            }
        };

        // Collect legend items if enabled
        let mut legend_items = Vec::new();
        if self.show_legend {
            // Add primary series to legend if it has a label
            if let Some(label) = &self.label {
                legend_items.push((self.color, label.clone()));
            }

            // Add all additional series to legend
            for series in &self.series {
                if let Some(label) = &series.label {
                    legend_items.push((series.color, label.clone()));
                }
            }
        }

        // Calculate legend width based on longest label
        // Formula: color_indicator_width + gap + estimated_text_width + padding
        // Color indicator: 16px, gap: 8px (gap_2), padding: 8px (p_2 on both sides)
        let legend_width = if legend_items.is_empty() {
            0.0
        } else {
            let max_label_len = legend_items
                .iter()
                .map(|(_, label)| label.len())
                .max()
                .unwrap_or(0);
            // Estimate ~7 pixels per character for text_xs font
            let estimated_text_width = (max_label_len as f32) * 7.0;
            16.0 + 8.0 + estimated_text_width + 16.0 // color + gap + text + padding
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
                VectorFontConfig::horizontal(DEFAULT_TITLE_FONT_SIZE, self.theme.title_color.into());
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

        // Add chart content and legend side-by-side
        if !legend_items.is_empty() {
            // Build vertical legend on the right
            let mut legend_column = div()
                .flex()
                .flex_col()
                .gap_2()
                .p_2();

            for (color, label) in legend_items {
                legend_column = legend_column.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w(px(16.0))
                                .h(px(3.0))
                                .bg(rgb(color)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(self.theme.legend_text_color)
                                .child(label),
                        ),
                );
            }

            // Horizontal layout: chart + legend
            container = container.child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(20.0))  // 20px gap between graph and legend
                    .child(div().flex_1().child(chart_content))
                    .child(div().w(px(legend_width)).child(legend_column)),
            );
        } else {
            // No legend, just add chart content
            container = container.child(div().relative().child(chart_content));
        }

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
        x_label: None,
        y_label: None,
        label: None,
        color: DEFAULT_COLOR,
        stroke_width: 2.0,
        opacity: 1.0,
        series: Vec::new(),
        curve: CurveType::Linear,
        show_points: false,
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
        x_scale_type: ScaleType::Linear,
        y_scale_type: ScaleType::Linear,
        show_legend: false,
        theme: ChartTheme::default(),
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

    #[test]
    fn test_line_log_x_scale() {
        let x = vec![10.0, 100.0, 1000.0, 10000.0];
        let y = vec![1.0, 2.0, 3.0, 4.0];
        let result = line(&x, &y).x_scale(ScaleType::Log).build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_line_log_y_scale() {
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let y = vec![10.0, 100.0, 1000.0, 10000.0];
        let result = line(&x, &y).y_scale(ScaleType::Log).build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_line_log_xy_scale() {
        let x = vec![10.0, 100.0, 1000.0];
        let y = vec![20.0, 200.0, 2000.0];
        let result = line(&x, &y)
            .x_scale(ScaleType::Log)
            .y_scale(ScaleType::Log)
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_line_log_x_negative_values() {
        let x = vec![-10.0, -5.0, 5.0, 10.0];
        let y = vec![1.0, 2.0, 3.0, 4.0];
        let result = line(&x, &y).x_scale(ScaleType::Log).build();
        assert!(matches!(
            result,
            Err(ChartError::InvalidData {
                field: "x",
                reason: "contains non-positive values for log scale"
            })
        ));
    }

    #[test]
    fn test_line_log_y_zero_value() {
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let y = vec![0.0, 1.0, 2.0, 3.0];
        let result = line(&x, &y).y_scale(ScaleType::Log).build();
        assert!(matches!(
            result,
            Err(ChartError::InvalidData {
                field: "y",
                reason: "contains non-positive values for log scale"
            })
        ));
    }

    #[test]
    fn test_line_log_scale_with_curve() {
        let x = vec![10.0, 100.0, 1000.0];
        let y = vec![1.0, 2.0, 3.0];
        let result = line(&x, &y)
            .title("Log Scale Line")
            .x_scale(ScaleType::Log)
            .curve(CurveType::Linear)
            .show_points(true)
            .build();
        assert!(result.is_ok());
    }
}
