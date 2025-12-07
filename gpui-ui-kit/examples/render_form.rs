impl Showcase {
    fn render_form_controls_section(
    &self,
    toggle_on: bool,
    checkbox_checked: bool,
    slider_value: f32,
    number_value: f64,
    number_freq: f64,
    number_db: f64,
    entity: Entity<Self>,
    cx: &mut Context<Self>,
) -> impl IntoElement {
    let section_title = cx.t(TranslationKey::SectionFormControls);
    let toggles_label = cx.t(TranslationKey::LabelToggles);
    let checkboxes_label = cx.t(TranslationKey::LabelCheckboxes);
    let slider_label = cx.t(TranslationKey::LabelSlider);
    let input_label = cx.t(TranslationKey::LabelInput);
    let small = cx.t(TranslationKey::LabelSmall);
    let medium = cx.t(TranslationKey::LabelMedium);
    let large = cx.t(TranslationKey::LabelLarge);
    let disabled = cx.t(TranslationKey::LabelDisabled);

    VStack::new()
        .spacing(StackSpacing::Lg)
        .child(self.section_header(section_title))
        // Toggles
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::new(toggles_label).weight(TextWeight::Medium))
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Lg)
                        .child(
                            Toggle::new("toggle-sm")
                                .size(ToggleSize::Sm)
                                .checked(toggle_on),
                        )
                        .child(
                            Toggle::new("toggle-md")
                                .size(ToggleSize::Md)
                                .checked(toggle_on),
                        )
                        .child(
                            Toggle::new("toggle-lg")
                                .size(ToggleSize::Lg)
                                .checked(!toggle_on),
                        )
                        .child(Toggle::new("toggle-disabled").disabled(true).checked(true)),
                ),
        )
        // Checkboxes
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::new(checkboxes_label).weight(TextWeight::Medium))
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Lg)
                        .child(
                            Checkbox::new("cb-sm")
                                .label(small)
                                .size(CheckboxSize::Sm)
                                .checked(checkbox_checked),
                        )
                        .child(
                            Checkbox::new("cb-md")
                                .label(medium)
                                .size(CheckboxSize::Md)
                                .checked(checkbox_checked),
                        )
                        .child(
                            Checkbox::new("cb-lg")
                                .label(large)
                                .size(CheckboxSize::Lg)
                                .checked(!checkbox_checked),
                        )
                        .child(
                            Checkbox::new("cb-disabled")
                                .label(disabled)
                                .disabled(true)
                                .checked(true),
                        ),
                ),
        )
        // Slider
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new(format!("{}: {:.0}%", slider_label, slider_value * 100.0))
                        .weight(TextWeight::Medium),
                )
                .child(
                    div().w(px(300.0)).child(
                        Slider::new("slider-demo")
                            .value(slider_value)
                            .min(0.0)
                            .max(1.0)
                            .size(SliderSize::Medium)
                            .on_change({
                                let entity = entity.clone();
                                move |value, _window, cx| {
                                    entity.update(cx, |showcase, _cx| {
                                        showcase.slider_value = value;
                                    });
                                }
                            }),
                    ),
                ),
        )
        // Number Input
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::new("Number Input").weight(TextWeight::Medium))
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Lg)
                        .align(StackAlign::End)
                        .child(
                            NumberInput::new("num-basic")
                                .value(number_value)
                                .min(0.0)
                                .max(100.0)
                                .step(1.0)
                                .decimals(0)
                                .label("Count")
                                .size(NumberInputSize::Md)
                                .width(120.0)
                                .on_change({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, _cx| {
                                            showcase.number_value = value;
                                        });
                                    }
                                }),
                        )
                        .child(
                            NumberInput::new("num-freq")
                                .value(number_freq)
                                .min(20.0)
                                .max(20000.0)
                                .step(100.0)
                                .decimals(0)
                                .unit("Hz")
                                .label("Frequency")
                                .size(NumberInputSize::Md)
                                .width(140.0)
                                .on_change({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, _cx| {
                                            showcase.number_freq = value;
                                        });
                                    }
                                }),
                        )
                        .child(
                            NumberInput::new("num-db")
                                .value(number_db)
                                .min(-12.0)
                                .max(12.0)
                                .step(0.5)
                                .decimals(1)
                                .unit("dB")
                                .label("Gain")
                                .size(NumberInputSize::Sm)
                                .width(100.0)
                                .on_change({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, _cx| {
                                            showcase.number_db = value;
                                        });
                                    }
                                }),
                        )
                        .child(
                            NumberInput::new("num-disabled")
                                .value(50.0)
                                .disabled(true)
                                .label("Disabled")
                                .size(NumberInputSize::Md)
                                .width(100.0),
                        ),
                ),
        )
        // Input
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::new(input_label).weight(TextWeight::Medium))
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .child(
                            Input::new("input-default")
                                .placeholder("Default input...")
                                .variant(InputVariant::Default),
                        )
                        .child(
                            Input::new("input-filled")
                                .placeholder("Filled input...")
                                .variant(InputVariant::Filled),
                        )
                        .child(
                            Input::new("input-disabled")
                                .placeholder("Disabled...")
                                .disabled(true),
                        ),
                ),
        )
    }
}
