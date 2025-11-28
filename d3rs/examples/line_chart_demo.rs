//! Line chart demonstration example
//!
//! This example demonstrates line chart rendering with different curve types.

use d3rs::prelude::*;
use d3rs::axis::{render_axis, AxisConfig, DefaultAxisTheme};
use d3rs::grid::{render_grid, GridConfig};
use gpui::*;

struct LineChartDemo {
    _unused: bool,
}

impl LineChartDemo {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self { _unused: false }
    }
}

impl Render for LineChartDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = DefaultAxisTheme;

        // Create scales
        let x_scale = LinearScale::new()
            .domain(0.0, 100.0)
            .range(0.0, 500.0);

        let y_scale = LinearScale::new()
            .domain(0.0, 100.0)
            .range(0.0, 300.0);

        // Sample data
        let data = vec![
            LinePoint::new(0.0, 20.0),
            LinePoint::new(20.0, 45.0),
            LinePoint::new(40.0, 35.0),
            LinePoint::new(60.0, 75.0),
            LinePoint::new(80.0, 60.0),
            LinePoint::new(100.0, 85.0),
        ];

        // Step data
        let step_data = vec![
            LinePoint::new(10.0, 30.0),
            LinePoint::new(30.0, 30.0),
            LinePoint::new(30.0, 60.0),
            LinePoint::new(50.0, 60.0),
            LinePoint::new(50.0, 40.0),
            LinePoint::new(70.0, 40.0),
            LinePoint::new(70.0, 80.0),
            LinePoint::new(90.0, 80.0),
        ];

        // Multiple series
        let series1 = vec![
            LinePoint::new(0.0, 25.0),
            LinePoint::new(25.0, 50.0),
            LinePoint::new(50.0, 40.0),
            LinePoint::new(75.0, 70.0),
            LinePoint::new(100.0, 65.0),
        ];

        let series2 = vec![
            LinePoint::new(0.0, 55.0),
            LinePoint::new(25.0, 30.0),
            LinePoint::new(50.0, 60.0),
            LinePoint::new(75.0, 45.0),
            LinePoint::new(100.0, 75.0),
        ];

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
                    .child("d3rs Line Chart Demonstration")
            )
            // Linear curve
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Linear Interpolation").mb_2())
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
                                    .child(render_line(
                                        &x_scale,
                                        &y_scale,
                                        &data,
                                        &LineConfig::new()
                                            .stroke_color(scheme.color(0))
                                            .curve(CurveType::Linear)
                                            .stroke_width(3.0),
                                    ))
                            )
                    )
                    .child(div().ml(px(60.0)).child(render_axis(&x_scale, &AxisConfig::bottom().with_ticks(5), 500.0, &theme)))
            )
            // Linear with points
            .child(
                div()
                    .flex()
                    .flex_col()
                    
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Linear with Points").mb_2())
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
                                    .child(render_line(
                                        &x_scale,
                                        &y_scale,
                                        &data,
                                        &LineConfig::new()
                                            .stroke_color(scheme.color(1))
                                            .curve(CurveType::Linear)
                                            .show_points(true)
                                            .point_radius(4.0)
                                            .point_fill_color(D3Color::from_hex(0xffffff)),
                                    ))
                            )
                    )
                    .child(div().ml(px(60.0)).child(render_axis(&x_scale, &AxisConfig::bottom().with_ticks(5), 500.0, &theme)))
            )
            // Step curve
            .child(
                div()
                    .flex()
                    .flex_col()
                    
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Step Interpolation").mb_2())
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
                                        &GridConfig::with_lines(),
                                        500.0,
                                        300.0,
                                        &theme,
                                    ))
                                    .child(render_line(
                                        &x_scale,
                                        &y_scale,
                                        &step_data,
                                        &LineConfig::new()
                                            .stroke_color(scheme.color(2))
                                            .curve(CurveType::Step)
                                            .stroke_width(2.5),
                                    ))
                            )
                    )
                    .child(div().ml(px(60.0)).child(render_axis(&x_scale, &AxisConfig::bottom().with_ticks(5), 500.0, &theme)))
            )
            // Multiple series
            .child(
                div()
                    .flex()
                    .flex_col()
                    
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Multiple Series").mb_2())
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
                                    // Series 1
                                    .child(render_line(
                                        &x_scale,
                                        &y_scale,
                                        &series1,
                                        &LineConfig::new()
                                            .stroke_color(scheme.color(4))
                                            .curve(CurveType::Linear)
                                            .show_points(true)
                                            .point_radius(4.0),
                                    ))
                                    // Series 2
                                    .child(render_line(
                                        &x_scale,
                                        &y_scale,
                                        &series2,
                                        &LineConfig::new()
                                            .stroke_color(scheme.color(6))
                                            .curve(CurveType::Linear)
                                            .show_points(true)
                                            .point_radius(4.0),
                                    ))
                            )
                    )
                    .child(div().ml(px(60.0)).child(render_axis(&x_scale, &AxisConfig::bottom().with_ticks(5), 500.0, &theme)))
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(900.0), px(1400.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("d3rs Line Chart Demo".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(LineChartDemo::new),
        )
        .unwrap();

        cx.activate(true);
    });
}
