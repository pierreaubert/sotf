// Section 3: Optimisation Goal — loss function and weighting strategy
// This file is include!()'d from render_body.rs, sharing its scope.
{
    let mut section = VStack::new().spacing(StackSpacing::Sm);

    section = section.child(
        VStack::new()
            .spacing(StackSpacing::None)
            .child(
                Text::new("Optimisation Goal")
                    .size(TextSize::Sm)
                    .weight(TextWeight::Semibold)
                    .color(theme.header_color),
            )
            .child(
                Text::new("How the optimizer evaluates correction quality")
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            ),
    );

    // Derive current goal from config state
    let current_goal = if config.loss_type == "epa" || config.psychoacoustic {
        "psychoacoustic"
    } else if config.asymmetric_loss || config.loss_type == "flat-asymmetric" {
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
                    .w(px(16.0))
                    .h(px(16.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(if is_selected { theme.accent } else { theme.border })
                    .when(is_selected, |el| el.bg(theme.accent)),
            )
            .child(
                Text::new(label)
                    .size(TextSize::Xs)
                    .weight(if is_selected { TextWeight::Semibold } else { TextWeight::Normal })
                    .color(theme.label_color),
            )
            .child(
                Text::new(description)
                    .size(TextSize::Xs)
                    .color(theme.description_color),
            );

        section = section.child(
            div()
                .px(px(8.0))
                .py(px(6.0))
                .rounded(px(6.0))
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
                                if let Some(ref h) = on_loss { h("flat-asymmetric", window, cx); }
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
