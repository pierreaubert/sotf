//! Federation Sources settings content

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Divider, HStack, Input, InputSize, StackSpacing, Text,
    TextSize, VStack,
};

impl PlayerView {
    /// Render federation sources settings content
    pub(crate) fn render_federation_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let sources = state.app.federation_sources.clone();

        let mut content = div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_primary)
                    .child("Library Sources"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_secondary)
                    .child("Configure remote music libraries (Subsonic, MPD, DLNA, Peer). All connections use TLS except DLNA (plain HTTP for device compatibility)."),
            )
            // Add Source buttons
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
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
                            .child(self.add_source_button("peer", "Peer", &theme, cx)),
                    ),
            );

        if sources.is_empty() {
            content = content.child(
                div()
                    .p_4()
                    .bg(theme.background_secondary)
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        Text::new(
                            "No remote sources configured yet. Add one using the buttons above.",
                        )
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

    fn add_source_button(
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
        .build()
        .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
            view.state.update(cx, |state, _cx| {
                state.app.add_federation_source(type_name);
            });
            cx.notify();
        }))
    }

    fn render_source_card(
        &self,
        source_idx: usize,
        source: &sotf_audio_player::federation_config::FederationSourceEntry,
        theme: &crate::app::theme::Theme,
        translations: &crate::app::i18n::Translations,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let type_name = source.connection.type_name();
        let is_enabled = source.is_enabled;
        let display_name = source.display_name.clone();
        let field_names = source.connection.field_names();
        let field_values: Vec<String> = (0..field_names.len())
            .map(|i| source.connection.field_value(i))
            .collect();

        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .bg(theme.background_secondary)
            .rounded_md()
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
                    .gap_3()
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
                            .px_2()
                            .py(px(2.0))
                            .bg(theme.background)
                            .rounded_sm()
                            .text_xs()
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
                        .build()
                        .on_click(
                            cx.listener(move |view, _: &ClickEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.toggle_federation_source(source_idx);
                                });
                                cx.notify();
                            }),
                        ),
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
                        .build()
                        .on_click(
                            cx.listener(move |view, _: &ClickEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.remove_federation_source(source_idx);
                                });
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .child(Divider::new().color(theme.border))
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

                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .w(px(120.0))
                                            .text_xs()
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
                            })
                            .collect::<Vec<_>>(),
                    )
                    .build(),
            )
    }
}
