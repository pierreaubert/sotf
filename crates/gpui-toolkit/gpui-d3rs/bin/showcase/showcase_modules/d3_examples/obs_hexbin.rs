//! Hexbin Chart -- Observable example using d3rs::examples::hexbin
//!
//! Renders hexagonal bins colored by density, matching the Observable @d3/hexbin example.
use crate::ShowcaseApp;
use d3rs::scale::{LogScale, Scale};
use gpui::prelude::*;
use gpui::*;

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
    // Generate diamond-like (carat, price) data — same distribution as golden test
    let data: Vec<(f64, f64)> = (0..200)
        .map(|i| {
            let t = i as f64 / 199.0;
            let carat = 0.2 + t * t * 4.8;
            let base_price = 326.0 * (carat / 0.2_f64).powf(1.8);
            let noise =
                1.0 + 0.3 * (i as f64 * 7.3).sin() + 0.15 * (i as f64 * 13.1).cos();
            let price = (base_price * noise).clamp(326.0, 18823.0);
            (carat, price)
        })
        .collect();

    let result = d3rs::examples::hexbin::compute(&data);

    let width = 700.0_f64;
    let height = 700.0_f64;
    let margin_left = 60.0_f64;
    let margin_top = 20.0_f64;
    let margin_bottom = 40.0_f64;
    let margin_right = 20.0_f64;
    let plot_w = width - margin_left - margin_right;
    let plot_h = height - margin_top - margin_bottom;

    // Log scales mapping data domain to plot area
    let x_scale = LogScale::new()
        .domain(result.x_domain[0].max(0.1), result.x_domain[1])
        .range(0.0, plot_w);
    let y_scale = LogScale::new()
        .domain(result.y_domain[0].max(100.0), result.y_domain[1])
        .range(plot_h, 0.0);

    // Rescale hex bins from compute's coordinate space (928x928) to our plot area
    let compute_margin_left = 40.0;
    let compute_margin_top = 20.0;
    let compute_chart_w = result.width - compute_margin_left - 20.0;
    let compute_chart_h = result.height - compute_margin_top - 20.0;
    let sx = plot_w / compute_chart_w;
    let sy = plot_h / compute_chart_h;

    let max_count = result.bins.iter().map(|b| b.count).max().unwrap_or(1);
    let hex_r = result.hex_radius * sx.min(sy);

    // Build a hexagon path for each bin (pointy-top like D3)
    let mut hex_paths: Vec<String> = Vec::new();
    let mut hex_colors: Vec<Hsla> = Vec::new();
    for bin in &result.bins {
        let cx = (bin.x - compute_margin_left) * sx;
        let cy = (bin.y - compute_margin_top) * sy;

        let mut path_d = String::new();
        for v in 0..6 {
            let angle = std::f64::consts::PI / 3.0 * v as f64 - std::f64::consts::FRAC_PI_2;
            let px_val = cx + hex_r * angle.cos();
            let py_val = cy + hex_r * angle.sin();
            if v == 0 {
                path_d.push_str(&format!("M {:.1} {:.1}", px_val, py_val));
            } else {
                path_d.push_str(&format!(" L {:.1} {:.1}", px_val, py_val));
            }
        }
        path_d.push_str(" Z");
        hex_paths.push(path_d);

        // Color: interpolateBuPu-style — light for few, deep purple for many
        let t = bin.count as f32 / max_count as f32;
        let color = hsla(0.7 - t * 0.15, 0.5 + t * 0.3, 0.88 - t * 0.50, 1.0);
        hex_colors.push(color);
    }

    let bin_count = result.bins.len();
    let data_count = data.len();

    // Log-scale friendly ticks
    let x_ticks: Vec<f64> = vec![0.2, 0.5, 1.0, 2.0, 5.0];
    let y_ticks: Vec<f64> = vec![500.0, 1000.0, 2000.0, 5000.0, 10000.0];

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
                .child("Hexbin Chart"),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x666666))
                .mb_2()
                .child("Source: observablehq.com/@d3/hexbin"),
        )
        .child(
            div()
                .flex()
                .gap_4()
                .mb_2()
                .child(
                    div().flex().items_center().gap_1()
                        .child(div().size_3().bg(hsla(0.68, 0.55, 0.85, 1.0)))
                        .child(div().text_xs().child("Few points")),
                )
                .child(
                    div().flex().items_center().gap_1()
                        .child(div().size_3().bg(hsla(0.55, 0.80, 0.38, 1.0)))
                        .child(div().text_xs().child("Many points")),
                )
                .child(
                    div().text_xs().text_color(rgb(0x999999))
                        .child(format!("{} points → {} bins", data_count, bin_count)),
                ),
        )
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
                        .left(px((margin_left + x - 15.0) as f32))
                        .top(px((margin_top + plot_h + 4.0) as f32))
                        .w(px(30.0))
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .text_color(rgb(0x888888))
                                .text_xs()
                                .child(if val < 1.0 {
                                    format!("{:.1}", val)
                                } else {
                                    format!("{:.0}", val)
                                }),
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
                // Plot area with hexbin
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
                                    hex_paths
                                        .iter()
                                        .map(|d| super::path_utils::parse_svg_path(d, bounds))
                                        .collect::<Vec<_>>()
                                },
                                move |_bounds, paths, window, _| {
                                    for (i, path_opt) in paths.into_iter().enumerate() {
                                        if let Some(path) = path_opt {
                                            window.paint_path(path, hex_colors[i]);
                                        }
                                    }
                                },
                            )
                            .size_full(),
                        ),
                ),
        )
}
