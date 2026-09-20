// Section 1: Capability — select the parent workflow's supported filter mode.
// Included from render_body.rs and render_body_room_eq.rs.
{
    use gpui_ui_kit::{RadioGroup, RadioGroupSize, RadioOption};

    let mut section = VStack::new().spacing(StackSpacing::Sm).child(
        VStack::new()
            .spacing(StackSpacing::None)
            .child(Text::section_header(translations.autoeq_form.capability).color(theme.header_color))
            .child(Text::caption(translations.autoeq_select_filter_engine).color(theme.description_color)),
    );
    let fir_duration_label = format!(
        "{}: {:.0} ms",
        translations.autoeq_form.blocks.fir_taps, fir_duration_ms
    );
    let modes = ["iir", "fir", "mixed", "mixed_phase"];
    let options = modes.into_iter().enumerate().filter_map(|(index, mode)| {
        let label = stage_text.mode_labels[index];
        let description = stage_text.mode_descriptions[index];
        if allowed_opt_modes.as_ref().is_some_and(|allowed| !allowed.iter().any(|value| value == mode)) {
            return None;
        }
        let label = if mode == "iir" {
            format!("{label} · {description}")
        } else {
            format!("{label} · {fir_duration_label} · {description}")
        };
        Some(RadioOption::new(mode, label))
    }).collect();
    let mut choices = RadioGroup::new((base_id.clone(), "capability"))
        .options(options)
        .selected(Some(config.eq_design.opt_mode.clone().into()))
        .size(RadioGroupSize::Sm)
        .disabled(disabled || on_opt_mode_change_rc.is_none())
        .aria_label(translations.autoeq_form.capability);
    if let Some(handler) = on_opt_mode_change_rc.clone() {
        choices = choices.on_change(move |value, window, cx| handler(value.as_ref(), window, cx));
    }
    #[cfg(feature = "dev-api")]
    let choices = {
        use crate::app::dev_api::DevTrackExt;
        choices.dev_track("autoeq.capability")
    };
    section = section.child(choices);
    Card::new().content(section)
}
