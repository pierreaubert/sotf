impl Showcase {
    fn render_progress_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let section_title = cx.t(TranslationKey::SectionProgress);

    VStack::new()
        .spacing(StackSpacing::Lg)
        .child(self.section_header(section_title))
        // Linear Progress
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::new("Linear Progress").weight(TextWeight::Medium))
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Md)
                        .child(
                            div()
                                .w(px(300.0))
                                .child(Progress::new(0.25).size(ProgressSize::Sm)),
                        )
                        .child(
                            div()
                                .w(px(300.0))
                                .child(Progress::new(0.50).size(ProgressSize::Md)),
                        )
                        .child(
                            div()
                                .w(px(300.0))
                                .child(Progress::new(0.75).size(ProgressSize::Lg)),
                        )
                        .child(
                            div()
                                .w(px(300.0))
                                .child(Progress::new(0.90).variant(ProgressVariant::Success)),
                        ),
                ),
        )
        // Circular Progress
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::new("Circular Progress").weight(TextWeight::Medium))
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Lg)
                        .child(CircularProgress::new(0.25).size(px(32.0)))
                        .child(CircularProgress::new(0.50).size(px(48.0)))
                        .child(CircularProgress::new(0.75).size(px(64.0)))
                        .child(
                            CircularProgress::new(0.90)
                                .size(px(48.0))
                                .variant(ProgressVariant::Success),
                        ),
                ),
        )
    }
}
