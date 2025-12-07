impl Showcase {
    fn render_potentiometer_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let pot_0 = self.pot_0;
    let pot_25 = self.pot_25;
    let pot_50 = self.pot_50;
    let pot_75 = self.pot_75;
    let pot_100 = self.pot_100;
    let pot_selected = self.pot_selected;
    let pot_lg = self.pot_lg;

    let entity = self.entity.clone();
    let section_title = cx.t(TranslationKey::SectionPotentiometers);
    let selected_label = cx.t(TranslationKey::LabelSelected);
    let large_label = cx.t(TranslationKey::LabelLarge);

    VStack::new()
        .spacing(StackSpacing::Lg)
        .child(self.section_header(section_title))
        .child(
            Text::new(
                "Circular knob controls for audio/visual applications (click or scroll to adjust):",
            )
            .muted(true),
        )
        .child(
            HStack::new()
                .spacing(StackSpacing::Xl)
                .align(StackAlign::End)
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Potentiometer::new("pot-0")
                                .value(pot_0)
                                .min(0.0)
                                .max(1.0)
                                .unit("%")
                                .label("0 to 1")
                                .size(PotentiometerSize::Sm)
                                .on_change({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, _cx| {
                                            showcase.pot_0 = value;
                                        });
                                    }
                                }),
                        )
                        .child(Text::new(format!("{:.0}%", pot_0 * 100.0)).size(TextSize::Xs)),
                )
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Potentiometer::new("pot-25")
                                .value(pot_25)
                                .min(-1.0)
                                .max(1.0)
                                .unit("%")
                                .label("-1 to 1")
                                .size(PotentiometerSize::Sm)
                                .on_change({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, _cx| {
                                            showcase.pot_25 = value;
                                        });
                                    }
                                }),
                        )
                        .child(Text::new(format!("{:.0}%", pot_25 * 100.0)).size(TextSize::Xs)),
                )
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Potentiometer::new("pot-50")
                                .value(pot_50)
                                .min(-3.0)
                                .max(3.0)
                                .unit("")
                                .label("-3 to 3")
                                .size(PotentiometerSize::Md)
                                .on_change({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, _cx| {
                                            showcase.pot_50 = value;
                                        });
                                    }
                                }),
                        )
                        .child(Text::new(format!("{:.0}", pot_50 * 100.0)).size(TextSize::Xs)),
                )
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Potentiometer::new("pot-75")
                                .value(pot_75)
                                .min(-10.0)
                                .max(10.0)
                                .unit("%")
                                .label("-10 to 10")
                                .size(PotentiometerSize::Sm)
                                .on_change({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, _cx| {
                                            showcase.pot_75 = value;
                                        });
                                    }
                                }),
                        )
                        .child(Text::new(format!("{:.0}%", pot_75 * 100.0)).size(TextSize::Xs)),
                )
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Potentiometer::new("pot-100")
                                .value(pot_100)
                                .min(0.0)
                                .max(1.0)
                                .unit("%")
                                .label("E")
                                .size(PotentiometerSize::Sm)
                                .on_change({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, _cx| {
                                            showcase.pot_100 = value;
                                        });
                                    }
                                }),
                        )
                        .child(Text::new(format!("{:.0}%", pot_100 * 100.0)).size(TextSize::Xs)),
                )
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Potentiometer::new("pot-selected")
                                .value(pot_selected)
                                .min(100.0)
                                .max(1000.0)
                                .unit("Hz")
                                .label("100-1000Hz")
                                .size(PotentiometerSize::Sm)
                                .selected(true)
                                .on_change({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, _cx| {
                                            showcase.pot_selected = value;
                                        });
                                    }
                                }),
                        )
                        .child(Text::new(selected_label).size(TextSize::Xs)),
                )
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Potentiometer::new("pot-lg")
                                .value(pot_lg)
                                .min(0.0)
                                .max(1.0)
                                .unit("%")
                                .label("Vol")
                                .size(PotentiometerSize::Lg)
                                .on_change({
                                    let entity = entity.clone();
                                    move |value, _window, cx| {
                                        entity.update(cx, |showcase, _cx| {
                                            showcase.pot_lg = value;
                                        });
                                    }
                                }),
                        )
                        .child(Text::new(large_label).size(TextSize::Xs)),
                ),
        )
    }
}
