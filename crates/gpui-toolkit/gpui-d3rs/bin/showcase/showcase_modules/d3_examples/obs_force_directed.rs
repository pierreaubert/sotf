//! Force-Directed Graph -- Observable example using d3rs::examples::force_directed
use crate::ShowcaseApp;
use d3rs::scale::{LinearScale, Scale};
use gpui::prelude::*;
use gpui::*;

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
    let (node_data, links) = d3rs::examples::force_directed::default_data();
    let result = d3rs::examples::force_directed::compute(&node_data, &links, 300);

    let group_colors = [
        rgb(0x999999), // placeholder for group 0 (unused)
        rgb(0x4e79a7), // group 1
        rgb(0xf28e2b), // group 2
        rgb(0xe15759), // group 3
        rgb(0x76b7b2), // group 4
        rgb(0x59a14f), // group 5
    ];

    let width = 700.0_f64;
    let height = 450.0_f64;
    let margin = 40.0_f64;

    // Find extent of node positions
    let x_ext = result
        .nodes
        .iter()
        .fold((f64::MAX, f64::MIN), |a, n| (a.0.min(n.x), a.1.max(n.x)));
    let y_ext = result
        .nodes
        .iter()
        .fold((f64::MAX, f64::MIN), |a, n| (a.0.min(n.y), a.1.max(n.y)));

    let x_scale = LinearScale::new()
        .domain(x_ext.0, x_ext.1)
        .range(margin, width - margin);
    let y_scale = LinearScale::new()
        .domain(y_ext.0, y_ext.1)
        .range(margin, height - margin);

    // Build node index map for link lookups
    let node_positions: Vec<(f64, f64, usize)> = result
        .nodes
        .iter()
        .map(|n| (x_scale.scale(n.x), y_scale.scale(n.y), n.group))
        .collect();
    let node_ids: Vec<&str> = result.nodes.iter().map(|n| n.id.as_str()).collect();

    let mut all_paths: Vec<String> = Vec::new();
    let mut all_colors: Vec<Hsla> = Vec::new();

    // Draw links as thin lines (rectangles)
    for link in &result.links {
        let si = node_ids.iter().position(|&id| id == link.source);
        let ti = node_ids.iter().position(|&id| id == link.target);
        if let (Some(si), Some(ti)) = (si, ti) {
            let (sx, sy, _) = node_positions[si];
            let (tx, ty, _) = node_positions[ti];

            let dx = tx - sx;
            let dy = ty - sy;
            let len = (dx * dx + dy * dy).sqrt().max(1e-6);
            let nx = -dy / len * 0.75;
            let ny = dx / len * 0.75;

            let path_d = format!(
                "M {} {} L {} {} L {} {} L {} {} Z",
                sx + nx, sy + ny,
                tx + nx, ty + ny,
                tx - nx, ty - ny,
                sx - nx, sy - ny
            );
            all_paths.push(path_d);
            all_colors.push(hsla(0.0, 0.0, 0.7, 0.5));
        }
    }

    // Draw nodes as filled circles (polygon approximation)
    let node_radius = 6.0;
    let n_sides = 12;
    for (px_val, py_val, group) in &node_positions {
        let mut path_d = String::new();
        for v in 0..n_sides {
            let angle = std::f64::consts::TAU * v as f64 / n_sides as f64;
            let x = px_val + node_radius * angle.cos();
            let y = py_val + node_radius * angle.sin();
            if v == 0 {
                path_d.push_str(&format!("M {} {}", x, y));
            } else {
                path_d.push_str(&format!(" L {} {}", x, y));
            }
        }
        path_d.push_str(" Z");
        all_paths.push(path_d);
        all_colors.push(group_colors[*group % group_colors.len()].into());
    }

    let legend_items: Vec<Div> = (1..=5)
        .map(|g| {
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().size_3().bg(group_colors[g]))
                .child(div().text_xs().child(format!("Group {}", g)))
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
                .child("Force-Directed Graph"),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x666666))
                .mb_2()
                .child("Source: observablehq.com/@d3/force-directed-graph"),
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
