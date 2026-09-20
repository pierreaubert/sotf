{
    let base_id = id.clone();
    let detail_level = ui_state.detail_level;
    {
        let level_label = match detail_level {
            DetailLevel::Simple => translations.autoeq_form.sections.simple,
            DetailLevel::Intermediate => translations.autoeq_form.sections.customize,
            DetailLevel::Expert => translations.autoeq_form.sections.all_parameters,
        };

        let next_label = match detail_level {
            DetailLevel::Simple => translations.autoeq_form.sections.customize,
            DetailLevel::Intermediate => translations.autoeq_form.sections.all_parameters,
            DetailLevel::Expert => translations.autoeq_form.sections.simple,
        };

        let mut detail_row = HStack::new()
            .spacing(StackSpacing::Sm)
            .align(StackAlign::Center)
            .justify(StackJustify::SpaceBetween);

        detail_row = detail_row.child(Text::label(level_label).color(theme.header_color));

        let mut toggle_btn = Button::new((base_id.clone(), "detail-toggle"), next_label)
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::Xs);
        if let Some(ref h) = on_detail_level_change_rc {
            let h = h.clone();
            let next = match detail_level {
                DetailLevel::Simple => "intermediate",
                DetailLevel::Intermediate => "expert",
                DetailLevel::Expert => "simple",
            };
            toggle_btn = toggle_btn.on_click(move |w, cx| {
                h(next, w, cx);
            });
        }
        #[cfg(feature = "dev-api")]
        let toggle_btn = {
            use crate::app::dev_api::DevTrackExt;
            toggle_btn.dev_track("autoeq.detail")
        };
        detail_row = detail_row.child(toggle_btn);
        detail_row
    }
}
