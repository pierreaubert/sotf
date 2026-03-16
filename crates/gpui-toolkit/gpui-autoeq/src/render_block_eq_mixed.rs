// === Shared: Mixed mode config ===
// Appends to block_out. Expects config.opt_mode == "mixed" check done by caller.
{
    let mut mixed_freq_input =
        NumberInput::new((base_id.clone(), "mixed-crossover-freq"))
            .value(config.mixed_crossover_freq)
            .min(ParamLimits::MIXED_CROSSOVER_FREQ.min)
            .max(ParamLimits::MIXED_CROSSOVER_FREQ.max)
            .step(ParamLimits::MIXED_CROSSOVER_FREQ.step)
            .decimals(0)
            .label("Crossover Freq (Hz)")
            .size(NumberInputSize::Sm)
            .width(140.0)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_mixed_crossover_freq_change_rc {
        let h = handler.clone();
        mixed_freq_input = mixed_freq_input.on_change(move |v, w, cx| h(v, w, cx));
    }

    let mixed_type_options: Vec<SelectOption> = MIXED_CROSSOVER_TYPE_OPTIONS
        .iter()
        .map(|(val, lbl)| SelectOption::new(*val, *lbl))
        .collect();

    let mut mixed_type_select =
        Select::new((base_id.clone(), "mixed-crossover-type"))
            .label("Crossover Type")
            .options(mixed_type_options)
            .selected(&config.mixed_crossover_type)
            .is_open(ui_state.mixed_crossover_type_open)
            .disabled(disabled)
            .size(SelectSize::Xs)
            .theme(theme.select_theme.clone());

    if let Some(ref handler) = on_mixed_crossover_type_toggle_rc {
        let h = handler.clone();
        mixed_type_select =
            mixed_type_select.on_toggle(move |open, w, cx| h(open, w, cx));
    }

    if let Some(ref handler) = on_mixed_crossover_type_change_rc {
        let h = handler.clone();
        mixed_type_select =
            mixed_type_select.on_change(move |val, w, cx| h(val.as_ref(), w, cx));
    }

    let mixed_band_options: Vec<SelectOption> = MIXED_FIR_BAND_OPTIONS
        .iter()
        .map(|(val, lbl)| SelectOption::new(*val, *lbl))
        .collect();

    let mut mixed_band_select = Select::new((base_id.clone(), "mixed-fir-band"))
        .label("FIR Band")
        .options(mixed_band_options)
        .selected(&config.mixed_fir_band)
        .is_open(ui_state.mixed_fir_band_open)
        .disabled(disabled)
        .size(SelectSize::Xs)
        .theme(theme.select_theme.clone());

    if let Some(ref handler) = on_mixed_fir_band_toggle_rc {
        let h = handler.clone();
        mixed_band_select =
            mixed_band_select.on_toggle(move |open, w, cx| h(open, w, cx));
    }

    if let Some(ref handler) = on_mixed_fir_band_change_rc {
        let h = handler.clone();
        mixed_band_select =
            mixed_band_select.on_change(move |val, w, cx| h(val.as_ref(), w, cx));
    }

    block_out = block_out
        .child(mixed_freq_input)
        .child(mixed_type_select)
        .child(mixed_band_select);
}
