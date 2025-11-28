//! Axis demonstration example
//!
//! This example demonstrates axis rendering in all four orientations with various formatters.

use d3rs::prelude::*;
use d3rs::axis::{render_axis, AxisConfig, DefaultAxisTheme};
use gpui::*;

struct AxisDemo {
    _unused: bool,
}

impl AxisDemo {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self { _unused: false }
    }
}

impl Render for AxisDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = DefaultAxisTheme;

        // Create scales for demonstration
        let x_scale = LinearScale::new()
            .domain(0.0, 100.0)
            .range(0.0, 400.0);

        let freq_scale = LogScale::new()
            .domain(20.0, 20000.0)
            .range(0.0, 400.0);

        let db_scale = LinearScale::new()
            .domain(-24.0, 24.0)
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
                    .child("d3rs Axis Demonstration")
            )
            // Bottom axis (linear)
            .child(
                div()
                    .flex()
                    .flex_col()
                    
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Bottom Axis (Linear 0-100)").mb_2())
                    .child(
                        div()
                            .child(render_axis(
                                &x_scale,
                                &AxisConfig::bottom().with_ticks(10),
                                400.0,
                                &theme,
                            ))
                    )
            )
            // Top axis (logarithmic with formatter)
            .child(
                div()
                    .flex()
                    .flex_col()
                    
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Top Axis (Log 20Hz-20kHz)").mb_2())
                    .child(
                        div()
                            .child(render_axis(
                                &freq_scale,
                                &AxisConfig::top()
                                    .with_ticks(10)
                                    .with_formatter(|f| {
                                        if f >= 1000.0 {
                                            format!("{:.0}k", f / 1000.0)
                                        } else {
                                            format!("{:.0}", f)
                                        }
                                    }),
                                400.0,
                                &theme,
                            ))
                    )
            )
            // Left and right axes side by side
            .child(
                div()
                    .flex()
                    .flex_col()
                    
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Left & Right Axes (dB scale -24 to +24)").mb_2())
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            // Left axis
                            .child(
                                div()
                                    .child(render_axis(
                                        &db_scale,
                                        &AxisConfig::left()
                                            .with_ticks(9)
                                            .with_formatter(|db| {
                                                if db > 0.0 {
                                                    format!("+{:.0}", db)
                                                } else {
                                                    format!("{:.0}", db)
                                                }
                                            }),
                                        300.0,
                                        &theme,
                                    ))
                            )
                            // Spacer
                            .child(div().w(px(200.0)).h(px(300.0)).bg(rgb(0xf0f0f0)).rounded_md())
                            // Right axis
                            .child(
                                div()
                                    .child(render_axis(
                                        &db_scale,
                                        &AxisConfig::right().with_ticks(9),
                                        300.0,
                                        &theme,
                                    ))
                            )
                    )
            )
            // Custom tick sizes and styling
            .child(
                div()
                    .flex()
                    .flex_col()
                    
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Custom Styling (larger ticks, no domain line)").mb_2())
                    .child(
                        div()
                            .child(render_axis(
                                &x_scale,
                                &AxisConfig::bottom()
                                    .with_ticks(5)
                                    .with_tick_size(12.0)
                                    .with_tick_padding(8.0)
                                    .hide_domain_line(),
                                400.0,
                                &theme,
                            ))
                    )
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(900.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("d3rs Axis Demo".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(AxisDemo::new),
        )
        .unwrap();

        cx.activate(true);
    });
}
