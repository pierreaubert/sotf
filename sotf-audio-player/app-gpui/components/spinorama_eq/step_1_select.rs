use crate::ui::PlayerView;
use d3rs::prelude::{render_line, D3Color, LineConfig, LinePoint, LinearScale, LogScale};
use d3rs::scale::Scale;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, Input, InputSize, Progress, ProgressSize,
    StackAlign, StackSpacing, Text, TextSize, TextWeight, VStack,
};

impl PlayerView {
    // ========================================================================
    // Step 1: Select Speaker
    // ========================================================================

    pub(crate) fn render_spinorama_select_speaker(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let spinorama = &state.app.spinorama_eq_state;

        let search_query = spinorama.speaker_search.clone();
        let selected_speaker = spinorama.selected_speaker.clone();
        let suggestions = spinorama.speaker_suggestions.clone();
        let is_loading = spinorama.loading_speakers;

        // Preview curves data
        let preview_loading = spinorama.loading_preview;
        let preview_error = spinorama.preview_error.clone();
        let preview_frequencies = spinorama.preview_frequencies.clone();
        let preview_input = spinorama.preview_input_curve.clone();
        let preview_target = spinorama.preview_target_curve.clone();
        let preview_deviation = spinorama.preview_deviation_curve.clone();
        let has_preview = !preview_frequencies.is_empty();

		VStack::new()
		    .spacing(StackSpacing::Lg)
		    .child(
			Text::new("Select Speaker")
			    .color(theme.text_primary)
			    .weight(TextWeight::Bold)
			    .size(TextSize::Lg),
		    )
		    .child(
			Text::new("Search for your speaker model from spinorama.org measurements.")
			    .size(TextSize::Sm)
			    .color(theme.text_secondary),
		    )
		    .child(
			Card::new()
			    .background(theme.surface)
			    .header_background(theme.background_secondary)
			    .border(theme.border)
			    .header(
				Text::new("Speaker Search")
				    .color(theme.text_primary)
				    .weight(TextWeight::Semibold),
			    )
			    .content(
				VStack::new()
				    .spacing(StackSpacing::Md)
				    .child(
					Text::new(
					    "Type your speaker brand and model to search the database.",
					)
					    .size(TextSize::Sm)
					    .color(theme.text_secondary),
				    )
				// Input handles focus and keyboard internally
				    .child({
					let state_for_start = self.state.clone();
					let state_for_text = self.state.clone();
					let state_for_end = self.state.clone();
					Input::new("speaker-search")
					    .placeholder("Type to search speakers...")
					    .value(SharedString::from(search_query.clone()))
					    .size(InputSize::Md)
					    .icon_left("🔍")
					    .bg_color(theme.surface)
					    .text_color(theme.text_primary)
					    .placeholder_color(theme.text_muted)
					    .on_edit_start({
						move |_window, cx| {
						    log::info!("[SPINORAMA] on_edit_start: entering SpinoramaSpeakerSearch mode");
						    state_for_start.update(cx, |state, _cx| {
							state.app.input_mode = crate::app::InputMode::SpinoramaSpeakerSearch;
						    });
						}
					    })
					    .on_text_change({
						move |text, _window, cx| {
						    log::info!("[SPINORAMA] on_text_change: {}", text);
						    state_for_text.update(cx, |state, _cx| {
							state.app.spinorama_eq_state.speaker_search = text;
							state.app.spinorama_eq_state.update_suggestions();
						    });
						}
					    })
					    .on_edit_end({
						move |_result, _window, cx| {
						    state_for_end.update(cx, |state, _cx| {
							state.app.input_mode = crate::app::InputMode::Normal;
						    });
						}
					    })
				    })
				    .child(
					HStack::new()
					    .spacing(StackSpacing::Sm)
					    .child(
						Button::new("refresh-speakers", "⟳ Refresh")
						    .variant(ButtonVariant::Secondary)
						    .size(ButtonSize::Sm)
						    .disabled(is_loading)
						    .build()
						    .on_mouse_up(
							MouseButton::Left,
							cx.listener(|view, _, _, cx| {
							    view.fetch_spinorama_speakers(cx);
							}),
						    ),
					    )
					    .when(is_loading, |hstack| {
						hstack.child(
						    Text::new("Loading...")
							.size(TextSize::Sm)
							.color(theme.text_muted),
						)
					    })
					    .when(
						!is_loading && !spinorama.available_speakers.is_empty(),
						|hstack| {
						    hstack.child(
							Text::new(format!(
							    "{} speakers",
							    spinorama.available_speakers.len()
							))
							    .size(TextSize::Sm)
							    .color(theme.text_muted),
						    )
						},
					    )
				    ),
			    ),
		    )
            // Speaker suggestions list
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                Text::new("Available Speakers")
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .child(
                                Text::new(format!("({} matches)", suggestions.len()))
                                    .size(TextSize::Sm)
                                    .color(theme.text_muted),
                            ),
                    )
                    .content(
                        div()
                            .id("speaker-suggestions-scroll")
                            .flex()
                            .flex_col()
                            .gap_1()
                            .max_h(px(300.0))
                            .overflow_y_scroll()
                            .when(suggestions.is_empty() && is_loading, |d| {
                                d.child(
                                    Text::new("Loading speakers from spinorama.org...")
                                        .size(TextSize::Sm)
                                        .color(theme.text_muted),
                                )
                            })
                            .when(suggestions.is_empty() && !is_loading, |d| {
                                d.child(
                                    Text::new(if search_query.is_empty() {
                                        "No speakers loaded. Click Refresh to load."
                                    } else {
                                        "No matching speakers found."
                                    })
                                    .size(TextSize::Sm)
                                    .color(theme.text_muted),
                                )
                            })
                            .children(suggestions.iter().map(|speaker| {
                                let is_selected = selected_speaker.as_ref() == Some(speaker);
                                let speaker_name = speaker.clone();
                                let accent = theme.accent;
                                let surface = theme.surface;
                                let text_primary = theme.text_primary;
                                let text_on_accent = theme.text_on_accent;

                                div()
                                    .id(SharedString::from(format!(
                                        "speaker-option-{}",
                                        speaker_name
                                    )))
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .bg(if is_selected { accent } else { surface })
                                    .text_color(if is_selected {
                                        text_on_accent
                                    } else {
                                        text_primary
                                    })
                                    .text_sm()
                                    .hover(|s| {
                                        s.bg(if is_selected {
                                            accent
                                        } else {
                                            theme.surface_hover
                                        })
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener({
                                            let speaker_name = speaker_name.clone();
                                            move |view, _, _, cx| {
                                                view.select_spinorama_speaker(&speaker_name, cx);
                                            }
                                        }),
                                    )
                                    .child(speaker_name)
                            })),
                    ),
            )
            // Selected speaker display with version and measurement selection
            .when_some(selected_speaker.clone(), |vstack, speaker| {
                let available_versions = spinorama.available_versions.clone();
                let available_measurements = spinorama.available_measurements.clone();
                let selected_version = spinorama.selected_version.clone();
                let selected_measurement = spinorama.selected_measurement.clone();
                let loading_versions = spinorama.loading_versions;
                let loading_measurements = spinorama.loading_measurements;
                let has_phase_data = spinorama.has_phase_data;

                vstack.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .header(
                            Text::new("Selected Speaker")
                                .color(theme.text_primary)
                                .weight(TextWeight::Semibold),
                        )
                        .content(
                            VStack::new()
                                .spacing(StackSpacing::Md)
                                .child(
                                    Text::new(speaker)
                                        .size(TextSize::Lg)
                                        .weight(TextWeight::Bold)
                                        .color(theme.accent),
                                )
                                // Version selection
                                .child(
                                    VStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .child(
                                            Text::new("Origin / Version")
                                                .size(TextSize::Sm)
                                                .weight(TextWeight::Semibold)
                                                .color(theme.text_primary),
                                        )
                                        .when(loading_versions, |vs| {
                                            vs.child(
                                                Text::new("Loading versions...")
                                                    .size(TextSize::Sm)
                                                    .color(theme.text_muted),
                                            )
                                        })
                                        .when(!loading_versions && available_versions.is_empty(), |vs| {
                                            vs.child(
                                                Text::new("No versions available")
                                                    .size(TextSize::Sm)
                                                    .color(theme.text_muted),
                                            )
                                        })
                                        .when(!loading_versions && !available_versions.is_empty(), |vs| {
                                            vs.child(
                                                HStack::new()
                                                    .spacing(StackSpacing::Sm)
                                                    .wrap(true)
                                                    .children(available_versions.iter().map(|version| {
                                                        let is_selected = selected_version == *version;
                                                        let version_clone = version.clone();
                                                        let accent = theme.accent;
                                                        let surface = theme.surface;
                                                        let text_primary = theme.text_primary;
                                                        let text_on_accent = theme.text_on_accent;
                                                        let surface_hover = theme.surface_hover;

                                                        div()
                                                            .id(SharedString::from(format!("version-{}", version)))
                                                            .px_3()
                                                            .py_1()
                                                            .rounded_md()
                                                            .cursor_pointer()
                                                            .bg(if is_selected { accent } else { surface })
                                                            .border_1()
                                                            .border_color(if is_selected { accent } else { theme.border })
                                                            .text_color(if is_selected { text_on_accent } else { text_primary })
                                                            .text_sm()
                                                            .hover(|s| s.bg(if is_selected { accent } else { surface_hover }))
                                                            .on_mouse_down(
                                                                MouseButton::Left,
                                                                cx.listener({
                                                                    let version_clone = version_clone.clone();
                                                                    move |view, _, _, cx| {
                                                                        view.select_spinorama_version(&version_clone, cx);
                                                                    }
                                                                }),
                                                            )
                                                            .child(version_clone)
                                                    })),
                                            )
                                        }),
                                )
                                // Measurement selection
                                .child(
                                    VStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .child(
                                            Text::new("Measurement")
                                                .size(TextSize::Sm)
                                                .weight(TextWeight::Semibold)
                                                .color(theme.text_primary),
                                        )
                                        .when(loading_measurements, |vs| {
                                            vs.child(
                                                Text::new("Loading measurements...")
                                                    .size(TextSize::Sm)
                                                    .color(theme.text_muted),
                                            )
                                        })
                                        .when(!loading_measurements && available_measurements.is_empty(), |vs| {
                                            vs.child(
                                                Text::new("No measurements available")
                                                    .size(TextSize::Sm)
                                                    .color(theme.text_muted),
                                            )
                                        })
                                        .when(!loading_measurements && !available_measurements.is_empty(), |vs| {
                                            vs.child(
                                                HStack::new()
                                                    .spacing(StackSpacing::Sm)
                                                    .wrap(true)
                                                    .children(available_measurements.iter().map(|measurement| {
                                                        let is_selected = selected_measurement == *measurement;
                                                        let measurement_clone = measurement.clone();
                                                        let accent = theme.accent;
                                                        let surface = theme.surface;
                                                        let text_primary = theme.text_primary;
                                                        let text_on_accent = theme.text_on_accent;
                                                        let surface_hover = theme.surface_hover;

                                                        div()
                                                            .id(SharedString::from(format!("measurement-{}", measurement)))
                                                            .px_3()
                                                            .py_1()
                                                            .rounded_md()
                                                            .cursor_pointer()
                                                            .bg(if is_selected { accent } else { surface })
                                                            .border_1()
                                                            .border_color(if is_selected { accent } else { theme.border })
                                                            .text_color(if is_selected { text_on_accent } else { text_primary })
                                                            .text_sm()
                                                            .hover(|s| s.bg(if is_selected { accent } else { surface_hover }))
                                                            .on_mouse_down(
                                                                MouseButton::Left,
                                                                cx.listener({
                                                                    let measurement_clone = measurement_clone.clone();
                                                                    move |view, _, _, cx| {
                                                                        view.select_spinorama_measurement(&measurement_clone, cx);
                                                                    }
                                                                }),
                                                            )
                                                            .child(measurement_clone)
                                                    })),
                                            )
                                        }),
                                )
                                // Phase data indicator
                                .child(
                                    HStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .child(
                                            Text::new("Phase Data:")
                                                .size(TextSize::Xs)
                                                .color(theme.text_muted),
                                        )
                                        .child(
                                            Text::new(if has_phase_data { "Available" } else { "Not Available" })
                                                .size(TextSize::Xs)
                                                .color(if has_phase_data { theme.success } else { theme.text_muted }),
                                        ),
                                )
                                .child(
                                    Text::new("Click 'Next' to configure optimization parameters.")
                                        .size(TextSize::Sm)
                                        .color(theme.text_secondary),
                                ),
                        ),
                )
            })
            // Preview chart showing input, target, and deviation curves (shown when measurement is selected)
            .when(selected_speaker.is_some(), |vstack| {
                let theme = theme.clone();
                vstack.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .header(
                            Text::new("Measurement Preview")
                                .color(theme.text_primary)
                                .weight(TextWeight::Semibold),
                        )
                        .content(
                            if preview_loading {
                                VStack::new()
                                    .spacing(StackSpacing::Md)
                                    .align(StackAlign::Center)
                                    .child(
                                        Text::new("Loading preview curves...")
                                            .size(TextSize::Sm)
                                            .color(theme.text_secondary),
                                    )
                                    .child(Progress::new(0.0).size(ProgressSize::Md))
                                    .into_any_element()
                            } else if let Some(err) = preview_error {
                                VStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(
                                        Text::new("Failed to load preview")
                                            .size(TextSize::Sm)
                                            .color(theme.error),
                                    )
                                    .child(
                                        Text::new(err)
                                            .size(TextSize::Xs)
                                            .color(theme.text_muted),
                                    )
                                    .into_any_element()
                            } else if has_preview {
                                self.render_speaker_preview_chart(
                                    &preview_frequencies,
                                    &preview_input,
                                    &preview_target,
                                    &preview_deviation,
                                    &theme,
                                )
                                .into_any_element()
                            } else {
                                Text::new("Select a measurement to see preview curves")
                                    .size(TextSize::Sm)
                                    .color(theme.text_muted)
                                    .into_any_element()
                            },
                        ),
                )
            })
    }

    /// Render the speaker preview chart with input, target, and deviation curves
    fn render_speaker_preview_chart(
        &self,
        frequencies: &[f64],
        input: &[f64],
        target: &[f64],
        deviation: &[f64],
        theme: &crate::app::theme::Theme,
    ) -> impl IntoElement {
        const GRAPH_WIDTH: f32 = 550.0;
        const GRAPH_HEIGHT: f32 = 150.0;

        // Create log scale for frequency (x-axis)
        let freq_min = frequencies.first().copied().unwrap_or(20.0).max(20.0);
        let freq_max = frequencies.last().copied().unwrap_or(20000.0);
        let freq_scale = LogScale::new()
            .domain(freq_min, freq_max)
            .range(0.0, GRAPH_WIDTH as f64);

        // Find y-axis range from all curves
        let all_values: Vec<f64> = input
            .iter()
            .chain(target.iter())
            .chain(deviation.iter())
            .copied()
            .collect();
        let y_min = all_values.iter().cloned().fold(f64::INFINITY, f64::min) - 2.0;
        let y_max = all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max) + 2.0;
        let db_scale = LinearScale::new()
            .domain(y_max, y_min) // Inverted for SVG coordinates
            .range(0.0, GRAPH_HEIGHT as f64);

        // Generate line points for each curve (use raw values, scale handles transformation)
        let input_points: Vec<LinePoint> = frequencies
            .iter()
            .zip(input.iter())
            .filter(|(f, _)| **f >= freq_min && **f <= freq_max)
            .map(|(f, db)| LinePoint::new(*f, *db))
            .collect();

        let target_points: Vec<LinePoint> = frequencies
            .iter()
            .zip(target.iter())
            .filter(|(f, _)| **f >= freq_min && **f <= freq_max)
            .map(|(f, db)| LinePoint::new(*f, *db))
            .collect();

        let deviation_points: Vec<LinePoint> = frequencies
            .iter()
            .zip(deviation.iter())
            .filter(|(f, _)| **f >= freq_min && **f <= freq_max)
            .map(|(f, db)| LinePoint::new(*f, *db))
            .collect();

        // Configure line styles
        let input_config = LineConfig::new()
            .stroke_width(1.5)
            .stroke_color(D3Color::from_hex(0x3498db)); // Blue

        let target_config = LineConfig::new()
            .stroke_width(1.5)
            .stroke_color(D3Color::from_hex(0x27ae60)); // Green

        let deviation_config = LineConfig::new()
            .stroke_width(1.0)
            .stroke_color(D3Color::from_hex(0xe74c3c)); // Red

        // Render the lines using d3rs
        let input_line = render_line(&freq_scale, &db_scale, &input_points, &input_config);
        let target_line = render_line(&freq_scale, &db_scale, &target_points, &target_config);
        let deviation_line = render_line(&freq_scale, &db_scale, &deviation_points, &deviation_config);

        // Calculate zero line position
        let zero_y = db_scale.scale(0.0) as f32;

        // Build legend
        let legend = HStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                HStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(2.0))
                            .bg(gpui::rgb(0x3498db)),
                    )
                    .child(Text::new("Input").size(TextSize::Xs).color(theme.text_secondary)),
            )
            .child(
                HStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(2.0))
                            .bg(gpui::rgb(0x27ae60)),
                    )
                    .child(Text::new("Target").size(TextSize::Xs).color(theme.text_secondary)),
            )
            .child(
                HStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(2.0))
                            .bg(gpui::rgb(0xe74c3c)),
                    )
                    .child(Text::new("Deviation").size(TextSize::Xs).color(theme.text_secondary)),
            );

        VStack::new()
            .spacing(StackSpacing::Sm)
            .child(legend)
            .child(
                div()
                    .w(px(GRAPH_WIDTH))
                    .h(px(GRAPH_HEIGHT))
                    .bg(theme.background)
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .relative()
                    .overflow_hidden()
                    // Zero line
                    .when(y_min <= 0.0 && y_max >= 0.0, |el| {
                        el.child(
                            div()
                                .absolute()
                                .top(px(zero_y))
                                .left_0()
                                .right_0()
                                .h(px(1.0))
                                .bg(theme.text_muted)
                                .opacity(0.3),
                        )
                    })
                    .child(input_line)
                    .child(target_line)
                    .child(deviation_line),
            )
            .child(
                Text::new("Frequency Response (dB)")
                    .size(TextSize::Xs)
                    .color(theme.text_muted),
            )
    }

}
