use crate::components::design::Ds;
use crate::components::graphs::speaker_graphs::{
    render_spinorama_cea2034_graph, render_spinorama_horizontal_graph, render_spinorama_pir_graph,
    render_spinorama_vertical_graph,
};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonTheme, ButtonVariant, Card, HStack, Input, InputSize, Spinner,
    SpinnerSize, StackAlign, StackSpacing, Text, TextSize, TextWeight, VStack,
};

impl PlayerView {
    // ========================================================================
    // Step 1: Select Speaker
    // ========================================================================

    pub(crate) fn render_spinorama_select_speaker(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let translations = state.app.ui_state.translations.clone();
        let theme = state.app.ui_state.theme.clone();
        let theme_id = state.app.ui_state.theme_id;
        let button_theme = ButtonTheme::from(&theme.to_ui_kit_theme(theme_id, cx));
        let spinorama = &state.app.measurement_state.spinorama_eq_state;
        let app_width = state.app.ui_state.window_width;

        let search_query = spinorama.speaker_search.clone();
        let selected_speaker = spinorama.selected_speaker.clone();
        let suggestions = spinorama.speaker_suggestions.clone();
        let is_loading = spinorama.loading_speakers;
        // Speaker / version / measurement fetch errors. These are also
        // dispatched as toast notifications from the fetch handlers
        // (see spinorama_eq/mod.rs), but keeping an inline banner means
        // the user can see the failure even after the toast auto-dismisses.
        let fetch_error_message = spinorama.error_message.clone();

        // Spinorama CEA2034 curves data
        let spinorama_curves = spinorama.spinorama_curves.clone();
        let spinorama_curves_loading = spinorama.loading_spinorama_curves;
        let spinorama_curves_error = spinorama.spinorama_curves_error.clone();
        let has_spinorama_curves = spinorama_curves.is_valid();
        let is_cea2034 = spinorama.selected_measurement == "CEA2034"
            || spinorama.selected_measurement == "CEA2034 Normalized";

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new(translations.spinorama_select_speaker)
                    .color(theme.text_primary)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            // Inline error banner — same render shape as step_2's error
            // rendering. Surfaces the spinorama fetch errors that previously
            // populated `error_message` but had no rendering site on this
            // screen (the user just saw the spinner stop with no result).
            .when_some(fetch_error_message, |vstack, msg| {
                vstack.child(Text::new(msg).size(TextSize::Xs).color(theme.error))
            })
            .child(
                Text::new(translations.spinorama_search_desc)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new(translations.spinorama_speaker_search)
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                Text::new(
                                    "Type your speaker brand and model to search the database.",
                                )
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                            )
                            // Input handles focus and keyboard internally
                            .child({
                                let state_for_start = self.state.clone();
                                let state_for_text = self.state.clone();
                                let state_for_end = self.state.clone();
                                Input::new("speaker-search")
                                    .aria_label("Search speakers")
                                    .placeholder("Type to search speakers...")
                                    .value(SharedString::from(search_query.clone()))
                                    .size(InputSize::Sm)
                                    .icon_left("🔍")
                                    .bg_color(theme.surface)
                                    .text_color(theme.text_primary)
                                    .placeholder_color(theme.text_muted)
                                    .on_edit_start({
                                        move |_window, cx| {
                                            log::info!("[SPINORAMA] on_edit_start: entering SpinoramaSpeakerSearch mode");
                                            state_for_start.update(cx, |state, _cx| {
                                                state.app.ui_state.input_mode =
                                                    crate::app::InputMode::SpinoramaSpeakerSearch;
                                            });
                                        }
                                    })
                                    .on_text_change({
                                        move |text, _window, cx| {
                                            log::info!("[SPINORAMA] on_text_change: {}", text);
                                            state_for_text.update(cx, |state, _cx| {
                                                state.app.measurement_state.spinorama_eq_state.speaker_search = text;
                                                state.app.measurement_state.spinorama_eq_state.update_suggestions();
                                            });
                                        }
                                    })
                                    .on_edit_end({
                                        move |_result, _window, cx| {
                                            state_for_end.update(cx, |state, _cx| {
                                                state.app.ui_state.input_mode =
                                                    crate::app::InputMode::Normal;
                                            });
                                        }
                                    })
                            })
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        Button::new("refresh-speakers", "⟳ Refresh")
                                            .variant(ButtonVariant::Secondary)
                                            .size(ButtonSize::Xs)
                                            .disabled(is_loading)
                                            .theme(button_theme.clone())
                                            .on_click_event(cx.listener(|view, _, _, cx| {
                                                    view.fetch_spinorama_speakers(cx);
                                                })),
                                    )
                                    .when(is_loading, |hstack| {
                                        hstack
                                            .child(Spinner::new().size(SpinnerSize::Sm))
                                            .child(Text::caption("Loading..."))
                                    })
                                    .when(
                                        !is_loading && !spinorama.available_speakers.is_empty(),
                                        |hstack| {
                                            hstack.child(Text::caption(format!(
                                                "{} speakers",
                                                spinorama.available_speakers.len()
                                            )))
                                        },
                                    ),
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
                            .spacing(StackSpacing::Xs)
                            .child(
                                Text::new(translations.spinorama_available_speakers)
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .child(Text::caption(format!(
                                "({} matches)",
                                suggestions.len()
                            ))),
                    )
                    .content(
                        div()
                            .id("speaker-suggestions-scroll")
                            .flex()
                            .flex_col()
                            .gap(d.grid)
                            // intentional: fixed scroll container height (not a spacing token)
                            .max_h(px(300.0))
                            .overflow_y_scroll()
                            .when(suggestions.is_empty() && is_loading, |el| {
                                el.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .gap(d.gap)
                                        .py(d.section)
                                        .child(Spinner::new().size(SpinnerSize::Md))
                                        .child(Text::caption(
                                            "Loading speakers from spinorama.org...",
                                        )),
                                )
                            })
                            .when(suggestions.is_empty() && !is_loading, |el| {
                                el.child(Text::caption(if search_query.is_empty() {
                                    "No speakers loaded. Click Refresh to load."
                                } else {
                                    "No matching speakers found."
                                }))
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
                                    .px(d.pad_x)
                                    .py(d.pad_y)
                                    .rounded(d.r_md)
                                    .cursor_pointer()
                                    .bg(if is_selected { accent } else { surface })
                                    .text_color(if is_selected {
                                        text_on_accent
                                    } else {
                                        text_primary
                                    })
                                    .text_size(d.text_sm)
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
            // Selected speaker display with version selection
            .when_some(selected_speaker.clone(), |vstack, speaker| {
                let available_versions = spinorama.available_versions.clone();
                let selected_version = spinorama.selected_version.clone();
                let loading_versions = spinorama.loading_versions;
                let has_phase_data = spinorama.has_phase_data;

                vstack.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .header(
                            Text::new(translations.spinorama_selected_speaker)
                                .color(theme.text_primary)
                                .weight(TextWeight::Semibold),
                        )
                        .content(
                            VStack::new()
                                .spacing(StackSpacing::Sm)
                                .child(
                                    // intentional: selected-speaker accent display, not a heading
                                    Text::new(speaker)
                                        .size(TextSize::Md)
                                        .weight(TextWeight::Bold)
                                        .color(theme.accent),
                                )
                                // Version selection
                                .child(
                                    VStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .child(
                                            Text::label("Origin / Version").color(theme.text_primary),
                                        )
                                        .when(loading_versions, |vs| {
                                            vs.child(Text::caption("Loading versions..."))
                                        })
                                        .when(
                                            !loading_versions && available_versions.is_empty(),
                                            |vs| {
                                                vs.child(Text::caption("No versions available"))
                                            },
                                        )
                                        .when(
                                            !loading_versions && !available_versions.is_empty(),
                                            |vs| {
                                                vs.child(
                                                    HStack::new()
                                                        .spacing(StackSpacing::Xs)
                                                        .wrap(true)
                                                        .children(available_versions.iter().map(
                                                            |version| {
                                                                let is_selected =
                                                                    selected_version == *version;
                                                                let version_clone = version.clone();
                                                                let accent = theme.accent;
                                                                let surface = theme.surface;
                                                                let text_primary =
                                                                    theme.text_primary;
                                                                let text_on_accent =
                                                                    theme.text_on_accent;
                                                                let surface_hover =
                                                                    theme.surface_hover;

                                                                div()
                                                                    .id(SharedString::from(
                                                                        format!(
                                                                            "version-{}",
                                                                            version
                                                                        ),
                                                                    ))
                                                                    .px(d.pad_x)
                                                                    .py(d.pad_y_half)
                                                                    .rounded(d.r_md)
                                                                    .cursor_pointer()
                                                                    .bg(if is_selected {
                                                                        accent
                                                                    } else {
                                                                        surface
                                                                    })
                                                                    .border_1()
                                                                    .border_color(if is_selected {
                                                                        accent
                                                                    } else {
                                                                        theme.border
                                                                    })
                                                                    .text_color(if is_selected {
                                                                        text_on_accent
                                                                    } else {
                                                                        text_primary
                                                                    })
                                                                    .text_size(d.text_sm)
                                                                    .hover(|s| {
                                                                        s.bg(if is_selected {
                                                                            accent
                                                                        } else {
                                                                            surface_hover
                                                                        })
                                                                    })
                                                                    .on_mouse_down(
                                                                        MouseButton::Left,
                                                                        cx.listener({
                                                                            let version_clone =
                                                                                version_clone.clone();
                                                                            move |view, _, _, cx| {
                                                                                view.select_spinorama_version(&version_clone, cx);
                                                                            }
                                                                        }),
                                                                    )
                                                                    .child(version_clone)
                                                            },
                                                        )),
                                                )
                                            },
                                        ),
                                )
                                // Phase data indicator
                                .child(
                                    HStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .child(Text::caption("Phase Data:"))
                                        .child(
                                            Text::new(if has_phase_data {
                                                "Available"
                                            } else {
                                                "Not Available"
                                            })
                                            .size(TextSize::Xs)
                                            .color(if has_phase_data {
                                                theme.success
                                            } else {
                                                theme.text_muted
                                            }),
                                        ),
                                ),
                        ),
                )
            })
            // CEA2034 Spinorama plot (shown only for CEA2034 measurements)
            .when(is_cea2034 && selected_speaker.is_some(), |vstack| {
                let theme = theme.clone();
                vstack.child(
                    // 2x2 Grid of plots when we have CEA2034 data
                    if spinorama_curves_loading {
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .header(
                                Text::new(translations.spinorama_loading)
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(
                                // Indeterminate Spinner — the curves fetch
                                // doesn't expose a fraction, and the previous
                                // `Progress::new(0.0)` bar was always frozen
                                // at 0%.
                                VStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .align(StackAlign::Center)
                                    .child(Spinner::new().size(SpinnerSize::Md))
                                    .child(
                                        Text::new(translations.spinorama_loading_curves)
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
                                    ),
                            )
                            .into_any_element()
                    } else if let Some(err) = spinorama_curves_error {
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .header(
                                Text::new(translations.spinorama_error_loading_data)
                                    .color(theme.error)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        Text::new(translations.spinorama_failed_load)
                                            .size(TextSize::Xs)
                                            .color(theme.error),
                                    )
                                    .child(Text::caption(err)),
                            )
                            .into_any_element()
                    } else if has_spinorama_curves {
                        // 2x2 grid of plots - calculate width based on app width
                        let available_width = (app_width - 32.0 - 16.0).max(600.0);
                        let plot_width = (available_width / 2.0).max(380.0);
			// ratio + space for legend and axis
                        let plot_height = plot_width / 1.5 + 80.0;

                        div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .gap(d.section)
                            // Row 1: CEA2034 Spinorama and PIR
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .w_full()
                                    .gap(d.section)
                                    // Left: CEA2034 Spinorama
                                    .child(
                                        div().flex_1().child(
                                            Card::new()
                                                .background(theme.surface)
                                                .header_background(theme.background_secondary)
                                                .border(theme.border)
                                                .header(
                                                    Text::new(translations.spinorama_title)
                                                        .color(theme.text_primary)
                                                        .weight(TextWeight::Semibold)
                                                        .size(TextSize::Xs),
                                                )
                                                .content(render_spinorama_cea2034_graph(
                                                    &spinorama_curves,
                                                    &theme,
                                                    plot_width,
                                                    plot_height,
                                                )),
                                        ),
                                    )
                                    // Right: PIR (Estimated In-Room Response)
                                    .child(
                                        div().flex_1().child(
                                            Card::new()
                                                .background(theme.surface)
                                                .header_background(theme.background_secondary)
                                                .border(theme.border)
                                                .header(
                                                    Text::new(translations.spinorama_pir)
                                                        .color(theme.text_primary)
                                                        .weight(TextWeight::Semibold)
                                                        .size(TextSize::Xs),
                                                )
                                                .content(render_spinorama_pir_graph(
                                                    &spinorama_curves,
                                                    &theme,
                                                    plot_width,
                                                    plot_height,
                                                )),
                                        ),
                                    ),
                            )
                            // Row 2: Horizontal and Vertical reflections
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .w_full()
                                    .gap(d.section)
                                    // Left: Horizontal reflections
                                    .child(
                                        div().flex_1().child(
                                            Card::new()
                                                .background(theme.surface)
                                                .header_background(theme.background_secondary)
                                                .border(theme.border)
                                                .header(
                                                    Text::new(translations.spinorama_horizontal)
                                                        .color(theme.text_primary)
                                                        .weight(TextWeight::Semibold)
                                                        .size(TextSize::Xs),
                                                )
                                                .content(render_spinorama_horizontal_graph(
                                                    &spinorama_curves,
                                                    &theme,
                                                    plot_width,
                                                    plot_height,
                                                )),
                                        ),
                                    )
                                    // Right: Vertical reflections
                                    .child(
                                        div().flex_1().child(
                                            Card::new()
                                                .background(theme.surface)
                                                .header_background(theme.background_secondary)
                                                .border(theme.border)
                                                .header(
                                                    Text::new(translations.spinorama_vertical)
                                                        .color(theme.text_primary)
                                                        .weight(TextWeight::Semibold)
                                                        .size(TextSize::Xs),
                                                )
                                                .content(render_spinorama_vertical_graph(
                                                    &spinorama_curves,
                                                    &theme,
                                                    plot_width,
                                                    plot_height,
                                                )),
                                        ),
                                    ),
                            )
                            .into_any_element()
                    } else {
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .header(
                                Text::new(translations.spinorama_cea2034)
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(Text::caption(
                                "Spinorama data will appear after selecting a speaker with CEA2034 measurement",
                            ))
                            .into_any_element()
                    },
                )
            })
    }
}
