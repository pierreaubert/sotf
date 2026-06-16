// Section 6: Multiple Measurements Per Speaker — strategy and weights
// This file is include!()'d from render_body.rs, sharing its scope.
{
    let mut section = VStack::new().spacing(StackSpacing::Sm);

    section = section.child(
        VStack::new()
            .spacing(StackSpacing::None)
            .child(
                Text::section_header("Multiple Measurements Per Speaker")
                    .color(theme.header_color),
            )
            .child(
                Text::new("Strategy for combining multiple measurement positions")
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            ),
    );

    // Enable toggle
    let mut mm_toggle = Toggle::new((base_id.clone(), "mm-enabled"))
        .size(ToggleSize::Sm)
        .checked(config.system_optimization.use_multi_measurement)
        .theme(toggle_theme.clone());

    if let Some(ref h) = on_use_multi_measurement_change_rc {
        let h = h.clone();
        mm_toggle = mm_toggle.on_change(move |v, w, cx| h(v, w, cx));
    }

    section = section.child(
        HStack::new()
            .justify(StackJustify::SpaceBetween)
            .child(Text::new("Enable Multi-Measurement").size(TextSize::Xs).color(theme.label_color))
            .child(mm_toggle),
    );

    if config.system_optimization.use_multi_measurement {
        // Strategy select
        let strategy_options: Vec<SelectOption> = MULTI_MEASUREMENT_STRATEGY_OPTIONS
            .iter()
            .map(|(val, lbl)| SelectOption::new(*val, *lbl))
            .collect();

        let mut strategy_select = Select::new((base_id.clone(), "mm-strategy"))
            .label("Strategy")
            .options(strategy_options)
            .selected(&config.system_optimization.multi_measurement_strategy)
            .is_open(ui_state.multi_measurement_strategy_open)
            .disabled(disabled)
            .size(SelectSize::Xs)
            .theme(theme.select_theme.clone());

        if let Some(ref h) = on_multi_measurement_strategy_toggle_rc {
            let h = h.clone();
            strategy_select = strategy_select.on_toggle(move |open, w, cx| h(open, w, cx));
        }
        if let Some(ref h) = on_multi_measurement_strategy_change_rc {
            let h = h.clone();
            strategy_select = strategy_select.on_change(move |val, w, cx| h(val.as_ref(), w, cx));
        }

        section = section.child(strategy_select);

        // Strategy-dependent params
        if config.system_optimization.multi_measurement_strategy == "variance_penalized" {
            let mut lambda_input = NumberInput::new((base_id.clone(), "mm-variance-lambda"))
                .value(config.system_optimization.multi_measurement_variance_lambda)
                .min(ParamLimits::VARIANCE_LAMBDA.min)
                .max(ParamLimits::VARIANCE_LAMBDA.max)
                .step(ParamLimits::VARIANCE_LAMBDA.step)
                .decimals(1)
                .label("Variance Lambda")
                .size(NumberInputSize::Sm)
                .disabled(disabled)
                .theme(theme.number_input_theme.clone());

            if let Some(ref h) = on_multi_measurement_variance_lambda_change_rc {
                let h = h.clone();
                lambda_input = lambda_input.on_change(move |v, w, cx| h(v, w, cx));
            }

            section = section.child(lambda_input);
        }

        // Per-measurement weights (for weighted_sum strategy)
        if config.system_optimization.multi_measurement_strategy == "weighted_sum" && !config.system_optimization.multi_measurement_weights.is_empty() {
            section = section.child(
                Text::label("Measurement Weights").color(theme.header_color),
            );

            for (i, weight) in config.system_optimization.multi_measurement_weights.iter().enumerate() {
                let label = config
                    .system_optimization
                    .multi_measurement_labels
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("Measurement {}", i + 1));

                let mut weight_input = NumberInput::new((base_id.clone(), &format!("mm-weight-{i}")))
                    .value(*weight)
                    .min(ParamLimits::WEIGHT.min)
                    .max(ParamLimits::WEIGHT.max)
                    .step(ParamLimits::WEIGHT.step)
                    .decimals(2)
                    .label(&label)
                    .size(NumberInputSize::Sm)
                    .disabled(disabled)
                    .theme(theme.number_input_theme.clone());

                if let Some(ref h) = on_multi_measurement_weight_change_rc {
                    let h = h.clone();
                    let idx = i;
                    weight_input = weight_input.on_change(move |v, w, cx| h(idx, v, w, cx));
                }

                section = section.child(weight_input);
            }
        }
    }

    Card::new().content(section)
}
