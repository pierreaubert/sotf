//! Parallel Coordinates - D3.js Example Port
//!
//! This example demonstrates parallel coordinates for visualizing multidimensional data,
//! ported from: <https://observablehq.com/@d3/parallel-coordinates>

use crate::ShowcaseApp;
use gpui::prelude::*;
use gpui::*;

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
    let width = 800.0;
    let height = 400.0;
    let margin = 50.0;
    let axis_spacing = 130.0;
    let num_axes = 6;

    // Sample car data: [MPG, Cylinders, Displacement, Horsepower, Weight, Acceleration]
    let car_data = vec![
        vec![18.0, 8.0, 307.0, 130.0, 3504.0, 12.0],
        vec![15.0, 8.0, 350.0, 165.0, 3693.0, 11.5],
        vec![27.0, 4.0, 97.0, 88.0, 2130.0, 14.5],
        vec![18.0, 6.0, 200.0, 100.0, 3332.0, 15.5],
        vec![14.0, 8.0, 302.0, 140.0, 3208.0, 10.5],
        vec![30.0, 4.0, 79.0, 70.0, 2074.0, 19.0],
        vec![26.0, 4.0, 97.0, 69.0, 1935.0, 20.0],
        vec![25.0, 4.0, 110.0, 87.0, 2672.0, 17.5],
        vec![24.0, 4.0, 107.0, 90.0, 2785.0, 14.5],
        vec![25.0, 4.0, 104.0, 95.0, 2780.0, 15.0],
    ];

    // Axis config: (name, min, max)
    let axes = vec![
        ("MPG", 10.0, 35.0),
        ("Cyl", 4.0, 8.0),
        ("Disp", 70.0, 400.0),
        ("HP", 50.0, 200.0),
        ("Wt", 1500.0, 4000.0),
        ("Acc", 8.0, 22.0),
    ];

    // Scale functions for each axis
    let scales: Vec<(f64, f64)> = axes.iter().map(|(name, min, max)| (*min, *max)).collect();

    // Normalize values to 0-1 range for each axis
    let normalized: Vec<Vec<f64>> = car_data
        .iter()
        .map(|car| {
            car.iter()
                .enumerate()
                .map(|(i, v)| {
                    let (min, max) = scales[i];
                    (v - min) / (max - min)
                })
                .collect()
        })
        .collect();

    // Colors for each car
    let colors = [
        rgb(0x1f77b4),
        rgb(0xff7f0e),
        rgb(0x2ca02c),
        rgb(0xd62728),
        rgb(0x9467bd),
        rgb(0x8c564b),
        rgb(0xe377c2),
        rgb(0x7f7f7f),
        rgb(0xbcbd22),
        rgb(0x17becf),
    ];

    let mut all_paths: Vec<String> = Vec::new();

    // 1. Axis lines (vertical)
    for i in 0..num_axes {
        let x = margin + i as f64 * axis_spacing;
        all_paths.push(format!(
            "M {:.1} {:.1} L {:.1} {:.1}",
            x,
            margin,
            x,
            height - margin
        ));
    }

    // 2. Horizontal grid lines
    for i in 0..=4 {
        let y = margin + (i as f64 / 4.0) * (height - 2.0 * margin);
        let path = format!(
            "M {:.1} {:.1} L {:.1} {:.1}",
            margin,
            y,
            margin + (num_axes - 1) as f64 * axis_spacing,
            y
        );
        all_paths.push(path);
    }

    // 3. Data polylines (one per car)
    for car_norm in &normalized {
        let mut path = String::new();
        for (i, &v) in car_norm.iter().enumerate() {
            let x = margin + i as f64 * axis_spacing;
            let y = margin + v * (height - 2.0 * margin);
            if i == 0 {
                path.push_str(&format!("M {:.1} {:.1}", x, y));
            } else {
                path.push_str(&format!(" L {:.1} {:.1}", x, y));
            }
        }
        all_paths.push(path);
    }

    let num_axis = num_axes;
    let num_grid = num_axes + 5;

    div()
        .flex()
        .flex_col()
        .gap_6()
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .child("Parallel Coordinates")
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x666666))
                .child("Ported from Observable: d3/parallel-coordinates")
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
                                .child("Car Specifications (Auto MPG Dataset)")
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
                                        for path_str in &all_paths {
                                            if let Some(p) = super::path_utils::parse_svg_path(path_str, bounds) {
                                                shapes.push(p);
                                            }
                                        }
                                        shapes
                                    },
                                    move |_bounds, shapes, window, _| {
                                        // Draw axes
                                        for (i, shape) in shapes.iter().enumerate() {
                                            if i < num_axis {
                                                window.paint_path(shape.clone(), rgb(0x333333));
                                            } else if i < num_grid {
                                                window.paint_path(shape.clone(), rgb(0xeeeeee));
                                            } else {
                                                // Data lines
                                                let car_idx = i - num_grid;
                                                if car_idx < colors.len() {
                                                    window.paint_path(shape.clone(), colors[car_idx]);
                                                }
                                            }
                                        }
                                    },
                                ))
                        )
                        // Axis labels
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .px_8()
                                .children(axes.iter().enumerate().map(|(i, (name, _, _))| {
                                    div().text_xs().text_color(rgb(0x666666)).child(*name)
                                }))
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
                                .child("Parallel coordinates display multivariate data as polylines crossing parallel axes.")
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .mt_4()
                                .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(rgb(0x888888)).child("DATA INFO"))
                                .child(div().text_sm().text_color(rgb(0x333333)).child(format!("Cars: {}", car_data.len())))
                                .child(div().text_sm().text_color(rgb(0x333333)).child(format!("Variables: {}", num_axes)))
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .mt_4()
                                .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(rgb(0x888888)).child("LEGEND"))
                                .children(colors.iter().enumerate().filter(|(i, _)| *i < 5).map(|(i, c)| {
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(div().w_4().h_1().bg(*c).rounded_sm())
                                        .child(div().text_xs().text_color(rgb(0x666666)).child(format!("Car {}", i + 1)))
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
                .child(div().text_xs().font_family("monospace").text_color(rgb(0xd4d4d4)).child("// Each line = one car, crossing 6 axes"))
        )
}
