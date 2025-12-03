use d3rs::axis::{AxisConfig, DefaultAxisTheme, render_axis};
use d3rs::color::ColorScheme;
use d3rs::grid::{GridConfig, render_grid};
use d3rs::prelude::*;
use gpui::*;

pub fn render(app: &ShowcaseApp) -> Div {
    let theme = DefaultAxisTheme;
    let x_scale = LinearScale::new().domain(0.0, 6.0).range(0.0, 500.0);
    let y_scale = LinearScale::new().domain(0.0, 100.0).range(0.0, 250.0);
    let scheme = ColorScheme::category10();

    let data = vec![
        BarDatum::new("Mon", 45.0),
        BarDatum::new("Tue", 68.0),
        BarDatum::new("Wed", 55.0),
        BarDatum::new("Thu", 82.0),
        BarDatum::new("Fri", 70.0),
        BarDatum::new("Sat", 38.0),
    ];

    let mixed_data = vec![
        BarDatum::new("A", 30.0),
        BarDatum::new("B", -15.0),
        BarDatum::new("C", 45.0),
        BarDatum::new("D", -25.0),
        BarDatum::new("E", 60.0),
    ];
    let mixed_y_scale = LinearScale::new().domain(-30.0, 70.0).range(0.0, 250.0);
    let mixed_x_scale = LinearScale::new().domain(0.0, 5.0).range(0.0, 500.0);

    div()
        .flex()
        .flex_col()
        .gap_6()
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .child("Bar Charts Demo"),
        )
        // Simple bar chart
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Simple Bar Chart"),
                )
                .child(
                    div()
                        .flex()
                        .child(render_axis(
                            &y_scale,
                            &AxisConfig::left().with_ticks(5),
                            250.0,
                            &theme,
                        ))
                        .child(
                            div()
                                .w(px(500.0))
                                .h(px(250.0))
                                .relative()
                                .bg(rgb(0xf8f8f8))
                                .border_1()
                                .border_color(rgb(0xcccccc))
                                .child(render_grid(
                                    &x_scale,
                                    &y_scale,
                                    &GridConfig::lines_only().with_line_opacity(0.2),
                                    500.0,
                                    250.0,
                                    &theme,
                                ))
                                .child(render_bars(
                                    &x_scale,
                                    &y_scale,
                                    &data,
                                    500.0,
                                    250.0,
                                    &BarConfig::new().fill_color(scheme.color(0)).opacity(0.85),
                                )),
                        ),
                )
                .child(div().ml(px(60.0)).child(render_axis(
                    &x_scale,
                    &AxisConfig::bottom().with_ticks(6),
                    500.0,
                    &theme,
                ))),
        )
        // Mixed positive/negative
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Mixed Positive/Negative Values"),
                )
                .child(
                    div()
                        .flex()
                        .child(render_axis(
                            &mixed_y_scale,
                            &AxisConfig::left().with_ticks(7).with_formatter(|v| {
                                if v > 0.0 {
                                    format!("+{:.0}", v)
                                } else {
                                    format!("{:.0}", v)
                                }
                            }),
                            250.0,
                            &theme,
                        ))
                        .child(
                            div()
                                .w(px(500.0))
                                .h(px(250.0))
                                .relative()
                                .bg(rgb(0xf8f8f8))
                                .border_1()
                                .border_color(rgb(0xcccccc))
                                .child(render_grid(
                                    &mixed_x_scale,
                                    &mixed_y_scale,
                                    &GridConfig::with_lines(),
                                    500.0,
                                    250.0,
                                    &theme,
                                ))
                                .child(render_bars(
                                    &mixed_x_scale,
                                    &mixed_y_scale,
                                    &mixed_data,
                                    500.0,
                                    250.0,
                                    &BarConfig::new().fill_color(scheme.color(2)).bar_gap(4.0),
                                )),
                        ),
                )
                .child(div().ml(px(60.0)).child(render_axis(
                    &mixed_x_scale,
                    &AxisConfig::bottom().with_ticks(5),
                    500.0,
                    &theme,
                ))),
        )
}

use super::ShowcaseApp;
