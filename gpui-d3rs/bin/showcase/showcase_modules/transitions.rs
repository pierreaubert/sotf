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
                .child("Transitions Demo"),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0x666666))
                .max_w(px(700.0))
                .child("The d3-transition module provides smooth transitions for animating values over time with easing functions."),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_4()
                .p_6()
                .bg(rgb(0xf5f5f5))
                .rounded_lg()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Basic Transition Example"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x333333))
                        .child("A transition from 0.0 to 100.0 over 1 second:"),
                )
                .child(
                    div()
                        .font_family("monospace")
                        .text_sm()
                        .p_4()
                        .bg(rgb(0xffffff))
                        .rounded_md()
                        .child("let mut transition = Transition::new()\n    .duration(1000.0)\n    .ease(ease_cubic_in_out)\n    .from_to(0.0, 100.0);"),
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
                        .child("Key Features"),
                )
                .child(div().flex().flex_col().gap_2()
                    .child(div().text_sm().child("• Easing functions: linear, cubic, elastic, bounce, back, etc."))
                    .child(div().text_sm().child("• Delayed transitions with .delay()"))
                    .child(div().text_sm().child("• Lifecycle callbacks: on_start, on_end, on_interrupt"))
                    .child(div().text_sm().child("• TransitionManager for multiple named transitions"))
                    .child(div().text_sm().child("• Automatic interruption and replacement"))
                )
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x666666))
                .italic()
                .child("Run 'cargo run --example transition_demo --no-default-features' to see animated examples."),
        )
}

use super::ShowcaseApp;
