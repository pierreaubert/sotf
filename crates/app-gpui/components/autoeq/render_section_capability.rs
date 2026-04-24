// Section 1: Capability — select filter mode (IIR / FIR / Mixed / Mixed Phase)
// This file is include!()'d from render_body.rs, sharing its scope.
{
    let d = crate::components::design::Ds::from_cx(cx);
    let mut section = VStack::new().spacing(StackSpacing::Sm);

    // Header
    section = section.child(
        VStack::new()
            .spacing(StackSpacing::None)
            .child(Text::section_header("Capability").color(theme.header_color))
            .child(
                Text::new("Select the filter engine for your correction")
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            ),
    );

    // Compute FIR latency string
    let fir_latency_label = format!("{:.0} ms", fir_latency_ms);

    // Mode definitions: (id, label, latency, description, recommended)
    let modes: Vec<(&str, &str, String, &str, bool)> = vec![
        ("iir", "IIR", "<1 ms".to_string(), "Parametric IIR only", true),
        ("fir", "FIR", fir_latency_label.clone(), "Classical FIR mode", false),
        ("mixed", "Mixed", fir_latency_label.clone(), "Mix IIR and FIR (lower latency FIR)", false),
        ("mixed_phase", "Mixed Phase", ">10 ms".to_string(), "Mix IIR and FIR on excess phase only", true),
    ];

    for (mode_id, label, latency, description, recommended) in &modes {
        // Skip modes not in allowed list
        if let Some(ref allowed) = allowed_opt_modes
            && !allowed.contains(&mode_id.to_string())
        {
            continue;
        }

        let is_selected = config.opt_mode == *mode_id;
        let on_opt_mode_change = on_opt_mode_change_rc.clone();
        let mode_id_owned = mode_id.to_string();

        let mut label_text = label.to_string();
        if *recommended {
            label_text.push_str(" (recommended)");
        }

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
            .child(
                Text::new(label_text)
                    .size(TextSize::Xs)
                    .weight(if is_selected { TextWeight::Semibold } else { TextWeight::Normal })
                    .color(theme.label_color),
            )
            .child(
                Text::new(latency.clone())
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            )
            .child(
                Text::new(*description)
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
                    if let Some(ref handler) = on_opt_mode_change {
                        handler(&mode_id_owned, window, cx);
                    }
                })
                .child(row),
        );
    }

    Card::new().content(section)
}
