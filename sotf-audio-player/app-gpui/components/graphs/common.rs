use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_px::ChartTheme;

/// Color palette for the plots
pub mod colors {
    use crate::theme::Theme;

    pub fn input(theme: &Theme) -> gpui::Rgba {
        theme.graph_colors.input // Indigo - input/original
    }
    pub fn target(theme: &Theme) -> gpui::Rgba {
        theme.success // Green - target
    }
    pub fn filter(theme: &Theme) -> gpui::Rgba {
        theme.warning // Amber - filter response
    }
    pub fn corrected(theme: &Theme) -> gpui::Rgba {
        theme.info // Blue - corrected
    }
    pub fn error(theme: &Theme) -> gpui::Rgba {
        theme.error // Red - error
    }
    pub fn deviation(theme: &Theme) -> gpui::Rgba {
        theme.graph_colors.deviation // Violet - deviation
    }
    pub fn secondary_line(theme: &Theme) -> gpui::Rgba {
        theme.graph_colors.secondary_line // Grey for secondary lines
    }
    pub fn directivity_er(theme: &Theme) -> gpui::Rgba {
        theme.graph_colors.directivity_er // Pink
    }
    pub fn directivity_sp(theme: &Theme) -> gpui::Rgba {
        theme.graph_colors.directivity_sp // Purple
    }
}

/// Convert Rgba to u32 hex color for gpui-px
pub fn rgba_to_u32(rgba: Rgba) -> u32 {
    let r = (rgba.r * 255.0) as u32;
    let g = (rgba.g * 255.0) as u32;
    let b = (rgba.b * 255.0) as u32;
    (r << 16) | (g << 8) | b
}

/// Create a new Rgba with modified alpha
pub fn with_alpha(rgba: Rgba, alpha: f32) -> Rgba {
    Rgba {
        r: rgba.r,
        g: rgba.g,
        b: rgba.b,
        a: alpha,
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

/// Color palette for filter bands - uses theme band_colors
pub fn band_color(index: usize, theme: &Theme) -> Rgba {
    theme
        .band_colors
        .get(index)
        .copied()
        .unwrap_or(theme.text_muted)
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

/// Wrap a plot with a title
pub fn render_plot_with_title(title: &str, plot: Div, theme: &Theme) -> Div {
    let title = SharedString::from(title.to_string());
    div()
        .flex()
        .flex_col()
        .flex_1()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_primary)
                .child(title),
        )
        .child(plot)
}

/// Render a compact horizontal legend
pub fn render_compact_legend(items: &[(String, Rgba)], theme: &Theme) -> Div {
    div()
        .h(px(16.0))
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .children(items.iter().map(|(label, color)| {
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().w(px(10.0)).h(px(2.0)).rounded_sm().bg(*color))
                .child(
                    div()
                        .text_size(px(9.0))
                        .text_color(theme.text_muted)
                        .child(label.clone()),
                )
        }))
}
