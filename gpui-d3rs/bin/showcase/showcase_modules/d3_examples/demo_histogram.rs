use crate::ShowcaseApp;
use d3rs::scale::{LinearScale, Scale};
use gpui::prelude::*;
use gpui::*;

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
    let width = 600.0;
    let height = 400.0;
    let margin = 40.0;

    // Generate random data binned
    // In a real app we'd use random generator, here fixed mock
    let data = vec![
        1.0, 2.0, 2.5, 3.0, 3.5, 3.5, 4.0, 4.0, 4.0, 5.0, 5.5, 6.0, 9.0,
    ];

    // Binning
    let min_val = data.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let max_val = data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let bin_count = 10;

    // Manual binning logic:
    let mut bins = vec![0; bin_count];
    let step = (max_val - min_val) / bin_count as f64;
    for &d in &data {
        let idx = ((d - min_val) / step).floor() as usize;
        if idx < bin_count {
            bins[idx] += 1;
        } else if idx == bin_count {
            // Handle max value edge case
            bins[idx - 1] += 1;
        }
    }

    // Scales
    let x_scale = LinearScale::new()
        .domain(min_val, max_val)
        .range(margin, width - margin);

    let max_bin = *bins.iter().max().unwrap_or(&0) as f64;
    let y_scale = LinearScale::new()
        .domain(0.0, max_bin)
        .range(height - margin, margin);

    let bar_width = (width - 2.0 * margin) / bin_count as f64 - 1.0;

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_4()
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::BOLD)
                .mb_4()
                .child("Histogram"),
        )
        .child(
            div()
                .w(px(width as f32))
                .h(px(height as f32))
                .bg(rgb(0xffffff))
                .relative()
                // Bars
                .children((0..bin_count).map(|i| {
                    let count = bins[i] as f64;
                    let x0 = x_scale.scale(min_val + i as f64 * step);
                    let y0 = y_scale.scale(count);
                    let h = y_scale.scale(0.0) - y0;

                    div()
                        .absolute()
                        .left(px(x0 as f32))
                        .top(px(y0 as f32))
                        .w(px(bar_width as f32))
                        .h(px(h as f32))
                        .bg(rgb(0x4682b4))
                }))
                // Axis Ticks & Labels
                .children((0..=bin_count).map(|i| {
                    let val = min_val + i as f64 * step;
                    let x = x_scale.scale(val);
                    let y = height - margin;

                    div()
                        .absolute()
                        .left(px(x as f32))
                        .top(px(y as f32))
                        .children(vec![
                            // Tick mark
                            div().w(px(1.0)).h(px(5.0)).bg(rgb(0x000000)),
                            // Label
                            div()
                                .absolute()
                                .top(px(8.0))
                                // .left(px(10.0)) // Center align approx
                                .text_xs()
                                .text_color(rgb(0x333333))
                                .child(format!("{:.1}", val)),
                        ])
                })),
        )
}
