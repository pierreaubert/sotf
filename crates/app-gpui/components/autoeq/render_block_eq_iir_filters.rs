// === Shared: IIR Filters + Sample Rate ===
// Appends to block_out. Expects is_iir check done by caller.
{
    let mut num_filters_input = NumberInput::new((base_id.clone(), "num-filters"))
        .value(config.num_filters as f64)
        .min(ParamLimits::NUM_FILTERS.min)
        .max(ParamLimits::NUM_FILTERS.max)
        .step(ParamLimits::NUM_FILTERS.step)
        .decimals(0)
        .label("Filters")
        .size(NumberInputSize::Sm)
        .width(100.0)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_num_filters_change_rc {
        let h = handler.clone();
        num_filters_input =
            num_filters_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
    }

    if !hide_sample_rate {
        let mut sample_rate_input = NumberInput::new((base_id.clone(), "sample-rate"))
            .value(config.sample_rate as f64)
            .min(ParamLimits::SAMPLE_RATE.min)
            .max(ParamLimits::SAMPLE_RATE.max)
            .step(ParamLimits::SAMPLE_RATE.step)
            .decimals(0)
            .label("Sample Rate")
            .size(NumberInputSize::Sm)
            .width(100.0)
            .disabled(disabled)
            .theme(theme.number_input_theme.clone());

        if let Some(ref handler) = on_sample_rate_change_rc {
            let h = handler.clone();
            sample_rate_input =
                sample_rate_input.on_change(move |v, w, cx| h(v.round() as usize, w, cx));
        }

        block_out = if is_narrow_layout {
            block_out.child(num_filters_input).child(sample_rate_input)
        } else {
            block_out.child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(num_filters_input)
                    .child(sample_rate_input),
            )
        };
    } else {
        block_out = block_out.child(num_filters_input);
    }
}
