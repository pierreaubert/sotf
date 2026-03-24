//! Federation Sources settings content

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Text, TextSize, VStack, StackSpacing};

impl PlayerView {
    /// Render federation sources settings content
    pub(crate) fn render_federation_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let sources = &state.app.federation_sources;

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
                        Text::new("No remote sources configured yet.")
                            .size(TextSize::Sm)
                            .color(theme.text_muted),
                    ),
            );
        } else {
            for source in sources {
                let type_name = source.connection.type_name();
                let enabled_label = if source.is_enabled { "Enabled" } else { "Disabled" };
                let enabled_color = if source.is_enabled {
                    theme.success
                } else {
                    theme.text_muted
                };

                content = content.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_4()
                        .bg(theme.background_secondary)
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(
                                    Text::new(source.display_name.clone())
                                        .size(TextSize::Sm)
                                        .weight(gpui_ui_kit::TextWeight::Bold)
                                        .color(theme.text_primary),
                                )
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
                                .child(
                                    div()
                                        .px_2()
                                        .py(px(2.0))
                                        .rounded_sm()
                                        .text_xs()
                                        .text_color(enabled_color)
                                        .child(enabled_label),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child(format!("priority: {}", source.priority)),
                                ),
                        )
                        .child(
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .children(
                                    source
                                        .connection
                                        .field_names()
                                        .iter()
                                        .enumerate()
                                        .map(|(i, name)| {
                                            let value = source.connection.field_value(i);
                                            div()
                                                .flex()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .w(px(120.0))
                                                        .text_xs()
                                                        .text_color(theme.text_secondary)
                                                        .child(*name),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme.text_primary)
                                                        .child(if value.is_empty() {
                                                            "(not set)".to_string()
                                                        } else {
                                                            value
                                                        }),
                                                )
                                                .into_any_element()
                                        })
                                        .collect::<Vec<_>>(),
                                )
                                .build(),
                        ),
                );
            }
        }

        content
    }
}
