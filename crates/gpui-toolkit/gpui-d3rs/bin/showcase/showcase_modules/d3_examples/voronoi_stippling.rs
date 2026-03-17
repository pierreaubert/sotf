//! Voronoi Stippling — weighted Lloyd's relaxation on an image.
//! Source: <https://observablehq.com/@mbostock/voronoi-stippling>

use crate::ShowcaseApp;
use gpui::prelude::*;
use gpui::*;

const WOOD_JPEG: &[u8] = include_bytes!("../../data/wood.jpeg");

fn decode_density(jpeg_bytes: &[u8]) -> (Vec<f64>, usize, usize) {
    let img = image::load_from_memory(jpeg_bytes).expect("failed to decode JPEG");
    let gray = img.to_luma8();
    let width = gray.width() as usize;
    let height = gray.height() as usize;
    let density: Vec<f64> = gray
        .pixels()
        .map(|p| (1.0 - p.0[0] as f64 / 255.0).max(0.0))
        .collect();
    (density, width, height)
}

pub fn render(app: &mut ShowcaseApp, cx: &mut Context<ShowcaseApp>) -> Div {
    let current = app.stippling_iterations;
    let target = app.stippling_target;

    // Animate: if current < target, advance by 1 each frame
    if current < target {
        cx.spawn(|entity, mut cx| async move {
            smol::Timer::after(std::time::Duration::from_millis(16)).await;
            cx.update(|cx| {
                entity
                    .update(cx, |this: &mut ShowcaseApp, cx| {
                        if this.stippling_iterations < this.stippling_target {
                            this.stippling_iterations += 1;
                            cx.notify();
                        }
                    })
                    .ok();
            })
            .ok();
        })
        .detach();
    }

    let (density, img_w, img_h) = decode_density(WOOD_JPEG);
    let n = (img_w * img_h) / 40;
    let result =
        d3rs::examples::voronoi_stippling::compute(&density, img_w, img_h, n, current);

    let width = result.width;
    let height = result.height;

    let d3_paths = result.dot_paths;
    let dot_color = hsla(0.0, 0.0, 1.0, 1.0);
    let all_colors: Vec<Hsla> = vec![dot_color; d3_paths.len()];

    let presets: Vec<usize> = vec![1, 5, 10, 20, 40, 60, 80, 120];

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
                .child("Voronoi Stippling"),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x666666))
                .mb_1()
                .child(format!(
                    "Source: observablehq.com/@mbostock/voronoi-stippling — {} dots, wood.jpeg {}×{}",
                    result.point_count, img_w, img_h
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .mb_2()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0x333333))
                        .child("Iterations:"),
                )
                .children(presets.into_iter().map(|preset| {
                    let is_selected = preset == target;
                    let bg = if is_selected {
                        rgb(0x007acc)
                    } else {
                        rgb(0xe8e8e8)
                    };
                    let text_color = if is_selected {
                        rgb(0xffffff)
                    } else {
                        rgb(0x333333)
                    };
                    div()
                        .id(ElementId::Name(format!("stip-{preset}").into()))
                        .px_2()
                        .py(px(2.0))
                        .rounded_md()
                        .bg(bg)
                        .text_xs()
                        .text_color(text_color)
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(0x005a9e)))
                        .child(format!("{preset}"))
                        .on_click(cx.listener(move |this, _, _, _| {
                            this.stippling_target = preset;
                            // Reset to 1 to animate from scratch
                            this.stippling_iterations = 1;
                        }))
                }))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if current < target {
                            rgb(0x007acc)
                        } else {
                            rgb(0x999999)
                        })
                        .ml_2()
                        .child(format!(
                            "{}{}",
                            current,
                            if current < target {
                                format!(" → {target}")
                            } else {
                                String::new()
                            }
                        )),
                ),
        )
        .child(
            div()
                .w(px(width as f32))
                .h(px(height as f32))
                .bg(rgb(0x111111))
                .border_1()
                .border_color(rgb(0x333333))
                .child(
                    canvas(
                        move |bounds, _, _| {
                            d3_paths
                                .iter()
                                .map(|p| {
                                    super::path_utils::d3rs_path_to_gpui_simple(
                                        p, bounds, 0.0, 0.0,
                                    )
                                })
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
