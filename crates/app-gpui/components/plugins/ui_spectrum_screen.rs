// intentional-file: measured chart geometry and cursor positions use pixels.
impl PlayerView {
    pub(crate) fn render_spectrum_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = SpectrumTranslations::for_language(state.app.ui_state.language);
        let desktop =
            crate::app::i18n::DesktopTranslations::for_language(state.app.ui_state.language);
        let labels = crate::app::i18n::SpectrumInspectionTranslations::for_language(
            state.app.ui_state.language,
        );
        let spectrum = &state.app.ui_state.spectrum_view;
        let held = spectrum.held_frame.as_ref().filter(|_| spectrum.hold);
        let sample_rate = held.map_or(state.app.playback.spectrum_output_sample_rate(), |frame| {
            frame.sample_rate
        });
        let info = if spectrum.hold {
            held.map(|frame| &frame.data)
        } else {
            state.app.playback.spectrum_info.as_ref()
        }
        .filter(|frame| SpectrumViewState::frequency_range(frame).is_some());
        let (min_frequency, max_frequency) = info
            .and_then(|frame| SpectrumViewState::frequency_range(frame))
            .unwrap_or((20.0, 20_000.0));
        let magnitudes = info.map(|frame| SpectrumViewState::magnitudes(frame, spectrum.smoothed));
        let inspected = info.and_then(|frame| {
            let index = spectrum.inspected_band(frame.magnitudes.len())?;
            let level = *magnitudes.as_ref()?.get(index)?;
            level
                .is_finite()
                .then_some((frame.frequencies[index], level))
        });
        let scale = crate::ui::compute_combined_scale(
            state.app.ui_state.window_width,
            state.app.ui_state.window_height,
            state.app.ui_state.font_scale,
            state.app.ui_state.min_font_size_px,
            state.app.ui_state.max_font_size_px,
        );
        let chart_height = self
            .spectrum_plot_size
            .map(|(_, height)| height - 32.0 * scale)
            .unwrap_or(240.0 * scale)
            .max(1.0);
        let content = if let Some(frame) = info {
            let count = frame.magnitudes.len();
            let bounds = std::rc::Rc::new(std::cell::Cell::new(None::<Bounds<Pixels>>));
            let measured = bounds.clone();
            let plot = div()
                .id("spectrum-inspection-plot")
                .relative()
                .flex_1()
                .min_w_0()
                .child(
                    SpectrumElement::new(
                        magnitudes
                            .clone()
                            .unwrap_or_else(|| frame.magnitudes.clone()),
                    )
                    .height(px(chart_height))
                    .frequency_range(min_frequency, max_frequency)
                    .colors(spectrum_colors_from_theme(
                        &theme.plugin_palette.spectrum_colors,
                    )),
                )
                .child(
                    canvas(move |rect, _, _| measured.set(Some(rect)), |_, _, _, _| {})
                        .absolute()
                        .size_full(),
                )
                .children(spectrum.cursor_fraction.map(|fraction| {
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(relative(fraction.clamp(0.0, 1.0)))
                        .border_l_1()
                        .border_color(theme.text_muted)
                }))
                .on_mouse_move(cx.listener(move |view, event: &MouseMoveEvent, _, cx| {
                    let Some(rect) = bounds.get() else {
                        return;
                    };
                    let width = f32::from(rect.size.width);
                    if width <= 0.0 || count == 0 {
                        return;
                    }
                    let fraction =
                        (f32::from(event.position.x - rect.origin.x) / width).clamp(0.0, 1.0);
                    let index = ((fraction * count as f32) as usize).min(count - 1);
                    let fraction = (index as f32 + 0.5) / count as f32;
                    if view
                        .state
                        .read(cx)
                        .app
                        .ui_state
                        .spectrum_view
                        .cursor_fraction
                        != Some(fraction)
                    {
                        view.state.update(cx, |state, _| {
                            state.app.ui_state.spectrum_view.cursor_fraction = Some(fraction)
                        });
                        cx.notify();
                    }
                }));
            div()
                .flex()
                .flex_col()
                .size_full()
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .gap(d.grid)
                        .child(render_spectrum_db_axis(spectrum_axis_theme(&d, &theme)))
                        .child(dev_track!(plot, "spectrum.plot")),
                )
                .child(
                    div()
                        .flex()
                        .child(spectrum_db_axis_spacer(&d, &theme))
                        .child(render_spectrum_frequency_axis(
                            min_frequency,
                            max_frequency,
                            spectrum_axis_theme(&d, &theme),
                        )),
                )
        } else {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .child(dev_track!(
                    render_empty_state(IconName::AudioWaveform, text.data_unavailable, &theme),
                    "spectrum.unavailable"
                ))
        };
        let hold_state = self.state.clone();
        let hold_view = cx.entity().downgrade();
        let smooth_state = self.state.clone();
        let smooth_view = cx.entity().downgrade();
        let measured_view = cx.entity().downgrade();
        let chart = div()
            .flex_1()
            .min_h(px(180.0 * scale))
            .flex_shrink_0()
            .min_w_0()
            .relative()
            .overflow_hidden()
            .child(
                canvas(
                    move |bounds, _, cx| {
                        let size = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                        let Some(view) = measured_view.upgrade() else {
                            return;
                        };
                        if size.0 <= 0.0
                            || size.1 <= 0.0
                            || view.read(cx).spectrum_plot_size.is_some_and(|old| {
                                (old.0 - size.0).abs() < 0.5 && (old.1 - size.1).abs() < 0.5
                            })
                        {
                            return;
                        }
                        cx.defer(move |cx| {
                            view.update(cx, |view, cx| {
                                view.spectrum_plot_size = Some(size);
                                cx.notify();
                            });
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(content);
        let mut root = div()
            .id("spectrum-screen-scroll")
            .track_scroll(&self.scroll.spectrum)
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .min_w_0()
            .p(d.card)
            .gap(d.gap)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .flex_shrink_0()
                    .items_center()
                    .justify_between()
                    .gap(d.gap)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(Text::section_header(text.analyzer).color(theme.text_primary))
                            .child(
                                Text::caption(if spectrum.hold {
                                    desktop.spectrum_held
                                } else if info.is_none() {
                                    desktop.spectrum_unavailable
                                } else if info.is_some_and(|frame| frame.peak_magnitude <= -100.0) {
                                    text.no_signal
                                } else {
                                    desktop.spectrum_live
                                })
                                .color(theme.text_muted),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(d.grid)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(d.gap)
                                    .child(Text::label(text.hold).color(theme.text_primary))
                                    .child(dev_track!(
                                        Toggle::new("spectrum-screen-hold")
                                            .checked(spectrum.hold)
                                            .disabled(!spectrum.hold && info.is_none())
                                            .aria_label(text.hold)
                                            .style(ToggleStyle::Segmented)
                                            .theme(theme.to_toggle_theme())
                                            .on_change(move |hold, _, cx| {
                                                hold_state.update(cx, |state, _| {
                                                    let rate = state
                                                        .app
                                                        .playback
                                                        .spectrum_output_sample_rate();
                                                    state.app.ui_state.spectrum_view.set_hold(
                                                        hold,
                                                        state.app.playback.spectrum_info.as_ref(),
                                                        rate,
                                                    );
                                                });
                                                let _ = hold_view.update(cx, |_, cx| cx.notify());
                                            }),
                                        "spectrum.hold"
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(d.gap)
                                    .child(Text::label(text.smoothing).color(theme.text_primary))
                                    .child(dev_track!(
                                        Toggle::new("spectrum-screen-smoothing")
                                            .checked(spectrum.smoothed)
                                            .aria_label(text.smoothing)
                                            .style(ToggleStyle::Segmented)
                                            .theme(theme.to_toggle_theme())
                                            .on_change(move |smoothed, _, cx| {
                                                smooth_state.update(cx, |state, _| {
                                                    state.app.ui_state.spectrum_view.smoothed =
                                                        smoothed
                                                });
                                                let _ = smooth_view.update(cx, |_, cx| cx.notify());
                                            }),
                                        "spectrum.smoothing"
                                    )),
                            ),
                    ),
            )
            .child(
                Text::caption(match sample_rate {
                    Some(rate) => format!(
                        "{}: {} Hz · {}: {} Hz",
                        labels.output_rate,
                        rate,
                        labels.nyquist,
                        f64::from(rate) * 0.5
                    ),
                    None => labels.rate_unknown.to_string(),
                })
                .color(theme.text_muted),
            )
            .child(dev_track!(
                div().child(
                    Text::body(match inspected {
                        Some((frequency, level)) => format!("{frequency:.0} Hz · {level:.1} dBFS"),
                        None => labels.inspect_hint.to_string(),
                    })
                    .color(theme.text_primary)
                ),
                "spectrum.readout"
            ))
            .child(chart)
            .child(dev_track!(
                Button::new("spectrum-analysis-details", labels.title)
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .on_click_event(cx.listener(|view, _, _, cx| {
                        view.state.update(cx, |state, _| {
                            state.app.ui_state.spectrum_view.details_open =
                                !state.app.ui_state.spectrum_view.details_open
                        });
                        cx.notify();
                    })),
                "spectrum.details"
            ));
        if spectrum.details_open {
            let mut body = div()
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap(d.gap)
                .child(Text::caption(labels.units).color(theme.text_muted))
                .child(Text::caption(labels.smoothing).color(theme.text_muted));
            if let Some(frame) = info {
                body = body.child(
                    Text::caption(format!(
                        "{}: {:.0}–{:.0} Hz · {}: {}",
                        labels.range,
                        min_frequency,
                        max_frequency,
                        labels.bands,
                        frame.magnitudes.len()
                    ))
                    .color(theme.text_muted),
                );
                let frame = frame.clone();
                let inspect_state = self.state.clone();
                let inspect_view = cx.entity().downgrade();
                body = body.child(Text::label(labels.frequency).color(theme.text_primary));
                body = body.child(dev_track!(
                    NumberInput::new("spectrum-inspect-frequency")
                        .aria_label(labels.frequency)
                        .value(f64::from(
                            inspected.map_or(frame.frequencies[0], |point| point.0)
                        ))
                        .min(f64::from(frame.frequencies[0]))
                        .max(f64::from(frame.frequencies[frame.frequencies.len() - 1]))
                        .step(1.0)
                        .decimals(0)
                        .size(NumberInputSize::Sm)
                        .on_change(move |value, _, cx| {
                            inspect_state.update(cx, |state, _| {
                                state
                                    .app
                                    .ui_state
                                    .spectrum_view
                                    .inspect_frequency(&frame, value)
                            });
                            let _ = inspect_view.update(cx, |_, cx| cx.notify());
                        }),
                    "spectrum.inspect-frequency"
                ));
            }
            root = root.child(dev_track!(body, "spectrum.analysis"));
        }
        dev_track!(root, "spectrum.content")
    }
}
