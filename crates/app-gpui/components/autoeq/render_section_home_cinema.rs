// Section 7: Home Cinema Specific — VoG, phase alignment, multi-seat
// This file is include!()'d from render_body.rs, sharing its scope.
{
    let mut section = VStack::new().spacing(StackSpacing::Sm);

    section = section.child(
        VStack::new()
            .spacing(StackSpacing::None)
            .child(Text::section_header(translations.autoeq_form.home_cinema).color(theme.header_color))
            .child(
                Text::new(translations.autoeq_multi_seat_desc)
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            ),
    );

    // --- Voice of God ---
    {
        let mut vog_toggle = Toggle::new((base_id.clone(), "hc-vog-enabled"))
            .size(ToggleSize::Sm)
            .checked(config.v2.vog_enabled)
            .theme(toggle_theme.clone());

        if let Some(ref h) = on_vog_enabled_change_rc {
            let h = h.clone();
            vog_toggle = vog_toggle.on_change(move |v, w, cx| h(v, w, cx));
        }

        section = section.child(
            HStack::new()
                .justify(StackJustify::SpaceBetween)
                .child(
                    VStack::new()
                        .spacing(StackSpacing::None)
                .child(
                    Text::new(translations.autoeq_voice_of_god_correction)
                        .size(TextSize::Xs)
                        .color(theme.label_color),
                )
                .child(
                    Text::new(translations.autoeq_timbre_matching)
                        .size(TextSize::Xs)
                        .color(theme.description_color),
                ),
                )
                .child(vog_toggle),
        );

        if config.v2.vog_enabled {
            let ref_channel_options: Vec<SelectOption> = ["C", "L", "R"]
                .iter()
                .map(|ch| SelectOption::new(*ch, *ch))
                .collect();

            let mut ref_select = Select::new((base_id.clone(), "hc-vog-ref-channel"))
                .label(translations.autoeq_form.reference_channel)
                .options(ref_channel_options)
                .selected(&config.v2.vog_reference_channel)
                .is_open(ui_state.vog_reference_channel_open)
                .disabled(disabled)
                .size(SelectSize::Xs)
                .theme(theme.select_theme.clone());

            if let Some(ref h) = on_vog_reference_channel_toggle_rc {
                let h = h.clone();
                ref_select = ref_select.on_toggle(move |open, w, cx| h(open, w, cx));
            }
            if let Some(ref h) = on_vog_reference_channel_change_rc {
                let h = h.clone();
                ref_select = ref_select.on_change(move |val, w, cx| h(val.as_ref(), w, cx));
            }

            section = section.child(ref_select);
        }
    }

    // --- Phase Alignment ---
    if !hide_phase_alignment {
        let mut phase_toggle = Toggle::new((base_id.clone(), "hc-phase-enabled"))
            .size(ToggleSize::Sm)
            .checked(config.system_optimization.use_phase_alignment)
            .theme(toggle_theme.clone());

        if let Some(ref h) = on_use_phase_alignment_change_rc {
            let h = h.clone();
            phase_toggle = phase_toggle.on_change(move |v, w, cx| h(v, w, cx));
        }

        section = section.child(
            HStack::new()
                .justify(StackJustify::SpaceBetween)
                .child(
                    Text::new(translations.autoeq_phase_alignment)
                        .size(TextSize::Xs)
                        .color(theme.label_color),
                )
                .child(phase_toggle),
        );

        if config.system_optimization.use_phase_alignment {
            let mut min_freq_input = NumberInput::new((base_id.clone(), "hc-phase-min-freq"))
                .value(config.system_optimization.phase_min_freq)
                .min(20.0).max(1000.0).step(1.0).decimals(0)
                .label(translations.autoeq_form.min_frequency_hz)
                .size(NumberInputSize::Sm)
                .disabled(disabled)
                .theme(theme.number_input_theme.clone());

            if let Some(ref h) = on_phase_min_freq_change_rc {
                let h = h.clone();
                min_freq_input = min_freq_input.on_change(move |v, w, cx| h(v, w, cx));
            }

            let mut max_freq_input = NumberInput::new((base_id.clone(), "hc-phase-max-freq"))
                .value(config.system_optimization.phase_max_freq)
                .min(20.0).max(1000.0).step(1.0).decimals(0)
                .label(translations.autoeq_form.max_frequency_hz)
                .size(NumberInputSize::Sm)
                .disabled(disabled)
                .theme(theme.number_input_theme.clone());

            if let Some(ref h) = on_phase_max_freq_change_rc {
                let h = h.clone();
                max_freq_input = max_freq_input.on_change(move |v, w, cx| h(v, w, cx));
            }

            section = section.child(
                HStack::new().spacing(StackSpacing::Md).child(min_freq_input).child(max_freq_input),
            );

            let mut polarity_toggle = Toggle::new((base_id.clone(), "hc-phase-polarity"))
                .size(ToggleSize::Sm)
                .checked(config.system_optimization.phase_optimize_polarity)
                .theme(toggle_theme.clone());

            if let Some(ref h) = on_phase_optimize_polarity_change_rc {
                let h = h.clone();
                polarity_toggle = polarity_toggle.on_change(move |v, w, cx| h(v, w, cx));
            }

            section = section.child(
                HStack::new()
                    .justify(StackJustify::SpaceBetween)
                    .child(
                        Text::new(translations.autoeq_optimize_polarity)
                            .size(TextSize::Xs)
                            .color(theme.label_color),
                    )
                    .child(polarity_toggle),
            );

            let mut p_max_delay = NumberInput::new((base_id.clone(), "hc-phase-max-delay"))
                .value(config.system_optimization.phase_max_delay_ms)
                .min(ParamLimits::DELAY_MS.min).max(ParamLimits::DELAY_MS.max).step(ParamLimits::DELAY_MS.step)
                .decimals(1)
                .label(translations.autoeq_form.max_delay_ms)
                .size(NumberInputSize::Sm)
                .disabled(disabled)
                .theme(theme.number_input_theme.clone());

            if let Some(ref h) = on_phase_max_delay_ms_change_rc {
                let h = h.clone();
                p_max_delay = p_max_delay.on_change(move |v, w, cx| h(v, w, cx));
            }
            section = section.child(p_max_delay);
        }
    }

    // --- Multi-Seat Optimization ---
    if !hide_multi_seat {
        let mut multi_seat_toggle = Toggle::new((base_id.clone(), "hc-multi-seat-enabled"))
            .size(ToggleSize::Sm)
            .checked(config.system_optimization.use_multi_seat)
            .theme(toggle_theme.clone());

        if let Some(ref h) = on_use_multi_seat_change_rc {
            let h = h.clone();
            multi_seat_toggle = multi_seat_toggle.on_change(move |v, w, cx| h(v, w, cx));
        }

        section = section.child(
            HStack::new()
                .justify(StackJustify::SpaceBetween)
                .child(
                    Text::new(translations.autoeq_multi_seat)
                        .size(TextSize::Xs)
                        .color(theme.label_color),
                )
                .child(multi_seat_toggle),
        );

        if config.system_optimization.use_multi_seat {
            let strategy_options: Vec<SelectOption> = MULTI_SEAT_STRATEGY_OPTIONS
                .iter()
                .map(|(val, lbl)| SelectOption::new(*val, *lbl))
                .collect();

            let mut strategy_select = Select::new((base_id.clone(), "hc-multi-seat-strategy"))
            .label(translations.autoeq_form.strategy)
                .options(strategy_options)
                .selected(&config.system_optimization.multi_seat_strategy)
                .is_open(ui_state.multi_seat_strategy_open)
                .size(SelectSize::Xs)
                .theme(theme.select_theme.clone());

            if let Some(ref h) = on_multi_seat_strategy_toggle_rc {
                let h = h.clone();
                strategy_select = strategy_select.on_toggle(move |open, w, cx| h(open, w, cx));
            }
            if let Some(ref h) = on_multi_seat_strategy_change_rc {
                let h = h.clone();
                strategy_select = strategy_select.on_change(move |val, w, cx| h(val.as_ref(), w, cx));
            }

            section = section.child(strategy_select);

            if config.system_optimization.multi_seat_strategy == "primary" {
                let mut primary_seat_input = NumberInput::new((base_id.clone(), "hc-multi-seat-primary"))
                    .value(config.system_optimization.multi_seat_primary_seat as f64)
                    .min(0.0).max(16.0).step(1.0).decimals(0)
                .label(translations.autoeq_form.primary_seat)
                    .size(NumberInputSize::Sm)
                    .disabled(disabled)
                    .theme(theme.number_input_theme.clone());

                if let Some(ref h) = on_multi_seat_primary_seat_change_rc {
                    let h = h.clone();
                    primary_seat_input = primary_seat_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
                }
                section = section.child(primary_seat_input);
            }

            let mut dev_input = NumberInput::new((base_id.clone(), "hc-multi-seat-max-dev"))
                .value(config.system_optimization.multi_seat_max_deviation_db)
                .min(1.0).max(12.0).step(0.5).decimals(1)
                .label(translations.autoeq_form.max_deviation_db)
                .size(NumberInputSize::Sm)
                .disabled(disabled)
                .theme(theme.number_input_theme.clone());

            if let Some(ref h) = on_multi_seat_max_deviation_db_change_rc {
                let h = h.clone();
                dev_input = dev_input.on_change(move |v, w, cx| h(v, w, cx));
            }
            section = section.child(dev_input);
        }
    }

    Card::new().content(section)
}
