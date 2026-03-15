//! Box Plot -- Observable example using d3rs::examples::box_plot
use crate::ShowcaseApp;
use d3rs::scale::{LinearScale, Scale};
use gpui::prelude::*;
use gpui::*;

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
    let data = d3rs::examples::box_plot::default_data();
    let result = d3rs::examples::box_plot::compute(&data);

    let box_color = rgb(0x4e79a7);
    let median_color = rgb(0xe15759);
    let whisker_color = rgb(0x333333);
    let outlier_color = rgb(0xf28e2b);

    let width = 700.0_f64;
    let height = 450.0_f64;
    let margin_top = 20.0_f64;
    let margin_bottom = 40.0_f64;
    let margin_left = 50.0_f64;
    let margin_right = 20.0_f64;
    let chart_width = width - margin_left - margin_right;
    let chart_height = height - margin_top - margin_bottom;

    // Scale from compute's coordinate space
    let compute_margin_left = 40.0;
    let compute_chart_width = result.width - compute_margin_left - 20.0;
    let bar_scale_x = chart_width / compute_chart_width;
    let bw = result.bandwidth * bar_scale_x;

    let y_scale = LinearScale::new()
        .domain(result.y_domain[0] - 5.0, result.y_domain[1] + 5.0)
        .range(chart_height, 0.0);

    // Build all the geometry as paths
    let mut all_paths: Vec<String> = Vec::new();
    let mut all_colors: Vec<Hsla> = Vec::new();

    for (gi, group) in result.groups.iter().enumerate() {
        let band_x = (result.band_positions[gi] - compute_margin_left) * bar_scale_x;
        let box_x = band_x + bw * 0.15;
        let box_w = bw * 0.7;

        // Box rectangle (q1 to q3)
        let q1_y = y_scale.scale(group.q1);
        let q3_y = y_scale.scale(group.q3);
        let top = q1_y.min(q3_y);
        let bottom = q1_y.max(q3_y);
        let box_h = (bottom - top).max(1.0);
        all_paths.push(format!(
            "M {} {} L {} {} L {} {} L {} {} Z",
            box_x, top,
            box_x + box_w, top,
            box_x + box_w, top + box_h,
            box_x, top + box_h
        ));
        all_colors.push(box_color.into());

        // Median line
        let med_y = y_scale.scale(group.median);
        let line_h = 2.0;
        all_paths.push(format!(
            "M {} {} L {} {} L {} {} L {} {} Z",
            box_x, med_y - line_h / 2.0,
            box_x + box_w, med_y - line_h / 2.0,
            box_x + box_w, med_y + line_h / 2.0,
            box_x, med_y + line_h / 2.0
        ));
        all_colors.push(median_color.into());

        let whisker_x = band_x + bw * 0.5;
        let whisker_w = 1.5;

        // Whisker: low vertical line
        let wl_y = y_scale.scale(group.whisker_low);
        let wl_top = wl_y.min(bottom);
        let wl_bottom = wl_y.max(bottom);
        all_paths.push(format!(
            "M {} {} L {} {} L {} {} L {} {} Z",
            whisker_x - whisker_w / 2.0, wl_top,
            whisker_x + whisker_w / 2.0, wl_top,
            whisker_x + whisker_w / 2.0, wl_bottom,
            whisker_x - whisker_w / 2.0, wl_bottom
        ));
        all_colors.push(whisker_color.into());

        // Whisker: high vertical line
        let wh_y = y_scale.scale(group.whisker_high);
        let wh_top = wh_y.min(top);
        let wh_bottom = wh_y.max(top);
        all_paths.push(format!(
            "M {} {} L {} {} L {} {} L {} {} Z",
            whisker_x - whisker_w / 2.0, wh_top,
            whisker_x + whisker_w / 2.0, wh_top,
            whisker_x + whisker_w / 2.0, wh_bottom,
            whisker_x - whisker_w / 2.0, wh_bottom
        ));
        all_colors.push(whisker_color.into());

        // Whisker caps (horizontal lines)
        let cap_w = bw * 0.3;
        let cap_h = 1.5;
        // Low cap
        all_paths.push(format!(
            "M {} {} L {} {} L {} {} L {} {} Z",
            whisker_x - cap_w / 2.0, wl_y - cap_h / 2.0,
            whisker_x + cap_w / 2.0, wl_y - cap_h / 2.0,
            whisker_x + cap_w / 2.0, wl_y + cap_h / 2.0,
            whisker_x - cap_w / 2.0, wl_y + cap_h / 2.0
        ));
        all_colors.push(whisker_color.into());
        // High cap
        all_paths.push(format!(
            "M {} {} L {} {} L {} {} L {} {} Z",
            whisker_x - cap_w / 2.0, wh_y - cap_h / 2.0,
            whisker_x + cap_w / 2.0, wh_y - cap_h / 2.0,
            whisker_x + cap_w / 2.0, wh_y + cap_h / 2.0,
            whisker_x - cap_w / 2.0, wh_y + cap_h / 2.0
        ));
        all_colors.push(whisker_color.into());

        // Outlier dots (small diamonds)
        for &val in &group.outliers {
            let oy = y_scale.scale(val);
            let dot_r = 3.0;
            all_paths.push(format!(
                "M {} {} L {} {} L {} {} L {} {} Z",
                whisker_x, oy - dot_r,
                whisker_x + dot_r, oy,
                whisker_x, oy + dot_r,
                whisker_x - dot_r, oy
            ));
            all_colors.push(outlier_color.into());
        }
    }

    // Y-axis ticks
    let y_min = result.y_domain[0] - 5.0;
    let y_max = result.y_domain[1] + 5.0;
    let y_range = y_max - y_min;
    let y_step = (y_range / 6.0).ceil();
    let y_min_tick = (y_min / y_step).floor() * y_step;
    let y_ticks: Vec<f64> = (0..=8)
        .map(|i| y_min_tick + i as f64 * y_step)
        .filter(|v| *v >= y_min - 0.1 && *v <= y_max + 0.1)
        .collect();

    // Group labels
    let group_labels: Vec<Div> = result
        .groups
        .iter()
        .enumerate()
        .map(|(gi, g)| {
            let band_x =
                margin_left + (result.band_positions[gi] - compute_margin_left) * bar_scale_x;
            div()
                .absolute()
                .left(px(band_x as f32))
                .top(px((margin_top + chart_height + 4.0) as f32))
                .w(px(bw as f32))
                .flex()
                .justify_center()
                .text_xs()
                .child(format!("Group {}", g.group))
        })
        .collect();

    let legend_items = vec![
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(div().size_3().bg(box_color))
            .child(div().text_xs().child("IQR (Q1-Q3)")),
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(div().size_3().bg(median_color))
            .child(div().text_xs().child("Median")),
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(div().size_3().bg(outlier_color))
            .child(div().text_xs().child("Outliers")),
    ];

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
                .child("Box Plot"),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x666666))
                .mb_2()
                .child("Source: observablehq.com/@d3/box-plot"),
        )
        .child(div().flex().gap_4().mb_2().children(legend_items))
        .child(
            div()
                .w(px(width as f32))
                .h(px(height as f32))
                .bg(rgb(0xffffff))
                .border_1()
                .border_color(rgb(0xcccccc))
                .relative()
                .children(group_labels)
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
                                    all_paths
                                        .iter()
                                        .map(|d| super::path_utils::parse_svg_path(d, bounds))
                                        .collect::<Vec<_>>()
                                },
                                move |_bounds, paths, window, _| {
                                    for (i, path_opt) in paths.into_iter().enumerate() {
                                        if let Some(path) = path_opt {
                                            window.paint_path(path, all_colors[i]);
                                        }
                                    }
                                },
                            )
                            .size_full(),
                        ),
                )
                // X-axis line
                .child(
                    div()
                        .absolute()
                        .left(px(margin_left as f32))
                        .top(px((margin_top + chart_height) as f32))
                        .w(px(chart_width as f32))
                        .h(px(1.0))
                        .bg(rgb(0x000000)),
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
