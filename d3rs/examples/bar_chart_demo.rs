//! Bar chart demonstration example
//!
//! This example demonstrates bar chart rendering with different configurations.

use d3rs::prelude::*;
use d3rs::axis::{render_axis, AxisConfig, DefaultAxisTheme};
use d3rs::grid::{render_grid, GridConfig};
use gpui::*;

struct BarChartDemo {
    _unused: bool,
}

impl BarChartDemo {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self { _unused: false }
    }
}

impl Render for BarChartDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = DefaultAxisTheme;

        // Create scales
        let x_scale = LinearScale::new()
            .domain(0.0, 6.0)
            .range(0.0, 500.0);

        let y_scale = LinearScale::new()
            .domain(0.0, 100.0)
            .range(0.0, 300.0);

        // Sample data
        let data = vec![
            BarDatum::new("Mon", 45.0),
            BarDatum::new("Tue", 68.0),
            BarDatum::new("Wed", 55.0),
            BarDatum::new("Thu", 82.0),
            BarDatum::new("Fri", 70.0),
            BarDatum::new("Sat", 38.0),
        ];

        // Data with negative values
        let mixed_data = vec![
            BarDatum::new("A", 30.0),
            BarDatum::new("B", -15.0),
            BarDatum::new("C", 45.0),
            BarDatum::new("D", -25.0),
            BarDatum::new("E", 60.0),
        ];

        let mixed_y_scale = LinearScale::new()
            .domain(-30.0, 70.0)
            .range(0.0, 300.0);

        let mixed_x_scale = LinearScale::new()
            .domain(0.0, 5.0)
            .range(0.0, 500.0);

        // Color scheme
        let scheme = ColorScheme::category10();

        div()
            .flex()
            .flex_col()
            .gap_8()
            .p_8()
            .bg(rgb(0xffffff))
            .size_full()
            // Title
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .child("d3rs Bar Chart Demonstration")
            )
            // Simple bar chart
            .child(
                div()
                    .flex()
                    .flex_col()
                    
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Simple Bar Chart").mb_2())
                    .child(
                        div()
                            .flex()
                            .child(render_axis(&y_scale, &AxisConfig::left().with_ticks(5), 300.0, &theme))
                            .child(
                                div()
                                    .w(px(500.0))
                                    .h(px(300.0))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .border_1()
                                    .border_color(rgb(0xcccccc))
                                    .child(render_grid(
                                        &x_scale,
                                        &y_scale,
                                        &GridConfig::lines_only().with_line_opacity(0.2),
                                        500.0,
                                        300.0,
                                        &theme,
                                    ))
                                    .child(render_bars(
                                        &x_scale,
                                        &y_scale,
                                        &data,
                                        500.0,
                                        300.0,
                                        &BarConfig::new()
                                            .fill_color(scheme.color(0))
                                            .opacity(0.85),
                                    ))
                            )
                    )
                    .child(div().ml(px(60.0)).child(render_axis(&x_scale, &AxisConfig::bottom().with_ticks(6), 500.0, &theme)))
            )
            // Colorful bars with stroke
            .child(
                div()
                    .flex()
                    .flex_col()
                    
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Styled Bars (stroke + rounded corners)").mb_2())
                    .child(
                        div()
                            .flex()
                            .child(render_axis(&y_scale, &AxisConfig::left().with_ticks(5), 300.0, &theme))
                            .child(
                                div()
                                    .w(px(500.0))
                                    .h(px(300.0))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .border_1()
                                    .border_color(rgb(0xcccccc))
                                    .child(render_grid(
                                        &x_scale,
                                        &y_scale,
                                        &GridConfig::dots_only(),
                                        500.0,
                                        300.0,
                                        &theme,
                                    ))
                                    .child(render_bars(
                                        &x_scale,
                                        &y_scale,
                                        &data,
                                        500.0,
                                        300.0,
                                        &BarConfig::new()
                                            .fill_color(scheme.color(1))
                                            .stroke_color(D3Color::from_hex(0x333333))
                                            .stroke_width(2.0)
                                            .border_radius(4.0)
                                            .opacity(0.9),
                                    ))
                            )
                    )
                    .child(div().ml(px(60.0)).child(render_axis(&x_scale, &AxisConfig::bottom().with_ticks(6), 500.0, &theme)))
            )
            // Mixed positive/negative values
            .child(
                div()
                    .flex()
                    .flex_col()
                    
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Mixed Positive/Negative Values").mb_2())
                    .child(
                        div()
                            .flex()
                            .child(render_axis(
                                &mixed_y_scale,
                                &AxisConfig::left()
                                    .with_ticks(7)
                                    .with_formatter(|v| {
                                        if v > 0.0 {
                                            format!("+{:.0}", v)
                                        } else {
                                            format!("{:.0}", v)
                                        }
                                    }),
                                300.0,
                                &theme
                            ))
                            .child(
                                div()
                                    .w(px(500.0))
                                    .h(px(300.0))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .border_1()
                                    .border_color(rgb(0xcccccc))
                                    .child(render_grid(
                                        &mixed_x_scale,
                                        &mixed_y_scale,
                                        &GridConfig::with_lines(),
                                        500.0,
                                        300.0,
                                        &theme,
                                    ))
                                    .child(render_bars(
                                        &mixed_x_scale,
                                        &mixed_y_scale,
                                        &mixed_data,
                                        500.0,
                                        300.0,
                                        &BarConfig::new()
                                            .fill_color(scheme.color(2))
                                            .bar_gap(4.0),
                                    ))
                            )
                    )
                    .child(div().ml(px(60.0)).child(render_axis(&mixed_x_scale, &AxisConfig::bottom().with_ticks(5), 500.0, &theme)))
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(900.0), px(1200.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("d3rs Bar Chart Demo".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(BarChartDemo::new),
        )
        .unwrap();

        cx.activate(true);
    });
}
