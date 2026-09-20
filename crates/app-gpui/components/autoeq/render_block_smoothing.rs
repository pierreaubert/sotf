// --- Smoothing ---
{
    section = section.child(
        Text::label(translations.autoeq_form.parameters.smoothing)
            .color(theme.header_color),
    );

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
                    .child(
                        Text::new(translations.autoeq_psychoacoustic_smoothing)
                            .size(TextSize::Xs)
                            .color(theme.label_color),
                    )
                    .child(
                        Text::new(translations.autoeq_form.parameters.smoothing_resolution)
                            .size(TextSize::Xs)
                            .color(theme.description_color),
                    ),
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
                    .child(
                        Text::new(translations.autoeq_curve_smoothing)
                            .size(TextSize::Xs)
                            .color(theme.label_color),
                    )
                    .child(
                        Text::new(translations.autoeq_fixed_octave_smoothing)
                            .size(TextSize::Xs)
                            .color(theme.description_color),
                    ),
            )
            .child(smooth_toggle),
    );

    if config.algorithm.smooth {
        let mut smooth_n_input = NumberInput::new((base_id.clone(), "alg-smooth-n"))
            .scroll_requires_alt(true)
            .value(config.algorithm.smooth_n as f64)
            .min(ParamLimits::SMOOTH_N.min)
            .max(ParamLimits::SMOOTH_N.max)
            .step(ParamLimits::SMOOTH_N.step)
            .decimals(0)
            .label(translations.autoeq_form.parameters.smooth_window_oct)
            .size(NumberInputSize::Sm)
            .width(120.0)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

        if let Some(ref handler) = on_smooth_n_change_rc {
            let h = handler.clone();
            smooth_n_input =
                smooth_n_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
        }

        section = section.child(smooth_n_input);
    }
}
