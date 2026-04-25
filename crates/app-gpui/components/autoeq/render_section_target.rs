// Section 2: Target — listening distance preset and tilt slope configuration
// This file is include!()'d from render_body.rs, sharing its scope.
{
    let d = crate::components::design::Ds::from_cx(cx);
    let mut section = VStack::new().spacing(StackSpacing::Sm);

    // Header
    section = section.child(
        VStack::new()
            .spacing(StackSpacing::None)
            .child(Text::section_header("Target").color(theme.header_color))
            .child(
                Text::new("Listening distance and target slope")
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            ),
    );

    // Derive selected distance from current slope (match against preset slopes)
    let active_distance = ui_state
        .selected_target_distance
        .as_deref()
        .or_else(|| {
            // Auto-detect from current slope value
            TARGET_DISTANCE_OPTIONS
                .iter()
                .find(|(id, _, _, slope)| *id != "custom" && (*slope - config.tilt_slope).abs() < 0.01)
                .map(|(id, _, _, _)| *id)
        });

    // Distance preset buttons
    for &(preset_id, label, description, recommended_slope) in TARGET_DISTANCE_OPTIONS {
        let is_selected = active_distance == Some(preset_id);

        let on_tilt_slope = on_tilt_slope_change_rc.clone();
        let on_tilt_enable = on_use_target_tilt_change_rc.clone();
        let on_target_distance = on_target_distance_change_rc.clone();
        let on_edit_custom = on_edit_custom_target_rc.clone();
        let preset_id_owned = preset_id.to_string();

        let row = HStack::new()
            .spacing(StackSpacing::Md)
            .align(StackAlign::Center)
            .child(
                div()
                    .flex_shrink_0()
                    .w(rems(1.0))
                    .h(rems(1.0))
                    .rounded(d.r_lg)
                    .border_1()
                    .border_color(if is_selected { theme.accent } else { theme.border })
                    .when(is_selected, |el| el.bg(theme.accent)),
            )
            .child(Text::selectable(label, is_selected).color(theme.label_color))
            .child(
                Text::new(description)
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            );

        section = section.child(
            div()
                .px(d.gap)
                .py(d.pad_y_half)
                .rounded(d.r_md)
                .border_1()
                .border_color(if is_selected { theme.accent } else { theme.border })
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                    if let Some(ref handler) = on_target_distance {
                        handler(&preset_id_owned, window, cx);
                    }
                    if preset_id == "custom" {
                        if let Some(ref handler) = on_edit_custom {
                            handler(window, cx);
                        }
                    } else {
                        if let Some(ref handler) = on_tilt_enable {
                            handler(true, window, cx);
                        }
                        if let Some(ref handler) = on_tilt_slope {
                            handler(recommended_slope, window, cx);
                        }
                    }
                })
                .child(row),
        );
    }

    // Current slope display
    section = section.child(
        HStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new(format!("Current slope: {:.1} dB/oct", config.tilt_slope))
                    .size(TextSize::Xs)
                    .color(theme.label_color),
            ),
    );

    // Slope input
    let mut slope_input = NumberInput::new((base_id.clone(), "target-slope"))
        .value(config.tilt_slope)
        .min(ParamLimits::TILT_SLOPE.min)
        .max(ParamLimits::TILT_SLOPE.max)
        .step(ParamLimits::TILT_SLOPE.step)
        .decimals(1)
        .label("Slope (dB/oct)")
        .size(NumberInputSize::Sm)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_tilt_slope_change_rc {
        let h = handler.clone();
        slope_input = slope_input.on_change(move |v, w, cx| h(v, w, cx));
    }

    section = section.child(slope_input);

    // Reference frequency and bass shelf
    let mut ref_freq_input = NumberInput::new((base_id.clone(), "target-ref-freq"))
        .value(config.tilt_reference_freq)
        .min(20.0)
        .max(20000.0)
        .step(10.0)
        .decimals(0)
        .label("Reference Freq (Hz)")
        .size(NumberInputSize::Sm)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_tilt_reference_freq_change_rc {
        let h = handler.clone();
        ref_freq_input = ref_freq_input.on_change(move |v, w, cx| h(v, w, cx));
    }

    let mut shelf_db_input = NumberInput::new((base_id.clone(), "target-shelf-db"))
        .value(config.tilt_bass_shelf_db)
        .min(ParamLimits::BASS_SHELF.min)
        .max(ParamLimits::BASS_SHELF.max)
        .step(ParamLimits::BASS_SHELF.step)
        .decimals(1)
        .label("Bass Boost (dB)")
        .size(NumberInputSize::Sm)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_tilt_bass_shelf_db_change_rc {
        let h = handler.clone();
        shelf_db_input = shelf_db_input.on_change(move |v, w, cx| h(v, w, cx));
    }

    let mut shelf_freq_input = NumberInput::new((base_id.clone(), "target-shelf-freq"))
        .value(config.tilt_bass_shelf_freq)
        .min(20.0)
        .max(1000.0)
        .step(10.0)
        .decimals(0)
        .label("Shelf Freq (Hz)")
        .size(NumberInputSize::Sm)
        .disabled(disabled)
        .theme(theme.number_input_theme.clone());

    if let Some(ref handler) = on_tilt_bass_shelf_freq_change_rc {
        let h = handler.clone();
        shelf_freq_input = shelf_freq_input.on_change(move |v, w, cx| h(v, w, cx));
    }

    section = section.child(
        HStack::new()
            .spacing(StackSpacing::Md)
            .child(ref_freq_input)
            .child(shelf_db_input)
            .child(shelf_freq_input),
    );

    Card::new().content(section)
}
