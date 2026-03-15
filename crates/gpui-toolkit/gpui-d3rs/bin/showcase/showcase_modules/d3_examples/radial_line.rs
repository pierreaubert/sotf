//! Radial Line Chart - D3.js Example Port
//!
//! This example demonstrates a radial line chart for visualizing cyclical data,
//! ported from: <https://observablehq.com/@d3/radial-line-chart>

use crate::ShowcaseApp;
use gpui::prelude::*;
use gpui::*;
use std::f64::consts::PI;

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
    let center = 220.0;
    let radius = 160.0;

    // Hourly temperature data (24 hours)
    let temps = [
        12.0, 11.0, 10.0, 10.0, 10.0, 11.0, 13.0, 16.0, 19.0, 22.0, 24.0, 26.0, 27.0, 28.0, 28.0,
        27.0, 25.0, 23.0, 21.0, 19.0, 17.0, 16.0, 14.0, 13.0,
    ];
    let min_temp = 8.0;
    let max_temp = 30.0;

    let mut all_paths: Vec<String> = Vec::new();

    // 1. Grid circles (concentric) - simplified as polygons
    for i in 0..=4 {
        let r = radius * (i as f64 / 4.0);
        // Draw as a polygon approximating circle
        let mut path = String::new();
        for j in 0..36 {
            let angle = (j as f64 / 36.0) * 2.0 * PI;
            let x = center + r * angle.cos();
            let y = center + r * angle.sin();
            if j == 0 {
                path.push_str(&format!("M {:.1} {:.1}", x, y));
            } else {
                path.push_str(&format!(" L {:.1} {:.1}", x, y));
            }
        }
        path.push_str(" Z");
        all_paths.push(path);
    }

    // 2. Radial lines (24 hours)
    for i in 0..24 {
        let angle = (i as f64 / 12.0) * PI - PI / 2.0;
        let x = center + radius * angle.cos();
        let y = center + radius * angle.sin();
        all_paths.push(format!("M {:.1} {:.1} L {:.1} {:.1}", center, center, x, y));
    }

    // 3. Area path (filled)
    let mut area_path = String::new();
    for (i, &temp) in temps.iter().enumerate() {
        let angle = (i as f64 / 12.0) * PI - PI / 2.0;
        let r = ((temp - min_temp) / (max_temp - min_temp)) * radius;
        let x = center + r * angle.cos();
        let y = center + r * angle.sin();

        if i == 0 {
            area_path.push_str(&format!("M {:.1} {:.1}", x, y));
        } else {
            area_path.push_str(&format!(" L {:.1} {:.1}", x, y));
        }
    }
    area_path.push_str(" Z");
    all_paths.push(area_path);

    // 4. Line path (stroke)
    let mut line_path = String::new();
    for (i, &temp) in temps.iter().enumerate() {
        let angle = (i as f64 / 12.0) * PI - PI / 2.0;
        let r = ((temp - min_temp) / (max_temp - min_temp)) * radius;
        let x = center + r * angle.cos();
        let y = center + r * angle.sin();

        if i == 0 {
            line_path.push_str(&format!("M {:.1} {:.1}", x, y));
        } else {
            line_path.push_str(&format!(" L {:.1} {:.1}", x, y));
        }
    }
    all_paths.push(line_path);

    // Labels positions
    let labels = vec![(0, "12am"), (6, "6am"), (12, "12pm"), (18, "6pm")];

    let num_grid = 5;
    let num_radial = 24;

    div()
        .flex()
        .flex_col()
        .gap_6()
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .child("Radial Line Chart")
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x666666))
                .child("Ported from Observable: d3/radial-line-chart")
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
                                .child("Hourly Temperature (°C)")
                        )
                        .child(
                            div()
                                .w(px(460.0))
                                .h(px(460.0))
                                .bg(rgb(0xfafafa))
                                .border_1()
                                .border_color(rgb(0xe0e0e0))
                                .rounded_md()
                                .child(canvas(
                                    move |bounds, _cx, _| {
                                        let mut shapes = Vec::new();
                                        for path_str in &all_paths {
                                            if let Some(p) = super::path_utils::parse_svg_path(path_str, bounds) {
                                                shapes.push(p);
                                            }
                                        }
                                        shapes
                                    },
                                    move |_bounds, shapes, window, _| {
                                        // Grid circles
                                        for (i, shape) in shapes.iter().enumerate() {
                                            if i < num_grid {
                                                window.paint_path(shape.clone(), rgb(0xe0e0e0));
                                            } else if i < num_grid + num_radial {
                                                window.paint_path(shape.clone(), rgb(0xeeeeee));
                                            } else if i == num_grid + num_radial {
                                                // Area fill - use lighter color instead of alpha
                                                window.paint_path(shape.clone(), rgb(0x8cb4d8));
                                            } else {
                                                // Line stroke
                                                window.paint_path(shape.clone(), rgb(0x2171b5));
                                            }
                                        }
                                    },
                                ))
                        )
                        // Legend
                        .child(
                            div()
                                .flex()
                                .justify_center()
                                .gap_6()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(div().w_4().h_4().bg(rgb(0x8cb4d8)).rounded_sm())
                                        .child(div().text_xs().text_color(rgb(0x666666)).child("Area"))
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(div().w_4().h_1().bg(rgb(0x2171b5)).rounded_sm())
                                        .child(div().text_xs().text_color(rgb(0x666666)).child("Line"))
                                )
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
                                .child("Radial line charts display cyclical data (like hours in a day) in polar coordinates.")
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .mt_4()
                                .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(rgb(0x888888)).child("DATA INFO"))
                                .child(div().text_sm().text_color(rgb(0x333333)).child(format!("Hours: {}", temps.len())))
                                .child(div().text_sm().text_color(rgb(0x333333)).child(format!("Min: {:.0}°C", min_temp)))
                                .child(div().text_sm().text_color(rgb(0x333333)).child(format!("Max: {:.0}°C", max_temp)))
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .mt_4()
                                .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(rgb(0x888888)).child("AXIS"))
                                .child(div().text_xs().text_color(rgb(0x666666)).child("Concentric circles: temperature scale"))
                                .child(div().text_xs().text_color(rgb(0x666666)).child("Radial lines: 24 hours"))
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
                .child(div().text_xs().font_family("monospace").text_color(rgb(0xd4d4d4)).child("// Polar coordinates: angle=hour, radius=value"))
        )
}
