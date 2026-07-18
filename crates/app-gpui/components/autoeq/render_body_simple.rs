// Simplified render body for Simple and Intermediate detail levels.
// This file is include!()'d from render.rs, sharing its scope.
//
// Available in scope: id, config, ui_state, disabled, theme, optimization_type,
// available_spinorama_curves, detail_level, all _rc callbacks, etc.
{
    use sotf_audio_player::autoeq::{
        DetailLevel, EqWorkflow, field_hint, field_warning, population_to_quality, preset_options,
        quality_label, quality_to_optimizer_params,
    };

    let workflow = match optimization_type {
        OptimizationType::Headphone => EqWorkflow::Headphone,
        OptimizationType::Speaker => match layout_mode {
            AutoEqLayoutMode::RoomEq => EqWorkflow::RoomEq,
            _ => EqWorkflow::Spinorama,
        },
    };

    let detail_level = ui_state.detail_level;
    let mut form = VStack::new().spacing(StackSpacing::Lg);
    let base_id = id.clone();

    // ================================================================
    // Detail Level Toggle (always shown)
    // ================================================================
    {
        let level_label = match detail_level {
            DetailLevel::Simple => "Simple",
            DetailLevel::Intermediate => "Customize",
            DetailLevel::Expert => "All Parameters",
        };

        let next_label = match detail_level {
            DetailLevel::Simple => "Customize",
            DetailLevel::Intermediate => "All Parameters",
            DetailLevel::Expert => "Simple",
        };

        let mut detail_row = HStack::new()
            .spacing(StackSpacing::Sm)
            .align(StackAlign::Center)
            .justify(StackJustify::SpaceBetween);

        detail_row = detail_row.child(
            Text::label(format!("Mode: {level_label}")).color(theme.header_color),
        );

        let mut toggle_btn =
            Button::new((base_id.clone(), "detail-toggle"), next_label)
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
        detail_row = detail_row.child(toggle_btn);
        form = form.child(detail_row);
    }

    // ================================================================
    // Preset Selector (Simple + Intermediate)
    // ================================================================
    {
        let presets = preset_options(workflow);
        let preset_opts: Vec<SelectOption> = presets
            .iter()
            .map(|(val, lbl)| SelectOption::new(*val, *lbl))
            .collect();

        let selected: String = ui_state
            .selected_preset
            .clone()
            .unwrap_or_else(|| {
                sotf_audio_player::autoeq::default_preset_id(workflow).to_string()
            });

        // Show preset description
        if let Some(preset) = sotf_audio_player::autoeq::find_preset(workflow, &selected) {
            form = form.child(
                VStack::new()
                    .spacing(StackSpacing::Xs)
                .child(Text::section_header(translations.autoeq_form.preset).color(theme.header_color))
                    .child(
                        Text::new(preset.description)
                            .size(TextSize::Xs)
                            .color(theme.description_color),
                    ),
            );
        }

        let mut preset_select = Select::new((base_id.clone(), "preset"))
            .label(translations.autoeq_form.preset)
            .options(preset_opts)
            .selected(selected.clone())
            .is_open(ui_state.preset_open)
            .disabled(disabled)
            .size(SelectSize::Xs)
            .theme(theme.select_theme.clone());

        if let Some(ref h) = on_preset_change_rc {
            let h = h.clone();
            preset_select = preset_select.on_change(move |value, w, cx| {
                h(value.as_ref(), w, cx);
            });
        }
        if let Some(ref h) = on_preset_toggle_rc {
            let h = h.clone();
            preset_select = preset_select.on_toggle(move |open, w, cx| h(open, w, cx));
        }

        form = form.child(preset_select);
    }

    // ================================================================
    // Target Curve (Simple + Intermediate)
    // ================================================================
    {
        let target_curve_options: Vec<SelectOption> = match optimization_type {
            OptimizationType::Headphone => {
                HEADPHONE_TARGET_CURVE_OPTIONS
                    .iter()
                    .map(|(val, lbl)| SelectOption::new(*val, *lbl))
                    .collect()
            }
            OptimizationType::Speaker => {
                let mut opts: Vec<SelectOption> = SPEAKER_TARGET_CURVE_OPTIONS
                    .iter()
                    .map(|(val, lbl)| SelectOption::new(*val, *lbl))
                    .collect();
                // Add available spinorama curves
                for curve_id in &available_spinorama_curves {
                    if let Some((_, label)) = SPINORAMA_CURVE_OPTIONS
                        .iter()
                        .find(|(id, _)| *id == curve_id.as_str())
                    {
                        opts.push(SelectOption::new(curve_id.clone(), *label));
                    }
                }
                opts
            }
        };

        let mut target_select = Select::new((base_id.clone(), "target-curve"))
            .label(translations.autoeq_form.target_curve)
            .options(target_curve_options)
            .selected(&config.goals.target_curve)
            .is_open(ui_state.target_curve_open)
            .disabled(disabled)
            .size(SelectSize::Xs)
            .theme(theme.select_theme.clone());

        if let Some(ref h) = on_target_curve_change_rc {
            let h = h.clone();
            target_select = target_select.on_change(move |value, w, cx| h(value.as_ref(), w, cx));
        }
        if let Some(ref h) = on_target_curve_toggle_rc {
            let h = h.clone();
            target_select = target_select.on_toggle(move |open, w, cx| h(open, w, cx));
        }

        form = form.child(target_select);
    }

    // ================================================================
    // Intermediate-only: Curated Parameters
    // ================================================================
    if detail_level == DetailLevel::Intermediate {
        // --- Optimization Goal ---
        form = form.child(
            VStack::new()
                .spacing(StackSpacing::None)
                .child(
                    Text::section_header(translations.autoeq_optimization_goals)
                        .color(theme.header_color),
                )
                .child(
                    Text::new(translations.autoeq_what_optimize)
                        .size(TextSize::Xs)
                        .color(theme.description_color),
                ),
        );

        // Loss function
        let loss_options_source = match optimization_type {
            OptimizationType::Headphone => HEADPHONE_LOSS_TYPE_OPTIONS,
            OptimizationType::Speaker => LOSS_TYPE_OPTIONS,
        };
        let loss_type_options: Vec<SelectOption> = loss_options_source
            .iter()
            .map(|(val, lbl)| SelectOption::new(*val, *lbl))
            .collect();

        let mut loss_select = Select::new((base_id.clone(), "loss-type"))
            .label(translations.autoeq_form.parameters.loss_function)
            .options(loss_type_options)
            .selected(&config.goals.loss_type)
            .is_open(ui_state.loss_type_open)
            .disabled(disabled)
            .size(SelectSize::Xs)
            .theme(theme.select_theme.clone());

        if let Some(ref h) = on_loss_type_change_rc {
            let h = h.clone();
            loss_select = loss_select.on_change(move |value, w, cx| h(value.as_ref(), w, cx));
        }
        if let Some(ref h) = on_loss_type_toggle_rc {
            let h = h.clone();
            loss_select = loss_select.on_toggle(move |open, w, cx| h(open, w, cx));
        }

        // Loss description tooltip
        let loss_desc = LOSS_TYPE_DESCRIPTIONS
            .iter()
            .find(|(id, _)| *id == config.goals.loss_type.as_str())
            .map(|(_, desc)| *desc);

        let mut loss_group = VStack::new().spacing(StackSpacing::Xs).child(loss_select);
        if let Some(desc) = loss_desc {
            loss_group = loss_group.child(
                Text::new(desc)
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            );
        }
        form = form.child(loss_group);

        // --- Filter Design ---
        form = form.child(
            VStack::new()
                .spacing(StackSpacing::None)
                .child(Text::section_header(translations.autoeq_form.filter_design).color(theme.header_color))
                .child(
                    Text::new(translations.autoeq_how_many_filters)
                        .size(TextSize::Xs)
                        .color(theme.description_color),
                ),
        );

        // Num filters
        let mut num_filters_input = NumberInput::new((base_id.clone(), "num-filters"))
            .label(translations.autoeq_form.parameters.number_filters)
            .value(config.eq_design.num_filters as f64)
            .min(ParamLimits::NUM_FILTERS.min)
            .max(ParamLimits::NUM_FILTERS.max)
            .step(ParamLimits::NUM_FILTERS.step)
            .disabled(disabled)
            .size(NumberInputSize::Xs)
            .theme(theme.number_input_theme.clone());

        if let Some(ref h) = on_num_filters_change_rc {
            let h = h.clone();
            num_filters_input =
                num_filters_input.on_change(move |v, w, cx| h(v as usize, w, cx));
        }

        let mut filters_group = VStack::new()
            .spacing(StackSpacing::Xs)
            .child(num_filters_input);

        // Warning for high filter count
        if let Some(warning) = field_warning("num_filters", config.eq_design.num_filters as f64) {
            filters_group = filters_group.child(
                Text::new(warning)
                    .size(TextSize::Xs)
                    .color(theme.accent),
            );
        }
        // Hint
        if let Some(hint) = field_hint("num_filters") {
            filters_group = filters_group.child(
                Text::new(hint)
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            );
        }
        form = form.child(filters_group);

        // PEQ model
        let peq_model_options: Vec<SelectOption> = PEQ_MODEL_OPTIONS
            .iter()
            .map(|(val, lbl)| SelectOption::new(*val, *lbl))
            .collect();

        let mut peq_select = Select::new((base_id.clone(), "peq-model"))
            .label(translations.autoeq_form.parameters.filter_type)
            .options(peq_model_options)
            .selected(&config.eq_design.peq_model)
            .is_open(ui_state.peq_model_open)
            .disabled(disabled)
            .size(SelectSize::Xs)
            .theme(theme.select_theme.clone());

        if let Some(ref h) = on_peq_model_change_rc {
            let h = h.clone();
            peq_select = peq_select.on_change(move |value, w, cx| h(value.as_ref(), w, cx));
        }
        if let Some(ref h) = on_peq_model_toggle_rc {
            let h = h.clone();
            peq_select = peq_select.on_toggle(move |open, w, cx| h(open, w, cx));
        }

        // PEQ model description
        let peq_desc = PEQ_MODEL_DESCRIPTIONS
            .iter()
            .find(|(id, _)| *id == config.eq_design.peq_model.as_str())
            .map(|(_, desc)| *desc);

        let mut peq_group = VStack::new().spacing(StackSpacing::Xs).child(peq_select);
        if let Some(desc) = peq_desc {
            peq_group = peq_group.child(
                Text::new(desc)
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            );
        }
        form = form.child(peq_group);

        // Frequency range (min/max)
        let mut freq_row = HStack::new().spacing(StackSpacing::Sm);

        let mut min_freq_input = NumberInput::new((base_id.clone(), "min-freq"))
            .label(translations.autoeq_form.min_frequency_hz)
            .value(config.eq_design.min_freq)
            .min(ParamLimits::FREQUENCY.min)
            .max(ParamLimits::FREQUENCY.max)
            .step(ParamLimits::FREQUENCY.step)
            .disabled(disabled)
            .size(NumberInputSize::Xs)
            .theme(theme.number_input_theme.clone());

        if let Some(ref h) = on_min_freq_change_rc {
            let h = h.clone();
            min_freq_input = min_freq_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        let mut max_freq_input = NumberInput::new((base_id.clone(), "max-freq"))
            .label(translations.autoeq_form.max_frequency_hz)
            .value(config.eq_design.max_freq)
            .min(ParamLimits::FREQUENCY.min)
            .max(ParamLimits::FREQUENCY.max)
            .step(ParamLimits::FREQUENCY.step)
            .disabled(disabled)
            .size(NumberInputSize::Xs)
            .theme(theme.number_input_theme.clone());

        if let Some(ref h) = on_max_freq_change_rc {
            let h = h.clone();
            max_freq_input = max_freq_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        freq_row = freq_row.child(min_freq_input).child(max_freq_input);

        let mut freq_group = VStack::new().spacing(StackSpacing::Xs).child(freq_row);
        if let Some(hint) = field_hint("min_freq") {
            freq_group = freq_group.child(
                Text::new(hint)
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            );
        }
        form = form.child(freq_group);

        // --- Quality Slider (maps to population + maxeval) ---
        form = form.child(
            VStack::new()
                .spacing(StackSpacing::None)
                .child(Text::section_header(translations.autoeq_form.optimization_quality).color(theme.header_color))
                .child(
                    Text::new(translations.autoeq_higher_quality)
                        .size(TextSize::Xs)
                        .color(theme.description_color),
                ),
        );

        // Compute current quality level from population
        let current_quality = population_to_quality(config.algorithm.population) as f64;

        let quality_label_text = quality_label(current_quality as f32);

        let mut quality_input = NumberInput::new((base_id.clone(), "quality"))
            .label(format!("Quality: {quality_label_text}"))
            .value(current_quality * 100.0) // Show as 0-100 percentage
            .min(0.0)
            .max(100.0)
            .step(10.0)
            .disabled(disabled)
            .size(NumberInputSize::Xs)
            .theme(theme.number_input_theme.clone());

        // When quality changes, update both population and maxeval
        if on_population_change_rc.is_some() || on_maxeval_change_rc.is_some() {
            let pop_cb = on_population_change_rc.clone();
            let eval_cb = on_maxeval_change_rc.clone();
            quality_input = quality_input.on_change(move |v, w, cx| {
                let q = (v / 100.0) as f32;
                let (pop, eval) = quality_to_optimizer_params(q);
                if let Some(ref h) = pop_cb {
                    h(pop, w, cx);
                }
                if let Some(ref h) = eval_cb {
                    h(eval, w, cx);
                }
            });
        }

        form = form.child(quality_input);
    }

    div().id(id).child(form)
}
