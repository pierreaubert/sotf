use crate::app::federation::test_federation_connection;
use crate::app::i18n::FederationTranslations;
use crate::components::design::Ds;
use crate::components::settings::settings_section_label;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Divider, HStack, Input, InputSize, StackSpacing, Text,
    TextSize, VStack,
};
use sotf_audio_player::federation_config::ConnectionStatus;

impl PlayerView {
    /// Render federation sources settings content
    pub(crate) fn render_federation_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let text = FederationTranslations::for_language(state.app.ui_state.language);
        let sources = state.app.federation.sources.clone();

        let mut content = div()
            .flex()
            .flex_col()
            .gap(d.section_lg)
            .child(
                div()
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_primary)
                    .child(text.streaming),
            )
            .child(
                div()
                    .text_size(d.text_xs)
                    .text_color(theme.text_secondary)
                    .child(text.description),
            )
            // Add Source buttons
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .child(format!("Sources ({})", sources.len())),
                    )
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Xs)
                            .child(self.add_source_button("subsonic", "Subsonic", &theme, cx))
                            .child(self.add_source_button("mpd", "MPD", &theme, cx))
                            .child(self.add_source_button("dlna", "DLNA", &theme, cx))
                            .child(self.add_source_button("tidal", "Tidal", &theme, cx))
                            .child(self.add_source_button("spotify", "Spotify", &theme, cx))
                            .child(self.add_source_button("icy_radio", "Radio", &theme, cx)),
                    ),
            )
            .child(settings_section_label("Remote Devices", &theme, &d))
            .child(self.render_remote_sotf_section(&theme, &d, cx));

        if sources.is_empty() {
            content = content.child(
                div()
                    .p(d.card)
                    .bg(theme.background_secondary)
                    .rounded(d.r_md)
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        Text::new(text.no_remote_sources)
                            .size(TextSize::Sm)
                            .color(theme.text_muted),
                    ),
            );
        } else {
            for (source_idx, source) in sources.iter().enumerate() {
                content = content.child(self.render_source_card(
                    source_idx,
                    source,
                    &theme,
                    &translations,
                    cx,
                ));
            }
        }

        content
    }

    pub(super) fn add_source_button(
        &self,
        type_name: &'static str,
        label: &'static str,
        theme: &crate::app::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        Button::new(
            SharedString::from(format!("add-{type_name}-btn")),
            format!("+ {label}"),
        )
        .variant(ButtonVariant::Secondary)
        .size(ButtonSize::Xs)
        .theme(theme.to_button_theme())
        .on_click_event(cx.listener(move |view, _: &ClickEvent, _window, cx| {
            view.state.update(cx, |state, _cx| {
                state.app.add_federation_source(type_name);
            });
            cx.notify();
        }))
    }

    pub(super) fn render_source_card(
        &self,
        source_idx: usize,
        source: &sotf_audio_player::federation_config::FederationSourceEntry,
        theme: &crate::app::theme::Theme,
        translations: &crate::app::i18n::Translations,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let text = FederationTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let type_name = source.connection.type_name();
        let is_enabled = source.is_enabled;
        let display_name = source.display_name.clone();
        let field_names = source.connection.field_names();
        let field_values: Vec<String> = (0..field_names.len())
            .map(|i| source.connection.field_value(i))
            .collect();

        let status = self
            .state
            .read(cx)
            .app
            .get_federation_source_status(&source.source_id)
            .cloned();
        let status_label = status
            .as_ref()
            .map(|s| s.label().to_string())
            .unwrap_or_else(|| "untested".to_string());
        let status_color = match &status {
            Some(ConnectionStatus::Connected { .. }) => theme.success,
            Some(ConnectionStatus::Error(_)) => theme.error,
            Some(ConnectionStatus::Testing) => theme.warning,
            Some(ConnectionStatus::Diagnostic(d)) => {
                if d.is_success() {
                    theme.success
                } else {
                    theme.error
                }
            }
            _ => theme.text_muted,
        };
        let diagnostic = match &status {
            Some(ConnectionStatus::Diagnostic(d)) => Some(d.clone()),
            _ => None,
        };

        div()
            .flex()
            .flex_col()
            .gap(d.gap)
            .p(d.card)
            .bg(theme.background_secondary)
            .rounded(d.r_md)
            .border_1()
            .border_color(if is_enabled {
                theme.accent
            } else {
                theme.border
            })
            // Header row: name, type badge, enable/disable toggle, remove button
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.gap_md)
                    // Editable display name
                    .child(
                        Input::new(SharedString::from(format!("source-name-{source_idx}")))
                            .value(SharedString::from(display_name))
                            .size(InputSize::Sm)
                            .on_change({
                                let state_entity = self.state.clone();
                                move |value: &str, _window, cx| {
                                    let name = value.to_string();
                                    state_entity.update(cx, |state, _cx| {
                                        state.app.update_federation_source_name(source_idx, &name);
                                    });
                                }
                            }),
                    )
                    // Type badge
                    .child(
                        div()
                            .px(d.pad_y)
                            .py(d.half_grid)
                            .bg(theme.background)
                            .rounded(d.r_sm)
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .child(type_name),
                    )
                    .child(div().flex_1())
                    // Enable/Disable toggle
                    .child(
                        Button::new(
                            SharedString::from(format!("toggle-source-{source_idx}")),
                            if is_enabled {
                                translations.settings_on
                            } else {
                                translations.settings_off
                            },
                        )
                        .variant(if is_enabled {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Xs)
                        .theme(theme.to_button_theme())
                        .on_click_event(cx.listener(
                            move |view, _: &ClickEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.toggle_federation_source(source_idx);
                                });
                                cx.notify();
                            },
                        )),
                    )
                    // Remove button
                    .child(
                        Button::new(
                            SharedString::from(format!("remove-source-{source_idx}")),
                            translations.settings_remove,
                        )
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Xs)
                        .theme(theme.to_button_theme())
                        .on_click_event(
                            cx.listener(move |view, _: &ClickEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.remove_federation_source(source_idx);
                                });
                                cx.notify();
                            }),
                        ),
                    )
                    // Test button
                    .child(
                        Button::new(
                            SharedString::from(format!("test-source-{source_idx}")),
                            text.test,
                        )
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Xs)
                        .theme(theme.to_button_theme())
                        .on_click_event(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                            let source_idx = source_idx;
                            let state_entity = view.state.clone();

                            let test_result = state_entity.update(cx, |state, _cx| {
                                state.app.start_federation_source_test(source_idx)
                            });

                            if let Some((source_id, source)) = test_result {
                                cx.spawn(async move |_: WeakEntity<PlayerView>, cx| {
                                    let status = test_federation_connection(&source);
                                    state_entity.update(cx, |state, cx| {
                                        state.app.set_federation_source_status(&source_id, status);
                                        cx.notify();
                                    });
                                }).detach();
                            }
                            cx.notify();
                        })),
                    )
                    // Scan button
                    .child(
                        Button::new(
                            SharedString::from(format!("scan-source-{source_idx}")),
                            text.scan,
                        )
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Xs)
                        .theme(theme.to_button_theme())
                        .on_click_event(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                            view.state.update(cx, |state, _cx| {
                                state.app.scan_federation_source(source_idx);
                            });
                            cx.notify();
                        })),
                    ),
            )
            .child(Divider::new().color(theme.border))
            // Status row
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.gap)
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                                .child(text.status),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(status_color)
                            .child(status_label),
                    ),
            )
            // Diagnostic steps (shown after a test with diagnostic results)
            .when_some(diagnostic, {
                let theme = theme.clone();
                move |card, diag| {
                    let mut steps_col = div()
                        .flex()
                        .flex_col()
                        .gap(d.grid)
                        .p(d.pad_y)
                        .bg(theme.background)
                        .rounded(d.r_sm)
                        .child(
                            div()
                                .text_size(d.text_xs)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_secondary)
                                .child(format!("Connection diagnostic: {}:{}", diag.host, diag.port)),
                        );

                    for (label, result) in diag.steps() {
                        use sotf_audio_player::federation_config::StepResult;
                        let (icon, color) = match result {
                            StepResult::Ok(_) => ("OK ", theme.success),
                            StepResult::Fail(_) => ("FAIL", theme.error),
                            StepResult::Skipped(_) => ("SKIP", theme.text_muted),
                        };
                        steps_col = steps_col.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(d.gap)
                                .child(
                                    div()
                                        .text_size(d.text_xs)
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(color)
                                        .w(rems(2.25))
                                        .child(icon),
                                )
                                .child(
                                    div()
                                        .text_size(d.text_xs)
                                        .text_color(theme.text_secondary)
                                        .w(rems(6.25))
                                        .child(label),
                                )
                                .child(
                                    div()
                                        .text_size(d.text_xs)
                                        .text_color(color)
                                        .child(result.message().to_string()),
                                ),
                        );
                    }

                    card.child(steps_col)
                }
            })
            // Editable fields
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .children(
                        field_names
                            .iter()
                            .enumerate()
                            .map(|(field_idx, &field_name)| {
                                let value = field_values[field_idx].clone();
                                let state_entity = self.state.clone();

                                if field_name == "Auth Mode" {
                                    // Render radio buttons for auth mode
                                    let auth_options = ["None", "Password", "SSL"];
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(d.gap)
                                        .child(
                                            div()
                                                .w(rems(7.5))
                                                .text_size(d.text_xs)
                                                .text_color(theme.text_secondary)
                                                .child(field_name),
                                        )
                                        .child(
                                            HStack::new()
                                                .spacing(StackSpacing::Xs)
                                                .children(auth_options.iter().map(|option| {
                                                    let is_selected = value == *option;
                                                    Button::new(
                                                        SharedString::from(format!("auth-{}-{}", source_idx, option)),
                                                        *option,
                                                    )
                                                    .variant(if is_selected {
                                                        ButtonVariant::Primary
                                                    } else {
                                                        ButtonVariant::Secondary
                                                    })
                                                    .size(ButtonSize::Xs)
                                                    .theme(theme.to_button_theme())
                                                    .on_click_event(cx.listener({
                                                        let option = (*option).to_string();
                                                        let state_entity = state_entity.clone();
                                                        move |_, _: &ClickEvent, _window, cx| {
                                                            state_entity.update(cx, |state, _cx| {
                                                                state.app.update_federation_source_field(
                                                                    source_idx, field_idx, &option,
                                                                );
                                                            });
                                                        }
                                                    }))
                                                    .into_any_element()
                                                }))
                                                .into_any_element(),
                                        )
                                        .into_any_element()
                                } else {
                                    // Regular input field
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(d.gap)
                                        .child(
                                            div()
                                                .w(rems(7.5))
                                                .text_size(d.text_xs)
                                                .text_color(theme.text_secondary)
                                                .child(field_name),
                                        )
                                        .child(
                                            div().flex_1().child(
                                                Input::new(SharedString::from(format!(
                                                    "source-{source_idx}-field-{field_idx}"
                                                )))
                                                    .value(SharedString::from(value))
                                                    .placeholder(SharedString::from(format!(
                                                        "Enter {field_name}"
                                                    )))
                                                    .size(InputSize::Sm)
                                                    .on_change(move |val: &str, _window, cx| {
                                                        let v = val.to_string();
                                                        state_entity.update(cx, |state, _cx| {
                                                            state.app.update_federation_source_field(
                                                                source_idx, field_idx, &v,
                                                            );
                                                        });
                                                    }),
                                            ),
                                        )
                                        .into_any_element()
                                }
                            })
                            .collect::<Vec<_>>(),
                    )
                    .build(),
            )
    }

    /// Render a single-line progress row for an active federation scan.
    /// Shown above the footer.
    pub(crate) fn render_federation_scan_progress(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let ui_text = FederationTranslations::for_language(state.app.ui_state.language);
        let progress = state.app.federation.scan_progress.clone();

        let (text, pct) = match &progress {
            Some(p) if p.albums_total > 0 => {
                let pct = (p.albums_merged as f32 / p.albums_total as f32).min(1.0);
                (
                    format!(
                        "Syncing \"{}\" — {}/{} albums, {} tracks",
                        p.source_name, p.albums_merged, p.albums_total, p.tracks_merged
                    ),
                    pct,
                )
            }
            Some(p) => (
                format!("Fetching albums from \"{}\"...", p.source_name),
                0.0,
            ),
            None => return div().into_any_element(),
        };

        let bar_width_pct = (pct * 100.0) as i32;

        div()
            .flex()
            .items_center()
            .gap(d.gap)
            .px(d.card)
            .h(rems(1.75))
            .bg(theme.background_secondary)
            .border_t_1()
            .border_color(theme.border)
            // Progress bar background
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(d.gap)
                    // Thin progress bar
                    .child(
                        div()
                            .w(rems(7.5))
                            .h(rems(0.25))
                            .bg(theme.background)
                            .rounded(d.r_sm)
                            .child(
                                div()
                                    .h_full()
                                    .rounded(d.r_sm)
                                    .bg(theme.accent)
                                    .w(relative(bar_width_pct as f32 / 100.0)),
                            ),
                    )
                    // Status text
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .child(text),
                    ),
            )
            // Cancel button
            .child(
                Button::new("cancel-federation-scan", "\u{2715}")
                    .aria_label(ui_text.cancel_scan)
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Xs)
                    .theme(theme.to_button_theme())
                    .on_click_event(cx.listener(|view, _: &ClickEvent, _window, cx| {
                        view.state.update(cx, |state, _cx| {
                            state.app.cancel_federation_scan();
                        });
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
