// Section 5: Delay Correction — delay and group delay optimization
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

    if config.allow_delay {
        // Group Delay optimization
        let mut gd_toggle = Toggle::new((base_id.clone(), "dc-gd-enabled"))
            .size(ToggleSize::Sm)
            .checked(config.gd_opt_enabled)
            .theme(toggle_theme.clone());

        if let Some(ref h) = on_gd_opt_enabled_change_rc {
            let h = h.clone();
            gd_toggle = gd_toggle.on_change(move |v, w, cx| h(v, w, cx));
        }

        section = section.child(
            HStack::new()
                .justify(StackJustify::SpaceBetween)
                .child(
                    VStack::new()
                        .spacing(StackSpacing::None)
                        .child(Text::new("Group Delay Correction").size(TextSize::Xs).color(theme.label_color))
                        .child(Text::new("Align group delay at crossover").size(TextSize::Xs).color(theme.description_color)),
                )
                .child(gd_toggle),
        );

        if config.gd_opt_enabled {
            let mut gd_target_input = NumberInput::new((base_id.clone(), "dc-gd-target-ms"))
                .value(config.gd_opt_target_ms)
                .min(ParamLimits::GD_TARGET_MS.min)
                .max(ParamLimits::GD_TARGET_MS.max)
                .step(ParamLimits::GD_TARGET_MS.step)
                .decimals(1)
                .label("Target (ms)")
                .size(NumberInputSize::Sm)
                .width(120.0)
                .disabled(disabled)
                .theme(theme.number_input_theme.clone());

            if let Some(ref h) = on_gd_opt_target_ms_change_rc {
                let h = h.clone();
                gd_target_input = gd_target_input.on_change(move |v, w, cx| h(v, w, cx));
            }

            section = section.child(gd_target_input);
        }
    }

    Card::new().content(section)
}
