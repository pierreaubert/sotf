// Section 8: Optimisation Algorithm Configuration — optimizer params, smoothing, seed
// This file is include!()'d from render_body.rs, sharing its scope.
{
    let mut section = VStack::new().spacing(StackSpacing::Sm);

    section = section.child(
        VStack::new()
            .spacing(StackSpacing::None)
            .child(
                Text::section_header(translations.autoeq_form.optimizer_configuration)
                    .color(theme.header_color),
            )
            .child(
                Text::new(translations.autoeq_fine_tune_optimizer)
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            ),
    );

    // Use the shared optimizer block
    let mut block_out = section;
    include!("render_block_optimizer.rs");
    section = block_out;

    if hide_room_sections && !hide_smoothing {
        include!("render_block_smoothing.rs");
    }

    if hide_room_sections {
        include!("render_block_goal_loss.rs");
    }

    // --- Seed ---
    {
        let mut seed_toggle = Toggle::new((base_id.clone(), "alg-seed-enabled"))
            .size(ToggleSize::Sm)
            .checked(config.v2.seed_enabled)
            .theme(toggle_theme.clone());

        if let Some(ref h) = on_seed_enabled_change_rc {
            let h = h.clone();
            seed_toggle = seed_toggle.on_change(move |v, w, cx| h(v, w, cx));
        }

        section = section.child(
            HStack::new()
                .justify(StackJustify::SpaceBetween)
                .child(
                    Text::new(translations.autoeq_reproducible_seed)
                        .size(TextSize::Xs)
                        .color(theme.label_color),
                )
                .child(seed_toggle),
        );

        if config.v2.seed_enabled {
            let mut seed_input = NumberInput::new((base_id.clone(), "alg-seed-value"))
                .scroll_requires_alt(true)
                .value(config.v2.seed as f64)
                .min(ParamLimits::SEED.min)
                .max(ParamLimits::SEED.max)
                .step(ParamLimits::SEED.step)
                .decimals(0)
                .label(translations.autoeq_form.parameters.seed)
                .size(NumberInputSize::Sm)
                .width(120.0)
                .disabled(disabled)
                .theme(theme.number_input_theme.clone());

            if let Some(ref h) = on_seed_change_rc {
                let h = h.clone();
                seed_input = seed_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
            }

            section = section.child(seed_input);
        }
    }

    Card::new().content(section)
}
