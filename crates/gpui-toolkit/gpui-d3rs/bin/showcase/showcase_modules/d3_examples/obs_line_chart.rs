//! Line Chart -- Observable example using d3rs::examples::line_chart
//!
//! Renders multiple curve interpolations with axes and tick labels.
use crate::ShowcaseApp;
use d3rs::scale::{LinearScale, Scale};
use gpui::prelude::*;
use gpui::*;

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
    let data = d3rs::examples::line_chart::default_data();
    let result = d3rs::examples::line_chart::compute(&data);

    let tableau10: [Rgba; 7] = [
        rgb(0x4e79a7),
        rgb(0xf28e2b),
        rgb(0xe15759),
        rgb(0x76b7b2),
        rgb(0x59a14f),
        rgb(0xedc948),
        rgb(0xb07aa1),
    ];

    let chart_w = 700.0_f64;
    let chart_h = 400.0_f64;
    let margin_left = 50.0;
    let margin_right = 20.0;
    let margin_top = 20.0;
    let margin_bottom = 40.0;
    let plot_w = chart_w - margin_left - margin_right;
    let plot_h = chart_h - margin_top - margin_bottom;

    // Scales mapping data domain to plot area
    let x_scale = LinearScale::new()
        .domain(result.x_domain[0], result.x_domain[1])
        .range(0.0, plot_w);
    let y_scale = LinearScale::new()
        .domain(result.y_domain[0], result.y_domain[1])
        .range(plot_h, 0.0);

    // Rebuild line paths in plot coordinates (result.paths are in compute's internal coords)
    // We re-project the raw data through our local scales for clean mapping
    let curves: Vec<(&str, d3rs::shape::curve::Curve)> = vec![
        ("linear", d3rs::shape::curve::Curve::linear()),
        ("step", d3rs::shape::curve::Curve::Step),
        ("basis", d3rs::shape::curve::Curve::basis()),
        ("cardinal", d3rs::shape::curve::Curve::cardinal(0.0)),
        ("natural", d3rs::shape::curve::Curve::natural()),
        ("monotoneX", d3rs::shape::curve::Curve::monotone_x()),
        ("catmullRom", d3rs::shape::curve::Curve::catmull_rom(0.5)),
    ];

    let points: Vec<d3rs::shape::path::Point> = data
        .iter()
        .map(|(x, y)| {
            d3rs::shape::path::Point::new(x_scale.scale(*x), y_scale.scale(*y))
        })
        .collect();

    let mut ribbon_paths: Vec<String> = Vec::new();
    let mut curve_names: Vec<String> = Vec::new();
    for (name, curve) in &curves {
        let interpolated = curve.interpolate(&points);
        let ribbon = points_to_ribbon(&interpolated, 1.8);
        ribbon_paths.push(ribbon);
        curve_names.push(name.to_string());
    }

    // Legend
    let legend_items: Vec<Div> = curve_names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().size_3().bg(tableau10[i % tableau10.len()]))
                .child(div().text_xs().child(name.clone()))
        })
        .collect();

    // X-axis ticks (every 5 days)
    let x_ticks: Vec<f64> = (0..=30).step_by(5).map(|v| v as f64).collect();
    // Y-axis ticks
    let y_range = result.y_domain[1] - result.y_domain[0];
    let y_step = (y_range / 5.0).ceil();
    let y_min_tick = (result.y_domain[0] / y_step).floor() * y_step;
    let y_ticks: Vec<f64> = (0..=6)
        .map(|i| y_min_tick + i as f64 * y_step)
        .filter(|v| *v >= result.y_domain[0] - 0.1 && *v <= result.y_domain[1] + 0.1)
        .collect();

    let colors = tableau10;
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
                .child("Line Chart — 7 Curve Types"),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x666666))
                .mb_2()
                .child("Source: observablehq.com/@d3/line-chart"),
        )
        .child(div().flex().gap_3().mb_3().flex_wrap().children(legend_items))
        .child(
            div()
                .w(px(chart_w as f32))
                .h(px(chart_h as f32))
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
                // Y-axis ticks + labels + grid
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
                // X-axis ticks + labels
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
                // X grid lines
                .children(x_ticks.iter().map(|&val| {
                    let x = x_scale.scale(val);
                    div()
                        .absolute()
                        .left(px((margin_left + x) as f32))
                        .top(px(margin_top as f32))
                        .w(px(1.0))
                        .h(px(plot_h as f32))
                        .bg(rgb(0xf0f0f0))
                }))
                // Plot area with curves
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
                                    ribbon_paths
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
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x999999))
                .mt_2()
                .child(format!(
                    "{} data points • x: [{:.0}..{:.0}] • y: [{:.1}..{:.1}]",
                    data.len(),
                    result.x_domain[0],
                    result.x_domain[1],
                    result.y_domain[0],
                    result.y_domain[1]
                )),
        )
}

/// Convert interpolated points to a thin filled ribbon to simulate a stroke.
fn points_to_ribbon(points: &[d3rs::shape::path::Point], thickness: f64) -> String {
    if points.len() < 2 {
        return String::new();
    }

    let half = thickness / 2.0;
    let mut upper = Vec::with_capacity(points.len());
    let mut lower = Vec::with_capacity(points.len());
    for i in 0..points.len() {
        let (dx, dy) = if i == 0 {
            (points[1].x - points[0].x, points[1].y - points[0].y)
        } else if i == points.len() - 1 {
            (
                points[i].x - points[i - 1].x,
                points[i].y - points[i - 1].y,
            )
        } else {
            (
                points[i + 1].x - points[i - 1].x,
                points[i + 1].y - points[i - 1].y,
            )
        };
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let nx = -dy / len * half;
        let ny = dx / len * half;
        upper.push((points[i].x + nx, points[i].y + ny));
        lower.push((points[i].x - nx, points[i].y - ny));
    }

    let mut path_d = format!("M {:.2} {:.2}", upper[0].0, upper[0].1);
    for p in upper.iter().skip(1) {
        path_d.push_str(&format!(" L {:.2} {:.2}", p.0, p.1));
    }
    for p in lower.iter().rev() {
        path_d.push_str(&format!(" L {:.2} {:.2}", p.0, p.1));
    }
    path_d.push_str(" Z");
    path_d
}
