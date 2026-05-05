// === Shared Optimizer Settings block ===
// Appends algorithm, population/maxeval, tolerance, DE params, and refinement to `block_out`.
//
// Required in scope:
//   block_out (mut VStack), base_id, config, ui_state, theme, disabled, is_narrow_layout,
//   toggle_theme (ToggleTheme), hide_tolerance, hide_de_params,
//   on_algo_change_rc, on_algo_toggle_rc, on_population_change_rc, on_maxeval_change_rc,
//   on_tolerance_change_rc, on_atolerance_change_rc,
//   on_bo_initial_samples_change_rc, on_bo_batch_size_change_rc,
//   on_bo_posterior_std_threshold_change_rc, on_bo_acquisition_change_rc,
//   on_bo_acquisition_toggle_rc, on_bo_ehvi_change_rc,
//   on_de_f_change_rc, on_de_cr_change_rc, on_strategy_change_rc, on_strategy_toggle_rc,
//   on_adaptive_weight_f_change_rc, on_adaptive_weight_cr_change_rc,
//   on_refine_change_rc, on_local_algo_change_rc, on_local_algo_toggle_rc

{
// --- Algorithm select ---
{
    let algo_options: Vec<SelectOption> = ALGORITHM_OPTIONS
        .iter()
        .map(|(val, lbl)| SelectOption::new(*val, *lbl))
        .collect();

    let mut algo_select = Select::new((base_id.clone(), "algo"))
        .label("Algorithm")
        .options(algo_options)
        .selected(&config.algo)
        .is_open(ui_state.algo_open)
        .disabled(disabled)
        .size(SelectSize::Xs)
        .theme(theme.select_theme.clone());

    if let Some(ref handler) = on_algo_toggle_rc {
        let h = handler.clone();
        algo_select = algo_select.on_toggle(move |open, w, cx| h(open, w, cx));
    }

    if let Some(ref handler) = on_algo_change_rc {
        let h = handler.clone();
        algo_select = algo_select.on_change(move |value, w, cx| h(value.as_ref(), w, cx));
    }

    block_out = block_out.child(algo_select);
}

// --- Population + Max Evals ---
{
    let mut population_input = NumberInput::new((base_id.clone(), "population"))
        .value(config.population as f64)
        .min(ParamLimits::POPULATION.min)
        .max(ParamLimits::POPULATION.max)
        .step(ParamLimits::POPULATION.step)
        .decimals(0)
        .label("Population")
        .size(NumberInputSize::Sm)
        .width(100.0)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_population_change_rc {
        let h = handler.clone();
        population_input =
            population_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
    }

    let mut maxeval_input = NumberInput::new((base_id.clone(), "maxeval"))
        .value(config.maxeval as f64)
        .min(ParamLimits::MAXEVAL.min)
        .max(ParamLimits::MAXEVAL.max)
        .step(ParamLimits::MAXEVAL.step)
        .decimals(0)
        .label("Max Evals")
        .size(NumberInputSize::Sm)
        .width(100.0)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_maxeval_change_rc {
        let h = handler.clone();
        maxeval_input =
            maxeval_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
    }

    block_out = if is_narrow_layout {
        block_out.child(population_input).child(maxeval_input)
    } else {
        block_out.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(population_input)
                .child(maxeval_input),
        )
    };
}

// --- Tolerance (conditional) ---
if !hide_tolerance {
    let mut tolerance_input = NumberInput::new((base_id.clone(), "tolerance"))
        .value(config.tolerance)
        .min(ParamLimits::TOLERANCE.min)
        .max(ParamLimits::TOLERANCE.max)
        .step(ParamLimits::TOLERANCE.step)
        .decimals(8)
        .label("Tolerance")
        .size(NumberInputSize::Sm)
        .width(120.0)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_tolerance_change_rc {
        let h = handler.clone();
        tolerance_input = tolerance_input.on_change(move |v, w, cx| h(v, w, cx));
    }

    let mut atolerance_input = NumberInput::new((base_id.clone(), "atolerance"))
        .value(config.atolerance)
        .min(ParamLimits::TOLERANCE.min)
        .max(ParamLimits::TOLERANCE.max)
        .step(ParamLimits::TOLERANCE.step)
        .decimals(8)
        .label("Abs Tolerance")
        .size(NumberInputSize::Sm)
        .width(120.0)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_atolerance_change_rc {
        let h = handler.clone();
        atolerance_input = atolerance_input.on_change(move |v, w, cx| h(v, w, cx));
    }

    block_out = if is_narrow_layout {
        block_out
            .child(tolerance_input)
            .child(atolerance_input)
    } else {
        block_out.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(tolerance_input)
                .child(atolerance_input),
        )
    };
}

// --- BO-specific settings (conditional) ---
if config.algo.eq_ignore_ascii_case("autoeq:bo") || config.algo.eq_ignore_ascii_case("bo") {
    let mut bo_initial_input = NumberInput::new((base_id.clone(), "bo-initial-samples"))
        .value(config.bo_initial_samples as f64)
        .min(ParamLimits::BO_INITIAL_SAMPLES.min)
        .max(ParamLimits::BO_INITIAL_SAMPLES.max)
        .step(ParamLimits::BO_INITIAL_SAMPLES.step)
        .decimals(0)
        .label("BO Initial")
        .size(NumberInputSize::Sm)
        .width(100.0)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_bo_initial_samples_change_rc {
        let h = handler.clone();
        bo_initial_input =
            bo_initial_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
    }

    let mut bo_batch_input = NumberInput::new((base_id.clone(), "bo-batch-size"))
        .value(config.bo_batch_size as f64)
        .min(ParamLimits::BO_BATCH_SIZE.min)
        .max(ParamLimits::BO_BATCH_SIZE.max)
        .step(ParamLimits::BO_BATCH_SIZE.step)
        .decimals(0)
        .label("BO Batch")
        .size(NumberInputSize::Sm)
        .width(100.0)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_bo_batch_size_change_rc {
        let h = handler.clone();
        bo_batch_input = bo_batch_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
    }

    let mut bo_std_input = NumberInput::new((base_id.clone(), "bo-posterior-std"))
        .value(config.bo_posterior_std_threshold)
        .min(ParamLimits::BO_POSTERIOR_STD.min)
        .max(ParamLimits::BO_POSTERIOR_STD.max)
        .step(ParamLimits::BO_POSTERIOR_STD.step)
        .decimals(3)
        .label("BO Std Stop")
        .size(NumberInputSize::Sm)
        .width(120.0)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_bo_posterior_std_threshold_change_rc {
        let h = handler.clone();
        bo_std_input = bo_std_input.on_change(move |v, w, cx| h(v, w, cx));
    }

    block_out = if is_narrow_layout {
        block_out
            .child(bo_initial_input)
            .child(bo_batch_input)
            .child(bo_std_input)
    } else {
        block_out.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(bo_initial_input)
                .child(bo_batch_input)
                .child(bo_std_input),
        )
    };

    let bo_acquisition_options: Vec<SelectOption> = BO_ACQUISITION_OPTIONS
        .iter()
        .map(|(val, lbl)| SelectOption::new(*val, *lbl))
        .collect();

    let mut bo_acquisition_select = Select::new((base_id.clone(), "bo-acquisition"))
        .label("BO Acquisition")
        .options(bo_acquisition_options)
        .selected(&config.bo_acquisition)
        .is_open(ui_state.bo_acquisition_open)
        .disabled(disabled)
        .size(SelectSize::Xs)
        .theme(theme.select_theme.clone());

    if let Some(ref handler) = on_bo_acquisition_toggle_rc {
        let h = handler.clone();
        bo_acquisition_select =
            bo_acquisition_select.on_toggle(move |open, w, cx| h(open, w, cx));
    }

    if let Some(ref handler) = on_bo_acquisition_change_rc {
        let h = handler.clone();
        bo_acquisition_select =
            bo_acquisition_select.on_change(move |value, w, cx| h(value.as_ref(), w, cx));
    }

    let mut bo_ehvi_toggle = Toggle::new((base_id.clone(), "bo-ehvi"))
        .size(ToggleSize::Sm)
        .checked(config.bo_ehvi)
        .theme(toggle_theme.clone());

    if let Some(ref handler) = on_bo_ehvi_change_rc {
        let h = handler.clone();
        bo_ehvi_toggle = bo_ehvi_toggle.on_change(move |v, w, cx| h(v, w, cx));
    }

    block_out = if is_narrow_layout {
        block_out.child(bo_acquisition_select).child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .justify(StackJustify::SpaceBetween)
                .child(
                    Text::new("qEHVI")
                        .size(TextSize::Xs)
                        .color(theme.label_color),
                )
                .child(bo_ehvi_toggle),
        )
    } else {
        block_out.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(bo_acquisition_select)
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .justify(StackJustify::SpaceBetween)
                        .child(
                            Text::new("qEHVI")
                                .size(TextSize::Xs)
                                .color(theme.label_color),
                        )
                        .child(bo_ehvi_toggle),
                ),
        )
    };
}

// --- DE-specific settings (conditional) ---
if !hide_de_params && config.algo.contains(":de") {
    {
        // DE Strategy dropdown
        let strategy_options: Vec<SelectOption> = DE_STRATEGY_OPTIONS
            .iter()
            .map(|(val, lbl)| SelectOption::new(*val, *lbl))
            .collect();

        let mut strategy_select = Select::new((base_id.clone(), "strategy"))
            .label("DE Strategy")
            .options(strategy_options)
            .selected(&config.strategy)
            .is_open(ui_state.strategy_open)
            .disabled(disabled)
            .size(SelectSize::Xs)
            .theme(theme.select_theme.clone());

        if let Some(ref handler) = on_strategy_toggle_rc {
            let h = handler.clone();
            strategy_select =
                strategy_select.on_toggle(move |open, w, cx| h(open, w, cx));
        }

        if let Some(ref handler) = on_strategy_change_rc {
            let h = handler.clone();
            strategy_select =
                strategy_select.on_change(move |value, w, cx| h(value.as_ref(), w, cx));
        }

        block_out = block_out.child(strategy_select);

        // DE F and CR row
        let mut de_f_input = NumberInput::new((base_id.clone(), "de-f"))
            .value(config.de_f)
            .min(ParamLimits::DE_FACTOR.min)
            .max(ParamLimits::DE_FACTOR.max)
            .step(ParamLimits::DE_FACTOR.step)
            .decimals(1)
            .label("Mutation (F)")
            .size(NumberInputSize::Sm)
            .width(100.0)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

        if let Some(ref handler) = on_de_f_change_rc {
            let h = handler.clone();
            de_f_input = de_f_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        let mut de_cr_input = NumberInput::new((base_id.clone(), "de-cr"))
            .value(config.de_cr)
            .min(ParamLimits::DE_CR.min)
            .max(ParamLimits::DE_CR.max)
            .step(ParamLimits::DE_CR.step)
            .decimals(1)
            .label("Recomb (CR)")
            .size(NumberInputSize::Sm)
            .width(100.0)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

        if let Some(ref handler) = on_de_cr_change_rc {
            let h = handler.clone();
            de_cr_input = de_cr_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        block_out = if is_narrow_layout {
            block_out.child(de_f_input).child(de_cr_input)
        } else {
            block_out.child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(de_f_input)
                    .child(de_cr_input),
            )
        };

        // Adaptive weight inputs (only for adaptive strategies)
        if config.strategy.starts_with("adaptive") {
            let mut weight_f_input = NumberInput::new((base_id.clone(), "adaptive-weight-f"))
                .value(config.adaptive_weight_f)
                .min(ParamLimits::ADAPTIVE_WEIGHT.min)
                .max(ParamLimits::ADAPTIVE_WEIGHT.max)
                .step(ParamLimits::ADAPTIVE_WEIGHT.step)
                .decimals(2)
                .label("Adapt Weight F")
                .size(NumberInputSize::Sm)
                .width(120.0)
                .disabled(disabled)
                .theme(theme.number_input_theme.clone());

            if let Some(ref handler) = on_adaptive_weight_f_change_rc {
                let h = handler.clone();
                weight_f_input = weight_f_input.on_change(move |v, w, cx| h(v, w, cx));
            }

            let mut weight_cr_input = NumberInput::new((base_id.clone(), "adaptive-weight-cr"))
                .value(config.adaptive_weight_cr)
                .min(ParamLimits::ADAPTIVE_WEIGHT.min)
                .max(ParamLimits::ADAPTIVE_WEIGHT.max)
                .step(ParamLimits::ADAPTIVE_WEIGHT.step)
                .decimals(2)
                .label("Adapt Weight CR")
                .size(NumberInputSize::Sm)
                .width(120.0)
                .disabled(disabled)
                .theme(theme.number_input_theme.clone());

            if let Some(ref handler) = on_adaptive_weight_cr_change_rc {
                let h = handler.clone();
                weight_cr_input = weight_cr_input.on_change(move |v, w, cx| h(v, w, cx));
            }

            block_out = if is_narrow_layout {
                block_out.child(weight_f_input).child(weight_cr_input)
            } else {
                block_out.child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .child(weight_f_input)
                        .child(weight_cr_input),
                )
            };
        }
    }
}

// --- Local Refinement ---
{
    let mut refine_toggle = Toggle::new((base_id.clone(), "refine"))
        .size(ToggleSize::Sm)
        .checked(config.refine)
        .theme(toggle_theme.clone());

    if let Some(ref handler) = on_refine_change_rc {
        let h = handler.clone();
        refine_toggle = refine_toggle.on_change(move |v, w, cx| h(v, w, cx));
    }

    block_out = block_out.child(
        HStack::new()
            .spacing(StackSpacing::Md)
            .justify(StackJustify::SpaceBetween)
            .child(
                Text::new("Local Refinement")
                    .size(TextSize::Xs)
                    .color(theme.label_color),
            )
            .child(refine_toggle),
    );

    if config.refine {
        let local_algo_options: Vec<SelectOption> = LOCAL_ALGO_OPTIONS
            .iter()
            .map(|(val, lbl)| SelectOption::new(*val, *lbl))
            .collect();

        let mut local_algo_select = Select::new((base_id.clone(), "local-algo"))
            .label("Local Algo")
            .options(local_algo_options)
            .selected(&config.local_algo)
            .is_open(ui_state.local_algo_open)
            .disabled(disabled)
            .size(SelectSize::Xs)
            .theme(theme.select_theme.clone());

        if let Some(ref handler) = on_local_algo_toggle_rc {
            let h = handler.clone();
            local_algo_select =
                local_algo_select.on_toggle(move |open, w, cx| h(open, w, cx));
        }

        if let Some(ref handler) = on_local_algo_change_rc {
            let h = handler.clone();
            local_algo_select =
                local_algo_select.on_change(move |value, w, cx| h(value.as_ref(), w, cx));
        }

        block_out = block_out.child(local_algo_select);
    }
}
}
