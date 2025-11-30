use d3rs::geo::{Equirectangular, Mercator, Projection};
use gpui::*;

pub fn render(app: &ShowcaseApp) -> Div {
    let mercator = Mercator::new()
        .scale(200.0)
        .translate(400.0, 300.0);

    let equirect = Equirectangular::new()
        .scale(200.0)
        .translate(400.0, 300.0);

    div()
        .flex()
        .flex_col()
        .gap_6()
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .child("Geographic Projections Demo"),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0x666666))
                .max_w(px(700.0))
                .child("The d3-geo module provides geographic projections for mapping spherical coordinates (longitude, latitude) to planar coordinates (x, y)."),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Mercator Projection Example"),
                )
                .child({
                    let (x, y) = mercator.project(0.0, 0.0);
                    div()
                        .text_sm()
                        .text_color(rgb(0x333333))
                        .child(format!("Point (lon=0°, lat=0°) → (x={:.2}, y={:.2})", x, y))
                })
                .child({
                    let (x, y) = mercator.project(-74.0, 40.7);  // New York
                    div()
                        .text_sm()
                        .text_color(rgb(0x333333))
                        .child(format!("New York (lon=-74°, lat=40.7°) → (x={:.2}, y={:.2})", x, y))
                })
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Equirectangular Projection Example"),
                )
                .child({
                    let (x, y) = equirect.project(0.0, 0.0);
                    div()
                        .text_sm()
                        .text_color(rgb(0x333333))
                        .child(format!("Point (lon=0°, lat=0°) → (x={:.2}, y={:.2})", x, y))
                })
                .child({
                    let (x, y) = equirect.project(139.7, 35.7);  // Tokyo
                    div()
                        .text_sm()
                        .text_color(rgb(0x333333))
                        .child(format!("Tokyo (lon=139.7°, lat=35.7°) → (x={:.2}, y={:.2})", x, y))
                })
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Available Projections"),
                )
                .child(div().flex().flex_col().gap_2()
                    .child(div().text_sm().child("• Mercator - conformal cylindrical projection"))
                    .child(div().text_sm().child("• Equirectangular - simple equidistant projection"))
                    .child(div().text_sm().child("• Albers - conic equal-area projection"))
                    .child(div().text_sm().child("• Orthographic - azimuthal projection"))
                    .child(div().text_sm().child("• Stereographic - conformal azimuthal projection"))
                    .child(div().text_sm().child("• Transverse Mercator - rotated Mercator"))
                )
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Graticule Support"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x333333))
                        .child("Generate coordinate grid lines (latitude/longitude) for map backgrounds."),
                )
        )
}

use super::ShowcaseApp;
