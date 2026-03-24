//! Server settings content

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{HStack, StackSpacing, Text, TextSize, VStack};

impl PlayerView {
    /// Render server settings content
    pub(crate) fn render_servers_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let server_config = &state.app.server_config;

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_primary)
                    .child("Server Settings"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.text_secondary)
                    .child("Configure servers that expose your library to other players. MPD uses TLS for security. DLNA uses plain HTTP for device compatibility."),
            )
            // MPD Server section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .bg(theme.background_secondary)
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new("MPD Server")
                                    .size(TextSize::Sm)
                                    .weight(gpui_ui_kit::TextWeight::Bold)
                                    .color(theme.text_primary),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py(px(2.0))
                                    .rounded_sm()
                                    .text_xs()
                                    .text_color(if server_config.mpd.enabled {
                                        theme.success
                                    } else {
                                        theme.text_muted
                                    })
                                    .child(if server_config.mpd.enabled {
                                        "Enabled"
                                    } else {
                                        "Disabled"
                                    }),
                            ),
                    )
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Xs)
                            .child(server_field_row(
                                "Bind Address",
                                &server_config.mpd.bind_address,
                                &theme,
                            ))
                            .child(server_field_row(
                                "Port",
                                &server_config.mpd.port.to_string(),
                                &theme,
                            ))
                            .child(server_field_row(
                                "TLS",
                                if server_config.mpd.tls_enabled {
                                    "Enabled"
                                } else {
                                    "Disabled"
                                },
                                &theme,
                            ))
                            .child(server_field_row(
                                "Password",
                                if server_config.mpd.password.is_some() {
                                    "(set)"
                                } else {
                                    "(none)"
                                },
                                &theme,
                            ))
                            .build(),
                    ),
            )
            // DLNA Server section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .bg(theme.background_secondary)
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new("DLNA Server")
                                    .size(TextSize::Sm)
                                    .weight(gpui_ui_kit::TextWeight::Bold)
                                    .color(theme.text_primary),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py(px(2.0))
                                    .rounded_sm()
                                    .text_xs()
                                    .text_color(if server_config.dlna.enabled {
                                        theme.success
                                    } else {
                                        theme.text_muted
                                    })
                                    .child(if server_config.dlna.enabled {
                                        "Enabled"
                                    } else {
                                        "Disabled"
                                    }),
                            ),
                    )
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Xs)
                            .child(server_field_row(
                                "Friendly Name",
                                &server_config.dlna.friendly_name,
                                &theme,
                            ))
                            .child(server_field_row(
                                "Port",
                                &server_config.dlna.port.to_string(),
                                &theme,
                            ))
                            .child(server_field_row(
                                "Protocol",
                                "HTTP (no TLS - device compat)",
                                &theme,
                            ))
                            .build(),
                    ),
            )
    }
}

fn server_field_row(
    label: &str,
    value: &str,
    theme: &crate::app::theme::Theme,
) -> impl IntoElement {
    div()
        .flex()
        .gap_2()
        .child(
            div()
                .w(px(120.0))
                .text_xs()
                .text_color(theme.text_secondary)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.text_primary)
                .child(value.to_string()),
        )
}
