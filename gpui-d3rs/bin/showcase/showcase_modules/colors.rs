use d3rs::color::{ColorScheme, D3Color};
use gpui::*;

pub fn render(app: &ShowcaseApp) -> Div {
    let category10 = ColorScheme::category10();

    div()
        .flex()
        .flex_col()
        .gap_6()
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .child("Colors Demo"),
        )
        // Category10
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Category10 Color Scheme"),
                )
                .child(div().flex().gap_2().children((0..10).map(|i| {
                    let color = category10.color(i);
                    div()
                        .w(px(40.0))
                        .h(px(40.0))
                        .rounded_md()
                        .bg(color.to_rgba())
                }))),
        )
        // Interpolation
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Color Interpolation (Red -> Blue)"),
                )
                .child(div().flex().gap_1().children((0..20).map(|i| {
                    let t = i as f32 / 19.0;
                    let red = D3Color::rgb(255, 0, 0);
                    let blue = D3Color::rgb(0, 0, 255);
                    let color = red.interpolate(&blue, t);
                    div().w(px(20.0)).h(px(40.0)).bg(color.to_rgba())
                }))),
        )
        // HSL Interpolation
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("HSL Gradient (Hue 0-360)"),
                )
                .child(div().flex().gap_1().children((0..36).map(|i| {
                    let hue = i as f32 * 10.0;
                    let color = D3Color::from_hsl(hue, 0.8, 0.5);
                    div().w(px(12.0)).h(px(40.0)).bg(color.to_rgba())
                }))),
        )
        // Lighten/Darken
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Lighten / Darken"),
                )
                .child(div().flex().gap_1().children((0..11).map(|i| {
                    let base = D3Color::rgb(0, 122, 204);
                    let amount = (i as f32 - 5.0) / 5.0;
                    let color = if amount < 0.0 {
                        base.darken(-amount)
                    } else {
                        base.lighten(amount)
                    };
                    div().w(px(36.0)).h(px(40.0)).bg(color.to_rgba())
                }))),
        )
}

use super::ShowcaseApp;
