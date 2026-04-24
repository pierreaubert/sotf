// intentional-file: shared chart-rendering primitives

use crate::components::icons::{Icon, IconName, IconSize};
use crate::theme::Theme;
use gpui::Rgba;
use gpui::prelude::*;
use gpui_px::ChartTheme;
use gpui_ui_kit::{Text, TextSize};

/// Create a new Rgba with modified alpha
fn with_alpha(rgba: Rgba, alpha: f32) -> Rgba {
    Rgba {
        r: rgba.r,
        g: rgba.g,
        b: rgba.b,
        a: alpha,
    }
}

/// Convert Rgba to u32 color value for gpui-px charts
pub fn rgba_to_u32(rgba: Rgba) -> u32 {
    let r = (rgba.r * 255.0) as u32;
    let g = (rgba.g * 255.0) as u32;
    let b = (rgba.b * 255.0) as u32;
    (r << 16) | (g << 8) | b
}

/// Render an empty-state placeholder with an icon and message.
pub fn render_empty_state(
    icon: IconName,
    message: &str,
    theme: &Theme,
) -> gpui::AnyElement {
    gpui::div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(gpui::px(8.0))
        .py(gpui::px(24.0))
        .child(Icon::new(icon).size(IconSize::Xl).color(theme.text_muted))
        .child(
            Text::new(message.to_string())
                .size(TextSize::Xs)
                .color(theme.text_muted),
        )
        .into_any_element()
}

/// Color accessor functions for graph theming
pub mod colors {
    use super::*;

    pub fn input(theme: &Theme) -> Rgba {
        theme.graph_colors.input
    }

    pub fn target(theme: &Theme) -> Rgba {
        theme.graph_colors.target
    }

    pub fn filter(theme: &Theme) -> Rgba {
        theme.graph_colors.filter_response
    }

    pub fn corrected(theme: &Theme) -> Rgba {
        theme.graph_colors.corrected
    }

    pub fn error(theme: &Theme) -> Rgba {
        theme.graph_colors.error
    }

    pub fn deviation(theme: &Theme) -> Rgba {
        theme.graph_colors.deviation
    }

    pub fn secondary_line(theme: &Theme) -> Rgba {
        theme.graph_colors.secondary_line
    }
}

/// Convert theme to gpui-px ChartTheme
pub fn theme_to_chart_theme(theme: &Theme) -> ChartTheme {
    ChartTheme {
        plot_background: theme.surface,
        grid_color: with_alpha(theme.text_muted, 0.3),
        axis_line_color: theme.border,
        axis_label_color: theme.text_muted,
        title_color: theme.text_primary,
        legend_text_color: theme.text_secondary,
    }
}
