impl Showcase {
    fn render_form_controls_section(
        &self,
        toggle_on: bool,
        toggle_lg: bool,
        checkbox_checked: bool,
        slider_value: f32,
        vertical_slider_value: f64,
        number_value: f64,
        number_freq: f64,
        number_db: f64,
        editing_number: Option<&'static str>,
        edit_text: String,
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
                                    .checked(toggle_on)
                                    .on_change({
                                        let entity = entity.clone();
                                        move |checked, _window, cx| {
                                            entity.update(cx, |showcase, cx| {
                                                showcase.toggle_on = checked;
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Toggle::new("toggle-md")
                                    .size(ToggleSize::Md)
                                    .checked(toggle_on)
                                    .on_change({
                                        let entity = entity.clone();
                                        move |checked, _window, cx| {
                                            entity.update(cx, |showcase, cx| {
                                                showcase.toggle_on = checked;
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Toggle::new("toggle-lg")
                                    .size(ToggleSize::Lg)
                                    .checked(toggle_lg)
                                    .on_change({
                                        let entity = entity.clone();
                                        move |checked, _window, cx| {
                                            entity.update(cx, |showcase, cx| {
                                                showcase.toggle_lg = checked;
                                            });
                                        }
                                    }),
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
                                    entity.update(cx, |showcase, cx| {
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
                .child(Text::new("Number Input (click value to edit)").weight(TextWeight::Medium))
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Lg)
                        .align(StackAlign::End)
                        .child({
                            let is_editing = editing_number == Some("basic");
                            let mut input = NumberInput::new("num-basic")
                                .value(number_value)
                                .min(0.0)
                                .max(100.0)
                                .step(1.0)
                                .decimals(0)
                                .label("Count")
                                .size(NumberInputSize::Md)
                                .width(120.0)
                                .editing(is_editing)
                                .on_change({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, cx| {
                                            showcase.number_value = value;
                                        });
                                    }
                                })
                                .on_edit_start({
                                    let entity = entity.clone();
                                    move |_window, cx| {
                                        entity.update(cx, |showcase, cx| {
                                            showcase.editing_number = Some("basic");
                                            showcase.edit_text = format!("{:.0}", showcase.number_value);
                                        });
                                    }
                                })
                                .on_edit_end({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, cx| {
                                            if let Some(v) = value {
                                                showcase.number_value = v;
                                            }
                                            showcase.editing_number = None;
                                            showcase.edit_text.clear();
                                        });
                                    }
                                })
                                .on_text_change({
                                    let entity = entity.clone();
                                    move |text, _window, cx| {
                                        entity.update(cx, |showcase, cx| {
                                            showcase.edit_text = text;
                                        });
                                    }
                                });
                            if is_editing {
                                input = input.edit_text(edit_text.clone());
                            }
                            input
                        })
                        .child({
                            let is_editing = editing_number == Some("freq");
                            let mut input = NumberInput::new("num-freq")
                                .value(number_freq)
                                .min(20.0)
                                .max(20000.0)
                                .step(100.0)
                                .decimals(0)
                                .unit("Hz")
                                .label("Frequency")
                                .size(NumberInputSize::Md)
                                .width(140.0)
                                .editing(is_editing)
                                .on_change({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, cx| {
                                            showcase.number_freq = value;
                                        });
                                    }
                                })
                                .on_edit_start({
                                    let entity = entity.clone();
                                    move |_window, cx| {
                                        entity.update(cx, |showcase, cx| {
                                            showcase.editing_number = Some("freq");
                                            showcase.edit_text = format!("{:.0}", showcase.number_freq);
                                        });
                                    }
                                })
                                .on_edit_end({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, cx| {
                                            if let Some(v) = value {
                                                showcase.number_freq = v;
                                            }
                                            showcase.editing_number = None;
                                            showcase.edit_text.clear();
                                        });
                                    }
                                })
                                .on_text_change({
                                    let entity = entity.clone();
                                    move |text, _window, cx| {
                                        entity.update(cx, |showcase, cx| {
                                            showcase.edit_text = text;
                                        });
                                    }
                                });
                            if is_editing {
                                input = input.edit_text(edit_text.clone());
                            }
                            input
                        })
                        .child({
                            let is_editing = editing_number == Some("db");
                            let mut input = NumberInput::new("num-db")
                                .value(number_db)
                                .min(-12.0)
                                .max(12.0)
                                .step(0.5)
                                .decimals(1)
                                .unit("dB")
                                .label("Gain")
                                .size(NumberInputSize::Sm)
                                .width(100.0)
                                .editing(is_editing)
                                .on_change({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, cx| {
                                            showcase.number_db = value;
                                        });
                                    }
                                })
                                .on_edit_start({
                                    let entity = entity.clone();
                                    move |_window, cx| {
                                        entity.update(cx, |showcase, cx| {
                                            showcase.editing_number = Some("db");
                                            showcase.edit_text = format!("{:.1}", showcase.number_db);
                                        });
                                    }
                                })
                                .on_edit_end({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, cx| {
                                            if let Some(v) = value {
                                                showcase.number_db = v;
                                            }
                                            showcase.editing_number = None;
                                            showcase.edit_text.clear();
                                        });
                                    }
                                })
                                .on_text_change({
                                    let entity = entity.clone();
                                    move |text, _window, cx| {
                                        entity.update(cx, |showcase, cx| {
                                            showcase.edit_text = text;
                                        });
                                    }
                                });
                            if is_editing {
                                input = input.edit_text(edit_text.clone());
                            }
                            input
                        })
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
        // Vertical Slider
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(Text::new("Vertical Slider").weight(TextWeight::Medium))
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Xl)
                        .align(StackAlign::End)
                        .child(
                            VerticalSlider::new("vslider-sm")
                                .value(vertical_slider_value)
                                .min(0.0)
                                .max(1.0)
                                .label("Vol")
                                .unit("%")
                                .size(VerticalSliderSize::Sm)
                                .on_change({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, cx| {
                                            showcase.vertical_slider_value = value;
                                        });
                                    }
                                }),
                        )
                        .child(
                            VerticalSlider::new("vslider-md")
                                .value(vertical_slider_value)
                                .min(-12.0)
                                .max(12.0)
                                .label("Gain")
                                .unit("dB")
                                .size(VerticalSliderSize::Md)
                                .on_change({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, cx| {
                                            showcase.vertical_slider_value = value;
                                        });
                                    }
                                }),
                        )
                        .child(
                            VerticalSlider::new("vslider-lg")
                                .value(vertical_slider_value)
                                .min(0.0)
                                .max(100.0)
                                .label("Level")
                                .size(VerticalSliderSize::Lg),
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
