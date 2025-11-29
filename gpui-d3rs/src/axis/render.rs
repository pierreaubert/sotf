//! Axis rendering functions

use super::{AxisConfig, AxisOrientation, AxisTheme};
use crate::scale::Scale;
use crate::text::{render_vector_text, VectorFontConfig};
use gpui::prelude::*;
use gpui::*;

/// Render an axis with the given scale
///
/// # Example
///
/// ```rust,no_run
/// use d3rs::prelude::*;
/// use d3rs::axis::{render_axis, AxisConfig, DefaultAxisTheme};
/// use gpui::*;
///
/// let scale = LinearScale::new().domain(0.0, 100.0).range(0.0, 400.0);
/// let config = AxisConfig::bottom().with_ticks(10);
/// let theme = DefaultAxisTheme;
///
/// // In a GPUI render context:
/// // render_axis(&scale, &config, 400.0, &theme)
/// ```
pub fn render_axis<S, T>(
    scale: &S,
    config: &AxisConfig,
    size: f32,
    theme: &T,
) -> AnyElement
where
    S: Scale<f64, f64>,
    T: AxisTheme,
{
    let ticks = scale.ticks(config.tick_count);

    match config.orientation {
        AxisOrientation::Bottom => render_bottom_axis(scale, &ticks, config, size, theme).into_any_element(),
        AxisOrientation::Top => render_top_axis(scale, &ticks, config, size, theme).into_any_element(),
        AxisOrientation::Left => render_left_axis(scale, &ticks, config, size, theme).into_any_element(),
        AxisOrientation::Right => render_right_axis(scale, &ticks, config, size, theme).into_any_element(),
    }
}

/// Render a bottom-oriented horizontal axis
fn render_bottom_axis<S, T>(
    scale: &S,
    ticks: &[f64],
    config: &AxisConfig,
    width: f32,
    theme: &T,
) -> impl IntoElement
where
    S: Scale<f64, f64>,
    T: AxisTheme,
{
    let height = config.total_size();
    let (range_min, range_max) = scale.range();
    let range_span = range_max - range_min;

    div()
        .w(px(width))
        .h(px(height))
        .relative()
        // Domain line
        .when(config.show_domain_line, |el| {
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top_0()
                    .h(px(config.domain_line_width))
                    .bg(theme.axis_line_color()),
            )
        })
        // Ticks and labels
        .children(ticks.iter().map(|&tick_value| {
            let range_value = scale.scale(tick_value);
            let x_pos = (range_value - range_min) / range_span;
            let label = format_tick(tick_value, &config.tick_format);
            let half_tick_width = config.domain_line_width / 2.0;
            let font_config = VectorFontConfig::horizontal(
                config.label_font_size,
                theme.axis_label_color().into(),
            );

            div()
                .absolute()
                .left(relative(x_pos as f32))
                .ml(px(-half_tick_width)) // Center the tick mark on the position
                .top_0()
                .flex()
                .flex_col()
                .items_center()
                // Tick mark
                .child(
                    div()
                        .w(px(config.domain_line_width))
                        .h(px(config.tick_size))
                        .bg(theme.axis_line_color()),
                )
                // Label (vector font)
                .child(
                    div()
                        .mt(px(config.tick_padding))
                        .child(render_vector_text(&label, &font_config)),
                )
        }))
        // Title (horizontal for bottom axis)
        .when(config.title.is_some(), |el| {
            let title = config.title.clone().unwrap_or_default();
            let title_top = config.tick_size + config.tick_padding + config.label_font_size + config.title_padding;
            let font_config = VectorFontConfig::horizontal(
                config.title_font_size,
                theme.axis_label_color().into(),
            );
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(title_top))
                    .flex()
                    .justify_center()
                    .child(render_vector_text(&title, &font_config)),
            )
        })
}

/// Render a top-oriented horizontal axis
fn render_top_axis<S, T>(
    scale: &S,
    ticks: &[f64],
    config: &AxisConfig,
    width: f32,
    theme: &T,
) -> impl IntoElement
where
    S: Scale<f64, f64>,
    T: AxisTheme,
{
    let height = config.total_size();
    let (range_min, range_max) = scale.range();
    let range_span = range_max - range_min;
    let domain_line_y = height - config.domain_line_width;

    div()
        .w(px(width))
        .h(px(height))
        .relative()
        // Title (horizontal for top axis, at the top)
        .when(config.title.is_some(), |el| {
            let title = config.title.clone().unwrap_or_default();
            let font_config = VectorFontConfig::horizontal(
                config.title_font_size,
                theme.axis_label_color().into(),
            );
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top_0()
                    .flex()
                    .justify_center()
                    .child(render_vector_text(&title, &font_config)),
            )
        })
        // Domain line
        .when(config.show_domain_line, |el| {
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(domain_line_y))
                    .h(px(config.domain_line_width))
                    .bg(theme.axis_line_color()),
            )
        })
        // Ticks and labels
        .children(ticks.iter().map(|&tick_value| {
            let range_value = scale.scale(tick_value);
            let x_pos = (range_value - range_min) / range_span;
            let label = format_tick(tick_value, &config.tick_format);
            let half_tick_width = config.domain_line_width / 2.0;
            let font_config = VectorFontConfig::horizontal(
                config.label_font_size,
                theme.axis_label_color().into(),
            );

            div()
                .absolute()
                .left(relative(x_pos as f32))
                .ml(px(-half_tick_width)) // Center the tick mark on the position
                .bottom_0()
                .flex()
                .flex_col_reverse()
                .items_center()
                // Label (vector font)
                .child(
                    div()
                        .mb(px(config.tick_padding))
                        .child(render_vector_text(&label, &font_config)),
                )
                // Tick mark
                .child(
                    div()
                        .w(px(config.domain_line_width))
                        .h(px(config.tick_size))
                        .bg(theme.axis_line_color()),
                )
        }))
}

/// Render a left-oriented vertical axis
fn render_left_axis<S, T>(
    scale: &S,
    ticks: &[f64],
    config: &AxisConfig,
    height: f32,
    theme: &T,
) -> impl IntoElement
where
    S: Scale<f64, f64>,
    T: AxisTheme,
{
    let width = config.total_size();
    let (range_min, range_max) = scale.range();
    let range_span = range_max - range_min;
    let domain_line_x = width - config.domain_line_width;

    div()
        .w(px(width))
        .h(px(height))
        .relative()
        // Title (rotated text for left axis - reading bottom-to-top)
        .when(config.title.is_some(), |el| {
            let title = config.title.clone().unwrap_or_default();
            let font_config = VectorFontConfig::vertical_bottom_to_top(
                config.title_font_size,
                theme.axis_label_color().into(),
            );
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(config.title_font_size + 4.0))
                    .flex()
                    .justify_center()
                    .items_center()
                    .child(render_vector_text(&title, &font_config)),
            )
        })
        // Domain line
        .when(config.show_domain_line, |el| {
            el.child(
                div()
                    .absolute()
                    .left(px(domain_line_x))
                    .top_0()
                    .bottom_0()
                    .w(px(config.domain_line_width))
                    .bg(theme.axis_line_color()),
            )
        })
        // Ticks and labels
        .children(ticks.iter().map(|&tick_value| {
            let range_value = scale.scale(tick_value);
            // Invert Y for screen coordinates (bottom-to-top becomes top-to-bottom)
            let y_pos = 1.0 - (range_value - range_min) / range_span;
            let label = format_tick(tick_value, &config.tick_format);
            let half_tick_height = config.domain_line_width / 2.0;
            let font_config = VectorFontConfig::horizontal(
                config.label_font_size,
                theme.axis_label_color().into(),
            );

            div()
                .absolute()
                .right_0()
                .top(relative(y_pos as f32))
                .mt(px(-half_tick_height)) // Center the tick mark on the position
                .flex()
                .flex_row_reverse()
                .items_center()
                // Tick mark
                .child(
                    div()
                        .w(px(config.tick_size))
                        .h(px(config.domain_line_width))
                        .bg(theme.axis_line_color()),
                )
                // Label (vector font)
                .child(
                    div()
                        .mr(px(config.tick_padding))
                        .child(render_vector_text(&label, &font_config)),
                )
        }))
}

/// Render a right-oriented vertical axis
fn render_right_axis<S, T>(
    scale: &S,
    ticks: &[f64],
    config: &AxisConfig,
    height: f32,
    theme: &T,
) -> impl IntoElement
where
    S: Scale<f64, f64>,
    T: AxisTheme,
{
    let width = config.total_size();
    let (range_min, range_max) = scale.range();
    let range_span = range_max - range_min;

    div()
        .w(px(width))
        .h(px(height))
        .relative()
        // Domain line
        .when(config.show_domain_line, |el| {
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(config.domain_line_width))
                    .bg(theme.axis_line_color()),
            )
        })
        // Ticks and labels
        .children(ticks.iter().map(|&tick_value| {
            let range_value = scale.scale(tick_value);
            // Invert Y for screen coordinates (bottom-to-top becomes top-to-bottom)
            let y_pos = 1.0 - (range_value - range_min) / range_span;
            let label = format_tick(tick_value, &config.tick_format);
            let half_tick_height = config.domain_line_width / 2.0;
            let font_config = VectorFontConfig::horizontal(
                config.label_font_size,
                theme.axis_label_color().into(),
            );

            div()
                .absolute()
                .left_0()
                .top(relative(y_pos as f32))
                .mt(px(-half_tick_height)) // Center the tick mark on the position
                .flex()
                .items_center()
                // Tick mark
                .child(
                    div()
                        .w(px(config.tick_size))
                        .h(px(config.domain_line_width))
                        .bg(theme.axis_line_color()),
                )
                // Label (vector font)
                .child(
                    div()
                        .ml(px(config.tick_padding))
                        .child(render_vector_text(&label, &font_config)),
                )
        }))
        // Title (rotated text for right axis - reading bottom-to-top)
        .when(config.title.is_some(), |el| {
            let title = config.title.clone().unwrap_or_default();
            let font_config = VectorFontConfig::vertical_bottom_to_top(
                config.title_font_size,
                theme.axis_label_color().into(),
            );
            el.child(
                div()
                    .absolute()
                    .right_0()
                    .top_0()
                    .bottom_0()
                    .w(px(config.title_font_size + 4.0))
                    .flex()
                    .justify_center()
                    .items_center()
                    .child(render_vector_text(&title, &font_config)),
            )
        })
}

/// Format a tick value using the optional custom formatter
fn format_tick(value: f64, formatter: &Option<fn(f64) -> String>) -> String {
    match formatter {
        Some(f) => f(value),
        None => {
            // Default formatting: remove trailing zeros
            if value.abs() < 1e-10 {
                "0".to_string()
            } else if value.abs() >= 1000.0 || value.abs() < 0.01 {
                format!("{:.1e}", value)
            } else if value.fract().abs() < 1e-10 {
                format!("{:.0}", value)
            } else {
                format!("{:.1}", value)
            }
        }
    }
}

// Tests for render functions require GPUI runtime, see examples instead
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tick_default() {
        assert_eq!(format_tick(0.0, &None), "0");
        assert_eq!(format_tick(10.0, &None), "10");
        assert_eq!(format_tick(10.5, &None), "10.5");
        assert_eq!(format_tick(1000.5, &None), "1.0e3");
    }

    #[test]
    fn test_format_tick_custom() {
        let formatter = |v: f64| format!("{:.2}Hz", v);
        assert_eq!(format_tick(440.0, &Some(formatter)), "440.00Hz");
    }
}
