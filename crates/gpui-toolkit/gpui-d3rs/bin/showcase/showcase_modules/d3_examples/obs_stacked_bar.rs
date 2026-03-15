//! Stacked Bar Chart -- Observable example using d3rs::examples::stacked_bar
use crate::ShowcaseApp;
use d3rs::scale::{LinearScale, Scale};
use gpui::prelude::*;
use gpui::*;

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
    let (states, ages, matrix) = d3rs::examples::stacked_bar::default_data();
    let result = d3rs::examples::stacked_bar::compute(&states, &ages, &matrix);

    let colors = [
        rgb(0x4e79a7),
        rgb(0xf28e2b),
        rgb(0xe15759),
        rgb(0x76b7b2),
        rgb(0x59a14f),
        rgb(0xedc948),
        rgb(0xb07aa1),
        rgb(0xff9da7),
    ];

    let width = 700.0_f64;
    let height = 450.0_f64;
    let margin_top = 20.0_f64;
    let margin_right = 20.0_f64;
    let margin_bottom = 40.0_f64;
    let margin_left = 50.0_f64;
    let chart_width = width - margin_left - margin_right;
    let chart_height = height - margin_top - margin_bottom;

    // Scale factors from compute's coordinate space to our chart
    let compute_chart_width = result.width - 50.0; // compute uses margin_left=40, margin_right=10
    let bar_scale_x = chart_width / compute_chart_width;
    let bw = result.bandwidth * bar_scale_x;

    let y_scale = LinearScale::new()
        .domain(result.y_domain[0], result.y_domain[1])
        .range(chart_height, 0.0);

    // Build rectangle paths for each series segment
    let mut rect_paths: Vec<String> = Vec::new();
    let mut rect_colors: Vec<Hsla> = Vec::new();
    for (si, series) in result.series.iter().enumerate() {
        for (gi, pos) in result.band_positions.iter().enumerate() {
            if let Some(v) = series.get(gi) {
                let x = (*pos - 40.0) * bar_scale_x; // subtract compute's margin_left
                let y0 = y_scale.scale(v[0]);
                let y1 = y_scale.scale(v[1]);
                let top = y0.min(y1);
                let bottom = y0.max(y1);
                let h = (bottom - top).max(0.5);
                let path_d = format!(
                    "M {} {} L {} {} L {} {} L {} {} Z",
                    x, top, x + bw, top, x + bw, top + h, x, top + h
                );
                rect_paths.push(path_d);
                rect_colors.push(colors[si % colors.len()].into());
            }
        }
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

    // Y-axis ticks
    let y_range = result.y_domain[1] - result.y_domain[0];
    let y_step = (y_range / 5.0).ceil();
    let y_min_tick = (result.y_domain[0] / y_step).floor() * y_step;
    let y_ticks: Vec<f64> = (0..=6)
        .map(|i| y_min_tick + i as f64 * y_step)
        .filter(|v| *v >= result.y_domain[0] - 0.1 && *v <= result.y_domain[1] + 0.1)
        .collect();

    // State labels along x-axis
    let state_labels: Vec<Div> = result
        .states
        .iter()
        .zip(result.band_positions.iter())
        .map(|(name, pos)| {
            let x = margin_left + (*pos - 40.0) * bar_scale_x;
            div()
                .absolute()
                .left(px(x as f32))
                .top(px((margin_top + chart_height + 4.0) as f32))
                .w(px(bw as f32))
                .text_xs()
                .child(name.clone())
        })
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
                .child("Stacked Bar Chart"),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x666666))
                .mb_2()
                .child("Source: observablehq.com/@d3/stacked-bar-chart"),
        )
        .child(div().flex().gap_3().mb_2().flex_wrap().children(legend_items))
        .child(
            div()
                .w(px(width as f32))
                .h(px(height as f32))
                .bg(rgb(0xffffff))
                .border_1()
                .border_color(rgb(0xcccccc))
                .relative()
                .children(state_labels)
                .child(
                    div()
                        .absolute()
                        .left(px(margin_left as f32))
                        .top(px(margin_top as f32))
                        .w(px(chart_width as f32))
                        .h(px(chart_height as f32))
                        .child(
                            canvas(
                                move |bounds, _, _| {
                                    rect_paths
                                        .iter()
                                        .map(|d| super::path_utils::parse_svg_path(d, bounds))
                                        .collect::<Vec<_>>()
                                },
                                move |_bounds, paths, window, _| {
                                    for (i, path_opt) in paths.into_iter().enumerate() {
                                        if let Some(path) = path_opt {
                                            window.paint_path(path, rect_colors[i]);
                                        }
                                    }
                                },
                            )
                            .size_full(),
                        ),
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
                        .child(div().text_color(rgb(0x888888)).text_xs().child(format!("{:.0}", val)))
                }))
                // Y grid lines
                .children(y_ticks.iter().map(|&val| {
                    let y = y_scale.scale(val);
                    div()
                        .absolute()
                        .left(px(margin_left as f32))
                        .top(px((margin_top + y) as f32))
                        .w(px(chart_width as f32))
                        .h(px(1.0))
                        .bg(rgb(0xf0f0f0))
                }))
                // X-axis line
                .child(
                    div()
                        .absolute()
                        .left(px(margin_left as f32))
                        .top(px((margin_top + chart_height) as f32))
                        .w(px(chart_width as f32))
                        .h(px(1.0))
                        .bg(rgb(0xcccccc)),
                )
                // Y-axis line
                .child(
                    div()
                        .absolute()
                        .left(px(margin_left as f32))
                        .top(px(margin_top as f32))
                        .w(px(1.0))
                        .h(px(chart_height as f32))
                        .bg(rgb(0x000000)),
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
                        .w(px(chart_width as f32))
                        .h(px(1.0))
                        .bg(rgb(0xf0f0f0))
                })),
        )
}
