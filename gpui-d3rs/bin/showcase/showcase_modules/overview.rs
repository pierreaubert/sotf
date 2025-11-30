use gpui::*;

pub fn render(app: &ShowcaseApp) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_6()
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .child("Welcome to d3rs"),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0x666666))
                .max_w(px(600.0))
                .child("d3rs is a D3.js-inspired plotting library for GPUI. It brings familiar D3 concepts like scales, axes, and shape generators to Rust applications built with GPUI."),
        )
        .child(
            div()
                .mt_4()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Features"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .ml_4()
                .child(feature_item("Scales", "Linear, logarithmic, ordinal, and band scales"))
                .child(feature_item("Axes", "Customizable axes with tick formatting"))
                .child(feature_item("Charts", "Bar charts, line charts, scatter plots"))
                .child(feature_item("Colors", "Color schemes and interpolation"))
                .child(feature_item("Shapes", "Arcs, pies, symbols, curves, and more"))
                .child(feature_item("Data", "Statistics, binning, and transformations")),
        )
        .child(
            div()
                .mt_6()
                .p_4()
                .bg(rgb(0xf5f5f5))
                .rounded_md()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Use the sidebar to explore different demos"),
                ),
        )
}

pub fn feature_item(title: &str, desc: &str) -> Div {
    div()
        .flex()
        .gap_2()
        .child(
            div()
                .text_color(rgb(0x007acc))
                .font_weight(FontWeight::SEMIBOLD)
                .child(format!("{title}:")),
        )
        .child(div().text_color(rgb(0x666666)).child(desc.to_string()))
}

use super::ShowcaseApp;
