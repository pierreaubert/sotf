//! Chord Diagram -- Observable example using d3rs::examples::chord
use crate::ShowcaseApp;
use d3rs::shape::arc::{ArcDatum, arc_points};
use gpui::prelude::*;
use gpui::*;

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
    let (names, matrix) = d3rs::examples::chord::default_matrix();
    let result = d3rs::examples::chord::compute(&names, &matrix);

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
    let scale_factor = height.min(width) / result.width.max(result.height);
    let outer_radius = result.outer_radius * scale_factor;
    let inner_radius = result.inner_radius * scale_factor;

    let mut all_paths: Vec<String> = Vec::new();
    let mut all_colors: Vec<Hsla> = Vec::new();

    // Draw group arcs
    for group in &result.chord_result.groups {
        let datum = ArcDatum {
            inner_radius,
            outer_radius,
            start_angle: group.start_angle,
            end_angle: group.end_angle,
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
        all_paths.push(path_d);
        all_colors.push(tableau10[group.index % tableau10.len()].into());
    }

    // Draw chords as filled ribbon paths between source and target arcs
    let pi_half = std::f64::consts::PI / 2.0;
    for chord in &result.chord_result.chords {
        let s_start = chord.source.start_angle - pi_half;
        let s_end = chord.source.end_angle - pi_half;
        let s_mid = (s_start + s_end) / 2.0;
        let t_start = chord.target.start_angle - pi_half;
        let t_end = chord.target.end_angle - pi_half;
        let t_mid = (t_start + t_end) / 2.0;
        let r = inner_radius;

        let mut path_d = String::new();

        // Source arc
        let sx1 = cx_center + r * s_start.cos();
        let sy1 = cy_center + r * s_start.sin();
        path_d.push_str(&format!("M {} {}", sx1, sy1));
        let n_seg = 8;
        for i in 1..=n_seg {
            let t = i as f64 / n_seg as f64;
            let a = s_start + (s_end - s_start) * t;
            path_d.push_str(&format!(
                " L {} {}",
                cx_center + r * a.cos(),
                cy_center + r * a.sin()
            ));
        }

        // Cross to target through middle
        let tm_x = cx_center + r * 0.3 * t_mid.cos();
        let tm_y = cy_center + r * 0.3 * t_mid.sin();
        path_d.push_str(&format!(" L {} {}", tm_x, tm_y));

        // Target arc
        let tx1 = cx_center + r * t_start.cos();
        let ty1 = cy_center + r * t_start.sin();
        path_d.push_str(&format!(" L {} {}", tx1, ty1));
        for i in 1..=n_seg {
            let t = i as f64 / n_seg as f64;
            let a = t_start + (t_end - t_start) * t;
            path_d.push_str(&format!(
                " L {} {}",
                cx_center + r * a.cos(),
                cy_center + r * a.sin()
            ));
        }

        // Cross back through middle
        let sm_x = cx_center + r * 0.3 * s_mid.cos();
        let sm_y = cy_center + r * 0.3 * s_mid.sin();
        path_d.push_str(&format!(" L {} {}", sm_x, sm_y));
        path_d.push_str(" Z");

        all_paths.push(path_d);
        // Semi-transparent version of source group color for the ribbon
        let mut lighter: Hsla = tableau10[chord.source.index % tableau10.len()].into();
        lighter.a = 0.4;
        all_colors.push(lighter);
    }

    let legend_items: Vec<Div> = result
        .names
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
                .child("Chord Diagram"),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x666666))
                .mb_2()
                .child("Source: observablehq.com/@d3/chord-diagram"),
        )
        .child(div().flex().gap_3().mb_2().flex_wrap().children(legend_items))
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
}
