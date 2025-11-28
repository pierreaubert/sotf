//! Scatter plot demonstration example
//!
//! This example demonstrates scatter plot rendering with different configurations.

use d3rs::prelude::*;
use d3rs::axis::{render_axis, AxisConfig, DefaultAxisTheme};
use d3rs::grid::{render_grid, GridConfig};
use gpui::*;

struct ScatterDemo {
    _unused: bool,
}

impl ScatterDemo {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self { _unused: false }
    }
}

impl Render for ScatterDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = DefaultAxisTheme;

        // Create scales
        let x_scale = LinearScale::new()
            .domain(0.0, 100.0)
            .range(0.0, 500.0);

        let y_scale = LinearScale::new()
            .domain(0.0, 100.0)
            .range(0.0, 300.0);

        // Sample data - random-ish points
        let data1 = vec![
            ScatterPoint::new(10.0, 20.0),
            ScatterPoint::new(25.0, 45.0),
            ScatterPoint::new(35.0, 30.0),
            ScatterPoint::new(50.0, 75.0),
            ScatterPoint::new(65.0, 55.0),
            ScatterPoint::new(75.0, 85.0),
            ScatterPoint::new(85.0, 65.0),
            ScatterPoint::new(90.0, 90.0),
        ];

        // Clustered data
        let cluster1: Vec<_> = (0..15)
            .map(|i| {
                let angle = i as f64 * 0.4;
                ScatterPoint::new(30.0 + angle.cos() * 15.0, 30.0 + angle.sin() * 15.0)
            })
            .collect();

        let cluster2: Vec<_> = (0..15)
            .map(|i| {
                let angle = i as f64 * 0.5;
                ScatterPoint::new(70.0 + angle.cos() * 12.0, 70.0 + angle.sin() * 12.0)
            })
            .collect();

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
                    .child("d3rs Scatter Plot Demonstration")
            )
            // Simple scatter plot
            .child(
                div()
                    .flex()
                    .flex_col()
                    
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Simple Scatter Plot").mb_2())
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
                                    .child(render_scatter(
                                        &x_scale,
                                        &y_scale,
                                        &data1,
                                        &ScatterConfig::new()
                                            .fill_color(scheme.color(0))
                                            .point_radius(6.0)
                                            .opacity(0.8),
                                    ))
                            )
                    )
                    .child(div().ml(px(60.0)).child(render_axis(&x_scale, &AxisConfig::bottom().with_ticks(5), 500.0, &theme)))
            )
            // Styled points (no stroke)
            .child(
                div()
                    .flex()
                    .flex_col()
                    
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Styled Points (no stroke, larger)").mb_2())
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
                                    .child(render_scatter(
                                        &x_scale,
                                        &y_scale,
                                        &data1,
                                        &ScatterConfig::new()
                                            .fill_color(scheme.color(3))
                                            .point_radius(8.0)
                                            .no_stroke()
                                            .opacity(0.6),
                                    ))
                            )
                    )
                    .child(div().ml(px(60.0)).child(render_axis(&x_scale, &AxisConfig::bottom().with_ticks(5), 500.0, &theme)))
            )
            // Multiple series (clusters)
            .child(
                div()
                    .flex()
                    .flex_col()
                    
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Multiple Series (2 clusters)").mb_2())
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
                                    // Cluster 1
                                    .child(render_scatter(
                                        &x_scale,
                                        &y_scale,
                                        &cluster1,
                                        &ScatterConfig::new()
                                            .fill_color(scheme.color(4))
                                            .point_radius(5.0)
                                            .stroke_color(D3Color::from_hex(0xffffff))
                                            .stroke_width(1.5),
                                    ))
                                    // Cluster 2
                                    .child(render_scatter(
                                        &x_scale,
                                        &y_scale,
                                        &cluster2,
                                        &ScatterConfig::new()
                                            .fill_color(scheme.color(6))
                                            .point_radius(5.0)
                                            .stroke_color(D3Color::from_hex(0xffffff))
                                            .stroke_width(1.5),
                                    ))
                            )
                    )
                    .child(div().ml(px(60.0)).child(render_axis(&x_scale, &AxisConfig::bottom().with_ticks(5), 500.0, &theme)))
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
                    title: Some("d3rs Scatter Plot Demo".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(ScatterDemo::new),
        )
        .unwrap();

        cx.activate(true);
    });
}
