{
    // --- Asymmetric Loss ---
    if !hide_asymmetric_loss {
        let mut asymmetric_toggle = Toggle::new((base_id.clone(), "alg-asymmetric-loss"))
            .size(ToggleSize::Sm)
            .checked(config.algorithm.asymmetric_loss)
            .theme(toggle_theme.clone());

        if let Some(ref handler) = on_asymmetric_loss_change_rc {
            let h = handler.clone();
            asymmetric_toggle = asymmetric_toggle.on_change(move |v, w, cx| h(v, w, cx));
        }

        section = section.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .justify(StackJustify::SpaceBetween)
                .child(
                    VStack::new()
                        .spacing(StackSpacing::None)
                        .child(
                            Text::new(translations.autoeq_asymmetric_loss)
                                .size(TextSize::Xs)
                                .color(theme.label_color),
                        )
                        .child(
                            Text::new(translations.autoeq_penalize_peaks)
                                .size(TextSize::Xs)
                                .color(theme.description_color),
                        ),
                )
                .child(asymmetric_toggle),
        );
    }

    // --- Broadband Target Matching ---
    if !hide_broadband_matching {
        let mut broadband_toggle = Toggle::new((base_id.clone(), "alg-broadband"))
            .size(ToggleSize::Sm)
            .checked(config.v2.broadband_target_matching)
            .theme(toggle_theme.clone());

        if let Some(ref h) = on_broadband_target_matching_change_rc {
            let h = h.clone();
            broadband_toggle = broadband_toggle.on_change(move |v, w, cx| h(v, w, cx));
        }

        section = section.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .justify(StackJustify::SpaceBetween)
                .child(
                    VStack::new()
                        .spacing(StackSpacing::None)
                        .child(
                            Text::new(translations.autoeq_broadband_target)
                                .size(TextSize::Xs)
                                .color(theme.label_color),
                        )
                        .child(
                            Text::new(translations.autoeq_shelf_filters)
                                .size(TextSize::Xs)
                                .color(theme.description_color),
                        ),
                )
                .child(broadband_toggle),
        );
    }
}
