// intentional-file: fixed pixel values here are graph and plugin control geometry.
use crate::components::design::Ds;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

/// Drag information for plugin reordering
#[derive(Clone)]
pub struct PluginDragInfo {
    pub source_index: usize,
    pub name: String,
    pub color: Rgba,
    pub icon: &'static str,
    pub surface: Rgba,
    pub text_on_accent: Rgba,
}

impl Render for PluginDragInfo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        // Drag preview — matches the Ozone-style card
        div()
            .w(rems(7.0))
            .h(rems(4.0))
            .flex()
            .flex_row()
            .rounded(d.r_md)
            .border_1()
            .border_color(self.color)
            .bg(Theme::opacity_20pct(self.surface))
            .shadow_lg()
            .opacity(0.85)
            // Left color bar
            .child(div().w(px(3.0)).h_full().bg(self.color).rounded_l_md())
            // Content
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(
                        div()
                            .px(d.pad_y)
                            .pt(d.pad_y_half)
                            .text_size(d.text_xs)
                            .text_color(self.color)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.name.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(d.text_lg)
                            .text_color(self.color)
                            .child(self.icon),
                    ),
            )
    }
}
