// === Shared EQ Design parameter block ===
// Appends EQ Design widgets to `block_out: VStack`.
//
// Required in scope:
//   block_out (mut VStack), base_id, config, ui_state, theme, disabled, is_narrow_layout,
//   is_fir, is_iir, hide_sample_rate, hide_spacing, eq_design_iir_before_fir (bool),
//   on_fir_taps_change_rc, on_fir_phase_change_rc, on_fir_phase_toggle_rc,
//   on_num_filters_change_rc, on_sample_rate_change_rc,
//   on_min_db_change_rc, on_max_db_change_rc, on_min_q_change_rc, on_max_q_change_rc,
//   on_min_freq_change_rc, on_max_freq_change_rc, on_peq_model_change_rc, on_peq_model_toggle_rc,
//   on_spacing_weight_change_rc, on_min_spacing_oct_change_rc,
//   on_mixed_crossover_freq_change_rc, on_mixed_crossover_type_change_rc,
//   on_mixed_crossover_type_toggle_rc, on_mixed_fir_band_change_rc, on_mixed_fir_band_toggle_rc

{
// --- IIR Filters + Sample Rate (shown first when eq_design_iir_before_fir) ---
if is_iir && eq_design_iir_before_fir {
    include!("render_block_eq_iir_filters.rs");
}

// --- FIR Taps + Phase (shown first when !eq_design_iir_before_fir) ---
if is_fir && !eq_design_iir_before_fir {
    include!("render_block_eq_fir.rs");
}

// --- Mixed mode config ---
if config.opt_mode == "mixed" {
    include!("render_block_eq_mixed.rs");
}

// --- IIR Filters + Sample Rate (shown second when !eq_design_iir_before_fir) ---
if is_iir && !eq_design_iir_before_fir {
    include!("render_block_eq_iir_filters.rs");
}

// --- FIR Taps + Phase (shown second when eq_design_iir_before_fir) ---
if is_fir && eq_design_iir_before_fir {
    include!("render_block_eq_fir.rs");
}

// --- dB Range ---
{
    let mut min_db_input = NumberInput::new((base_id.clone(), "min-db"))
        .value(config.min_db)
        .min(ParamLimits::DB.min)
        .max(ParamLimits::DB.max)
        .step(ParamLimits::DB.step)
        .decimals(1)
        .label("Min dB")
        .size(NumberInputSize::Sm)
        .width(100.0)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_min_db_change_rc {
        let h = handler.clone();
        min_db_input = min_db_input.on_change(move |v, w, cx| h(v, w, cx));
    }

    let mut max_db_input = NumberInput::new((base_id.clone(), "max-db"))
        .value(config.max_db)
        .min(ParamLimits::DB.min)
        .max(ParamLimits::DB.max)
        .step(ParamLimits::DB.step)
        .decimals(1)
        .label("Max dB")
        .size(NumberInputSize::Sm)
        .width(100.0)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_max_db_change_rc {
        let h = handler.clone();
        max_db_input = max_db_input.on_change(move |v, w, cx| h(v, w, cx));
    }

    block_out = if is_narrow_layout {
        block_out.child(min_db_input).child(max_db_input)
    } else {
        block_out.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(min_db_input)
                .child(max_db_input),
        )
    };
}

// --- Q Range (IIR only) ---
if is_iir {
    let mut min_q_input = NumberInput::new((base_id.clone(), "min-q"))
        .value(config.min_q)
        .min(ParamLimits::Q.min)
        .max(ParamLimits::Q.max)
        .step(ParamLimits::Q.step)
        .decimals(1)
        .label("Min Q")
        .size(NumberInputSize::Sm)
        .width(100.0)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_min_q_change_rc {
        let h = handler.clone();
        min_q_input = min_q_input.on_change(move |v, w, cx| h(v, w, cx));
    }

    let mut max_q_input = NumberInput::new((base_id.clone(), "max-q"))
        .value(config.max_q)
        .min(ParamLimits::Q.min)
        .max(ParamLimits::Q.max)
        .step(ParamLimits::Q.step)
        .decimals(1)
        .label("Max Q")
        .size(NumberInputSize::Sm)
        .width(100.0)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_max_q_change_rc {
        let h = handler.clone();
        max_q_input = max_q_input.on_change(move |v, w, cx| h(v, w, cx));
    }

    block_out = if is_narrow_layout {
        block_out.child(min_q_input).child(max_q_input)
    } else {
        block_out.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(min_q_input)
                .child(max_q_input),
        )
    };
}

// --- Frequency Range ---
{
    let mut min_freq_input = NumberInput::new((base_id.clone(), "min-freq"))
        .value(config.min_freq)
        .min(ParamLimits::FREQUENCY.min)
        .max(ParamLimits::FREQUENCY.max)
        .step(ParamLimits::FREQUENCY.step)
        .decimals(0)
        .label("Min Freq")
        .size(NumberInputSize::Sm)
        .width(100.0)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_min_freq_change_rc {
        let h = handler.clone();
        min_freq_input = min_freq_input.on_change(move |v, w, cx| h(v, w, cx));
    }

    let mut max_freq_input = NumberInput::new((base_id.clone(), "max-freq"))
        .value(config.max_freq)
        .min(ParamLimits::FREQUENCY.min)
        .max(ParamLimits::FREQUENCY.max)
        .step(ParamLimits::FREQUENCY.step)
        .decimals(0)
        .label("Max Freq")
        .size(NumberInputSize::Sm)
        .width(100.0)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_max_freq_change_rc {
        let h = handler.clone();
        max_freq_input = max_freq_input.on_change(move |v, w, cx| h(v, w, cx));
    }

    block_out = if is_narrow_layout {
        block_out.child(min_freq_input).child(max_freq_input)
    } else {
        block_out.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(min_freq_input)
                .child(max_freq_input),
        )
    };
}

// --- PEQ Model (IIR only) ---
if is_iir {
    let peq_model_options: Vec<SelectOption> = PEQ_MODEL_OPTIONS
        .iter()
        .map(|(val, lbl)| SelectOption::new(*val, *lbl))
        .collect();

    let mut peq_model_select = Select::new((base_id.clone(), "peq-model"))
        .label("PEQ Model")
        .options(peq_model_options)
        .selected(&config.peq_model)
        .is_open(ui_state.peq_model_open)
        .disabled(disabled)
        .size(SelectSize::Xs)
        .theme(theme.select_theme.clone());

    if let Some(ref handler) = on_peq_model_toggle_rc {
        let h = handler.clone();
        peq_model_select = peq_model_select.on_toggle(move |open, w, cx| h(open, w, cx));
    }

    if let Some(ref handler) = on_peq_model_change_rc {
        let h = handler.clone();
        peq_model_select =
            peq_model_select.on_change(move |value, w, cx| h(value.as_ref(), w, cx));
    }

    block_out = block_out.child(peq_model_select);

    // Spacing constraints (hidden in some contexts)
    if !hide_spacing {
        let mut spacing_weight_input =
            NumberInput::new((base_id.clone(), "spacing-weight"))
                .value(config.spacing_weight)
                .min(ParamLimits::SPACING_WEIGHT.min)
                .max(ParamLimits::SPACING_WEIGHT.max)
                .step(ParamLimits::SPACING_WEIGHT.step)
                .decimals(1)
                .label("Spacing Weight")
                .size(NumberInputSize::Sm)
                .width(100.0)
                .disabled(disabled)
                .theme(theme.number_input_theme.clone());

        if let Some(ref handler) = on_spacing_weight_change_rc {
            let h = handler.clone();
            spacing_weight_input = spacing_weight_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        let mut min_spacing_oct_input =
            NumberInput::new((base_id.clone(), "min-spacing-oct"))
                .value(config.min_spacing_oct)
                .min(ParamLimits::MIN_SPACING_OCT.min)
                .max(ParamLimits::MIN_SPACING_OCT.max)
                .step(ParamLimits::MIN_SPACING_OCT.step)
                .decimals(2)
                .label("Min Spacing (oct)")
                .size(NumberInputSize::Sm)
                .width(120.0)
                .disabled(disabled)
                .theme(theme.number_input_theme.clone());

        if let Some(ref handler) = on_min_spacing_oct_change_rc {
            let h = handler.clone();
            min_spacing_oct_input =
                min_spacing_oct_input.on_change(move |v, w, cx| h(v, w, cx));
        }

        block_out = if is_narrow_layout {
            block_out
                .child(spacing_weight_input)
                .child(min_spacing_oct_input)
        } else {
            block_out.child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(spacing_weight_input)
                    .child(min_spacing_oct_input),
            )
        };
    }
}
}
