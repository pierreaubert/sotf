//! Grid demonstration example
//!
//! This example demonstrates grid rendering with different configurations.

use d3rs::prelude::*;
use d3rs::axis::{render_axis, AxisConfig, DefaultAxisTheme};
use d3rs::grid::{render_grid, GridConfig};
use gpui::*;

struct GridDemo {
    _unused: bool,
}

impl GridDemo {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self { _unused: false }
    }
}

impl Render for GridDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = DefaultAxisTheme;

        // Create scales
        let x_scale = LinearScale::new()
            .domain(0.0, 100.0)
            .range(0.0, 400.0);

        let y_scale = LinearScale::new()
            .domain(0.0, 100.0)
            .range(0.0, 300.0);

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
                    .child("d3rs Grid Demonstration")
            )
            // Grid with dots only
            .child(
                div()
                    .flex()
                    .flex_col()
                    
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Grid with Dots Only").mb_2())
                    .child(
                        div()
                            .flex()
                            .child(render_axis(&y_scale, &AxisConfig::left().with_ticks(10), 300.0, &theme))
                            .child(
                                div()
                                    .w(px(400.0))
                                    .h(px(300.0))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .border_1()
                                    .border_color(rgb(0xcccccc))
                                    .child(render_grid(
                                        &x_scale,
                                        &y_scale,
                                        &GridConfig::dots_only(),
                                        400.0,
                                        300.0,
                                        &theme,
                                    ))
                            )
                    )
                    .child(div().ml(px(60.0)).child(render_axis(&x_scale, &AxisConfig::bottom().with_ticks(10), 400.0, &theme)))
            )
            // Grid with lines and dots
            .child(
                div()
                    .flex()
                    .flex_col()
                    
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Grid with Lines and Dots").mb_2())
                    .child(
                        div()
                            .flex()
                            .child(render_axis(&y_scale, &AxisConfig::left().with_ticks(10), 300.0, &theme))
                            .child(
                                div()
                                    .w(px(400.0))
                                    .h(px(300.0))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .border_1()
                                    .border_color(rgb(0xcccccc))
                                    .child(render_grid(
                                        &x_scale,
                                        &y_scale,
                                        &GridConfig::with_lines(),
                                        400.0,
                                        300.0,
                                        &theme,
                                    ))
                            )
                    )
                    .child(div().ml(px(60.0)).child(render_axis(&x_scale, &AxisConfig::bottom().with_ticks(10), 400.0, &theme)))
            )
            // Grid with lines only
            .child(
                div()
                    .flex()
                    .flex_col()
                    
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Grid with Lines Only").mb_2())
                    .child(
                        div()
                            .flex()
                            .child(render_axis(&y_scale, &AxisConfig::left().with_ticks(10), 300.0, &theme))
                            .child(
                                div()
                                    .w(px(400.0))
                                    .h(px(300.0))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .border_1()
                                    .border_color(rgb(0xcccccc))
                                    .child(render_grid(
                                        &x_scale,
                                        &y_scale,
                                        &GridConfig::lines_only()
                                            .with_line_opacity(0.3),
                                        400.0,
                                        300.0,
                                        &theme,
                                    ))
                            )
                    )
                    .child(div().ml(px(60.0)).child(render_axis(&x_scale, &AxisConfig::bottom().with_ticks(10), 400.0, &theme)))
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(900.0), px(1100.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("d3rs Grid Demo".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(GridDemo::new),
        )
        .unwrap();

        cx.activate(true);
    });
}
