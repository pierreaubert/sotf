// Section 5: Delay Correction — inter-speaker time alignment
// This file is include!()'d from render_body.rs, sharing its scope.
{
    let mut section = VStack::new().spacing(StackSpacing::Sm);

    section = section.child(
        VStack::new()
            .spacing(StackSpacing::None)
            .child(
                Text::new("Delay Correction")
                    .size(TextSize::Sm)
                    .weight(TextWeight::Semibold)
                    .color(theme.header_color),
            )
            .child(
                Text::new("Requires a computer or HW interface that supports delay")
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            ),
    );

    // Allow Delay toggle
    let mut delay_toggle = Toggle::new((base_id.clone(), "dc-allow-delay"))
        .size(ToggleSize::Sm)
        .checked(config.allow_delay)
        .theme(toggle_theme.clone());

    if let Some(ref h) = on_allow_delay_change_rc {
        let h = h.clone();
        delay_toggle = delay_toggle.on_change(move |v, w, cx| h(v, w, cx));
    }

    section = section.child(
        HStack::new()
            .justify(StackJustify::SpaceBetween)
            .child(
                VStack::new()
                    .spacing(StackSpacing::None)
                    .child(Text::new("Enable Delay Correction").size(TextSize::Xs).color(theme.label_color))
                    .child(Text::new("Enable inter-speaker time alignment").size(TextSize::Xs).color(theme.description_color)),
            )
            .child(delay_toggle),
    );

    Card::new().content(section)
}
