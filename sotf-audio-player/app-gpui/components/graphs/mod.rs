//! EQ Frequency Response Graph Module
//!
//! A graphing library for frequency/SPL visualizations using gpui-px:
//! - Logarithmic frequency axis (20Hz - 20kHz)
//! - Linear SPL axis (configurable dB range)
//! - Built-in grid and axis rendering
//! - Configurable legend
//! - Aspect ratio preservation

pub mod common;
pub mod eq_band_controls;
pub mod freq_response_graph;
pub mod headphone_graphs;
pub mod result_graphs; // Keeping result_graphs for backward compatibility if needed, but it should be empty or re-exporting
pub mod speaker_graphs;

// Re-export common types and functions for convenience
pub use common::{band_color, format_frequency};
pub use eq_band_controls::render_eq_band_controls;
pub use freq_response_graph::render_freq_response_graph;

use crate::theme::Theme;
use d3rs::legend::{LegendConfig, LegendPosition};
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::EQFilter;

/// Default aspect ratio (width / height) for the graph area
const DEFAULT_ASPECT_RATIO: f32 = 1.4;

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
            legend: LegendConfig::new(),
            aspect_ratio: DEFAULT_ASPECT_RATIO,
            min_height: 150.0,
            show_response_curve: true,
        }
    }
}

impl GraphConfig {
    /// Create a graph with legend on the right
    pub fn with_legend_right(mut self) -> Self {
        self.legend = LegendConfig::new().position(LegendPosition::Right);
        self
    }

    /// Create a graph with legend below
    pub fn with_legend_below(mut self) -> Self {
        self.legend = LegendConfig::new().position(LegendPosition::Bottom);
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

/// Legacy compatibility: Render the EQ visualization (bar-based)
pub fn render_eq_visualization(
    filters: &[EQFilter],
    selected_band: Option<usize>,
    theme: &Theme,
    available_width: f32,
) -> impl IntoElement {
    render_freq_response_graph(
        filters,
        selected_band,
        GraphConfig::default().with_grid_lines(), // Enable grid lines
        theme,
        available_width,
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