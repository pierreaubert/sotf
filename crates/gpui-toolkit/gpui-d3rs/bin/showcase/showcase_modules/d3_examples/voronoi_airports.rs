//! World Airports Voronoi — Observable example using d3rs::examples::voronoi_airports
//!
//! Renders Voronoi cells for ~3000 airports projected onto an equirectangular map.
//! Demonstrates: `Delaunay`, `Voronoi`, `PathBuilder`.
//!
//! Source: <https://observablehq.com/@d3/world-airports-voronoi>

use crate::ShowcaseApp;
use d3rs::color::ColorScheme;
use gpui::prelude::*;
use gpui::*;

const AIRPORTS_CSV: &str = include_str!("../../data/airports.csv");

pub fn render(_app: &ShowcaseApp, _cx: &mut Context<ShowcaseApp>) -> Div {
    let coords = d3rs::examples::voronoi_airports::load_csv(AIRPORTS_CSV);
    let result = d3rs::examples::voronoi_airports::compute(&coords);

    let width = result.width;
    let height = result.height;

    let d3_paths = result.voronoi_paths;
    let cell_count = d3_paths.len();
    let point_count = result.point_count;

    // Color each cell by index using a color scheme
    let scheme = ColorScheme::tableau10();
    let all_colors: Vec<Hsla> = (0..cell_count)
        .map(|i| Hsla::from(scheme.color(i % scheme.len()).to_rgba()).opacity(0.4))
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
                .child("World Airports Voronoi"),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x666666))
                .mb_2()
                .child(format!(
                    "Source: observablehq.com/@d3/world-airports-voronoi — {} airports, {} Voronoi cells",
                    point_count, cell_count
                )),
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
                                    super::path_utils::d3rs_path_to_gpui_simple(p, bounds, 0.0, 0.0)
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
