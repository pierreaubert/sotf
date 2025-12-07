impl Showcase {
    fn render_layout_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let section_title = cx.t(TranslationKey::SectionLayout);
    let theme = cx.theme();

    VStack::new()
        .spacing(StackSpacing::Lg)
        .child(self.section_header(section_title))
        // HStack and VStack
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::new("HStack & VStack").weight(TextWeight::Medium))
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Lg)
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(
                                    div()
                                        .p_2()
                                        .bg(theme.surface_hover)
                                        .rounded_md()
                                        .child("VStack Item 1"),
                                )
                                .child(
                                    div()
                                        .p_2()
                                        .bg(theme.surface_hover)
                                        .rounded_md()
                                        .child("VStack Item 2"),
                                )
                                .child(
                                    div()
                                        .p_2()
                                        .bg(theme.surface_hover)
                                        .rounded_md()
                                        .child("VStack Item 3"),
                                ),
                        )
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(div().p_2().bg(theme.surface_hover).rounded_md().child("H1"))
                                .child(div().p_2().bg(theme.surface_hover).rounded_md().child("H2"))
                                .child(
                                    div().p_2().bg(theme.surface_hover).rounded_md().child("H3"),
                                ),
                        ),
                ),
        )
        // Spacer demonstration
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::new("Spacer").weight(TextWeight::Medium))
                .child(
                    HStack::new().spacing(StackSpacing::Md).child(
                        div()
                            .w(px(400.0))
                            .p_3()
                            .bg(theme.surface)
                            .rounded_md()
                            .flex()
                            .items_center()
                            .child(Text::new("Left"))
                            .child(Spacer::new())
                            .child(Text::new("Right")),
                    ),
                ),
        )
        // Dividers
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::new("Dividers").weight(TextWeight::Medium))
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Md)
                        .child(
                            div()
                                .w(px(300.0))
                                .child(Divider::new().color(theme.border_hover).build()),
                        )
                        .child(
                            div().w(px(300.0)).child(
                                Divider::new()
                                    .thickness(px(2.0))
                                    .color(theme.accent)
                                    .build(),
                            ),
                        )
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Md)
                                .child(Text::new("Left"))
                                .child(
                                    div().h(px(20.0)).child(
                                        Divider::vertical().color(theme.border_hover).build(),
                                    ),
                                )
                                .child(Text::new("Right")),
                        ),
                ),
        )
    }
}
