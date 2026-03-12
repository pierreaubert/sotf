impl Showcase {
    fn render_popover_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let section_title = cx.t(TranslationKey::SectionPopover);

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(self.section_header(section_title))
            .child(
                Text::new("Popover placement options (shown as badges, actual popover requires a trigger element):")
                    .muted(true),
            )
            // Placement options
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(Badge::new("Top").variant(BadgeVariant::Primary))
                            .child(Badge::new("TopStart").variant(BadgeVariant::Primary))
                            .child(Badge::new("TopEnd").variant(BadgeVariant::Primary)),
                    )
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(Badge::new("Bottom (default)").variant(BadgeVariant::Info))
                            .child(Badge::new("BottomStart").variant(BadgeVariant::Info))
                            .child(Badge::new("BottomEnd").variant(BadgeVariant::Info)),
                    )
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(Badge::new("Left").variant(BadgeVariant::Success))
                            .child(Badge::new("Right").variant(BadgeVariant::Success)),
                    ),
            )
    }
}
