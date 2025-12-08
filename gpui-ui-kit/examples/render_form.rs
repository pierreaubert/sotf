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
        text_selected: bool,
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
                                            entity.update(cx, |showcase, _cx| {
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
                                            entity.update(cx, |showcase, _cx| {
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
                                            entity.update(cx, |showcase, _cx| {
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
                                .checked(checkbox_checked)
                                .on_change({
                                    let entity = entity.clone();
                                    move |checked, _window, cx| {
                                        entity.update(cx, |showcase, _| {
                                            showcase.checkbox_checked = checked;
                                        });
                                    }
                                }),
                        )
                        .child(
                            Checkbox::new("cb-md")
                                .label(medium)
                                .size(CheckboxSize::Md)
                                .checked(checkbox_checked)
                                .on_change({
                                    let entity = entity.clone();
                                    move |checked, _window, cx| {
                                        entity.update(cx, |showcase, _| {
                                            showcase.checkbox_checked = checked;
                                        });
                                    }
                                }),
                        )
                        .child(
                            Checkbox::new("cb-lg")
                                .label(large)
                                .size(CheckboxSize::Lg)
                                .checked(!checkbox_checked)
                                .on_change({
                                    let entity = entity.clone();
                                    move |checked, _window, cx| {
                                        entity.update(cx, |showcase, _| {
                                            showcase.checkbox_checked = !checked;
                                        });
                                    }
                                }),
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
                                        entity.update(cx, |showcase, _cx| {
                                            showcase.number_value = value;
                                        });
                                    }
                                })
                                .on_edit_start({
                                    let entity = entity.clone();
                                    move |_window, cx| {
                                        entity.update(cx, |showcase, _| {
                                            showcase.editing_number = Some("basic");
                                            showcase.edit_text = format!("{:.0}", showcase.number_value);
                                            showcase.text_selected = true;
                                        });
                                    }
                                })
                                .on_edit_end({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, _cx| {
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
                                        entity.update(cx, |showcase, _cx| {
                                            showcase.edit_text = text;
                                        });
                                    }
                                });
                            if is_editing {
                                input = input.edit_text(edit_text.clone()).text_selected(text_selected);
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
                                        entity.update(cx, |showcase, _cx| {
                                            showcase.number_freq = value;
                                        });
                                    }
                                })
                                .on_edit_start({
                                    let entity = entity.clone();
                                    move |_window, cx| {
                                        entity.update(cx, |showcase, _| {
                                            showcase.editing_number = Some("freq");
                                            showcase.edit_text = format!("{:.0}", showcase.number_freq);
                                            showcase.text_selected = true;
                                        });
                                    }
                                })
                                .on_edit_end({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, _cx| {
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
                                        entity.update(cx, |showcase, _cx| {
                                            showcase.edit_text = text;
                                        });
                                    }
                                });
                            if is_editing {
                                input = input.edit_text(edit_text.clone()).text_selected(text_selected);
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
                                        entity.update(cx, |showcase, _cx| {
                                            showcase.number_db = value;
                                        });
                                    }
                                })
                                .on_edit_start({
                                    let entity = entity.clone();
                                    move |_window, cx| {
                                        entity.update(cx, |showcase, _| {
                                            showcase.editing_number = Some("db");
                                            showcase.edit_text = format!("{:.1}", showcase.number_db);
                                            showcase.text_selected = true;
                                        });
                                    }
                                })
                                .on_edit_end({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, _cx| {
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
                                        entity.update(cx, |showcase, _cx| {
                                            showcase.edit_text = text;
                                        });
                                    }
                                });
                            if is_editing {
                                input = input.edit_text(edit_text.clone()).text_selected(text_selected);
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
                                        entity.update(cx, |showcase, _cx| {
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
                                        entity.update(cx, |showcase, _cx| {
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
                                .size(VerticalSliderSize::Lg)
                                .on_change({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, _| {
                                            showcase.vertical_slider_value = value;
                                        });
                                    }
                                }),
                        ),
                ),
        )
        // Text Input (display only)
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new("Text Input (display only)")
                        .weight(TextWeight::Medium)
                )
                .child(
                    Text::new("Note: Full keyboard editing requires GPUI TextElement integration")
                        .size(TextSize::Xs)
                        .muted(true)
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .child(
                            Input::new("input-default")
                                .value("Display value")
                                .variant(InputVariant::Default),
                        )
                        .child(
                            Input::new("input-filled")
                                .placeholder("Filled variant...")
                                .variant(InputVariant::Filled),
                        )
                        .child(
                            Input::new("input-disabled")
                                .value("Disabled")
                                .disabled(true),
                        )
                )
        )
        // Select with keyboard navigation
        .child({
            let select_value = self.select_value.clone();
            let select_open = self.select_open;
            let select_highlighted = self.select_highlighted;
            let entity_sel = entity.clone();

            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new("Select Dropdown (✨ NEW: Arrow keys, Enter, Escape, Space!)")
                        .weight(TextWeight::Medium)
                )
                .child(
                    Text::new("Try: Space to open, ↑↓ to navigate, Enter to select, Esc to close")
                        .size(TextSize::Xs)
                        .muted(true)
                )
                .child(
                    div().w(px(200.0)).child(
                        Select::new("select-demo")
                            .options(vec![
                                SelectOption::new("apple", "🍎 Apple"),
                                SelectOption::new("banana", "🍌 Banana"),
                                SelectOption::new("cherry", "🍒 Cherry"),
                                SelectOption::new("grape", "🍇 Grape"),
                                SelectOption::new("orange", "🍊 Orange"),
                            ])
                            .selected(select_value.unwrap_or("apple".into()))
                            .placeholder("Choose a fruit...")
                            .label("Fruit Selection")
                            .is_open(select_open)
                            .highlighted_index(select_highlighted)
                            .on_change({
                                let entity = entity_sel.clone();
                                move |value, _window, cx| {
                                    entity.update(cx, |showcase, _cx| {
                                        showcase.select_value = Some(value.clone());
                                        showcase.select_open = false;
                                        showcase.select_highlighted = None;
                                    });
                                }
                            })
                            .on_toggle({
                                let entity = entity_sel.clone();
                                move |open, _window, cx| {
                                    entity.update(cx, |showcase, _cx| {
                                        showcase.select_open = open;
                                    });
                                }
                            })
                            .on_highlight({
                                let entity = entity_sel.clone();
                                move |idx, _window, cx| {
                                    entity.update(cx, |showcase, _cx| {
                                        showcase.select_highlighted = idx;
                                    });
                                }
                            })
                    )
                )
        })
        // Keyboard support summary
        .child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new("⌨️ Keyboard Support Summary")
                        .weight(TextWeight::Bold)
                        .size(TextSize::Lg)
                )
                .child(Divider::new().build())
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Xl)
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(Text::new("Checkbox").weight(TextWeight::Medium))
                                .child(Text::new("• Space/Enter: Toggle").size(TextSize::Xs).muted(true))
                        )
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(Text::new("Number Input").weight(TextWeight::Medium))
                                .child(Text::new("• Click value to edit").size(TextSize::Xs).muted(true))
                                .child(Text::new("• ↑↓: Inc/Dec, Enter/Esc").size(TextSize::Xs).muted(true))
                        )
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(Text::new("Select").weight(TextWeight::Medium))
                                .child(Text::new("• Space: Toggle open/close").size(TextSize::Xs).muted(true))
                                .child(Text::new("• ↑↓: Navigate options").size(TextSize::Xs).muted(true))
                                .child(Text::new("• Enter: Select, Esc: Close").size(TextSize::Xs).muted(true))
                        )
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Xl)
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(Text::new("Volume Knob").weight(TextWeight::Medium))
                                .child(Text::new("• ↑↓/←→: Adjust volume").size(TextSize::Xs).muted(true))
                                .child(Text::new("• M: Toggle mute").size(TextSize::Xs).muted(true))
                        )
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(Text::new("Sliders").weight(TextWeight::Medium))
                                .child(Text::new("• ↑↓/←→: Adjust value").size(TextSize::Xs).muted(true))
                                .child(Text::new("• Home/End: Min/Max").size(TextSize::Xs).muted(true))
                        )
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(Text::new("Potentiometer").weight(TextWeight::Medium))
                                .child(Text::new("• ↑↓/←→: Adjust value").size(TextSize::Xs).muted(true))
                                .child(Text::new("• Esc: Reset to default").size(TextSize::Xs).muted(true))
                        )
                )
        )
    }
}
