//! Pie Chart -- Observable example using d3rs::examples::pie_chart
use crate::ShowcaseApp;
use d3rs::shape::arc::{ArcDatum, arc_points};
use gpui::prelude::*;
use gpui::*;

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
    let result = d3rs::examples::pie_chart::compute(d3rs::examples::pie_chart::DEFAULT_DATA);

    let tableau10 = [
        rgb(0x4e79a7),
        rgb(0xf28e2b),
        rgb(0xe15759),
        rgb(0x76b7b2),
        rgb(0x59a14f),
        rgb(0xedc948),
        rgb(0xb07aa1),
        rgb(0xff9da7),
        rgb(0x9c755f),
        rgb(0xbab0ac),
    ];

    let width = 700.0_f64;
    let height = 450.0_f64;
    let cx_center = width / 2.0;
    let cy_center = height / 2.0;
    let radius = width.min(height) / 2.0 - 20.0;

    // Build SVG path strings for each slice using arc_points
    let mut slice_paths: Vec<String> = Vec::new();
    let mut slice_names: Vec<String> = Vec::new();
    for s in &result.slices {
        let datum = ArcDatum {
            inner_radius: 0.0,
            outer_radius: radius,
            start_angle: s.start_angle,
            end_angle: s.end_angle,
            corner_radius: 0.0,
            pad_angle: 0.0,
        };
        let points = arc_points(&datum, 64, cx_center, cy_center);
        let mut path_d = String::new();
        for (i, p) in points.iter().enumerate() {
            if i == 0 {
                path_d.push_str(&format!("M {} {}", p.x, p.y));
            } else {
                path_d.push_str(&format!(" L {} {}", p.x, p.y));
            }
        }
        path_d.push_str(" Z");
        slice_paths.push(path_d);
        slice_names.push(s.name.clone());
    }

    let legend_items: Vec<Div> = slice_names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let pct = (result.slices[i].end_angle - result.slices[i].start_angle)
                / std::f64::consts::TAU
                * 100.0;
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().size_3().bg(tableau10[i % tableau10.len()]))
                .child(
                    div()
                        .text_xs()
                        .child(format!("{}: {:.1}%", name, pct)),
                )
        })
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
                .child("Pie Chart"),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x666666))
                .mb_2()
                .child("Source: observablehq.com/@d3/pie-chart"),
        )
        .child(div().flex().gap_4().mb_2().flex_wrap().children(legend_items))
        .child(
            div()
                .w(px(width as f32))
                .h(px(height as f32))
                .bg(rgb(0xffffff))
                .border_1()
                .border_color(rgb(0xcccccc))
                .child(
                    canvas(
                        move |bounds, _, _| {
                            slice_paths
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
        )
}
