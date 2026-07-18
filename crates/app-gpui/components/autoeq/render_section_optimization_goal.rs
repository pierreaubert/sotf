// Section 3: Optimisation Goal — loss function and weighting strategy
// This file is include!()'d from render_body.rs, sharing its scope.
{
    let d = crate::components::design::Ds::from_cx(cx);
    let mut section = VStack::new().spacing(StackSpacing::Sm);

    section = section.child(
        VStack::new()
            .spacing(StackSpacing::None)
            .child(Text::section_header(translations.autoeq_form.optimization_goal).color(theme.header_color))
            .child(
                Text::new(translations.autoeq_how_optimizer_evaluates)
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            ),
    );

    // Derive current goal from config state
    let current_goal = if config.goals.loss_type == "epa" || config.algorithm.psychoacoustic {
        "psychoacoustic"
    } else if config.algorithm.asymmetric_loss || config.goals.loss_type == "flat-asymmetric" {
        "natural"
    } else {
        "match_target"
    };

    for &(goal_id, label, description) in OPTIMIZATION_GOAL_OPTIONS {
        let is_selected = current_goal == goal_id;
        let on_goal = on_optimization_goal_change_rc.clone();
        let on_loss = on_loss_type_change_rc.clone();
        let on_asym = on_asymmetric_loss_change_rc.clone();
        let on_psycho = on_psychoacoustic_change_rc.clone();
        let goal_id_owned = goal_id.to_string();

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
                    // Try composite callback first
                    if let Some(ref handler) = on_goal {
                        handler(&goal_id_owned, window, cx);
                    } else {
                        // Fallback: directly set the underlying config fields
                        match goal_id {
                            "match_target" => {
                                if let Some(ref h) = on_loss { h("flat", window, cx); }
                                if let Some(ref h) = on_asym { h(false, window, cx); }
                                if let Some(ref h) = on_psycho { h(false, window, cx); }
                            }
                            "natural" => {
                                if let Some(ref h) = on_loss { h("flat", window, cx); }
                                if let Some(ref h) = on_asym { h(true, window, cx); }
                                if let Some(ref h) = on_psycho { h(false, window, cx); }
                            }
                            "psychoacoustic" => {
                                if let Some(ref h) = on_loss { h("epa", window, cx); }
                                if let Some(ref h) = on_asym { h(false, window, cx); }
                                if let Some(ref h) = on_psycho { h(true, window, cx); }
                            }
                            _ => {}
                        }
                    }
                })
                .child(row),
        );
    }

    Card::new().content(section)
}
