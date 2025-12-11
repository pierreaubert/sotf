use crate::ShowcaseApp;
use d3rs::shape::stack::Stack;
use d3rs::scale::{LinearScale, Scale};
use gpui::prelude::*;
use gpui::*;

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
    let width = 800.0;
    let height = 400.0;
    
    // Mock data generation
    // ... (keep same logic) ...
    let years = 1973..2018;
    let n_years = years.end - years.start;
    let mut data: Vec<Vec<f64>> = Vec::new();
    for i in 0..n_years {
        let y = i as f64;
        let vinyl = if y < 15.0 { 20.0 - (y-5.0).abs() } else { (y-35.0).max(0.0) * 0.5 };
        let cassette = if y > 5.0 && y < 25.0 { 15.0 - (y-15.0).abs() } else { 0.0 };
        let cd = if y > 10.0 && y < 35.0 { 40.0 - (y-25.0).abs() * 1.5 } else { 0.0 };
        let download = if y > 30.0 && y < 40.0 { 10.0 - (y-35.0).abs() } else { 0.0 };
        let streaming = if y > 35.0 { (y-35.0) * 3.0 } else { 0.0 };
        data.push(vec![vinyl.max(0.0), cassette.max(0.0), cd.max(0.0), download.max(0.0), streaming.max(0.0)]);
    }

    // Stack and Shape logic
    let labels = ["Vinyl", "Cassette", "CD", "Download", "Streaming"];
    let keys: Vec<String> = labels.iter().map(|s| s.to_string()).collect();
    
    let stack = Stack::new().keys(keys).offset(d3rs::shape::stack::StackOffset::Wiggle).order(d3rs::shape::stack::StackOrder::InsideOut);
    let series = stack.generate(&data);
    let x_scale = LinearScale::new().domain(1973.0, 2018.0).range(40.0, width - 40.0);
    
    let max_y = series.iter().flat_map(|s| s.values.iter()).map(|p| p[1]).fold(0.0f64, f64::max);
    let y_scale = LinearScale::new().domain(0.0, max_y).range(height - 40.0, 40.0);

    // Generate path strings
    let mut series_paths = Vec::new();
    for s in series {
        let mut path_d = String::new();
        if let Some(first) = s.values.first() {
             path_d.push_str(&format!("M {} {}", x_scale.scale(1973.0), y_scale.scale(first[1])));
        }
        for (j, p) in s.values.iter().enumerate() {
            path_d.push_str(&format!(" L {} {}", x_scale.scale(1973.0 + j as f64), y_scale.scale(p[1])));
        }
        if let Some(last) = s.values.last() {
             path_d.push_str(&format!(" L {} {}", x_scale.scale(1973.0 + (s.values.len()-1) as f64), y_scale.scale(last[0])));
        }
        for (j, p) in s.values.iter().enumerate().rev() {
            path_d.push_str(&format!(" L {} {}", x_scale.scale(1973.0 + j as f64), y_scale.scale(p[0])));
        }
        path_d.push_str(" Z");
        series_paths.push(path_d);
    }
    
    let colors = [
        rgb(0x8dd3c7), rgb(0xffffb3), rgb(0xbebada), rgb(0xfb8072), rgb(0x80b1d3),
    ];
    let labels = ["Vinyl", "Cassette", "CD", "Download", "Streaming"];

    let legend_items = labels.iter().enumerate().map(|(i, &label)| {
        div().flex().items_center().gap_1()
            .child(div().size_3().bg(colors[i]))
            .child(div().text_xs().child(label))
    }).collect::<Vec<_>>();

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_4()
        .child(
            div().text_lg().font_weight(FontWeight::BOLD).mb_4().child("Revenue by Music Format 1973–2018"),
        )
        .child(
            div().flex().gap_4().mb_4().children(legend_items)
        )
        .child(
            div()
                .w(px(width as f32))
                .h(px(height as f32))
                .bg(rgb(0xffffff))
                .relative()
                .child(
                    canvas(
                        move |bounds, _, _| {
                            // Parse paths with bounds offset
                            let parsed: Vec<_> = series_paths.iter().map(|d| {
                                super::path_utils::parse_svg_path(d, bounds)
                            }).collect();
                            parsed
                        },
                        move |_bounds, paths, window, _| {
                            for (i, path_opt) in paths.into_iter().enumerate() {
                                if let Some(path) = path_opt {
                                    window.paint_path(path, colors[i % colors.len()]);
                                }
                            }
                        }
                    )
                )
        )
}
