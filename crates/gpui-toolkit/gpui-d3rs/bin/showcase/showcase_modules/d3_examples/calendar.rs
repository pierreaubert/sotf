//! Calendar Heatmap - D3.js Example Port
//!
//! This example demonstrates a calendar heatmap for visualizing time-based data,
//! ported from: <https://observablehq.com/@d3/calendar>

use crate::ShowcaseApp;
use gpui::prelude::*;
use gpui::*;

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
    let width = 800.0;
    let height = 180.0;
    let cell_size = 12.0;
    let cell_pad = 2.0;

    // Generate sample data - 12 months x 4 weeks x 7 days
    let weeks = 52;
    let days = 7;
    let total_cells = weeks * days;

    // Sample values that vary over time (simulating some metric)
    let mut cell_paths: Vec<String> = Vec::new();
    let mut cell_colors: Vec<u32> = Vec::new();

    for i in 0..total_cells {
        let week = i / days;
        let day = i % days;

        let x = week as f64 * (cell_size + cell_pad);
        let y = day as f64 * (cell_size + cell_pad) + 20.0;

        // Simulated value that varies - higher in summer months (middle weeks)
        let seasonal = ((week as f64 - 26.0) / 26.0).sin() * 0.5 + 0.5;
        let noise = ((i * 17 % 10) as f64) / 10.0;
        let value = seasonal * 0.7 + noise * 0.3;

        let path = format!(
            "M {:.1} {:.1} h {:.1} v {:.1} h -{:.1} Z",
            x, y, cell_size, cell_size, cell_size
        );

        cell_paths.push(path);

        // Color based on value -RdYlGn style (green=high, red=low)
        let color = if value > 0.7 {
            0x2ca02c // Green
        } else if value > 0.5 {
            0x2ca02c
        } else if value > 0.3 {
            0xff7f0e // Orange
        } else {
            0xd62728 // Red
        };
        cell_colors.push(color);
    }

    // Month labels positions
    let month_labels = vec![
        (0, "Jan"),
        (4, "Feb"),
        (8, "Mar"),
        (13, "Apr"),
        (17, "May"),
        (22, "Jun"),
        (26, "Jul"),
        (31, "Aug"),
        (35, "Sep"),
        (39, "Oct"),
        (44, "Nov"),
        (48, "Dec"),
    ];

    div()
        .flex()
        .flex_col()
        .gap_6()
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .child("Calendar Heatmap")
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x666666))
                .child("Ported from Observable: d3/calendar")
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
                                .child("Daily Values (2024)")
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
                                        for path_str in &cell_paths {
                                            if let Some(p) = super::path_utils::parse_svg_path(path_str, bounds) {
                                                shapes.push(p);
                                            }
                                        }
                                        shapes
                                    },
                                    move |_bounds, shapes, window, _| {
                                        for (i, shape) in shapes.iter().enumerate() {
                                            if i < cell_colors.len() {
                                                window.paint_path(shape.clone(), rgb(cell_colors[i]));
                                            }
                                        }
                                    },
                                ))
                        )
                        // Legend
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().text_xs().text_color(rgb(0x666666)).child("Low"))
                                .child(
                                    div()
                                        .flex()
                                        .h_3()
                                        .w_24()
                                        .rounded_sm()
                                        .overflow_hidden()
                                        .children(vec![
                                            div().flex_1().h_full().bg(rgb(0xd62728)),
                                            div().flex_1().h_full().bg(rgb(0xff7f0e)),
                                            div().flex_1().h_full().bg(rgb(0x2ca02c)),
                                        ])
                                )
                                .child(div().text_xs().text_color(rgb(0x666666)).child("High"))
                        )
                        // Day labels
                        .child(
                            div()
                                .flex()
                                .gap_1()
                                .ml_8()
                                .children(vec![
                                    div().w_3().text_xs().text_color(rgb(0x888888)).child("S"),
                                    div().w_3().text_xs().text_color(rgb(0x888888)).child("M"),
                                    div().w_3().text_xs().text_color(rgb(0x888888)).child("T"),
                                    div().w_3().text_xs().text_color(rgb(0x888888)).child("W"),
                                    div().w_3().text_xs().text_color(rgb(0x888888)).child("T"),
                                    div().w_3().text_xs().text_color(rgb(0x888888)).child("F"),
                                    div().w_3().text_xs().text_color(rgb(0x888888)).child("S"),
                                ])
                        )
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .w(px(280.0))
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
                                .child("Calendar heatmaps display values organized by day and week, similar to GitHub contribution graphs.")
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .mt_4()
                                .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(rgb(0x888888)).child("DATA INFO"))
                                .child(div().text_sm().text_color(rgb(0x333333)).child(format!("Weeks: {}", weeks)))
                                .child(div().text_sm().text_color(rgb(0x333333)).child(format!("Cells: {}", total_cells)))
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
                .child(div().text_xs().font_family("monospace").text_color(rgb(0xd4d4d4)).child("// Grid of colored cells by week/day"))
        )
}
