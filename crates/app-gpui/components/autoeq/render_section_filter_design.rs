// Section 4: Filter Design — sample rate, IIR/FIR params, crossover, bass management
// This file is include!()'d from render_body.rs, sharing its scope.
{
    let mut section = VStack::new().spacing(StackSpacing::Sm);

    // Header
    section = section.child(
        VStack::new()
            .spacing(StackSpacing::None)
            .child(Text::section_header("Filter Design").color(theme.header_color))
            .child(
                Text::new("Configure filter characteristics and frequency ranges")
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            ),
    );

    // Determine which subsections to show
    let is_iir = matches!(config.eq_design.opt_mode.as_str(), "iir" | "mixed" | "mixed_phase");
    let is_fir = matches!(config.eq_design.opt_mode.as_str(), "fir" | "mixed" | "mixed_phase");
    let is_mixed = matches!(config.eq_design.opt_mode.as_str(), "mixed" | "mixed_phase");

    // --- IIR Subsection ---
    if is_iir {
        section = section.child(Text::label("IIR Parameters").color(theme.header_color));

        // Sample rate + Num filters (side by side)
        let mut sr_input = NumberInput::new((base_id.clone(), "fd-sample-rate"))
            .value(config.eq_design.sample_rate as f64)
            .min(ParamLimits::SAMPLE_RATE.min)
            .max(ParamLimits::SAMPLE_RATE.max)
            .step(ParamLimits::SAMPLE_RATE.step)
            .decimals(0)
            .label("Sample Rate (Hz)")
            .size(NumberInputSize::Sm)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

        if let Some(ref handler) = on_sample_rate_change_rc {
            let h = handler.clone();
            sr_input = sr_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
        }

        let mut nf_input = NumberInput::new((base_id.clone(), "fd-num-filters"))
            .value(config.eq_design.num_filters as f64)
            .min(ParamLimits::NUM_FILTERS.min)
            .max(ParamLimits::NUM_FILTERS.max)
            .step(ParamLimits::NUM_FILTERS.step)
            .decimals(0)
            .label("Number of Filters")
            .size(NumberInputSize::Sm)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

        if let Some(ref handler) = on_num_filters_change_rc {
            let h = handler.clone();
            nf_input = nf_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
        }

        section = section.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(sr_input)
                .child(nf_input),
        );

        // PEQ Model
        let peq_options: Vec<SelectOption> = PEQ_MODEL_OPTIONS
            .iter()
            .map(|(val, lbl)| SelectOption::new(*val, *lbl))
            .collect();

        let mut peq_select = Select::new((base_id.clone(), "fd-peq-model"))
            .label("Filter Type")
            .options(peq_options)
            .selected(&config.eq_design.peq_model)
            .is_open(ui_state.peq_model_open)
            .disabled(disabled)
            .size(SelectSize::Xs)
            .theme(theme.select_theme.clone());

        if let Some(ref h) = on_peq_model_toggle_rc {
            let h = h.clone();
            peq_select = peq_select.on_toggle(move |open, w, cx| h(open, w, cx));
        }
        if let Some(ref h) = on_peq_model_change_rc {
            let h = h.clone();
            peq_select = peq_select.on_change(move |val, w, cx| h(val.as_ref(), w, cx));
        }

        section = section.child(peq_select);

        // Frequency range
        let mut min_freq_input = NumberInput::new((base_id.clone(), "fd-min-freq"))
            .value(config.eq_design.min_freq)
            .min(ParamLimits::FREQUENCY.min)
            .max(ParamLimits::FREQUENCY.max)
            .step(ParamLimits::FREQUENCY.step)
            .decimals(0)
            .label("Min Freq (Hz)")
            .size(NumberInputSize::Sm)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

        if let Some(ref h) = on_min_freq_change_rc {
            let h = h.clone();
            min_freq_input = min_freq_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        let mut max_freq_input = NumberInput::new((base_id.clone(), "fd-max-freq"))
            .value(config.eq_design.max_freq)
            .min(ParamLimits::FREQUENCY.min)
            .max(ParamLimits::FREQUENCY.max)
            .step(ParamLimits::FREQUENCY.step)
            .decimals(0)
            .label("Max Freq (Hz)")
            .size(NumberInputSize::Sm)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

        if let Some(ref h) = on_max_freq_change_rc {
            let h = h.clone();
            max_freq_input = max_freq_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        section = section.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(min_freq_input)
                .child(max_freq_input),
        );

        // Q range
        let mut min_q_input = NumberInput::new((base_id.clone(), "fd-min-q"))
            .value(config.eq_design.min_q)
            .min(ParamLimits::Q.min)
            .max(ParamLimits::Q.max)
            .step(ParamLimits::Q.step)
            .decimals(1)
            .label("Min Q")
            .size(NumberInputSize::Sm)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

        if let Some(ref h) = on_min_q_change_rc {
            let h = h.clone();
            min_q_input = min_q_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        let mut max_q_input = NumberInput::new((base_id.clone(), "fd-max-q"))
            .value(config.eq_design.max_q)
            .min(ParamLimits::Q.min)
            .max(ParamLimits::Q.max)
            .step(ParamLimits::Q.step)
            .decimals(1)
            .label("Max Q")
            .size(NumberInputSize::Sm)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

        if let Some(ref h) = on_max_q_change_rc {
            let h = h.clone();
            max_q_input = max_q_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        section = section.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(min_q_input)
                .child(max_q_input),
        );

        // dB range
        let mut min_db_input = NumberInput::new((base_id.clone(), "fd-min-db"))
            .value(config.eq_design.min_db)
            .min(ParamLimits::DB.min)
            .max(ParamLimits::DB.max)
            .step(ParamLimits::DB.step)
            .decimals(1)
            .label("Min dB")
            .size(NumberInputSize::Sm)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

        if let Some(ref h) = on_min_db_change_rc {
            let h = h.clone();
            min_db_input = min_db_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        let mut max_db_input = NumberInput::new((base_id.clone(), "fd-max-db"))
            .value(config.eq_design.max_db)
            .min(ParamLimits::DB.min)
            .max(ParamLimits::DB.max)
            .step(ParamLimits::DB.step)
            .decimals(1)
            .label("Max dB")
            .size(NumberInputSize::Sm)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

        if let Some(ref h) = on_max_db_change_rc {
            let h = h.clone();
            max_db_input = max_db_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        section = section.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(min_db_input)
                .child(max_db_input),
        );

        // Spacing weight + min spacing
        let mut sw_input = NumberInput::new((base_id.clone(), "fd-spacing-weight"))
            .value(config.eq_design.spacing_weight)
            .min(ParamLimits::SPACING_WEIGHT.min)
            .max(ParamLimits::SPACING_WEIGHT.max)
            .step(ParamLimits::SPACING_WEIGHT.step)
            .decimals(1)
            .label("Spacing Weight")
            .size(NumberInputSize::Sm)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

        if let Some(ref h) = on_spacing_weight_change_rc {
            let h = h.clone();
            sw_input = sw_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        let mut ms_input = NumberInput::new((base_id.clone(), "fd-min-spacing"))
            .value(config.eq_design.min_spacing_oct)
            .min(ParamLimits::MIN_SPACING_OCT.min)
            .max(ParamLimits::MIN_SPACING_OCT.max)
            .step(ParamLimits::MIN_SPACING_OCT.step)
            .decimals(2)
            .label("Min Spacing (oct)")
            .size(NumberInputSize::Sm)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

        if let Some(ref h) = on_min_spacing_oct_change_rc {
            let h = h.clone();
            ms_input = ms_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        section = section.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(sw_input)
                .child(ms_input),
        );
    }

    // --- FIR Subsection ---
    if is_fir {
        section = section.child(Text::label("FIR Parameters").color(theme.header_color));

        let mut taps_input = NumberInput::new((base_id.clone(), "fd-fir-taps"))
            .value(config.eq_design.fir_taps as f64)
            .min(ParamLimits::FIR_TAPS.min)
            .max(ParamLimits::FIR_TAPS.max)
            .step(ParamLimits::FIR_TAPS.step)
            .decimals(0)
            .label(format!("Taps (latency: {fir_latency_ms:.1} ms)"))
            .size(NumberInputSize::Sm)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

        if let Some(ref h) = on_fir_taps_change_rc {
            let h = h.clone();
            taps_input = taps_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
        }

        section = section.child(taps_input);

        // FIR Phase select
        let phase_options: Vec<SelectOption> = FIR_PHASE_OPTIONS
            .iter()
            .map(|(val, lbl)| SelectOption::new(*val, *lbl))
            .collect();

        let mut phase_select = Select::new((base_id.clone(), "fd-fir-phase"))
            .label("Regularization")
            .options(phase_options)
            .selected(&config.eq_design.fir_phase)
            .is_open(ui_state.fir_phase_open)
            .disabled(disabled)
            .size(SelectSize::Xs)
            .theme(theme.select_theme.clone());

        if let Some(ref h) = on_fir_phase_toggle_rc {
            let h = h.clone();
            phase_select = phase_select.on_toggle(move |open, w, cx| h(open, w, cx));
        }
        if let Some(ref h) = on_fir_phase_change_rc {
            let h = h.clone();
            phase_select = phase_select.on_change(move |val, w, cx| h(val.as_ref(), w, cx));
        }

        section = section.child(phase_select);
    }

    // --- Crossover Subsection ---
    if is_mixed {
        section =
            section.child(Text::label("Crossover Configuration").color(theme.header_color));

        let mut xo_freq_input = NumberInput::new((base_id.clone(), "fd-xo-freq"))
            .value(config.v2.mixed_crossover_freq)
            .min(ParamLimits::MIXED_CROSSOVER_FREQ.min)
            .max(ParamLimits::MIXED_CROSSOVER_FREQ.max)
            .step(ParamLimits::MIXED_CROSSOVER_FREQ.step)
            .decimals(0)
            .label("Crossover Freq (Hz)")
            .size(NumberInputSize::Sm)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

        if let Some(ref h) = on_mixed_crossover_freq_change_rc {
            let h = h.clone();
            xo_freq_input = xo_freq_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        let xo_type_options: Vec<SelectOption> = MIXED_CROSSOVER_TYPE_OPTIONS
            .iter()
            .map(|(val, lbl)| SelectOption::new(*val, *lbl))
            .collect();

        let mut xo_type_select = Select::new((base_id.clone(), "fd-xo-type"))
            .label("Crossover Type")
            .options(xo_type_options)
            .selected(&config.v2.mixed_crossover_type)
            .is_open(ui_state.mixed_crossover_type_open)
            .disabled(disabled)
            .size(SelectSize::Xs)
            .theme(theme.select_theme.clone());

        if let Some(ref h) = on_mixed_crossover_type_toggle_rc {
            let h = h.clone();
            xo_type_select = xo_type_select.on_toggle(move |open, w, cx| h(open, w, cx));
        }
        if let Some(ref h) = on_mixed_crossover_type_change_rc {
            let h = h.clone();
            xo_type_select = xo_type_select.on_change(move |val, w, cx| h(val.as_ref(), w, cx));
        }

        let fir_band_options: Vec<SelectOption> = MIXED_FIR_BAND_OPTIONS
            .iter()
            .map(|(val, lbl)| SelectOption::new(*val, *lbl))
            .collect();

        let mut fir_band_select = Select::new((base_id.clone(), "fd-fir-band"))
            .label("FIR Band")
            .options(fir_band_options)
            .selected(&config.v2.mixed_fir_band)
            .is_open(ui_state.mixed_fir_band_open)
            .disabled(disabled)
            .size(SelectSize::Xs)
            .theme(theme.select_theme.clone());

        if let Some(ref h) = on_mixed_fir_band_toggle_rc {
            let h = h.clone();
            fir_band_select = fir_band_select.on_toggle(move |open, w, cx| h(open, w, cx));
        }
        if let Some(ref h) = on_mixed_fir_band_change_rc {
            let h = h.clone();
            fir_band_select = fir_band_select.on_change(move |val, w, cx| h(val.as_ref(), w, cx));
        }

        section = section.child(xo_freq_input);
        section = section.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(xo_type_select)
                .child(fir_band_select),
        );
    }

    // --- Bass Management Subsection ---
    if !hide_bass_management {
        section = section.child(Text::label("Bass Management").color(theme.header_color));

        // Excursion Protection
        {
            let mut excursion_toggle = Toggle::new((base_id.clone(), "fd-excursion-enabled"))
                .size(ToggleSize::Sm)
                .checked(config.room_correction.use_excursion_protection)
                .theme(toggle_theme.clone());

            if let Some(ref h) = on_use_excursion_protection_change_rc {
                let h = h.clone();
                excursion_toggle = excursion_toggle.on_change(move |v, w, cx| h(v, w, cx));
            }

            section = section.child(
                HStack::new()
                    .justify(StackJustify::SpaceBetween)
                    .child(
                        Text::new("Excursion Protection")
                            .size(TextSize::Xs)
                            .color(theme.label_color),
                    )
                    .child(excursion_toggle),
            );

            if config.room_correction.use_excursion_protection {
                let mut auto_f3_toggle = Toggle::new((base_id.clone(), "fd-excursion-auto-f3"))
                    .size(ToggleSize::Sm)
                    .checked(config.room_correction.excursion_auto_detect_f3)
                    .theme(toggle_theme.clone());

                if let Some(ref h) = on_excursion_auto_detect_f3_change_rc {
                    let h = h.clone();
                    auto_f3_toggle = auto_f3_toggle.on_change(move |v, w, cx| h(v, w, cx));
                }

                section = section.child(
                    HStack::new()
                        .justify(StackJustify::SpaceBetween)
                        .child(
                            Text::new("Auto-detect F3")
                                .size(TextSize::Xs)
                                .color(theme.label_color),
                        )
                        .child(auto_f3_toggle),
                );

                if !config.room_correction.excursion_auto_detect_f3 {
                    let mut f3_input = NumberInput::new((base_id.clone(), "fd-excursion-manual-f3"))
                        .value(config.room_correction.excursion_manual_f3)
                        .min(10.0)
                        .max(500.0)
                        .step(1.0)
                        .decimals(0)
                        .label("Manual F3 (Hz)")
                        .size(NumberInputSize::Sm)
                        .disabled(disabled)
                        .theme(theme.number_input_theme.clone());

                    if let Some(ref h) = on_excursion_manual_f3_change_rc {
                        let h = h.clone();
                        f3_input = f3_input.on_change(move |v, w, cx| h(v, w, cx));
                    }
                    section = section.child(f3_input);
                }

                let hp_options: Vec<SelectOption> = HIGHPASS_TYPE_OPTIONS
                    .iter()
                    .map(|(val, lbl)| SelectOption::new(*val, *lbl))
                    .collect();

                let mut hp_select = Select::new((base_id.clone(), "fd-excursion-hp-type"))
                    .label("Filter Type")
                    .options(hp_options)
                    .selected(&config.room_correction.excursion_filter_type)
                    .is_open(ui_state.excursion_filter_type_open)
                    .size(SelectSize::Xs)
                    .theme(theme.select_theme.clone());

                if let Some(ref h) = on_excursion_filter_type_toggle_rc {
                    let h = h.clone();
                    hp_select = hp_select.on_toggle(move |open, w, cx| h(open, w, cx));
                }
                if let Some(ref h) = on_excursion_filter_type_change_rc {
                    let h = h.clone();
                    hp_select = hp_select.on_change(move |val, w, cx| h(val.as_ref(), w, cx));
                }

                let mut order_input = NumberInput::new((base_id.clone(), "fd-excursion-order"))
                    .value(config.room_correction.excursion_filter_order as f64)
                    .min(2.0)
                    .max(8.0)
                    .step(2.0)
                    .decimals(0)
                    .label("Order")
                    .size(NumberInputSize::Sm)
                    .disabled(disabled)
                    .theme(theme.number_input_theme.clone());

                if let Some(ref h) = on_excursion_filter_order_change_rc {
                    let h = h.clone();
                    order_input = order_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
                }

                section = section.child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .child(hp_select)
                        .child(order_input),
                );

                let mut margin_input = NumberInput::new((base_id.clone(), "fd-excursion-margin"))
                    .value(config.room_correction.excursion_margin_octaves)
                    .min(0.0)
                    .max(1.0)
                    .step(0.05)
                    .decimals(2)
                    .label("Safety Margin (oct)")
                    .size(NumberInputSize::Sm)
                    .disabled(disabled)
                    .theme(theme.number_input_theme.clone());

                if let Some(ref h) = on_excursion_margin_octaves_change_rc {
                    let h = h.clone();
                    margin_input = margin_input.on_change(move |v, w, cx| h(v, w, cx));
                }

                section = section.child(margin_input);
            }
        }

        // Schroeder Split
        {
            let mut schroeder_toggle = Toggle::new((base_id.clone(), "fd-schroeder-enabled"))
                .size(ToggleSize::Sm)
                .checked(config.room_correction.use_schroeder_split)
                .theme(toggle_theme.clone());

            if let Some(ref h) = on_use_schroeder_split_change_rc {
                let h = h.clone();
                schroeder_toggle = schroeder_toggle.on_change(move |v, w, cx| h(v, w, cx));
            }

            section = section.child(
                HStack::new()
                    .justify(StackJustify::SpaceBetween)
                    .child(
                        Text::new("Schroeder Split")
                            .size(TextSize::Xs)
                            .color(theme.label_color),
                    )
                    .child(schroeder_toggle),
            );

            if config.room_correction.use_schroeder_split {
                let mut s_freq_input = NumberInput::new((base_id.clone(), "fd-schroeder-freq"))
                    .value(config.room_correction.schroeder_freq)
                    .min(ParamLimits::SCHROEDER_FREQ.min)
                    .max(ParamLimits::SCHROEDER_FREQ.max)
                    .step(ParamLimits::SCHROEDER_FREQ.step)
                    .decimals(0)
                    .label("Split Freq (Hz)")
                    .size(NumberInputSize::Sm)
                    .disabled(disabled)
                    .theme(theme.number_input_theme.clone());

                if let Some(ref h) = on_schroeder_freq_change_rc {
                    let h = h.clone();
                    s_freq_input = s_freq_input.on_change(move |v, w, cx| h(v, w, cx));
                }

                section = section.child(s_freq_input);

                let mut low_q_input = NumberInput::new((base_id.clone(), "fd-schroeder-low-q"))
                    .value(config.room_correction.schroeder_low_max_q)
                    .min(1.0)
                    .max(20.0)
                    .step(0.5)
                    .decimals(1)
                    .label("LF Max Q")
                    .size(NumberInputSize::Sm)
                    .disabled(disabled)
                    .theme(theme.number_input_theme.clone());

                if let Some(ref h) = on_schroeder_low_max_q_change_rc {
                    let h = h.clone();
                    low_q_input = low_q_input.on_change(move |v, w, cx| h(v, w, cx));
                }

                let mut high_q_input = NumberInput::new((base_id.clone(), "fd-schroeder-high-q"))
                    .value(config.room_correction.schroeder_high_max_q)
                    .min(0.5)
                    .max(5.0)
                    .step(0.1)
                    .decimals(1)
                    .label("HF Max Q")
                    .size(NumberInputSize::Sm)
                    .disabled(disabled)
                    .theme(theme.number_input_theme.clone());

                if let Some(ref h) = on_schroeder_high_max_q_change_rc {
                    let h = h.clone();
                    high_q_input = high_q_input.on_change(move |v, w, cx| h(v, w, cx));
                }

                section = section.child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .child(low_q_input)
                        .child(high_q_input),
                );

                let mut boost_toggle = Toggle::new((base_id.clone(), "fd-schroeder-boost"))
                    .size(ToggleSize::Sm)
                    .checked(config.room_correction.schroeder_low_allow_boost)
                    .theme(toggle_theme.clone());

                if let Some(ref h) = on_schroeder_low_allow_boost_change_rc {
                    let h = h.clone();
                    boost_toggle = boost_toggle.on_change(move |v, w, cx| h(v, w, cx));
                }

                let mut shelve_toggle = Toggle::new((base_id.clone(), "fd-schroeder-shelve"))
                    .size(ToggleSize::Sm)
                    .checked(config.room_correction.schroeder_high_shelving_only)
                    .theme(toggle_theme.clone());

                if let Some(ref h) = on_schroeder_high_shelving_only_change_rc {
                    let h = h.clone();
                    shelve_toggle = shelve_toggle.on_change(move |v, w, cx| h(v, w, cx));
                }

                section = section.child(
                    HStack::new()
                        .justify(StackJustify::SpaceBetween)
                        .child(Text::new("Allow LF Boost").size(TextSize::Xs).color(theme.label_color))
                        .child(boost_toggle),
                );

                section = section.child(
                    HStack::new()
                        .justify(StackJustify::SpaceBetween)
                        .child(Text::new("HF Shelving Only").size(TextSize::Xs).color(theme.label_color))
                        .child(shelve_toggle),
                );
            }
        }
    }

    Card::new().content(section)
}
