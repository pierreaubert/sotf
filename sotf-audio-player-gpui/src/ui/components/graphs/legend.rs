//! Legend rendering for frequency/SPL graphs
//!
//! Provides legend display on the right or below the graph.

use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

/// Legend position relative to the graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LegendPosition {
    /// Legend on the right side of the graph
    #[default]
    Right,
    /// Legend below the graph
    Below,
    /// No legend
    Hidden,
}

/// A single legend entry
#[derive(Debug, Clone)]
pub struct LegendEntry {
    /// Label text
    pub label: String,
    /// Color for the entry indicator
    pub color: Rgba,
    /// Optional secondary text (e.g., value)
    pub value: Option<String>,
    /// Whether this entry is currently selected/active
    pub active: bool,
}

impl LegendEntry {
    pub fn new(label: impl Into<String>, color: Rgba) -> Self {
        Self {
            label: label.into(),
            color,
            value: None,
            active: false,
        }
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn with_active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
}

/// Legend configuration
#[derive(Debug, Clone)]
pub struct LegendConfig {
    /// Position of the legend
    pub position: LegendPosition,
    /// Width for right-positioned legend
    pub width: f32,
    /// Height for bottom-positioned legend
    pub height: f32,
    /// Padding inside the legend
    pub padding: f32,
    /// Gap between entries
    pub entry_gap: f32,
    /// Size of the color indicator
    pub indicator_size: f32,
}

impl Default for LegendConfig {
    fn default() -> Self {
        Self {
            position: LegendPosition::Right,
            width: 120.0,
            height: 60.0,
            padding: 8.0,
            entry_gap: 4.0,
            indicator_size: 8.0,
        }
    }
}

impl LegendConfig {
    pub fn right() -> Self {
        Self {
            position: LegendPosition::Right,
            ..Self::default()
        }
    }

    pub fn below() -> Self {
        Self {
            position: LegendPosition::Below,
            ..Self::default()
        }
    }

    pub fn hidden() -> Self {
        Self {
            position: LegendPosition::Hidden,
            ..Self::default()
        }
    }
}

/// Render a single legend entry
fn render_legend_entry(
    entry: &LegendEntry,
    config: &LegendConfig,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(config.entry_gap))
        // Color indicator
        .child(
            div()
                .w(px(config.indicator_size))
                .h(px(config.indicator_size))
                .rounded(px(2.0))
                .bg(entry.color)
                .when(entry.active, |el| el.border_1().border_color(theme.text_primary)),
        )
        // Label
        .child(
            div()
                .flex_1()
                .text_xs()
                .text_color(if entry.active {
                    theme.text_primary
                } else {
                    theme.text_secondary
                })
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(entry.label.clone()),
        )
        // Optional value
        .when_some(entry.value.clone(), |el, value| {
            el.child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_primary)
                    .child(value),
            )
        })
}

/// Render legend on the right side (vertical layout)
pub fn render_legend_right(
    entries: &[LegendEntry],
    config: &LegendConfig,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .id("legend-right")
        .w(px(config.width))
        .h_full()
        .flex()
        .flex_col()
        .p(px(config.padding))
        .gap(px(config.entry_gap))
        .bg(theme.surface)
        .border_l_1()
        .border_color(theme.border)
        .overflow_y_scroll()
        .children(
            entries
                .iter()
                .map(|entry| render_legend_entry(entry, config, theme)),
        )
}

/// Render legend below the graph (horizontal layout)
pub fn render_legend_below(
    entries: &[LegendEntry],
    config: &LegendConfig,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .id("legend-below")
        .w_full()
        .h(px(config.height))
        .flex()
        .flex_wrap()
        .items_start()
        .p(px(config.padding))
        .gap(px(config.entry_gap * 2.0))
        .bg(theme.surface)
        .border_t_1()
        .border_color(theme.border)
        .overflow_x_scroll()
        .children(
            entries
                .iter()
                .map(|entry| render_legend_entry(entry, config, theme)),
        )
}

/// Render the legend based on configuration
pub fn render_legend(
    entries: &[LegendEntry],
    config: &LegendConfig,
    theme: &Theme,
) -> Option<impl IntoElement> {
    match config.position {
        LegendPosition::Right => Some(render_legend_right(entries, config, theme)),
        LegendPosition::Below => None, // Handled separately due to layout
        LegendPosition::Hidden => None,
    }
}

/// Calculate the space taken by the legend
pub fn legend_dimensions(config: &LegendConfig) -> (f32, f32) {
    match config.position {
        LegendPosition::Right => (config.width, 0.0),
        LegendPosition::Below => (0.0, config.height),
        LegendPosition::Hidden => (0.0, 0.0),
    }
}
