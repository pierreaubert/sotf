use crate::app::types::headphone_eq::HeadphoneMeasurementSource;
use crate::components::design::Ds;
use crate::i18n::HeadphoneEqTranslations;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonTheme, ButtonVariant, Card, HStack, Input, InputSize, StackSpacing,
    Text, TextSize, TextWeight, VStack,
};

impl PlayerView {
    // ========================================================================
    // Step 1: Measurement & Target
    // ========================================================================

    pub(crate) fn render_headphone_eq_measurement_target(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = HeadphoneEqTranslations::for_language(state.app.ui_state.language);
        let workflow_text =
            crate::app::i18n::WorkflowTranslations::for_language(state.app.ui_state.language);
        let discovery_text =
            crate::app::i18n::EqDiscoveryTranslations::for_language(state.app.ui_state.language);
        let theme_id = state.app.ui_state.theme_id;
        let button_theme = ButtonTheme::from(&theme.to_ui_kit_theme(theme_id, cx));
        let headphone_eq = &state.app.measurement_state.headphone_eq_state;

        let measurement_source = headphone_eq.measurement_source;
        let measurement_path = headphone_eq.model.measurement_path.clone();
        let downloaded_curve = headphone_eq.downloaded_curve.clone();
        let app_width = state.app.ui_state.window_width;

        // Pre-extract spinorama state to avoid borrow conflicts
        let search_query = headphone_eq.headphone_search.clone();
        let suggestions = headphone_eq.headphone_suggestions.clone();
        let selected_headphone = headphone_eq.selected_headphone.clone();
        let is_loading = headphone_eq.loading_headphones;
        let is_downloading = headphone_eq.loading_download;
        let available_headphones_count = headphone_eq.available_headphones.len();
        let runtime_text =
            crate::app::i18n::RuntimeMessageTranslations::for_language(state.app.ui_state.language);
        let error_message = headphone_eq
            .error_message
            .as_deref()
            .map(|message| runtime_text.translate(message).into_owned());

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new(translations.select_measurement)
                    .color(theme.text_primary)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(
                Text::new(translations.select_measurement_description)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            // Source toggle buttons
            .child(
                HStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        Button::new("source-file", discovery_text.load_from_file)
                            .variant(if measurement_source == HeadphoneMeasurementSource::File {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            })
                            .size(ButtonSize::Sm)
                            .theme(button_theme.clone())
                            .on_click_event(cx.listener(|view, _, _, cx| {
                                    view.state.update(cx, |state, _| {
                                        state
                                            .app
                                            .measurement_state
                                            .headphone_eq_state
                                            .measurement_source =
                                            HeadphoneMeasurementSource::File;
                                    });
                                })),
                    )
                    .child(
                        Button::new(
                            "source-spinorama",
                            discovery_text.download_from_spinorama,
                        )
                            .variant(
                                if measurement_source == HeadphoneMeasurementSource::Spinorama {
                                    ButtonVariant::Primary
                                } else {
                                    ButtonVariant::Secondary
                                },
                            )
                            .size(ButtonSize::Sm)
                            .theme(button_theme.clone())
                            .on_click_event(cx.listener(|view, _, _, cx| {
                                    view.state.update(cx, |state, _| {
                                        state
                                            .app
                                            .measurement_state
                                            .headphone_eq_state
                                            .measurement_source =
                                            HeadphoneMeasurementSource::Spinorama;
                                    });
                                    // Auto-fetch headphone list if needed
                                    let needs_refresh = view
                                        .state
                                        .read(cx)
                                        .app
                                        .measurement_state
                                        .headphone_eq_state
                                        .needs_headphone_refresh();
                                    if needs_refresh {
                                        view.fetch_headphone_list(cx);
                                    }
                                })),
                    ),
            )
            // File mode: Browse for CSV
            .when(
                measurement_source == HeadphoneMeasurementSource::File,
                |vstack| {
                    vstack.child(
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .header(
                            Text::new(translations.measurement_file)
                                    .color(theme.text_primary)
                                    .weight(TextWeight::Semibold),
                            )
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                            Text::new(translations.measurement_file_description)
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
                                    )
                                    .child(
                                        HStack::new()
                                            .spacing(StackSpacing::Xs)
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .px(d.pad_x)
                                                    .py(d.pad_y)
                                                    .rounded(d.r_md)
                                                    .bg(theme.background_secondary)
                                                    .text_size(d.text_sm)
                                                    .text_color(if measurement_path.is_empty() {
                                                        theme.text_muted
                                                    } else {
                                                        theme.text_primary
                                                    })
                                                    .child(if measurement_path.is_empty() {
                                                        "No file selected".to_string()
                                                    } else {
                                                        measurement_path.clone()
                                                    }),
                                            )
                                            .child(
                                                Button::new(
                                                    "browse-measurement",
                                                    discovery_text.browse,
                                                )
                                                    .variant(ButtonVariant::Secondary)
                                                    .size(ButtonSize::Sm)
                                                    .theme(button_theme.clone())
                                                    .on_click_event(cx.listener(|view, _, _, cx| {
                                                            view.browse_headphone_eq_measurement(cx);
                                                        })),
                                            ),
                                    ),
                            ),
                    )
                },
            )
            // Spinorama mode: Search and download
            .when(
                measurement_source == HeadphoneMeasurementSource::Spinorama,
                |vstack| {
                    vstack
                        // Search card
                        .child(
                            Card::new()
                                .background(theme.surface)
                                .header_background(theme.background_secondary)
                                .border(theme.border)
                                .header(
                            Text::new(translations.headphone_search)
                                        .color(theme.text_primary)
                                        .weight(TextWeight::Semibold),
                                )
                                .content(
                                    VStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .child(
                                            Text::new(discovery_text.headphone_search_description)
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
                                        )
                                        .child({
                                            let state_for_start = self.state.clone();
                                            let state_for_text = self.state.clone();
                                            let state_for_end = self.state.clone();
                                            Input::new("headphone-search")
                                                .aria_label(discovery_text.search_headphones)
                                    .placeholder(translations.search_placeholder)
                                                .value(SharedString::from(search_query.clone()))
                                                .size(InputSize::Sm)
                                                .bg_color(theme.surface)
                                                .text_color(theme.text_primary)
                                                .placeholder_color(theme.text_muted)
                                                .on_edit_start({
                                                    move |_window, cx| {
                                                        state_for_start.update(cx, |state, _cx| {
                                                            state.app.ui_state.input_mode =
                                                                crate::app::InputMode::HeadphoneSearch;
                                                        });
                                                    }
                                                })
                                                .on_text_change({
                                                    move |text, _window, cx| {
                                                        state_for_text.update(cx, |state, _cx| {
                                                            state.app.measurement_state.headphone_eq_state.model.headphone_search = text;
                                                            state.app.measurement_state.headphone_eq_state.model.update_headphone_suggestions();
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
                                                    Button::new(
                                                        "refresh-headphones",
                                                        discovery_text.refresh,
                                                    )
                                                        .variant(ButtonVariant::Secondary)
                                                        .size(ButtonSize::Xs)
                                                        .disabled(is_loading)
                                                        .theme(button_theme.clone())
                                                        .on_click_event(cx.listener(|view, _, _, cx| {
                                                                view.fetch_headphone_list(cx);
                                                            })),
                                                )
                                                .when(is_loading, |hstack| {
                                                    hstack.child(Text::caption(workflow_text.loading))
                                                })
                                                .when(
                                                    !is_loading
                                                        && available_headphones_count > 0,
                                                    |hstack| {
                                                        hstack.child(Text::caption(format!(
                                                            "{} headphones",
                                                            available_headphones_count
                                                        )))
                                                    },
                                                ),
                                        ),
                                ),
                        )
                        // Suggestions list
                        .child(
                            Card::new()
                                .background(theme.surface)
                                .header_background(theme.background_secondary)
                                .border(theme.border)
                                .header(
                                    HStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .child(
                            Text::new(translations.available_headphones)
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
                                        .id("headphone-suggestions-scroll")
                                        .flex()
                                        .flex_col()
                                        .gap(d.grid)
                                        .max_h(px(300.0)) // intentional: fixed scroll container max-height
                                        .overflow_y_scroll()
                                        .when(suggestions.is_empty() && is_loading, |el| {
                                            el.child(Text::caption(
                                                discovery_text.loading_headphones,
                                            ))
                                        })
                                        .when(suggestions.is_empty() && !is_loading, |el| {
                                            el.child(Text::caption(if search_query.is_empty() {
                                                "No headphones loaded. Click Refresh to load."
                                            } else {
                                                "No matching headphones found."
                                            }))
                                        })
                                        .children(suggestions.iter().map(|headphone| {
                                            let is_selected =
                                                selected_headphone.as_ref() == Some(headphone);
                                            let headphone_name = headphone.clone();
                                            let accent = theme.accent;
                                            let surface = theme.surface;
                                            let text_primary = theme.text_primary;
                                            let text_on_accent = theme.text_on_accent;

                                            div()
                                                .id(SharedString::from(format!(
                                                    "headphone-option-{}",
                                                    headphone_name
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
                                                        let name = headphone_name.clone();
                                                        move |view, _, _, cx| {
                                                            view.select_headphone(&name, cx);
                                                        }
                                                    }),
                                                )
                                                .child(headphone_name)
                                        })),
                                ),
                        )
                        // Error message
                        .when_some(error_message, |vstack, msg| {
                            vstack.child(
                                Text::new(msg)
                                    .size(TextSize::Xs)
                                    .color(theme.error),
                            )
                        })
                        // Download status
                        .when(is_downloading, |vstack| {
                            vstack.child(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(Text::caption(
                                        workflow_text.downloading_measurement,
                                    )),
                            )
                        })
                        // Frequency response preview after download. Navigation
                        // remains in the shared wizard header.
                        .when_some(
                            downloaded_curve,
                            |vstack, curve_data| {
                                vstack.child(self.render_headphone_measurement_graph(
                                    &curve_data,
                                    &theme,
                                    app_width,
                                    cx,
                                ))
                            },
                        )
                },
            )
    }

    fn render_headphone_measurement_graph(
        &self,
        curve_data: &[(f64, f64)],
        theme: &crate::theme::Theme,
        app_width: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        use crate::components::graphs::common::{colors, rgba_to_u32, theme_to_chart_theme};
        use gpui_px::{ScaleType, line};

        let translations =
            HeadphoneEqTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let chart_theme = theme_to_chart_theme(theme);
        let color = rgba_to_u32(colors::input(theme));

        let freqs: Vec<f64> = curve_data.iter().map(|(f, _)| *f).collect();
        let spls: Vec<f64> = curve_data.iter().map(|(_, s)| *s).collect();

        let width = (app_width - 80.0).max(600.0);
        let height = 300.0;

        let chart = line(&freqs, &spls)
            .x_scale(ScaleType::Log)
            .x_range(20.0, 20000.0)
            .y_label("SPL (dB)")
            .label(translations.frequency_response)
            .color(color)
            .stroke_width(2.0)
            .theme(chart_theme)
            .size(width, height)
            .build();

        match chart {
            Ok(el) => div().flex().flex_col().items_center().w_full().child(el),
            Err(e) => div().child(
                gpui_ui_kit::Text::new(format!("Graph error: {}", e))
                    .size(gpui_ui_kit::TextSize::Xs)
                    .color(theme.text_muted),
            ),
        }
    }
}
