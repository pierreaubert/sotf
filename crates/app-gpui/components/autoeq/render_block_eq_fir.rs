// === Shared: FIR Taps + Phase ===
// Appends to block_out. Expects is_fir check done by caller.
{
    let mut fir_taps_input = NumberInput::new((base_id.clone(), "fir-taps"))
        .value(config.eq_design.fir_taps as f64)
        .min(ParamLimits::FIR_TAPS.min)
        .max(ParamLimits::FIR_TAPS.max)
        .step(ParamLimits::FIR_TAPS.step)
        .decimals(0)
        .label(translations.autoeq_form.blocks.fir_taps)
        .size(NumberInputSize::Sm)
        .width(100.0)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_fir_taps_change_rc {
        let h = handler.clone();
        fir_taps_input =
            fir_taps_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
    }

    let fir_phase_options: Vec<SelectOption> = FIR_PHASE_OPTIONS
        .iter()
        .map(|(val, lbl)| SelectOption::new(*val, *lbl))
        .collect();

    let mut fir_phase_select = Select::new((base_id.clone(), "fir-phase"))
        .label(translations.autoeq_form.blocks.phase)
        .options(fir_phase_options)
        .selected(&config.eq_design.fir_phase)
        .is_open(ui_state.fir_phase_open)
        .disabled(disabled)
        .size(SelectSize::Xs)
        .theme(theme.select_theme.clone());

    if let Some(ref handler) = on_fir_phase_toggle_rc {
        let h = handler.clone();
        fir_phase_select = fir_phase_select.on_toggle(move |open, w, cx| h(open, w, cx));
    }

    if let Some(ref handler) = on_fir_phase_change_rc {
        let h = handler.clone();
        fir_phase_select =
            fir_phase_select.on_change(move |value, w, cx| h(value.as_ref(), w, cx));
    }

    block_out = if is_narrow_layout {
        block_out.child(fir_taps_input).child(fir_phase_select)
    } else {
        block_out.child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(fir_taps_input)
                .child(fir_phase_select),
        )
    };
}
