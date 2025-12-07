impl Showcase {
    fn render_tabs_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let section_title = cx.t(TranslationKey::SectionTabs);

    VStack::new()
        .spacing(StackSpacing::Lg)
        .child(self.section_header(section_title))
        // Underline Tabs
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::new("Underline Variant").weight(TextWeight::Medium))
                .child(
                    Tabs::new()
                        .variant(TabVariant::Underline)
                        .selected_index(self.selected_tab)
                        .tabs(vec![
                            TabItem::new("tab-1", "Overview").icon("O"),
                            TabItem::new("tab-2", "Analytics").icon("A"),
                            TabItem::new("tab-3", "Reports").icon("R"),
                            TabItem::new("tab-4", "Settings").icon("S"),
                        ]),
                ),
        )
        // Pills Tabs
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::new("Pills Variant").weight(TextWeight::Medium))
                .child(
                    Tabs::new()
                        .variant(TabVariant::Pills)
                        .selected_index(1)
                        .tabs(vec![
                            TabItem::new("pill-1", "All"),
                            TabItem::new("pill-2", "Active"),
                            TabItem::new("pill-3", "Completed"),
                        ]),
                ),
        )
        // Enclosed Tabs
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::new("Enclosed Variant").weight(TextWeight::Medium))
                .child(
                    Tabs::new()
                        .variant(TabVariant::Enclosed)
                        .selected_index(0)
                        .tabs(vec![
                            TabItem::new("enc-1", "Files"),
                            TabItem::new("enc-2", "Folders"),
                            TabItem::new("enc-3", "Trash").badge("3"),
                        ]),
                ),
        )
    }
}
