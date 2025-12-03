//! EQ Frequency Response Graph Module
//!
//! A graphing library for frequency/SPL visualizations using gpui-d3rs:
//! - Logarithmic frequency axis (20Hz - 20kHz)
//! - Linear SPL axis (configurable dB range)
//! - Grid with dots at tick intersections
//! - Configurable legend (right or below)
//! - Aspect ratio preservation

pub mod legend;

use crate::theme::Theme;
use autoeq_iir::Biquad;
use gpui::prelude::*;
use gpui::*;
use d3rs::axis::{render_axis, AxisConfig, AxisTheme};
use d3rs::color::D3Color;
use d3rs::grid::{render_grid, GridConfig as D3GridConfig};
use d3rs::scale::{LinearScale, LogScale, Scale};
use d3rs::shape::{render_line, LineConfig, LinePoint};
use legend::{LegendConfig, LegendEntry, LegendPosition, legend_dimensions, render_legend_below, render_legend_right};
use sotf_audio_player::EQFilter;

/// Default sample rate for filter calculations
const SAMPLE_RATE: f64 = 48000.0;

/// Default aspect ratio (width / height) for the graph area
const DEFAULT_ASPECT_RATIO: f32 = 1.4;

/// Standard frequency ticks for audio graphs (logarithmic spacing)
const FREQ_TICKS: [f64; 10] = [20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0];

/// Theme adapter for gpui-d3rs axis rendering
struct EqAxisTheme {
    line_color: Rgba,
    label_color: Rgba,
}

impl EqAxisTheme {
    fn from_theme(theme: &Theme) -> Self {
        Self {
            line_color: theme.border,
            label_color: theme.text_muted,
        }
    }
}

impl AxisTheme for EqAxisTheme {
    fn axis_line_color(&self) -> Rgba {
        self.line_color
    }

    fn axis_label_color(&self) -> Rgba {
        self.label_color
    }
}

/// Graph configuration
#[derive(Debug, Clone)]
pub struct GraphConfig {
    /// Minimum frequency (Hz)
    pub min_freq: f64,
    /// Maximum frequency (Hz)
    pub max_freq: f64,
    /// Minimum dB
    pub min_db: f64,
    /// Maximum dB
    pub max_db: f64,
    /// Whether to show vertical grid lines
    pub show_freq_lines: bool,
    /// Whether to show horizontal grid lines
    pub show_db_lines: bool,
    /// Whether to show dots at grid intersections
    pub show_dots: bool,
    /// Legend configuration
    pub legend: LegendConfig,
    /// Aspect ratio (width / height) for the graph area
    pub aspect_ratio: f32,
    /// Minimum height for the graph
    pub min_height: f32,
    /// Whether to show the combined response curve
    pub show_response_curve: bool,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            min_freq: 20.0,
            max_freq: 20000.0,
            min_db: -24.0,
            max_db: 24.0,
            show_freq_lines: false,
            show_db_lines: false,
            show_dots: true,
            legend: LegendConfig::hidden(),
            aspect_ratio: DEFAULT_ASPECT_RATIO,
            min_height: 150.0,
            show_response_curve: true,
        }
    }
}

impl GraphConfig {
    /// Create a graph with legend on the right
    pub fn with_legend_right(mut self) -> Self {
        self.legend = LegendConfig::right();
        self
    }

    /// Create a graph with legend below
    pub fn with_legend_below(mut self) -> Self {
        self.legend = LegendConfig::below();
        self
    }

    /// Set custom aspect ratio
    pub fn with_ratio(mut self, ratio: f32) -> Self {
        self.aspect_ratio = ratio;
        self
    }

    /// Set custom dB range
    pub fn with_db_range(mut self, min_db: f64, max_db: f64) -> Self {
        self.min_db = min_db;
        self.max_db = max_db;
        self
    }

    /// Enable grid lines
    pub fn with_grid_lines(mut self) -> Self {
        self.show_freq_lines = true;
        self.show_db_lines = true;
        self
    }
}

/// Color palette for filter bands
pub fn band_color(index: usize, _theme: &Theme) -> Rgba {
    let colors = [
        rgb(0xef4444), // Red
        rgb(0xf97316), // Orange
        rgb(0xeab308), // Yellow
        rgb(0x22c55e), // Green
        rgb(0x14b8a6), // Teal
        rgb(0x3b82f6), // Blue
        rgb(0x8b5cf6), // Violet
        rgb(0xec4899), // Pink
        rgb(0x6366f1), // Indigo
        rgb(0x06b6d4), // Cyan
    ];
    colors.get(index).copied().unwrap_or(rgb(0x9ca3af))
}

/// Format frequency value for display
pub fn format_frequency(freq: f64) -> String {
    if freq >= 1000.0 {
        let k = freq / 1000.0;
        if k.fract() < 0.001 {
            format!("{}k", k as i32)
        } else {
            format!("{:.1}k", k)
        }
    } else {
        format!("{:.0}", freq)
    }
}

/// Calculate the combined response in dB at a given frequency
fn calculate_response_at_freq(filters: &[EQFilter], freq: f64) -> f64 {
    if filters.is_empty() {
        return 0.0;
    }
    filters
        .iter()
        .map(|f| {
            let biquad = Biquad::new(
                f.filter_type.clone(),
                f.frequency,
                SAMPLE_RATE,
                f.q,
                f.gain_db,
            );
            biquad.log_result(freq)
        })
        .sum()
}

/// Generate dB tick values for axis
fn generate_db_ticks(min_db: f64, max_db: f64) -> Vec<f64> {
    let range = max_db - min_db;
    let step = if range <= 24.0 {
        6.0
    } else if range <= 48.0 {
        12.0
    } else {
        24.0
    };

    let mut ticks = Vec::new();
    let mut db = (min_db / step).ceil() * step;
    while db <= max_db {
        ticks.push(db);
        db += step;
    }
    ticks
}

/// Render the main frequency response graph
pub fn render_freq_response_graph(
    filters: &[EQFilter],
    selected_band: Option<usize>,
    config: GraphConfig,
    theme: &Theme,
    available_width: f32,
) -> impl IntoElement {
    let theme = theme.clone();
    let axis_theme = EqAxisTheme::from_theme(&theme);

    // Calculate dimensions based on legend position
    let (legend_width, legend_height) = legend_dimensions(&config.legend);
    let left_axis_width = 40.0_f32;
    let bottom_axis_height = 24.0_f32;

    // Calculate graph area dimensions to maintain aspect ratio
    let graph_area_width = available_width - legend_width - left_axis_width;
    let graph_area_height = (graph_area_width / config.aspect_ratio).max(config.min_height);

    // Total height including labels and legend below
    let total_height = graph_area_height + bottom_axis_height + legend_height;

    // Create scales using gpui-d3rs
    let freq_scale = LogScale::new()
        .domain(config.min_freq, config.max_freq)
        .range(0.0, graph_area_width as f64);

    let db_scale = LinearScale::new()
        .domain(config.min_db, config.max_db)
        .range(graph_area_height as f64, 0.0); // Inverted for screen coordinates

    // Generate frequency points for smooth curve
    let num_points = 120;
    let freq_points: Vec<f64> = (0..num_points)
        .map(|i| {
            let t = i as f64 / (num_points - 1) as f64;
            let log_min = config.min_freq.ln();
            let log_max = config.max_freq.ln();
            (log_min + t * (log_max - log_min)).exp()
        })
        .collect();

    // Calculate response curve data points
    let curve_data: Vec<LinePoint> = freq_points
        .iter()
        .map(|&freq| {
            let response_db = calculate_response_at_freq(filters, freq);
            LinePoint::new(freq, response_db)
        })
        .collect();

    // Build legend entries if needed
    let legend_entries: Vec<LegendEntry> = filters
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let color = band_color(i, &theme);
            let label = format!(
                "{} {} {}",
                i + 1,
                f.filter_type.short_name(),
                format_frequency(f.frequency)
            );
            LegendEntry::new(label, color)
                .with_value(format!("{:+.1}dB", f.gain_db))
                .with_active(selected_band == Some(i))
        })
        .collect();

    // Get tick values
    let freq_ticks: Vec<f64> = FREQ_TICKS.iter().copied().collect();
    let db_ticks = generate_db_ticks(config.min_db, config.max_db);

    // Create d3rs grid config
    let grid_config = D3GridConfig::new()
        .with_vertical_lines(config.show_freq_lines)
        .with_horizontal_lines(config.show_db_lines)
        .with_dots(config.show_dots)
        .with_vertical_values(freq_ticks.clone())
        .with_horizontal_values(db_ticks.clone())
        .with_dot_opacity(0.3)
        .with_line_opacity(0.3);

    // Axis configs
    let bottom_axis_config = AxisConfig::bottom()
        .with_tick_values(freq_ticks)
        .with_formatter(|v| format_frequency(v))
        .with_tick_size(4.0)
        .with_label_font_size(9.0);

    let left_axis_config = AxisConfig::left()
        .with_tick_values(db_ticks)
        .with_formatter(|v| {
            if v > 0.0 {
                format!("+{:.0}", v)
            } else {
                format!("{:.0}", v)
            }
        })
        .with_tick_size(4.0)
        .with_label_font_size(9.0);

    // Line config for response curve
    let line_config = LineConfig::new()
        .stroke_width(2.0)
        .stroke_color(D3Color::from_rgba(theme.accent));

    // Main container
    div()
        .w(px(available_width))
        .h(px(total_height))
        .flex()
        .flex_col()
        // Top section: left axis + graph + legend (if right)
        .child(
            div()
                .flex()
                .h(px(graph_area_height))
                // Left: dB axis
                .child(
                    div()
                        .w(px(left_axis_width))
                        .h_full()
                        .child(render_axis(&db_scale, &left_axis_config, graph_area_height, &axis_theme)),
                )
                // Center: Graph area
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .bg(theme.surface)
                        .rounded_lg()
                        .border_1()
                        .border_color(theme.border)
                        .relative()
                        .overflow_hidden()
                        // Grid
                        .child(render_grid(
                            &freq_scale,
                            &db_scale,
                            &grid_config,
                            graph_area_width,
                            graph_area_height,
                            &axis_theme,
                        ))
                        // 0 dB reference line
                        .when(config.min_db <= 0.0 && config.max_db >= 0.0, |el| {
                            let zero_pos = db_scale.scale(0.0) / graph_area_height as f64;
                            el.child(
                                div()
                                    .absolute()
                                    .top(relative(zero_pos as f32))
                                    .left_0()
                                    .right_0()
                                    .h(px(1.0))
                                    .bg(theme.text_muted)
                                    .opacity(0.5),
                            )
                        })
                        // Response curve
                        .when(config.show_response_curve && !filters.is_empty(), |el| {
                            el.child(render_line(
                                &freq_scale,
                                &db_scale,
                                &curve_data,
                                &line_config,
                            ))
                        })
                        // Filter point indicators
                        .child(render_filter_points(
                            filters,
                            selected_band,
                            &freq_scale,
                            &db_scale,
                            graph_area_width,
                            graph_area_height,
                            &theme,
                        )),
                )
                // Right: Legend (if position is Right)
                .when(config.legend.position == LegendPosition::Right, |el| {
                    el.child(render_legend_right(&legend_entries, &config.legend, &theme))
                }),
        )
        // Bottom: Frequency axis
        .child(
            div()
                .w_full()
                .h(px(bottom_axis_height))
                .ml(px(left_axis_width))
                .child(render_axis(&freq_scale, &bottom_axis_config, graph_area_width, &axis_theme)),
        )
        // Bottom: Legend (if position is Below)
        .when(config.legend.position == LegendPosition::Below, |el| {
            el.child(render_legend_below(&legend_entries, &config.legend, &theme))
        })
}

/// Render filter point indicators
fn render_filter_points(
    filters: &[EQFilter],
    selected_band: Option<usize>,
    freq_scale: &LogScale,
    db_scale: &LinearScale,
    width: f32,
    height: f32,
    theme: &Theme,
) -> impl IntoElement {
    let (freq_range_min, freq_range_max) = freq_scale.range();
    let freq_range_span = freq_range_max - freq_range_min;
    let (db_range_min, db_range_max) = db_scale.range();
    let db_range_span = db_range_max - db_range_min;

    div()
        .absolute()
        .inset_0()
        .children(filters.iter().enumerate().map(|(i, f)| {
            let x_range = freq_scale.scale(f.frequency);
            let x_pos = ((x_range - freq_range_min) / freq_range_span) as f32;

            let y_range = db_scale.scale(f.gain_db);
            let y_pos = ((y_range - db_range_min) / db_range_span) as f32;

            let is_selected = selected_band == Some(i);
            let color = band_color(i, theme);
            let size = if is_selected { 16.0 } else { 12.0 };

            div()
                .absolute()
                .left(px(x_pos * width))
                .top(px(y_pos * height))
                .w(px(size))
                .h(px(size))
                .ml(px(-size / 2.0))
                .mt(px(-size / 2.0))
                .rounded_full()
                .bg(color)
                .border_2()
                .border_color(if is_selected { theme.text_primary } else { color })
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(theme.text_on_accent)
                .font_weight(FontWeight::BOLD)
                .child(format!("{}", i + 1))
        }))
}

/// Render EQ band control buttons
pub fn render_eq_band_controls(
    filters: &[EQFilter],
    selected_band: Option<usize>,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .gap_2()
        .flex_wrap()
        .children(filters.iter().enumerate().map(|(i, f)| {
            let is_selected = selected_band == Some(i);
            let color = band_color(i, theme);
            let filter_type_name = f.filter_type.short_name();

            div()
                .id(SharedString::from(format!("band-{}", i)))
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .px_3()
                .py_2()
                .rounded_lg()
                .bg(if is_selected {
                    theme.accent_muted
                } else {
                    theme.surface
                })
                .border_2()
                .border_color(if is_selected { color } else { theme.border })
                .min_w(px(75.0))
                .cursor_pointer()
                .hover(|s| s.border_color(color))
                // Band indicator
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(color))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.text_primary)
                                .child(format!("{} {}", i + 1, filter_type_name)),
                        ),
                )
                // Frequency
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child(format_frequency(f.frequency)),
                )
                // Gain
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if f.gain_db > 0.5 {
                            theme.success
                        } else if f.gain_db < -0.5 {
                            theme.error
                        } else {
                            theme.text_muted
                        })
                        .child(format!("{:+.1}dB", f.gain_db)),
                )
                // Q
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child(format!("Q:{:.1}", f.q)),
                )
        }))
}

/// Legacy compatibility: Render the EQ visualization (bar-based)
pub fn render_eq_visualization(
    filters: &[EQFilter],
    selected_band: Option<usize>,
    theme: &Theme,
) -> impl IntoElement {
    render_freq_response_graph(
        filters,
        selected_band,
        GraphConfig::default(),
        theme,
        400.0, // Default width
    )
}

/// Legacy compatibility: Frequency axis labels
pub fn render_freq_labels(theme: &Theme) -> impl IntoElement {
    let freq_labels = ["20Hz", "100", "1k", "10k", "20kHz"];
    div().flex().justify_between().w_full().px_2().children(
        freq_labels
            .iter()
            .map(|label| div().text_xs().text_color(theme.text_muted).child(*label)),
    )
}

/// Legacy compatibility: dB axis labels
pub fn render_db_labels(theme: &Theme) -> impl IntoElement {
    let db_labels = ["+24", "+12", "0dB", "-12", "-24"];
    div()
        .flex()
        .flex_col()
        .justify_between()
        .h_full()
        .pr_2()
        .children(
            db_labels
                .iter()
                .map(|label| div().text_xs().text_color(theme.text_muted).child(*label)),
        )
}
