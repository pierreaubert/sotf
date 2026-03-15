//! Sankey Diagram - D3.js Example Port
//!
//! This example demonstrates a Sankey diagram for visualizing flow relationships,
//! ported from: <https://observablehq.com/@d3/sankey-diagram>

use crate::ShowcaseApp;
use gpui::prelude::*;
use gpui::*;

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
    let width = 800.0;
    let height = 450.0;

    // Node data: (name, layer, y, height)
    let nodes = vec![
        // Layer 0 - Sources
        ("Solar", 0, 30.0, 80.0),
        ("Wind", 0, 120.0, 60.0),
        ("Bioenergy", 0, 190.0, 100.0),
        // Layer 1 - Processing
        ("Electricity", 1, 40.0, 120.0),
        ("Heat", 1, 170.0, 100.0),
        // Layer 2 - End users
        ("Home", 2, 30.0, 100.0),
        ("Business", 2, 140.0, 80.0),
        ("Industry", 2, 230.0, 120.0),
    ];

    let layer_width = width / 3.0;

    // Generate node rectangles
    let mut node_paths: Vec<String> = Vec::new();
    let node_layers: Vec<usize> = nodes.iter().map(|n| n.1).collect();
    let node_colors = [
        rgb(0x1f77b4), // Blue
        rgb(0xff7f0e), // Orange
        rgb(0x2ca02c), // Green
        rgb(0xd62728), // Red
    ];

    for (name, layer, y, h) in &nodes {
        let x = *layer as f64 * layer_width;
        let path = format!(
            "M {:.1} {:.1} h {:.1} v {:.1} h -{:.1} Z",
            x + 5.0,
            *y,
            layer_width - 15.0,
            *h,
            layer_width - 15.0
        );
        node_paths.push(path);
    }

    // Generate flow links with smooth curves
    let link_data = vec![
        // (from_idx, to_idx, flow_y, flow_height)
        (0, 3, 50.0, 20.0),  // Solar -> Electricity
        (1, 3, 90.0, 25.0),  // Wind -> Electricity
        (2, 3, 130.0, 30.0), // Bioenergy -> Electricity
        (2, 4, 180.0, 25.0), // Bioenergy -> Heat
        (3, 5, 50.0, 35.0),  // Electricity -> Home
        (3, 6, 100.0, 30.0), // Electricity -> Business
        (4, 5, 90.0, 25.0),  // Heat -> Home
        (4, 7, 200.0, 35.0), // Heat -> Industry
        (3, 7, 150.0, 20.0), // Electricity -> Industry
    ];

    let mut link_paths: Vec<String> = Vec::new();
    for (from_idx, to_idx, flow_y, flow_h) in &link_data {
        let (_, from_layer, from_y, _) = nodes[*from_idx];
        let (_, to_layer, to_y, _) = nodes[*to_idx];

        let src_x = from_layer as f64 * layer_width + layer_width - 10.0;
        let tgt_x = to_layer as f64 * layer_width + 5.0;

        // Use bezier curves for smooth flow
        let path = format!(
            "M {:.1} {:.1} C {:.1} {:.1} {:.1} {:.1} L {:.1} {:.1} C {:.1} {:.1} {:.1} {:.1} L {:.1} {:.1} Z",
            src_x, flow_y + from_y,
            src_x + 60.0, flow_y + from_y,
            tgt_x - 60.0, flow_y + to_y,
            tgt_x, flow_y + to_y,
            tgt_x - 60.0, flow_y + to_y + flow_h,
            src_x + 60.0, from_y + flow_y + flow_h,
            src_x, from_y + flow_y + flow_h
        );
        link_paths.push(path);
    }

    let num_links = link_paths.len();

    div()
        .flex()
        .flex_col()
        .gap_6()
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .child("Sankey Diagram")
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x666666))
                .child("Ported from Observable: d3/sankey-diagram")
        )
        .child(
            div()
                .flex()
                .gap_8()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Energy Flow: Sources → Processing → End Users")
                        )
                        .child(
                            div()
                                .w(px(width as f32))
                                .h(px(height as f32))
                                .bg(rgb(0xfafafa))
                                .border_1()
                                .border_color(rgb(0xe0e0e0))
                                .rounded_md()
                                .child(canvas(
                                    move |bounds, _cx, _| {
                                        let mut shapes = Vec::new();
                                        // First parse all paths
                                        for path_str in &link_paths {
                                            if let Some(p) = super::path_utils::parse_svg_path(path_str, bounds) {
                                                shapes.push(p);
                                            }
                                        }
                                        for path_str in &node_paths {
                                            if let Some(p) = super::path_utils::parse_svg_path(path_str, bounds) {
                                                shapes.push(p);
                                            }
                                        }
                                        shapes
                                    },
                                    move |_bounds, shapes, window, _| {
                                        // Draw links first (background)
                                        for (i, shape) in shapes.iter().enumerate() {
                                            if i < num_links {
                                                window.paint_path(shape.clone(), rgb(0xcccccc));
                                            } else {
                                                // Draw nodes
                                                let node_idx = i - num_links;
                                                if node_idx < node_layers.len() {
                                                    let layer = node_layers[node_idx];
                                                    window.paint_path(shape.clone(), node_colors[layer % node_colors.len()]);
                                                }
                                            }
                                        }
                                    },
                                ))
                        )
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .w(px(250.0))
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("About")
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x666666))
                                .child("Sankey diagrams show flow quantities between stages. Width represents the flow amount.")
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .mt_4()
                                .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(rgb(0x888888)).child("DATA INFO"))
                                .child(div().text_sm().text_color(rgb(0x333333)).child(format!("Nodes: {}", nodes.len())))
                                .child(div().text_sm().text_color(rgb(0x333333)).child(format!("Links: {}", link_data.len())))
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .mt_4()
                                .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(rgb(0x888888)).child("LEGEND"))
                                .children(node_colors.iter().enumerate().map(|(i, c)| {
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(div().w_4().h_4().bg(*c).rounded_sm())
                                        .child(div().text_xs().text_color(rgb(0x666666)).child(format!("Layer {}", i)))
                                }))
                        )
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_4()
                .bg(rgb(0x1e1e1e))
                .border_1()
                .border_color(rgb(0x333333))
                .rounded_lg()
                .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(rgb(0x888888)).child("IMPLEMENTATION NOTES"))
                .child(div().text_xs().font_family("monospace").text_color(rgb(0xd4d4d4)).child("// Nodes: rectangles | Links: cubic bezier curves"))
        )
}
