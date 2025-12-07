impl Showcase {
    fn render_accordion_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let section_title = cx.t(TranslationKey::SectionAccordion);
    let getting_started = cx.t(TranslationKey::AccordionGettingStarted);
    let features = cx.t(TranslationKey::AccordionFeatures);
    let configuration = cx.t(TranslationKey::AccordionConfiguration);

    VStack::new()
            .spacing(StackSpacing::Lg)
            .child(self.section_header(section_title))
            .child(Text::new("Expandable content sections:").muted(true))
            .child(
                div()
                    .w(px(400.0))
                    .child(
                        Accordion::new()
                            .mode(AccordionMode::Single)
                            .items(vec![
                                AccordionItem::new("section-1", getting_started)
                                    .content("Welcome to the UI Kit! This accordion demonstrates expandable sections that can contain any content."),
                                AccordionItem::new("section-2", features)
                                    .content("* Multiple accordion modes\n* Custom themes\n* Keyboard navigation\n* Animated transitions"),
                                AccordionItem::new("section-3", configuration)
                                    .content("Accordions support single or multiple expansion modes. Use the mode() method to configure behavior."),
                            ])
                            .expanded(vec!["section-1".into()]),
                    ),
            )
            // Mode info
            .child(
                HStack::new()
                    .spacing(StackSpacing::Lg)
                    .child(Badge::new("Single Mode").variant(BadgeVariant::Primary))
                    .child(Badge::new("Multiple Mode").variant(BadgeVariant::Default))
                    .child(Badge::new("Collapsible").variant(BadgeVariant::Default)),
            )
    }
}
