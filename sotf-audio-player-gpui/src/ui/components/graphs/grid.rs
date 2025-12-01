//! Grid rendering for frequency/SPL graphs
//!
//! Provides grid lines and dot markers at tick points.

use super::axis::{FrequencyAxis, SplAxis};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

/// Grid configuration
#[derive(Debug, Clone)]
pub struct GridConfig {
    /// Whether to show vertical lines at frequency ticks
    pub show_freq_lines: bool,
    /// Whether to show horizontal lines at dB ticks
    pub show_db_lines: bool,
    /// Whether to show dots at intersections
    pub show_dots: bool,
    /// Dot radius in pixels
    pub dot_radius: f32,
    /// Line width in pixels
    pub line_width: f32,
    /// Whether to highlight the 0 dB line
    pub highlight_zero_db: bool,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            show_freq_lines: false,
            show_db_lines: false,
            show_dots: true,
            dot_radius: 1.5,
            line_width: 1.0,
            highlight_zero_db: true,
        }
    }
}

impl GridConfig {
    /// Create a grid with only dots (no lines)
    pub fn dots_only() -> Self {
        Self::default()
    }

    /// Create a grid with lines and dots
    pub fn with_lines() -> Self {
        Self {
            show_freq_lines: true,
            show_db_lines: true,
            show_dots: true,
            ..Self::default()
        }
    }

    /// Create a minimal grid with just the 0 dB line
    pub fn minimal() -> Self {
        Self {
            show_freq_lines: false,
            show_db_lines: false,
            show_dots: false,
            highlight_zero_db: true,
            ..Self::default()
        }
    }
}

/// Render grid dots at tick intersections
pub fn render_grid_dots(
    freq_axis: &FrequencyAxis,
    spl_axis: &SplAxis,
    config: &GridConfig,
    theme: &Theme,
) -> impl IntoElement {
    let freq_positions = freq_axis.tick_positions();
    let db_positions = spl_axis.tick_positions();

    div()
        .absolute()
        .inset_0()
        .overflow_hidden()
        .children(db_positions.iter().flat_map(|&(_db, y_pos)| {
            freq_positions.iter().map(move |&(_freq, x_pos)| {
                div()
                    .absolute()
                    .left(relative(x_pos as f32))
                    .top(relative(y_pos as f32))
                    .w(px(config.dot_radius * 2.0))
                    .h(px(config.dot_radius * 2.0))
                    .rounded_full()
                    .bg(theme.text_muted)
                    .opacity(0.3)
                    // Center the dot on the intersection
                    .ml(px(-config.dot_radius))
                    .mt(px(-config.dot_radius))
            })
        }))
}

/// Render vertical grid lines at frequency ticks
pub fn render_freq_lines(
    freq_axis: &FrequencyAxis,
    config: &GridConfig,
    theme: &Theme,
) -> impl IntoElement {
    let freq_positions = freq_axis.tick_positions();

    div()
        .absolute()
        .inset_0()
        .overflow_hidden()
        .children(freq_positions.iter().map(|&(_freq, x_pos)| {
            div()
                .absolute()
                .left(relative(x_pos as f32))
                .top_0()
                .bottom_0()
                .w(px(config.line_width))
                .bg(theme.border)
                .opacity(0.3)
        }))
}

/// Render horizontal grid lines at dB ticks
pub fn render_db_lines(spl_axis: &SplAxis, config: &GridConfig, theme: &Theme) -> impl IntoElement {
    let db_positions = spl_axis.tick_positions();

    div()
        .absolute()
        .inset_0()
        .overflow_hidden()
        .children(db_positions.iter().map(|&(db, y_pos)| {
            let is_zero = (db.abs() < 0.001) && config.highlight_zero_db;

            div()
                .absolute()
                .top(relative(y_pos as f32))
                .left_0()
                .right_0()
                .h(px(if is_zero {
                    config.line_width * 1.5
                } else {
                    config.line_width
                }))
                .bg(if is_zero {
                    theme.text_secondary
                } else {
                    theme.border
                })
                .when(!is_zero, |el| el.opacity(0.3))
        }))
}

/// Render the 0 dB reference line
pub fn render_zero_db_line(spl_axis: &SplAxis, config: &GridConfig, theme: &Theme) -> Div {
    if let Some(zero_pos) = spl_axis.zero_db_position() {
        div()
            .absolute()
            .top(relative(zero_pos as f32))
            .left_0()
            .right_0()
            .h(px(config.line_width))
            .bg(theme.text_muted)
    } else {
        div()
    }
}

/// Render the complete grid overlay
pub fn render_grid(
    freq_axis: &FrequencyAxis,
    spl_axis: &SplAxis,
    config: &GridConfig,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        // Frequency lines (behind)
        .when(config.show_freq_lines, |el| {
            el.child(render_freq_lines(freq_axis, config, theme))
        })
        // dB lines (behind)
        .when(config.show_db_lines, |el| {
            el.child(render_db_lines(spl_axis, config, theme))
        })
        // 0 dB line (highlighted)
        .when(config.highlight_zero_db, |el| {
            el.child(render_zero_db_line(spl_axis, config, theme))
        })
        // Dots (on top)
        .when(config.show_dots, |el| {
            el.child(render_grid_dots(freq_axis, spl_axis, config, theme))
        })
}
