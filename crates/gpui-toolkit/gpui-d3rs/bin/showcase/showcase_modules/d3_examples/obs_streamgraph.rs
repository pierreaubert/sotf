//! Streamgraph -- Observable example using d3rs::examples::streamgraph
use crate::ShowcaseApp;
use d3rs::scale::{LinearScale, Scale};
use gpui::prelude::*;
use gpui::*;

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
    let (categories, matrix) = d3rs::examples::streamgraph::default_data();
    let result = d3rs::examples::streamgraph::compute(&categories, &matrix);

    let colors = [
        rgb(0x4e79a7),
        rgb(0xf28e2b),
        rgb(0xe15759),
        rgb(0x76b7b2),
        rgb(0x59a14f),
    ];

    let width = 700.0_f64;
    let height = 450.0_f64;
    let margin_left = 50.0_f64;
    let margin_top = 20.0_f64;
    let margin_bottom = 40.0_f64;
    let margin_right = 20.0_f64;
    let plot_w = width - margin_left - margin_right;
    let plot_h = height - margin_top - margin_bottom;
    let n = matrix.len();

    let x_scale = LinearScale::new()
        .domain(0.0, (n - 1) as f64)
        .range(0.0, plot_w);

    let y_scale = LinearScale::new()
        .domain(result.y_extent[0], result.y_extent[1])
        .range(plot_h, 0.0);

    let mut series_paths: Vec<String> = Vec::new();
    for s in &result.series {
        let mut path_d = String::new();
        // Top line (y1 values)
        for i in 0..n {
            if let Some(v) = s.get(i) {
                let x = x_scale.scale(i as f64);
                let y = y_scale.scale(v[1]);
                if i == 0 {
                    path_d.push_str(&format!("M {} {}", x, y));
                } else {
                    path_d.push_str(&format!(" L {} {}", x, y));
                }
            }
        }
        // Bottom line reversed (y0 values)
        for i in (0..n).rev() {
            if let Some(v) = s.get(i) {
                let x = x_scale.scale(i as f64);
                let y = y_scale.scale(v[0]);
                path_d.push_str(&format!(" L {} {}", x, y));
            }
        }
        path_d.push_str(" Z");
        series_paths.push(path_d);
    }

    let legend_items: Vec<Div> = result
        .categories
        .iter()
        .enumerate()
        .map(|(i, name)| {
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().size_3().bg(colors[i % colors.len()]))
                .child(div().text_xs().child(name.clone()))
        })
        .collect();

    // X-axis ticks (every 2 time steps)
    let x_ticks: Vec<f64> = (0..n).step_by(2).map(|v| v as f64).collect();

    // Y-axis ticks
    let y_range = result.y_extent[1] - result.y_extent[0];
    let y_step = (y_range / 5.0).ceil();
    let y_min_tick = (result.y_extent[0] / y_step).floor() * y_step;
    let y_ticks: Vec<f64> = (0..=8)
        .map(|i| y_min_tick + i as f64 * y_step)
        .filter(|v| *v >= result.y_extent[0] - 0.1 && *v <= result.y_extent[1] + 0.1)
        .collect();

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_4()
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::BOLD)
                .mb_2()
                .child("Streamgraph"),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x666666))
                .mb_2()
                .child("Source: observablehq.com/@d3/streamgraph"),
        )
        .child(div().flex().gap_4().mb_2().flex_wrap().children(legend_items))
        .child(
            div()
                .w(px(width as f32))
                .h(px(height as f32))
                .bg(rgb(0xffffff))
                .border_1()
                .border_color(rgb(0xcccccc))
                .relative()
                // Y-axis line
                .child(
                    div()
                        .absolute()
                        .left(px(margin_left as f32))
                        .top(px(margin_top as f32))
                        .w(px(1.0))
                        .h(px(plot_h as f32))
                        .bg(rgb(0xcccccc)),
                )
                // X-axis line
                .child(
                    div()
                        .absolute()
                        .left(px(margin_left as f32))
                        .top(px((margin_top + plot_h) as f32))
                        .w(px(plot_w as f32))
                        .h(px(1.0))
                        .bg(rgb(0xcccccc)),
                )
                // Y-axis tick labels
                .children(y_ticks.iter().map(|&val| {
                    let y = y_scale.scale(val);
                    div()
                        .absolute()
                        .left(px(0.0))
                        .top(px((margin_top + y - 6.0) as f32))
                        .w(px(margin_left as f32))
                        .flex()
                        .justify_end()
                        .pr_1()
                        .child(
                            div()
                                .text_color(rgb(0x888888))
                                .text_xs()
                                .child(format!("{:.0}", val)),
                        )
                }))
                // Y grid lines
                .children(y_ticks.iter().map(|&val| {
                    let y = y_scale.scale(val);
                    div()
                        .absolute()
                        .left(px(margin_left as f32))
                        .top(px((margin_top + y) as f32))
                        .w(px(plot_w as f32))
                        .h(px(1.0))
                        .bg(rgb(0xf0f0f0))
                }))
                // X-axis tick labels
                .children(x_ticks.iter().map(|&val| {
                    let x = x_scale.scale(val);
                    div()
                        .absolute()
                        .left(px((margin_left + x - 10.0) as f32))
                        .top(px((margin_top + plot_h + 4.0) as f32))
                        .w(px(20.0))
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .text_color(rgb(0x888888))
                                .text_xs()
                                .child(format!("{:.0}", val)),
                        )
                }))
                // Plot area with streamgraph
                .child(
                    div()
                        .absolute()
                        .left(px(margin_left as f32))
                        .top(px(margin_top as f32))
                        .w(px(plot_w as f32))
                        .h(px(plot_h as f32))
                        .child(
                            canvas(
                                move |bounds, _, _| {
                                    series_paths
                                        .iter()
                                        .map(|d| super::path_utils::parse_svg_path(d, bounds))
                                        .collect::<Vec<_>>()
                                },
                                move |_bounds, paths, window, _| {
                                    for (i, path_opt) in paths.into_iter().enumerate() {
                                        if let Some(path) = path_opt {
                                            window.paint_path(path, colors[i % colors.len()]);
                                        }
                                    }
                                },
                            )
                            .size_full(),
                        ),
                ),
        )
}
