//! Server settings content

use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Divider, HStack, Input, InputSize, StackSpacing, Text,
    TextSize, TextWeight, VStack,
};

impl PlayerView {
    /// Render server settings content
    pub(crate) fn render_servers_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let server_config = state.app.federation.server_config.clone();

        div()
            .flex()
            .flex_col()
            .gap(d.section_lg)
            .child(
                div()
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_primary)
                    .child("Server Settings"),
            )
            .child(
                div()
                    .text_size(d.text_xs)
                    .text_color(theme.text_secondary)
                    .child("Configure servers that expose your library to other players. MPD uses TLS for security. DLNA uses plain HTTP for device compatibility."),
            )
            // MPD Server section
            .child(self.render_mpd_section(&server_config, &theme, &translations, &d, cx))
            // DLNA Server section
            .child(self.render_dlna_section(&server_config, &theme, &translations, &d, cx))
    }

    fn render_mpd_section(
        &self,
        server_config: &sotf_audio_player::federation_config::ServerConfig,
        theme: &crate::app::theme::Theme,
        translations: &crate::app::i18n::Translations,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mpd = &server_config.mpd;
        let mpd_enabled = mpd.enabled;
        let bind_address = mpd.bind_address.clone();
        let port = mpd.port.to_string();
        let tls_enabled = mpd.tls_enabled;
        let has_password = mpd.password.is_some();
        let cert_auth =
            mpd.auth_mode == sotf_audio_player::federation_config::MpdAuthMode::Certificate;

        let state_for_bind = self.state.clone();
        let state_for_port = self.state.clone();
        let state_for_pw = self.state.clone();

        div()
            .flex()
            .flex_col()
            .gap(d.gap_md)
            .p(d.card)
            .bg(theme.background_secondary)
            .rounded(d.r_md)
            .border_1()
            .border_color(if mpd_enabled {
                theme.accent
            } else {
                theme.border
            })
            // Header with toggle
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        Text::new("MPD Server")
                            .size(TextSize::Sm)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new(
                            "toggle-mpd",
                            if mpd_enabled {
                                translations.settings_on
                            } else {
                                translations.settings_off
                            },
                        )
                        .variant(if mpd_enabled {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Xs)
                        .theme(theme.to_button_theme())
                        .build()
                        .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                            view.state.update(cx, |state, _cx| {
                                state.app.toggle_mpd_server();
                            });
                            cx.notify();
                        })),
                    ),
            )
            .child(Divider::new().color(theme.border))
            // Editable fields
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    // Bind Address
                    .child(server_editable_field(
                        "mpd-bind",
                        "Bind Address",
                        &bind_address,
                        "0.0.0.0",
                        theme,
                        d,
                        move |val, _window, cx| {
                            let v = val.to_string();
                            state_for_bind.update(cx, |state, _cx| {
                                state.app.update_mpd_field("bind_address", &v);
                            });
                        },
                    ))
                    // Port
                    .child(server_editable_field(
                        "mpd-port",
                        "Port",
                        &port,
                        "6600",
                        theme,
                        d,
                        move |val, _window, cx| {
                            let v = val.to_string();
                            state_for_port.update(cx, |state, _cx| {
                                state.app.update_mpd_field("port", &v);
                            });
                        },
                    ))
                    // TLS toggle
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(d.gap)
                            .child(
                                div()
                                    .w(rems(7.5))
                                    .text_size(d.text_xs)
                                    .text_color(theme.text_secondary)
                                    .child("TLS"),
                            )
                            .child(
                                Button::new(
                                    "mpd-tls-toggle",
                                    if tls_enabled {
                                        translations.settings_on
                                    } else {
                                        translations.settings_off
                                    },
                                )
                                .variant(if tls_enabled {
                                    ButtonVariant::Primary
                                } else {
                                    ButtonVariant::Secondary
                                })
                                .size(ButtonSize::Xs)
                                .theme(theme.to_button_theme())
                                .build()
                                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                                    let new_val = (!tls_enabled).to_string();
                                    view.state.update(cx, |state, _cx| {
                                        state.app.update_mpd_field("tls_enabled", &new_val);
                                    });
                                    cx.notify();
                                })),
                            ),
                    )
                    // Auth mode toggle (Certificate / Password)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(d.gap)
                            .child(
                                div()
                                    .w(rems(7.5))
                                    .text_size(d.text_xs)
                                    .text_color(theme.text_secondary)
                                    .child("Auth"),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        Button::new("mpd-auth-cert", "Certificate")
                                            .variant(if cert_auth {
                                                ButtonVariant::Primary
                                            } else {
                                                ButtonVariant::Ghost
                                            })
                                            .size(ButtonSize::Xs)
                                            .theme(theme.to_button_theme())
                                            .build()
                                            .on_click(cx.listener(
                                                move |view, _: &ClickEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state
                                                            .app
                                                            .update_mpd_field("auth_mode", "certificate");
                                                    });
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                    .child(
                                        Button::new("mpd-auth-pw", "Password")
                                            .variant(if cert_auth {
                                                ButtonVariant::Ghost
                                            } else {
                                                ButtonVariant::Primary
                                            })
                                            .size(ButtonSize::Xs)
                                            .theme(theme.to_button_theme())
                                            .build()
                                            .on_click(cx.listener(
                                                move |view, _: &ClickEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state
                                                            .app
                                                            .update_mpd_field("auth_mode", "password");
                                                    });
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                    .build(),
                            ),
                    )
                    // Password (only shown when auth mode is Password)
                    .when(!cert_auth, |stack| {
                        stack.child(server_editable_field(
                            "mpd-password",
                            "Password",
                            "",
                            if has_password {
                                "Password is set (enter new to replace)"
                            } else {
                                "Enter password"
                            },
                            theme,
                            d,
                            move |val, _window, cx| {
                                let v = val.to_string();
                                state_for_pw.update(cx, |state, _cx| {
                                    state.app.update_mpd_field("password", &v);
                                });
                            },
                        ))
                    })
                    // Certificate info (shown when auth mode is Certificate)
                    .when(cert_auth, |stack| {
                        stack.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(d.gap)
                                .child(
                                    div()
                                        .w(rems(7.5))
                                        .text_size(d.text_xs)
                                        .text_color(theme.text_secondary)
                                        .child(""),
                                )
                                .child(
                                    div()
                                        .text_size(d.text_xs)
                                        .text_color(theme.text_muted)
                                        .child("Clients authenticate via TLS certificate. Add trusted client fingerprints to allow access."),
                                ),
                        )
                    })
                    .build(),
            )
    }

    fn render_dlna_section(
        &self,
        server_config: &sotf_audio_player::federation_config::ServerConfig,
        theme: &crate::app::theme::Theme,
        translations: &crate::app::i18n::Translations,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let dlna = &server_config.dlna;
        let dlna_enabled = dlna.enabled;
        let friendly_name = dlna.friendly_name.clone();
        let port = dlna.port.to_string();

        let state_for_name = self.state.clone();
        let state_for_port = self.state.clone();

        div()
            .flex()
            .flex_col()
            .gap(d.gap_md)
            .p(d.card)
            .bg(theme.background_secondary)
            .rounded(d.r_md)
            .border_1()
            .border_color(if dlna_enabled {
                theme.accent
            } else {
                theme.border
            })
            // Header with toggle
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        Text::new("DLNA Server")
                            .size(TextSize::Sm)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new(
                            "toggle-dlna",
                            if dlna_enabled {
                                translations.settings_on
                            } else {
                                translations.settings_off
                            },
                        )
                        .variant(if dlna_enabled {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Xs)
                        .theme(theme.to_button_theme())
                        .build()
                        .on_click(cx.listener(
                            move |view, _: &ClickEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.toggle_dlna_server();
                                });
                                cx.notify();
                            },
                        )),
                    ),
            )
            .child(Divider::new().color(theme.border))
            // Editable fields
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    // Friendly Name
                    .child(server_editable_field(
                        "dlna-name",
                        "Friendly Name",
                        &friendly_name,
                        "SOTF Media Server",
                        theme,
                        d,
                        move |val, _window, cx| {
                            let v = val.to_string();
                            state_for_name.update(cx, |state, _cx| {
                                state.app.update_dlna_field("friendly_name", &v);
                            });
                        },
                    ))
                    // Port
                    .child(server_editable_field(
                        "dlna-port",
                        "Port",
                        &port,
                        "8200",
                        theme,
                        d,
                        move |val, _window, cx| {
                            let v = val.to_string();
                            state_for_port.update(cx, |state, _cx| {
                                state.app.update_dlna_field("port", &v);
                            });
                        },
                    ))
                    // Protocol (read-only info)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(d.gap)
                            .child(
                                div()
                                    .w(rems(7.5))
                                    .text_size(d.text_xs)
                                    .text_color(theme.text_secondary)
                                    .child("Protocol"),
                            )
                            .child(
                                div()
                                    .text_size(d.text_xs)
                                    .text_color(theme.text_muted)
                                    .child("HTTP (no TLS - device compat)"),
                            ),
                    )
                    .build(),
            )
    }
}

fn server_editable_field(
    id: &str,
    label: &str,
    value: &str,
    placeholder: &str,
    theme: &crate::app::theme::Theme,
    d: &Ds,
    on_change: impl Fn(&str, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(d.gap)
        .child(
            div()
                .w(rems(7.5))
                .text_size(d.text_xs)
                .text_color(theme.text_secondary)
                .child(label.to_string()),
        )
        .child(
            div().flex_1().child(
                Input::new(SharedString::from(id.to_string()))
                    .value(SharedString::from(value.to_string()))
                    .placeholder(SharedString::from(placeholder.to_string()))
                    .size(InputSize::Sm)
                    .on_change(on_change),
            ),
        )
}
