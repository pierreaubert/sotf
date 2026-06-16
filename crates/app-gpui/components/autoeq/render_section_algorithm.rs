// Section 8: Optimisation Algorithm Configuration — optimizer params, smoothing, seed
// This file is include!()'d from render_body.rs, sharing its scope.
{
    let mut section = VStack::new().spacing(StackSpacing::Sm);

    section = section.child(
        VStack::new()
            .spacing(StackSpacing::None)
            .child(
                Text::section_header("Optimisation Algorithm Configuration")
                    .color(theme.header_color),
            )
            .child(
                Text::new("Fine-tune the optimization engine")
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            ),
    );

    // Use the shared optimizer block
    let mut block_out = section;
    include!("render_block_optimizer.rs");
    section = block_out;

    // --- Smoothing ---
    if !hide_smoothing {
        section = section.child(Text::label("Smoothing").color(theme.header_color));

        // Psychoacoustic toggle (disabled when curve smoothing is on)
        let mut psycho_toggle = Toggle::new((base_id.clone(), "alg-psychoacoustic"))
            .size(ToggleSize::Sm)
            .checked(config.algorithm.psychoacoustic)
            .disabled(config.algorithm.smooth)
            .theme(toggle_theme.clone());

        if let Some(ref handler) = on_psychoacoustic_change_rc {
            let h = handler.clone();
            psycho_toggle = psycho_toggle.on_change(move |v, w, cx| h(v, w, cx));
        }

        section = section.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .justify(StackJustify::SpaceBetween)
                .child(
                    VStack::new()
                        .spacing(StackSpacing::None)
                        .child(Text::new("Psychoacoustic Smoothing").size(TextSize::Xs).color(theme.label_color))
                        .child(Text::new("1/48 oct bass, 1/6 oct treble").size(TextSize::Xs).color(theme.description_color)),
                )
                .child(psycho_toggle),
        );

        // Curve smoothing toggle (disabled when psychoacoustic is on)
        let mut smooth_toggle = Toggle::new((base_id.clone(), "alg-smooth"))
            .size(ToggleSize::Sm)
            .checked(config.algorithm.smooth)
            .disabled(config.algorithm.psychoacoustic)
            .theme(toggle_theme.clone());

        if let Some(ref handler) = on_smooth_change_rc {
            let h = handler.clone();
            smooth_toggle = smooth_toggle.on_change(move |v, w, cx| h(v, w, cx));
        }

        section = section.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .justify(StackJustify::SpaceBetween)
                .child(
                    VStack::new()
                        .spacing(StackSpacing::None)
                        .child(Text::new("Curve Smoothing").size(TextSize::Xs).color(theme.label_color))
                        .child(Text::new("Fixed-width octave smoothing").size(TextSize::Xs).color(theme.description_color)),
                )
                .child(smooth_toggle),
        );

        if config.algorithm.smooth {
            let mut smooth_n_input = NumberInput::new((base_id.clone(), "alg-smooth-n"))
                .value(config.algorithm.smooth_n as f64)
                .min(ParamLimits::SMOOTH_N.min)
                .max(ParamLimits::SMOOTH_N.max)
                .step(ParamLimits::SMOOTH_N.step)
                .decimals(0)
                .label("Smooth Window (1/N oct)")
                .size(NumberInputSize::Sm)
                .width(120.0)
                .disabled(disabled)
                .theme(theme.number_input_theme.clone());

            if let Some(ref handler) = on_smooth_n_change_rc {
                let h = handler.clone();
                smooth_n_input = smooth_n_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
            }

            section = section.child(smooth_n_input);
        }
    }

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
                        .child(Text::new("Asymmetric Loss").size(TextSize::Xs).color(theme.label_color))
                        .child(Text::new("Penalize peaks more than dips").size(TextSize::Xs).color(theme.description_color)),
                )
                .child(asymmetric_toggle),
        );
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
                .child(Text::new("Reproducible Seed").size(TextSize::Xs).color(theme.label_color))
                .child(seed_toggle),
        );

        if config.v2.seed_enabled {
            let mut seed_input = NumberInput::new((base_id.clone(), "alg-seed-value"))
                .value(config.v2.seed as f64)
                .min(ParamLimits::SEED.min)
                .max(ParamLimits::SEED.max)
                .step(ParamLimits::SEED.step)
                .decimals(0)
                .label("Seed")
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
                        .child(Text::new("Broadband Target Matching").size(TextSize::Xs).color(theme.label_color))
                        .child(Text::new("Shelf filters for broad tonal balance").size(TextSize::Xs).color(theme.description_color)),
                )
                .child(broadband_toggle),
        );
    }

    Card::new().content(section)
}
