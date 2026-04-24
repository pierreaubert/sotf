{
        let mut form = VStack::new().spacing(StackSpacing::Lg);
        let base_id = id.clone();
        let is_narrow_layout = is_narrow_room_eq_layout(available_width);

        let toggle_theme = theme.toggle_theme();

        let is_fir = matches!(config.opt_mode.as_str(), "fir" | "mixed" | "mixed_phase");
        let is_iir = matches!(config.opt_mode.as_str(), "iir" | "mixed" | "mixed_phase");

        // ========================================
        // Section 2: Room Configuration
        // ========================================
        let room_config_card = {
            let mut target_col = VStack::new().spacing(StackSpacing::Sm);
            let mut options_col = VStack::new().spacing(StackSpacing::Sm);

            // --- Target sub-section ---
            target_col = target_col.child(Text::eyebrow("TARGET").color(theme.accent));

            // Target Tilt
            let mut tilt_toggle = Toggle::new((base_id.clone(), "tilt-enabled"))
                .size(ToggleSize::Sm)
                .checked(config.use_target_tilt)
                .theme(toggle_theme.clone());

            if let Some(ref h) = on_use_target_tilt_change_rc {
                let h = h.clone();
                tilt_toggle = tilt_toggle.on_change(move |v, w, cx| h(v, w, cx));
            }

            target_col = target_col.child(
                HStack::new()
                    .justify(StackJustify::SpaceBetween)
                    .child(
                        Text::new("Target Tilt")
                            .size(TextSize::Xs)
                            .color(theme.label_color),
                    )
                    .child(tilt_toggle),
            );

            if config.use_target_tilt {
                let tilt_options: Vec<SelectOption> = ROOMEQ_TILT_TYPE_OPTIONS
                    .iter()
                    .map(|(val, lbl)| SelectOption::new(*val, *lbl))
                    .collect();

                let mut tilt_select = Select::new((base_id.clone(), "tilt-type"))
                    .options(tilt_options)
                    .selected(&config.tilt_type)
                    .is_open(ui_state.tilt_type_open)
                    .size(SelectSize::Xs)
                .theme(theme.select_theme.clone());

                if let Some(ref h) = on_tilt_type_toggle_rc {
                    let h = h.clone();
                    tilt_select = tilt_select.on_toggle(move |open, w, cx| h(open, w, cx));
                }

                if let Some(ref h) = on_tilt_type_change_rc {
                    let h = h.clone();
                    tilt_select = tilt_select.on_change(move |val, w, cx| h(val.as_ref(), w, cx));
                }

                target_col = target_col.child(tilt_select);

                if config.tilt_type == "custom" || config.tilt_type == "harman" {
                    let mut slope_input = NumberInput::new((base_id.clone(), "tilt-slope"))
                        .value(config.tilt_slope)
                        .min(ParamLimits::TILT_SLOPE.min)
                        .max(ParamLimits::TILT_SLOPE.max)
                        .step(ParamLimits::TILT_SLOPE.step)
                        .decimals(1)
                        .label("Slope (dB/oct)")
                        .size(NumberInputSize::Sm)
                        .theme(theme.number_input_theme.clone());

                    if let Some(ref h) = on_tilt_slope_change_rc {
                        let h = h.clone();
                        slope_input = slope_input.on_change(move |v, w, cx| h(v, w, cx));
                    }

                    let mut ref_freq_input = NumberInput::new((base_id.clone(), "tilt-ref-freq"))
                        .value(config.tilt_reference_freq)
                        .min(20.0)
                        .max(20000.0)
                        .step(10.0)
                        .decimals(0)
                        .label("Ref Freq (Hz)")
                        .size(NumberInputSize::Sm)
                        .theme(theme.number_input_theme.clone());

                    if let Some(ref h) = on_tilt_reference_freq_change_rc {
                        let h = h.clone();
                        ref_freq_input = ref_freq_input.on_change(move |v, w, cx| h(v, w, cx));
                    }

                    target_col = if is_narrow_layout {
                        target_col.child(slope_input).child(ref_freq_input)
                    } else {
                        target_col.child(
                            HStack::new()
                                .spacing(StackSpacing::Md)
                                .child(slope_input)
                                .child(ref_freq_input),
                        )
                    };

                    let mut shelf_db_input = NumberInput::new((base_id.clone(), "tilt-shelf-db"))
                        .value(config.tilt_bass_shelf_db)
                        .min(ParamLimits::BASS_SHELF.min)
                        .max(ParamLimits::BASS_SHELF.max)
                        .step(ParamLimits::BASS_SHELF.step)
                        .decimals(1)
                        .label("Bass Boost (dB)")
                        .size(NumberInputSize::Sm)
                        .theme(theme.number_input_theme.clone());

                    if let Some(ref h) = on_tilt_bass_shelf_db_change_rc {
                        let h = h.clone();
                        shelf_db_input = shelf_db_input.on_change(move |v, w, cx| h(v, w, cx));
                    }

                    let mut shelf_freq_input =
                        NumberInput::new((base_id.clone(), "tilt-shelf-freq"))
                            .value(config.tilt_bass_shelf_freq)
                            .min(20.0)
                            .max(1000.0)
                            .step(10.0)
                            .decimals(0)
                            .label("Shelf Freq (Hz)")
                            .size(NumberInputSize::Sm)
                            .theme(theme.number_input_theme.clone());

                    if let Some(ref h) = on_tilt_bass_shelf_freq_change_rc {
                        let h = h.clone();
                        shelf_freq_input = shelf_freq_input.on_change(move |v, w, cx| h(v, w, cx));
                    }

                    target_col = if is_narrow_layout {
                        target_col.child(shelf_db_input).child(shelf_freq_input)
                    } else {
                        target_col.child(
                            HStack::new()
                                .spacing(StackSpacing::Md)
                                .child(shelf_db_input)
                                .child(shelf_freq_input),
                        )
                    };
                }
            }

            // Broadband Target Matching (in Target section)
            let mut broadband_toggle =
                Toggle::new((base_id.clone(), "broadband-target-matching"))
                    .size(ToggleSize::Sm)
                    .checked(config.broadband_target_matching)
                    .theme(toggle_theme.clone());

            if let Some(ref h) = on_broadband_target_matching_change_rc {
                let h = h.clone();
                broadband_toggle = broadband_toggle.on_change(move |v, w, cx| h(v, w, cx));
            }

            target_col = target_col.child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .justify(StackJustify::SpaceBetween)
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::None)
                            .child(
                                Text::new("Broadband Target Matching")
                                    .size(TextSize::Xs)
                                    .color(theme.label_color),
                            )
                            .child(
                                Text::new("Shelf filters for broad tonal balance")
                                    .size(TextSize::Xs)
                                    .color(theme.description_color),
                            ),
                    )
                    .child(broadband_toggle),
            );

            // Edit Custom Target Curve button
            if let Some(ref handler) = on_edit_custom_target_rc {
                let h = handler.clone();
                target_col = target_col.child(
                    Button::new("edit-custom-target", "Edit Custom Target Curve")
                        .variant(if config.target_curve == "custom" {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Xs)
                        .on_click(move |w, cx| {
                            h(w, cx);
                        }),
                );
            }

            // --- Smoothing sub-section ---
            options_col = options_col.child(Text::eyebrow("SMOOTHING").color(theme.accent));

            // Psychoacoustic Smoothing
            let mut psycho_toggle = Toggle::new((base_id.clone(), "psychoacoustic"))
                .size(ToggleSize::Sm)
                .checked(config.psychoacoustic)
                .disabled(config.smooth)
                .theme(toggle_theme.clone());

            if let Some(ref handler) = on_psychoacoustic_change_rc {
                let h = handler.clone();
                psycho_toggle = psycho_toggle.on_change(move |v, w, cx| h(v, w, cx));
            }

            options_col = options_col.child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .justify(StackJustify::SpaceBetween)
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::None)
                            .child(
                                Text::new("Psychoacoustic Smoothing")
                                    .size(TextSize::Xs)
                                    .color(theme.label_color),
                            )
                            .child(
                                Text::new("1/48 oct bass, 1/6 oct treble")
                                    .size(TextSize::Xs)
                                    .color(theme.description_color),
                            ),
                    )
                    .child(psycho_toggle),
            );

            // Curve Smoothing (mutually exclusive with psychoacoustic)
            let mut smooth_toggle = Toggle::new((base_id.clone(), "smooth"))
                .size(ToggleSize::Sm)
                .checked(config.smooth)
                .disabled(config.psychoacoustic)
                .theme(toggle_theme.clone());

            if let Some(ref handler) = on_smooth_change_rc {
                let h = handler.clone();
                smooth_toggle = smooth_toggle.on_change(move |v, w, cx| h(v, w, cx));
            }

            options_col = options_col.child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .justify(StackJustify::SpaceBetween)
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::None)
                            .child(
                                Text::new("Curve Smoothing")
                                    .size(TextSize::Xs)
                                    .color(theme.label_color),
                            )
                            .child(
                                Text::new("Fixed-width octave smoothing")
                                    .size(TextSize::Xs)
                                    .color(theme.description_color),
                            ),
                    )
                    .child(smooth_toggle),
            );

            if config.smooth {
                let mut smooth_n_input = NumberInput::new((base_id.clone(), "smooth-n"))
                    .value(config.smooth_n as f64)
                    .min(ParamLimits::SMOOTH_N.min)
                    .max(ParamLimits::SMOOTH_N.max)
                    .step(ParamLimits::SMOOTH_N.step)
                    .decimals(0)
                    .label("Smooth Window (1/N oct)")
                    .size(NumberInputSize::Sm)
                    .disabled(disabled)
                    .theme(theme.number_input_theme.clone());

                if let Some(ref handler) = on_smooth_n_change_rc {
                    let h = handler.clone();
                    smooth_n_input =
                        smooth_n_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
                }

                options_col = options_col.child(smooth_n_input);
            }

            // --- Recommended sub-section ---
            options_col = options_col.child(
                Text::eyebrow("RECOMMENDED").color(theme.accent),
            );

            // Asymmetric Loss
            let mut asymmetric_toggle = Toggle::new((base_id.clone(), "asymmetric-loss"))
                .size(ToggleSize::Sm)
                .checked(config.asymmetric_loss)
                .theme(toggle_theme.clone());

            if let Some(ref handler) = on_asymmetric_loss_change_rc {
                let h = handler.clone();
                asymmetric_toggle = asymmetric_toggle.on_change(move |v, w, cx| h(v, w, cx));
            }

            options_col = options_col.child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .justify(StackJustify::SpaceBetween)
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::None)
                            .child(
                                Text::new("Asymmetric Loss")
                                    .size(TextSize::Xs)
                                    .color(theme.label_color),
                            )
                            .child(
                                Text::new("Penalize peaks more than dips")
                                    .size(TextSize::Xs)
                                    .color(theme.description_color),
                            ),
                    )
                    .child(asymmetric_toggle),
            );

            // Excursion Protection
            let mut excursion_toggle = Toggle::new((base_id.clone(), "excursion-enabled"))
                .size(ToggleSize::Sm)
                .checked(config.use_excursion_protection)
                .theme(toggle_theme.clone());

            if let Some(ref h) = on_use_excursion_protection_change_rc {
                let h = h.clone();
                excursion_toggle = excursion_toggle.on_change(move |v, w, cx| h(v, w, cx));
            }

            options_col = options_col.child(
                HStack::new()
                    .justify(StackJustify::SpaceBetween)
                    .child(
                        Text::new("Excursion Protection")
                            .size(TextSize::Xs)
                            .color(theme.label_color),
                    )
                    .child(excursion_toggle),
            );

            if config.use_excursion_protection {
                let mut auto_f3_toggle = Toggle::new((base_id.clone(), "excursion-auto-f3"))
                    .size(ToggleSize::Sm)
                    .checked(config.excursion_auto_detect_f3)
                    .theme(toggle_theme.clone());

                if let Some(ref h) = on_excursion_auto_detect_f3_change_rc {
                    let h = h.clone();
                    auto_f3_toggle = auto_f3_toggle.on_change(move |v, w, cx| h(v, w, cx));
                }

                options_col = options_col.child(
                    HStack::new()
                        .justify(StackJustify::SpaceBetween)
                        .child(
                            Text::new("Auto-detect F3")
                                .size(TextSize::Xs)
                                .color(theme.label_color),
                        )
                        .child(auto_f3_toggle),
                );

                if !config.excursion_auto_detect_f3 {
                    let mut f3_input = NumberInput::new((base_id.clone(), "excursion-manual-f3"))
                        .value(config.excursion_manual_f3)
                        .min(10.0)
                        .max(500.0)
                        .step(1.0)
                        .decimals(0)
                        .label("Manual F3 (Hz)")
                        .size(NumberInputSize::Sm)
                        .theme(theme.number_input_theme.clone());

                    if let Some(ref h) = on_excursion_manual_f3_change_rc {
                        let h = h.clone();
                        f3_input = f3_input.on_change(move |v, w, cx| h(v, w, cx));
                    }
                    options_col = options_col.child(f3_input);
                }

                let hp_options: Vec<SelectOption> = HIGHPASS_TYPE_OPTIONS
                    .iter()
                    .map(|(val, lbl)| SelectOption::new(*val, *lbl))
                    .collect();

                let mut hp_select = Select::new((base_id.clone(), "excursion-hp-type"))
                    .options(hp_options)
                    .selected(&config.excursion_filter_type)
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

                let mut order_input = NumberInput::new((base_id.clone(), "excursion-order"))
                    .value(config.excursion_filter_order as f64)
                    .min(2.0)
                    .max(8.0)
                    .step(2.0)
                    .decimals(0)
                    .label("Order")
                    .size(NumberInputSize::Sm)
                    .theme(theme.number_input_theme.clone());

                if let Some(ref h) = on_excursion_filter_order_change_rc {
                    let h = h.clone();
                    order_input =
                        order_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
                }

                options_col = if is_narrow_layout {
                    options_col.child(hp_select).child(order_input)
                } else {
                    options_col.child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(hp_select)
                            .child(order_input),
                    )
                };

                let mut margin_input = NumberInput::new((base_id.clone(), "excursion-margin"))
                    .value(config.excursion_margin_octaves)
                    .min(0.0)
                    .max(1.0)
                    .step(0.05)
                    .decimals(2)
                    .label("Safety Margin (oct)")
                    .size(NumberInputSize::Sm)
                    .theme(theme.number_input_theme.clone());

                if let Some(ref h) = on_excursion_margin_octaves_change_rc {
                    let h = h.clone();
                    margin_input = margin_input.on_change(move |v, w, cx| h(v, w, cx));
                }

                options_col = options_col.child(margin_input);
            }

            // Schroeder Split
            let mut schroeder_toggle = Toggle::new((base_id.clone(), "schroeder-enabled"))
                .size(ToggleSize::Sm)
                .checked(config.use_schroeder_split)
                .theme(toggle_theme.clone());

            if let Some(ref h) = on_use_schroeder_split_change_rc {
                let h = h.clone();
                schroeder_toggle = schroeder_toggle.on_change(move |v, w, cx| h(v, w, cx));
            }

            options_col = options_col.child(
                HStack::new()
                    .justify(StackJustify::SpaceBetween)
                    .child(
                        Text::new("Schroeder Split")
                            .size(TextSize::Xs)
                            .color(theme.label_color),
                    )
                    .child(schroeder_toggle),
            );

            if config.use_schroeder_split {
                let mut s_freq_input = NumberInput::new((base_id.clone(), "schroeder-freq"))
                    .value(config.schroeder_freq)
                    .min(ParamLimits::SCHROEDER_FREQ.min)
                    .max(ParamLimits::SCHROEDER_FREQ.max)
                    .step(ParamLimits::SCHROEDER_FREQ.step)
                    .decimals(0)
                    .label("Split Freq (Hz)")
                    .size(NumberInputSize::Sm)
                    .theme(theme.number_input_theme.clone());

                if let Some(ref h) = on_schroeder_freq_change_rc {
                    let h = h.clone();
                    s_freq_input = s_freq_input.on_change(move |v, w, cx| h(v, w, cx));
                }

                options_col = options_col.child(s_freq_input);

                let mut low_q_input = NumberInput::new((base_id.clone(), "schroeder-low-q"))
                    .value(config.schroeder_low_max_q)
                    .min(1.0)
                    .max(20.0)
                    .step(0.5)
                    .decimals(1)
                    .label("LF Max Q")
                    .size(NumberInputSize::Sm)
                    .theme(theme.number_input_theme.clone());

                if let Some(ref h) = on_schroeder_low_max_q_change_rc {
                    let h = h.clone();
                    low_q_input = low_q_input.on_change(move |v, w, cx| h(v, w, cx));
                }

                let mut high_q_input = NumberInput::new((base_id.clone(), "schroeder-high-q"))
                    .value(config.schroeder_high_max_q)
                    .min(0.5)
                    .max(5.0)
                    .step(0.1)
                    .decimals(1)
                    .label("HF Max Q")
                    .size(NumberInputSize::Sm)
                    .theme(theme.number_input_theme.clone());

                if let Some(ref h) = on_schroeder_high_max_q_change_rc {
                    let h = h.clone();
                    high_q_input = high_q_input.on_change(move |v, w, cx| h(v, w, cx));
                }

                options_col = if is_narrow_layout {
                    options_col.child(low_q_input).child(high_q_input)
                } else {
                    options_col.child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(low_q_input)
                            .child(high_q_input),
                    )
                };

                let mut boost_toggle = Toggle::new((base_id.clone(), "schroeder-boost"))
                    .size(ToggleSize::Sm)
                    .checked(config.schroeder_low_allow_boost)
                    .theme(toggle_theme.clone());

                if let Some(ref h) = on_schroeder_low_allow_boost_change_rc {
                    let h = h.clone();
                    boost_toggle = boost_toggle.on_change(move |v, w, cx| h(v, w, cx));
                }

                let mut shelve_toggle = Toggle::new((base_id.clone(), "schroeder-shelve"))
                    .size(ToggleSize::Sm)
                    .checked(config.schroeder_high_shelving_only)
                    .theme(toggle_theme.clone());

                if let Some(ref h) = on_schroeder_high_shelving_only_change_rc {
                    let h = h.clone();
                    shelve_toggle = shelve_toggle.on_change(move |v, w, cx| h(v, w, cx));
                }

                options_col = options_col.child(
                    HStack::new()
                        .justify(StackJustify::SpaceBetween)
                        .child(
                            Text::new("Allow LF Boost")
                                .size(TextSize::Xs)
                                .color(theme.label_color),
                        )
                        .child(boost_toggle),
                );

                options_col = options_col.child(
                    HStack::new()
                        .justify(StackJustify::SpaceBetween)
                        .child(
                            Text::new("HF Shelving Only")
                                .size(TextSize::Xs)
                                .color(theme.label_color),
                        )
                        .child(shelve_toggle),
                );
            }

            // --- Delay sub-section ---
            options_col = options_col.child(
                Text::eyebrow("DELAY").color(theme.accent),
            );

            // Allow Delay
            let mut delay_toggle = Toggle::new((base_id.clone(), "allow-delay"))
                .size(ToggleSize::Sm)
                .checked(config.allow_delay)
                .theme(toggle_theme.clone());

            if let Some(ref h) = on_allow_delay_change_rc {
                let h = h.clone();
                delay_toggle = delay_toggle.on_change(move |v, w, cx| h(v, w, cx));
            }

            options_col = if is_narrow_layout {
                options_col.child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::None)
                                .child(
                                    Text::new("Allow Delay")
                                        .size(TextSize::Xs)
                                        .color(theme.label_color),
                                )
                                .child(
                                    Text::new("Enable inter-speaker time alignment")
                                        .size(TextSize::Xs)
                                        .color(theme.description_color),
                                ),
                        )
                        .child(delay_toggle),
                )
            } else {
                options_col.child(
                    HStack::new()
                        .justify(StackJustify::SpaceBetween)
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::None)
                                .child(
                                    Text::new("Allow Delay")
                                        .size(TextSize::Xs)
                                        .color(theme.label_color),
                                )
                                .child(
                                    Text::new("Enable inter-speaker time alignment")
                                        .size(TextSize::Xs)
                                        .color(theme.description_color),
                                ),
                        )
                        .child(delay_toggle),
                )
            };

            // --- Home Cinema sub-section ---
            options_col = options_col.child(
                Text::eyebrow("HOME CINEMA").color(theme.accent),
            );

            // Voice of God
            let mut vog_toggle = Toggle::new((base_id.clone(), "vog-enabled"))
                .size(ToggleSize::Sm)
                .checked(config.vog_enabled)
                .theme(toggle_theme.clone());

            if let Some(ref h) = on_vog_enabled_change_rc {
                let h = h.clone();
                vog_toggle = vog_toggle.on_change(move |v, w, cx| h(v, w, cx));
            }

            options_col = if is_narrow_layout {
                options_col.child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::None)
                                .child(
                                    Text::new("Voice of God")
                                        .size(TextSize::Xs)
                                        .color(theme.label_color),
                                )
                                .child(
                                    Text::new("Timbre matching across channels")
                                        .size(TextSize::Xs)
                                        .color(theme.description_color),
                                ),
                        )
                        .child(vog_toggle),
                )
            } else {
                options_col.child(
                    HStack::new()
                        .justify(StackJustify::SpaceBetween)
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::None)
                                .child(
                                    Text::new("Voice of God")
                                        .size(TextSize::Xs)
                                        .color(theme.label_color),
                                )
                                .child(
                                    Text::new("Timbre matching across channels")
                                        .size(TextSize::Xs)
                                        .color(theme.description_color),
                                ),
                        )
                        .child(vog_toggle),
                )
            };

            if config.vog_enabled {
                let ref_channel_options: Vec<SelectOption> = ["C", "L", "R"]
                    .iter()
                    .map(|ch| SelectOption::new(*ch, *ch))
                    .collect();

                let mut ref_select = Select::new((base_id.clone(), "vog-ref-channel"))
                    .label("Reference Channel")
                    .options(ref_channel_options)
                    .selected(&config.vog_reference_channel)
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

                options_col = options_col.child(ref_select);
            }

            // Phase Alignment (hidden for IIR mode)
            if !hide_phase_alignment {
                let mut phase_toggle = Toggle::new((base_id.clone(), "phase-enabled"))
                    .size(ToggleSize::Sm)
                    .checked(config.use_phase_alignment)
                    .theme(toggle_theme.clone());

                if let Some(ref h) = on_use_phase_alignment_change_rc {
                    let h = h.clone();
                    phase_toggle = phase_toggle.on_change(move |v, w, cx| h(v, w, cx));
                }

                options_col = options_col.child(
                    HStack::new()
                        .justify(StackJustify::SpaceBetween)
                        .child(
                            Text::new("Phase Alignment")
                                .size(TextSize::Xs)
                                .color(theme.label_color),
                        )
                        .child(phase_toggle),
                );

                if config.use_phase_alignment {
                    let mut min_freq_input = NumberInput::new((base_id.clone(), "phase-min-freq"))
                        .value(config.phase_min_freq)
                        .min(20.0)
                        .max(1000.0)
                        .step(1.0)
                        .decimals(0)
                        .label("Min Freq (Hz)")
                        .size(NumberInputSize::Sm)
                        .theme(theme.number_input_theme.clone());

                    if let Some(ref h) = on_phase_min_freq_change_rc {
                        let h = h.clone();
                        min_freq_input = min_freq_input.on_change(move |v, w, cx| h(v, w, cx));
                    }

                    let mut max_freq_input = NumberInput::new((base_id.clone(), "phase-max-freq"))
                        .value(config.phase_max_freq)
                        .min(20.0)
                        .max(1000.0)
                        .step(1.0)
                        .decimals(0)
                        .label("Max Freq (Hz)")
                        .size(NumberInputSize::Sm)
                        .theme(theme.number_input_theme.clone());

                    if let Some(ref h) = on_phase_max_freq_change_rc {
                        let h = h.clone();
                        max_freq_input = max_freq_input.on_change(move |v, w, cx| h(v, w, cx));
                    }

                    options_col = if is_narrow_layout {
                        options_col.child(min_freq_input).child(max_freq_input)
                    } else {
                        options_col.child(
                            HStack::new()
                                .spacing(StackSpacing::Md)
                                .child(min_freq_input)
                                .child(max_freq_input),
                        )
                    };

                    let mut polarity_toggle = Toggle::new((base_id.clone(), "phase-polarity"))
                        .size(ToggleSize::Sm)
                        .checked(config.phase_optimize_polarity)
                        .theme(toggle_theme.clone());

                    if let Some(ref h) = on_phase_optimize_polarity_change_rc {
                        let h = h.clone();
                        polarity_toggle = polarity_toggle.on_change(move |v, w, cx| h(v, w, cx));
                    }

                    options_col = options_col.child(
                        HStack::new()
                            .justify(StackJustify::SpaceBetween)
                            .child(
                                Text::new("Optimize Polarity")
                                    .size(TextSize::Xs)
                                    .color(theme.label_color),
                            )
                            .child(polarity_toggle),
                    );

                    let mut p_max_delay = NumberInput::new((base_id.clone(), "phase-max-delay"))
                        .value(config.phase_max_delay_ms)
                        .min(ParamLimits::DELAY_MS.min)
                        .max(ParamLimits::DELAY_MS.max)
                        .step(ParamLimits::DELAY_MS.step)
                        .decimals(1)
                        .label("Max Delay (ms)")
                        .size(NumberInputSize::Sm)
                        .theme(theme.number_input_theme.clone());

                    if let Some(ref h) = on_phase_max_delay_ms_change_rc {
                        let h = h.clone();
                        p_max_delay = p_max_delay.on_change(move |v, w, cx| h(v, w, cx));
                    }
                    options_col = options_col.child(p_max_delay);
                }
            }

            // Multi-Seat
            if !hide_multi_seat {
                let mut multi_seat_toggle = Toggle::new((base_id.clone(), "multi-seat-enabled"))
                    .size(ToggleSize::Sm)
                    .checked(config.use_multi_seat)
                    .theme(toggle_theme.clone());

                if let Some(ref h) = on_use_multi_seat_change_rc {
                    let h = h.clone();
                    multi_seat_toggle = multi_seat_toggle.on_change(move |v, w, cx| h(v, w, cx));
                }

                options_col = options_col.child(
                    HStack::new()
                        .justify(StackJustify::SpaceBetween)
                        .child(
                            Text::new("Multi-Seat Optimization")
                                .size(TextSize::Xs)
                                .color(theme.label_color),
                        )
                        .child(multi_seat_toggle),
                );

                if config.use_multi_seat {
                    let strategy_options: Vec<SelectOption> = MULTI_SEAT_STRATEGY_OPTIONS
                        .iter()
                        .map(|(val, lbl)| SelectOption::new(*val, *lbl))
                        .collect();

                    let mut strategy_select = Select::new((base_id.clone(), "multi-seat-strategy"))
                        .options(strategy_options)
                        .selected(&config.multi_seat_strategy)
                        .is_open(ui_state.multi_seat_strategy_open)
                        .size(SelectSize::Xs)
                .theme(theme.select_theme.clone());

                    if let Some(ref h) = on_multi_seat_strategy_toggle_rc {
                        let h = h.clone();
                        strategy_select = strategy_select.on_toggle(move |open, w, cx| h(open, w, cx));
                    }

                    if let Some(ref h) = on_multi_seat_strategy_change_rc {
                        let h = h.clone();
                        strategy_select =
                            strategy_select.on_change(move |val, w, cx| h(val.as_ref(), w, cx));
                    }

                    options_col = options_col.child(strategy_select);

                    if config.multi_seat_strategy == "primary" {
                        let mut primary_seat_input =
                            NumberInput::new((base_id.clone(), "multi-seat-primary"))
                                .value(config.multi_seat_primary_seat as f64)
                                .min(0.0)
                                .max(16.0)
                                .step(1.0)
                                .decimals(0)
                                .label("Primary Seat")
                                .size(NumberInputSize::Sm)
                                .theme(theme.number_input_theme.clone());

                        if let Some(ref h) = on_multi_seat_primary_seat_change_rc {
                            let h = h.clone();
                            primary_seat_input = primary_seat_input
                                .on_change(move |v, w, cx| h(v.round() as usize, w, cx));
                        }
                        options_col = options_col.child(primary_seat_input);
                    }

                    let mut dev_input = NumberInput::new((base_id.clone(), "multi-seat-max-dev"))
                        .value(config.multi_seat_max_deviation_db)
                        .min(1.0)
                        .max(12.0)
                        .step(0.5)
                        .decimals(1)
                        .label("Max Deviation (dB)")
                        .size(NumberInputSize::Sm)
                        .theme(theme.number_input_theme.clone());

                    if let Some(ref h) = on_multi_seat_max_deviation_db_change_rc {
                        let h = h.clone();
                        dev_input = dev_input.on_change(move |v, w, cx| h(v, w, cx));
                    }
                    options_col = options_col.child(dev_input);
                }
            }

            // Multi-Measurement Optimization
            if !hide_multi_measurement {
                let mut multi_meas_toggle = Toggle::new((base_id.clone(), "multi-meas-enabled"))
                    .size(ToggleSize::Sm)
                    .checked(config.use_multi_measurement)
                    .theme(toggle_theme.clone());

                if let Some(ref h) = on_use_multi_measurement_change_rc {
                    let h = h.clone();
                    multi_meas_toggle = multi_meas_toggle.on_change(move |v, w, cx| h(v, w, cx));
                }

                options_col = options_col.child(
                    HStack::new()
                        .align(StackAlign::Center)
                        .justify(StackJustify::SpaceBetween)
                        .child(
                            Text::new("Multi-Measurement Optimization")
                                .size(TextSize::Xs)
                                .color(theme.label_color),
                        )
                        .child(multi_meas_toggle),
                );

                if config.use_multi_measurement {
                    let strategy_options: Vec<SelectOption> = MULTI_MEASUREMENT_STRATEGY_OPTIONS
                        .iter()
                        .map(|&(v, l)| SelectOption::new(v, l))
                        .collect();

                    let mut strategy_select = Select::new((base_id.clone(), "multi-meas-strategy"))
                        .options(strategy_options)
                        .selected(&config.multi_measurement_strategy)
                        .is_open(ui_state.multi_measurement_strategy_open)
                        .size(SelectSize::Xs)
                        .theme(theme.select_theme.clone());

                    if let Some(ref h) = on_multi_measurement_strategy_toggle_rc {
                        let h = h.clone();
                        strategy_select = strategy_select.on_toggle(move |open, w, cx| h(open, w, cx));
                    }

                    if let Some(ref h) = on_multi_measurement_strategy_change_rc {
                        let h = h.clone();
                        strategy_select =
                            strategy_select.on_change(move |v, w, cx| h(v.as_ref(), w, cx));
                    }
                    options_col = options_col.child(strategy_select);

                    // Weighted Sum: show per-measurement weight inputs
                    if config.multi_measurement_strategy == "weighted_sum"
                        && !config.multi_measurement_weights.is_empty()
                    {
                        options_col =
                            options_col.child(Text::label("Weights").color(theme.text_muted));
                        for (i, &weight) in config.multi_measurement_weights.iter().enumerate() {
                            let label = config
                                .multi_measurement_labels
                                .get(i)
                                .cloned()
                                .unwrap_or_else(|| format!("Meas {}", i + 1));

                            let mut weight_input =
                                NumberInput::new((base_id.clone(), SharedString::from(format!("multi-meas-weight-{}", i))))
                                    .value(weight)
                                    .min(ParamLimits::WEIGHT.min)
                                    .max(ParamLimits::WEIGHT.max)
                                    .step(ParamLimits::WEIGHT.step)
                                    .decimals(2)
                                    .label(label)
                                    .size(NumberInputSize::Xs)
                                    .theme(theme.number_input_theme.clone());

                            if let Some(ref h) = on_multi_measurement_weight_change_rc {
                                let h = h.clone();
                                weight_input =
                                    weight_input.on_change(move |v, w, cx| h(i, v, w, cx));
                            }
                            options_col = options_col.child(weight_input);
                        }
                    }

                    // Variance Penalized: show lambda input
                    if config.multi_measurement_strategy == "variance_penalized" {
                        let mut lambda_input =
                            NumberInput::new((base_id.clone(), "multi-meas-lambda"))
                                .value(config.multi_measurement_variance_lambda)
                                .min(ParamLimits::VARIANCE_LAMBDA.min)
                                .max(ParamLimits::VARIANCE_LAMBDA.max)
                                .step(ParamLimits::VARIANCE_LAMBDA.step)
                                .decimals(1)
                                .label("Variance Lambda")
                                .size(NumberInputSize::Xs)
                                .theme(theme.number_input_theme.clone());

                        if let Some(ref h) = on_multi_measurement_variance_lambda_change_rc {
                            let h = h.clone();
                            lambda_input = lambda_input.on_change(move |v, w, cx| h(v, w, cx));
                        }
                        options_col = options_col.child(lambda_input);
                    }
                }
            }

            // Assemble Room Configuration card
            let mut room_config_section = VStack::new().spacing(StackSpacing::Sm);

            // Header
            room_config_section = room_config_section.child(
                VStack::new()
                    .spacing(StackSpacing::None)
                    .child(
                        Text::section_header("Room Configuration").color(theme.header_color),
                    )
                    .child(
                        Text::new("Target curve and room correction options")
                            .size(TextSize::Xs)
                            .color(theme.description_color),
                    ),
            );

            // Always single column within the card
            room_config_section = room_config_section
                .child(target_col)
                .child(options_col);

            // Room config card stored for later assembly with optimiser config
            Card::new().content(room_config_section)
        };

        // ========================================
        // Section 3: Optimiser Configuration
        // ========================================
        {
            let mut section = VStack::new().spacing(StackSpacing::Sm);

            // Header
            section = section.child(
                VStack::new()
                    .spacing(StackSpacing::None)
                    .child(
                        Text::section_header("Optimiser Configuration").color(theme.header_color),
                    )
                    .child(
                        Text::new("EQ design and optimisation algorithm settings")
                            .size(TextSize::Xs)
                            .color(theme.description_color),
                    ),
            );

            // Two-column layout: EQ Design (left) + Optimisation (right)
            let mut left_col = VStack::new().spacing(StackSpacing::Sm);
            let mut right_col = VStack::new().spacing(StackSpacing::Sm);

            // Left column: EQ Design params via shared block
            left_col = left_col.child(Text::eyebrow("EQ DESIGN").color(theme.accent));
            {
                let eq_design_iir_before_fir = true; // room EQ: IIR shown before FIR
                let mut block_out = left_col;
                include!("render_block_eq_design.rs");
                left_col = block_out;
            }

            // Right column: Optimisation params via shared block
            right_col = right_col.child(Text::eyebrow("OPTIMISATION").color(theme.accent));
            {
                let mut block_out = right_col;
                include!("render_block_optimizer.rs");
                right_col = block_out;
            }

            // Always single column within the card
            section = section
                .child(left_col)
                .child(right_col);

            let opt_config_card = Card::new().content(section);

            // Assemble Room Config and Optimiser Config side-by-side when wide enough
            if !is_narrow_layout {
                let on_block_focus_tilt = on_block_focus_rc.clone();
                let on_block_focus_opt = on_block_focus_rc.clone();
                form = form.child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .align(StackAlign::Start)
                        .child(
                            div()
                                .flex_1()
                                .on_mouse_down(MouseButton::Left, move |_event, _window, _cx| {
                                    if let Some(ref cb) = on_block_focus_tilt {
                                        cb(docs::BLOCK_TARGET_TILT, _window, _cx);
                                    }
                                })
                                .child(room_config_card),
                        )
                        .child(
                            div()
                                .flex_1()
                                .on_mouse_down(MouseButton::Left, move |_event, _window, _cx| {
                                    if let Some(ref cb) = on_block_focus_opt {
                                        cb(docs::BLOCK_OPTIMIZER, _window, _cx);
                                    }
                                })
                                .child(opt_config_card),
                        ),
                );
            } else {
                let on_block_focus_tilt = on_block_focus_rc.clone();
                let on_block_focus_opt = on_block_focus_rc.clone();
                form = form
                    .child(
                        div()
                            .on_mouse_down(MouseButton::Left, move |_event, _window, _cx| {
                                if let Some(ref cb) = on_block_focus_tilt {
                                    cb(docs::BLOCK_TARGET_TILT, _window, _cx);
                                }
                            })
                            .child(room_config_card),
                    )
                    .child(
                        div()
                            .on_mouse_down(MouseButton::Left, move |_event, _window, _cx| {
                                if let Some(ref cb) = on_block_focus_opt {
                                    cb(docs::BLOCK_OPTIMIZER, _window, _cx);
                                }
                            })
                            .child(opt_config_card),
                    );
            }
        }

        div().id(id).child(form)
}
