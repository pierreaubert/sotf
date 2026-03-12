impl Showcase {
    fn render_qr_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let section_title = cx.t(TranslationKey::SectionQrCode);

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(self.section_header(section_title))
            .child(
                HStack::new()
                    .spacing(StackSpacing::Lg)
                    .align(StackAlign::End)
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(Text::new("Default").weight(TextWeight::Medium))
                            .child(QrCode::new("https://example.com")),
                    )
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(Text::new("Small").weight(TextWeight::Medium))
                            .child(QrCode::new("https://example.com").size(px(120.0))),
                    )
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(Text::new("Custom Colors").weight(TextWeight::Medium))
                            .child(
                                QrCode::new("https://example.com")
                                    .size(px(150.0))
                                    .fg(rgba(0x2da44eff))
                                    .bg(rgba(0x1a1a2eff)),
                            ),
                    ),
            )
    }
}
