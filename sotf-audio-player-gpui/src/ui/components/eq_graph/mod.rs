//! EQ Frequency Response Graph Module
//!
//! A graphing library for frequency/SPL visualizations with:
//! - Logarithmic frequency axis (20Hz - 20kHz)
//! - Linear SPL axis (configurable dB range)
//! - Grid with dots at tick intersections
//! - Configurable legend (right or below)
//! - Aspect ratio preservation

pub mod axis;
pub mod grid;
pub mod label;
pub mod legend;

use axis::{FrequencyAxis, SplAxis};
use grid::{GridConfig, render_grid};
use label::{LabelConfig, render_db_labels_vertical, render_freq_labels_horizontal, db_label_width, freq_label_height, format_frequency};
use legend::{LegendConfig, LegendEntry, LegendPosition, legend_dimensions, render_legend_right, render_legend_below};

use crate::theme::Theme;
use autoeq_iir::Biquad;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::EQFilter;

// Re-export commonly used types
pub use axis::{FrequencyAxis as FreqAxis, SplAxis as DbAxis};
pub use grid::GridConfig as GraphGridConfig;
pub use label::LabelConfig as GraphLabelConfig;
pub use legend::LegendConfig as GraphLegendConfig;

/// Default sample rate for filter calculations
const SAMPLE_RATE: f64 = 48000.0;

/// Default aspect ratio (width / height) for the graph area
const DEFAULT_ASPECT_RATIO: f32 = 1.4;

/// Graph configuration
#[derive(Debug, Clone)]
pub struct GraphConfig {
    /// Frequency axis configuration
    pub freq_axis: FrequencyAxis,
    /// SPL/dB axis configuration
    pub spl_axis: SplAxis,
    /// Grid configuration
    pub grid: GridConfig,
    /// Label configuration
    pub labels: LabelConfig,
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
            freq_axis: FrequencyAxis::default(),
            spl_axis: SplAxis::default(),
            grid: GridConfig::dots_only(),
            labels: LabelConfig::default(),
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
        self.spl_axis = SplAxis::new(min_db, max_db);
        self
    }

    /// Enable grid lines
    pub fn with_grid_lines(mut self) -> Self {
        self.grid = GridConfig::with_lines();
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

/// Calculate the combined response in dB at a given frequency
fn calculate_response_at_freq(filters: &[EQFilter], freq: f64) -> f64 {
    if filters.is_empty() {
        return 0.0;
    }
    filters
        .iter()
        .map(|f| {
            let biquad =
                Biquad::new(f.filter_type.clone(), f.frequency, SAMPLE_RATE, f.q, f.gain_db);
            biquad.log_result(freq)
        })
        .sum()
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

    // Calculate dimensions based on legend position
    let (legend_width, legend_height) = legend_dimensions(&config.legend);
    let label_left_width = db_label_width(&config.spl_axis);
    let label_bottom_height = freq_label_height(&config.freq_axis);

    // Calculate graph area dimensions to maintain aspect ratio
    let graph_area_width = available_width - legend_width - label_left_width;
    let graph_area_height = (graph_area_width / config.aspect_ratio).max(config.min_height);

    // Total height including labels and legend below
    let total_height = graph_area_height + label_bottom_height + legend_height;

    // Create frequency bands for visualization
    let freq_bands: Vec<f64> = (0..60)
        .map(|i| {
            let t = i as f64 / 59.0;
            config.freq_axis.normalized_to_freq(t)
        })
        .collect();

    // Calculate response at each band
    let responses: Vec<(f64, f64)> = freq_bands
        .iter()
        .map(|&freq| (freq, calculate_response_at_freq(filters, freq)))
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

    // Main container
    div()
        .w(px(available_width))
        .h(px(total_height))
        .flex()
        .flex_col()
        // Top section: labels + graph + legend (if right)
        .child(
            div()
                .flex()
                .h(px(graph_area_height))
                // Left: dB labels
                .child(
                    div()
                        .w(px(label_left_width))
                        .h_full()
                        .child(render_db_labels_vertical(
                            &config.spl_axis,
                            &config.labels,
                            &theme,
                        )),
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
                            &config.freq_axis,
                            &config.spl_axis,
                            &config.grid,
                            &theme,
                        ))
                        // Response visualization
                        .when(config.show_response_curve, |el| {
                            el.child(render_response_bars(
                                &responses,
                                &config.spl_axis,
                                &theme,
                            ))
                        })
                        // Filter point indicators
                        .child(render_filter_points(
                            filters,
                            selected_band,
                            &config.freq_axis,
                            &config.spl_axis,
                            &theme,
                        )),
                )
                // Right: Legend (if position is Right)
                .when(config.legend.position == LegendPosition::Right, |el| {
                    el.child(render_legend_right(&legend_entries, &config.legend, &theme))
                }),
        )
        // Bottom: Frequency labels
        .child(
            div()
                .w_full()
                .h(px(label_bottom_height))
                .ml(px(label_left_width))
                .child(render_freq_labels_horizontal(
                    &config.freq_axis,
                    &config.labels,
                    &theme,
                )),
        )
        // Bottom: Legend (if position is Below)
        .when(config.legend.position == LegendPosition::Below, |el| {
            el.child(render_legend_below(&legend_entries, &config.legend, &theme))
        })
}

/// Render response bars (simplified visualization)
fn render_response_bars(
    responses: &[(f64, f64)],
    spl_axis: &SplAxis,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_end()
        .gap_px()
        .p_1()
        .children(responses.iter().map(|(_freq, db)| {
            let height_percent = 1.0 - spl_axis.db_to_normalized(*db);
            let is_boost = *db > 0.5;
            let is_cut = *db < -0.5;

            div()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .justify_center()
                .child(
                    div()
                        .w_full()
                        .h(relative(height_percent as f32))
                        .rounded_t_sm()
                        .bg(if is_boost {
                            rgba(0x22c55e60) // Green semi-transparent
                        } else if is_cut {
                            rgba(0xef444460) // Red semi-transparent
                        } else {
                            theme.accent_muted
                        }),
                )
        }))
}

/// Render filter point indicators
fn render_filter_points(
    filters: &[EQFilter],
    selected_band: Option<usize>,
    freq_axis: &FrequencyAxis,
    spl_axis: &SplAxis,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        .children(filters.iter().enumerate().map(|(i, f)| {
            let x_pos = freq_axis.freq_to_normalized(f.frequency);
            let y_pos = spl_axis.db_to_normalized(f.gain_db);
            let is_selected = selected_band == Some(i);
            let color = band_color(i, theme);
            let size = if is_selected { 16.0 } else { 12.0 };

            div()
                .absolute()
                .left(relative(x_pos as f32))
                .top(relative(y_pos as f32))
                .w(px(size))
                .h(px(size))
                .ml(px(-size / 2.0))
                .mt(px(-size / 2.0))
                .rounded_full()
                .bg(color)
                .border_2()
                .border_color(if is_selected { rgb(0xffffff) } else { color })
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(rgb(0xffffff))
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
                        .child(
                            div()
                                .w(px(8.0))
                                .h(px(8.0))
                                .rounded_full()
                                .bg(color),
                        )
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
                            rgb(0x22c55e)
                        } else if f.gain_db < -0.5 {
                            rgb(0xef4444)
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
    div()
        .flex()
        .justify_between()
        .w_full()
        .px_2()
        .children(freq_labels.iter().map(|label| {
            div().text_xs().text_color(theme.text_muted).child(*label)
        }))
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
        .children(db_labels.iter().map(|label| {
            div().text_xs().text_color(theme.text_muted).child(*label)
        }))
}
