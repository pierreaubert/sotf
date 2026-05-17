impl SpinoramaApp {
    fn render_legend(&self, colors: &HashMap<&'static str, D3Color>, ds: &DesignSystem, theme: &Theme) -> Div {
        div()
            .flex()
            .flex_wrap()
            .gap(px(ds.spacing.section_gap))
            .p(px(ds.spacing.card_padding))
            .bg(theme.muted)
            .rounded(px(ds.corners.md))
            .children(CEA2034_CURVES.iter().map(|&name| {
                let color = colors
                    .get(name)
                    .cloned()
                    .unwrap_or(D3Color::rgb(128, 128, 128));
                let (r, g, b) = (
                    (color.r * 255.0) as u32,
                    (color.g * 255.0) as u32,
                    (color.b * 255.0) as u32,
                );
                let font_config = GlyphTextConfig::horizontal((12.0 * self.font_scale()).round(), Hsla::from(theme.text_primary));

                div()
                    .flex()
                    .items_center()
                    .gap(px(ds.spacing.control_gap))
                    .child(div().w(px(16.0)).h(px(3.0)).bg(rgb(r << 16 | g << 8 | b)))
                    .child(render_glyph_text(name, &font_config))
            }))
    }
}
