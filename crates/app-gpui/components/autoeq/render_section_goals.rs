// Section 0: Goals — loss type and target curve selection
// This file is include!()'d from render_body.rs, sharing its scope.
{
    let mut section = VStack::new().spacing(StackSpacing::Sm);

    section = section.child(
        VStack::new()
            .spacing(StackSpacing::None)
            .child(
                Text::new("Goals & Configuration")
                    .size(TextSize::Sm)
                    .weight(TextWeight::Semibold)
                    .color(theme.header_color),
            )
            .child(
                Text::new("Optimization goals and target curve")
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            ),
    );

    // System Type dropdown — only for Speaker optimization
    if optimization_type == OptimizationType::Speaker {
        let system_type_options: Vec<SelectOption> = SYSTEM_TYPE_OPTIONS
            .iter()
            .map(|(val, lbl)| SelectOption::new(*val, *lbl))
            .collect();

        let mut system_type_select = Select::new((base_id.clone(), "goals-system-type"))
            .label("System Type")
            .options(system_type_options)
            .selected(&config.system_type)
            .is_open(ui_state.system_type_open)
            .disabled(disabled)
            .size(SelectSize::Xs)
            .theme(theme.select_theme.clone());

        if let Some(ref h) = on_system_type_toggle_rc {
            let h = h.clone();
            system_type_select = system_type_select.on_toggle(move |open, w, cx| h(open, w, cx));
        }
        if let Some(ref h) = on_system_type_change_rc {
            let h = h.clone();
            system_type_select =
                system_type_select.on_change(move |value, w, cx| h(value.as_ref(), w, cx));
        }

        section = section.child(system_type_select);
    }

    // Loss Type dropdown — use override if provided, otherwise default per optimization type
    let loss_options_source: &[(&str, &str)] = if let Some(overrides) = loss_type_options_override {
        overrides
    } else {
        match optimization_type {
            OptimizationType::Headphone => HEADPHONE_LOSS_TYPE_OPTIONS,
            OptimizationType::Speaker => LOSS_TYPE_OPTIONS,
        }
    };
    let loss_type_options: Vec<SelectOption> = loss_options_source
        .iter()
        .map(|(val, lbl)| SelectOption::new(*val, *lbl))
        .collect();

    let mut loss_type_select = Select::new((base_id.clone(), "goals-loss-type"))
        .label("Optimization Mode")
        .options(loss_type_options)
        .selected(&config.loss_type)
        .is_open(ui_state.loss_type_open)
        .disabled(disabled)
        .size(SelectSize::Xs)
        .theme(theme.select_theme.clone());

    if let Some(ref h) = on_loss_type_toggle_rc {
        let h = h.clone();
        loss_type_select = loss_type_select.on_toggle(move |open, w, cx| h(open, w, cx));
    }
    if let Some(ref h) = on_loss_type_change_rc {
        let h = h.clone();
        loss_type_select =
            loss_type_select.on_change(move |value, w, cx| h(value.as_ref(), w, cx));
    }

    section = section.child(loss_type_select);

    // Target Curve dropdown
    let target_curve_options: Vec<SelectOption> = match optimization_type {
        OptimizationType::Headphone => {
            HEADPHONE_TARGET_CURVE_OPTIONS
                .iter()
                .map(|(val, lbl)| SelectOption::new(*val, *lbl))
                .collect()
        }
        OptimizationType::Speaker => {
            let mut options: Vec<SelectOption> = SPEAKER_TARGET_CURVE_OPTIONS
                .iter()
                .map(|(val, lbl)| SelectOption::new(*val, *lbl))
                .collect();
            for (val, lbl) in SPINORAMA_CURVE_OPTIONS {
                if available_spinorama_curves.iter().any(|c| c == *val) {
                    options.push(SelectOption::new(*val, *lbl));
                }
            }
            options
        }
    };

    let mut target_curve_select = Select::new((base_id.clone(), "goals-target-curve"))
        .label("Target Curve")
        .options(target_curve_options)
        .selected(&config.target_curve)
        .is_open(ui_state.target_curve_open)
        .disabled(disabled)
        .size(SelectSize::Xs)
        .theme(theme.select_theme.clone());

    if let Some(ref h) = on_target_curve_change_rc {
        let h = h.clone();
        target_curve_select =
            target_curve_select.on_change(move |value, w, cx| h(value.as_ref(), w, cx));
    }
    if let Some(ref h) = on_target_curve_toggle_rc {
        let h = h.clone();
        target_curve_select =
            target_curve_select.on_toggle(move |open, w, cx| h(open, w, cx));
    }

    section = section.child(target_curve_select);

    // Show "Edit" button when custom target curve is selected
    if config.target_curve == "custom"
        && let Some(ref handler) = on_edit_custom_target_rc
    {
        let h = handler.clone();
        section = section.child(
            Button::new("edit-custom-target", "Edit Target Curve")
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Xs)
                .on_click(move |w, cx| {
                    h(w, cx);
                }),
        );
    }

    Card::new().content(section)
}
