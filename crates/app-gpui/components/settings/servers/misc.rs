use crate::app::i18n::ServerSettingsTranslations;
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSet, ButtonSetOption, ButtonSetSize, ButtonSize, ButtonVariant, Divider, HStack,
    Input, InputSize, QrCode, StackSpacing, Text, TextSize, TextWeight, VStack,
};
use std::rc::Rc;

const SERVER_QR_CODE_SIZE_PX: f32 = 220.0;

macro_rules! dev_track {
    ($element:expr, $selector:expr $(,)?) => {{
        #[cfg(feature = "dev-api")]
        {
            use crate::app::dev_api::DevTrackExt;
            $element.dev_track($selector)
        }
        #[cfg(not(feature = "dev-api"))]
        {
            $element
        }
    }};
}

impl PlayerView {
    /// Render server settings content
    pub(crate) fn render_servers_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = ServerSettingsTranslations::for_language(state.app.ui_state.language);
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
                    .child(text.serves_media),
            )
            // SOTF API section
            .child(self.render_sotf_api_section(&server_config, &theme, &d, cx))
            // MPD Server section
            .child(self.render_mpd_section(&server_config, &theme, &translations, &d, cx))
            // DLNA Server section
            .child(self.render_dlna_section(&server_config, &theme, &d, cx))
            // Remote SOTF players are a client-side connection setting, not a
            // hosted media-server option, but belong alongside the other
            // server configuration surfaces.
            .child(self.render_remote_sotf_section(&theme, &d, cx))
    }

    pub(super) fn render_sotf_api_section(
        &self,
        server_config: &sotf_audio_player::federation_config::ServerConfig,
        theme: &crate::app::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let text =
            ServerSettingsTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let api = &server_config.api;
        let url = sotf_audio_player::server::sotf_api_server_url_for_settings(api);
        let has_token = api
            .auth_token
            .as_deref()
            .is_some_and(|token| !token.trim().is_empty());
        let selected_api_state = if api.enabled { "enabled" } else { "disabled" };
        let state_for_api_enabled = self.state.clone();
        let (show_qr, qr_data) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.show_sotf_api_connection_qr,
                state.app.sotf_api_connection_qr_data(),
            )
        };

        div()
            .flex()
            .flex_col()
            .gap(d.gap_md)
            .p(d.card)
            .bg(theme.background_secondary)
            .rounded(d.r_md)
            .border_1()
            .border_color(if api.enabled {
                theme.accent
            } else {
                theme.border
            })
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        Text::new(text.sotf_api)
                            .size(TextSize::Sm)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(div().flex_1())
                    .child(
                        ButtonSet::new("sotf-api-enabled")
                            .options(vec![
                                ButtonSetOption::new("enabled", text.enable),
                                ButtonSetOption::new("disabled", text.disable),
                            ])
                            .selected(selected_api_state)
                            .size(ButtonSetSize::Xs)
                            .theme(theme.to_button_set_theme())
                            .on_change(move |value, _window, cx| {
                                state_for_api_enabled.update(cx, |state, _cx| {
                                    state.app.set_sotf_api_enabled(value.as_ref() == "enabled");
                                });
                            }),
                    )
                    .child(
                        Button::new(
                            "toggle-sotf-api-connection-qr",
                            if show_qr { "Hide QR" } else { "Show QR" },
                        )
                        .variant(if show_qr {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Xs)
                        .theme(theme.to_button_theme())
                        .disabled(!api.enabled)
                        .on_click_event(cx.listener(
                            |view, _: &ClickEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    if let Err(err) = state.app.toggle_sotf_api_connection_qr() {
                                        state.app.ui_state.toast_message =
                                            Some(crate::app::ToastMessage::error(err));
                                    }
                                });
                                cx.notify();
                            },
                        )),
                    )
                    .build(),
            )
            .child(Divider::new().color(theme.border))
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
                            .child(text.url),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_primary)
                            .child(url),
                    ),
            )
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
                            .child(text.token),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(if has_token {
                                theme.success
                            } else {
                                theme.warning
                            })
                            .child(if has_token {
                                "Configured"
                            } else {
                                "Generated when QR is shown"
                            }),
                    ),
            )
            .when(show_qr, |section| {
                section.child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(d.gap_md)
                        .py(d.pad_y)
                        .when_some(qr_data, |el, data| {
                            el.child(QrCode::new(data).size(px(SERVER_QR_CODE_SIZE_PX)))
                                .child(
                                    div()
                                        .text_size(d.text_xs)
                                        .text_color(theme.text_muted)
                                        .child(text.scan_to_add),
                                )
                        }),
                )
            })
    }

    pub(super) fn render_mpd_section(
        &self,
        server_config: &sotf_audio_player::federation_config::ServerConfig,
        theme: &crate::app::theme::Theme,
        translations: &crate::app::i18n::Translations,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let text =
            ServerSettingsTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let mpd = &server_config.mpd;
        let mpd_enabled = mpd.enabled;
        let bind_address = mpd.bind_address.clone();
        let port = mpd.port.to_string();
        let tls_enabled = mpd.tls_enabled;
        let has_password = mpd.password.is_some();
        let show_mpd_password = self.state.read(cx).app.settings.show_mpd_password;
        let selected_mpd_state = if mpd_enabled { "enabled" } else { "disabled" };
        let cert_auth =
            mpd.auth_mode == sotf_audio_player::federation_config::MpdAuthMode::Certificate;

        let state_for_mpd_enabled = self.state.clone();
        let state_for_bind = self.state.clone();
        let state_for_port = self.state.clone();
        let state_for_pw = self.state.clone();
        let state_for_pw_reveal = self.state.clone();
        let view_for_pw_reveal = cx.entity().clone();

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
                        Text::new(text.mpd_server)
                            .size(TextSize::Sm)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(div().flex_1())
                    .child(
                        ButtonSet::new("mpd-enabled")
                            .options(vec![
                                ButtonSetOption::new("enabled", text.enable),
                                ButtonSetOption::new("disabled", text.disable),
                            ])
                            .selected(selected_mpd_state)
                            .size(ButtonSetSize::Xs)
                            .theme(theme.to_button_set_theme())
                            .on_change(move |value, _window, cx| {
                                state_for_mpd_enabled.update(cx, |state, _cx| {
                                    state
                                        .app
                                        .set_mpd_server_enabled(value.as_ref() == "enabled");
                                });
                            }),
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
                                    .child(text.tls),
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
                                .on_click_event(cx.listener(
                                    move |view, _: &ClickEvent, _window, cx| {
                                        let new_val = (!tls_enabled).to_string();
                                        view.state.update(cx, |state, _cx| {
                                            state.app.update_mpd_field("tls_enabled", &new_val);
                                        });
                                        cx.notify();
                                    },
                                )),
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
                                    .child(text.authentication),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        Button::new("mpd-auth-cert", text.certificate)
                                            .variant(if cert_auth {
                                                ButtonVariant::Primary
                                            } else {
                                                ButtonVariant::Ghost
                                            })
                                            .size(ButtonSize::Xs)
                                            .theme(theme.to_button_theme())
                                            .on_click_event(cx.listener(
                                                move |view, _: &ClickEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state.app.update_mpd_field(
                                                            "auth_mode",
                                                            "certificate",
                                                        );
                                                    });
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                    .child(
                                        Button::new("mpd-auth-pw", text.password)
                                            .variant(if cert_auth {
                                                ButtonVariant::Ghost
                                            } else {
                                                ButtonVariant::Primary
                                            })
                                            .size(ButtonSize::Xs)
                                            .theme(theme.to_button_theme())
                                            .on_click_event(cx.listener(
                                                move |view, _: &ClickEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state.app.update_mpd_field(
                                                            "auth_mode",
                                                            "password",
                                                        );
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
                        stack.child(server_secret_field(
                            "mpd-password",
                            text.password,
                            "",
                            if has_password {
                                "Password is set (enter new to replace)"
                            } else {
                                "Enter password"
                            },
                            theme,
                            d,
                            show_mpd_password,
                            move |val, _window, cx| {
                                let v = val.to_string();
                                state_for_pw.update(cx, |state, _cx| {
                                    state.app.update_mpd_field("password", &v);
                                });
                            },
                            move |cx| {
                                state_for_pw_reveal.update(cx, |state, _| {
                                    state.app.settings.show_mpd_password =
                                        !state.app.settings.show_mpd_password;
                                });
                                view_for_pw_reveal.update(cx, |_, cx| cx.notify());
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
                                        .child(text.client_certificate_help),
                                ),
                        )
                    })
                    .build(),
            )
    }

    pub(super) fn render_dlna_section(
        &self,
        server_config: &sotf_audio_player::federation_config::ServerConfig,
        theme: &crate::app::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let text =
            ServerSettingsTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let dlna = &server_config.dlna;
        let dlna_enabled = dlna.enabled;
        let friendly_name = dlna.friendly_name.clone();
        let port = dlna.port.to_string();
        let selected_dlna_state = if dlna_enabled { "enabled" } else { "disabled" };

        let state_for_dlna_enabled = self.state.clone();
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
                        Text::new(text.dlna_server)
                            .size(TextSize::Sm)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(div().flex_1())
                    .child(
                        ButtonSet::new("dlna-enabled")
                            .options(vec![
                                ButtonSetOption::new("enabled", text.enable),
                                ButtonSetOption::new("disabled", text.disable),
                            ])
                            .selected(selected_dlna_state)
                            .size(ButtonSetSize::Xs)
                            .theme(theme.to_button_set_theme())
                            .on_change(move |value, _window, cx| {
                                state_for_dlna_enabled.update(cx, |state, _cx| {
                                    state
                                        .app
                                        .set_dlna_server_enabled(value.as_ref() == "enabled");
                                });
                            }),
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
                                    .child(text.protocol),
                            )
                            .child(
                                div()
                                    .text_size(d.text_xs)
                                    .text_color(theme.text_muted)
                                    .child(text.http_compatibility),
                            ),
                    )
                    .build(),
            )
    }

    pub(crate) fn render_remote_sotf_section(
        &self,
        theme: &crate::app::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let text =
            ServerSettingsTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let remote = {
            let state = self.state.read(cx);
            (
                state.app.remote.server_store.clone(),
                state.app.remote.discovered_servers.len(),
                state.app.remote.discovery_running,
                state.app.remote.discovery_error.clone(),
                state.app.remote.manual_server_name.clone(),
                state.app.remote.manual_api_base_url.clone(),
                state.app.remote.manual_auth_token.clone(),
                state.app.remote.server_tokens.clone(),
                state.app.remote.server_probe_statuses.clone(),
            )
        };
        let (
            store,
            discovered_count,
            discovery_running,
            discovery_error,
            manual_name,
            manual_url,
            manual_token,
            server_tokens,
            probe_statuses,
        ) = remote;
        let selected_id = store.selected_server_id.clone();
        let server_count = store.servers.len();
        let show_manual_remote_token = self.state.read(cx).app.settings.show_manual_remote_token;
        let state_for_name = self.state.clone();
        let state_for_url = self.state.clone();
        let state_for_token = self.state.clone();
        let state_for_token_reveal = self.state.clone();
        let view_for_token_reveal = cx.entity().clone();

        let mut section = div()
            .flex()
            .flex_col()
            .gap(d.gap_md)
            .p(d.card)
            .bg(theme.background_secondary)
            .rounded(d.r_md)
            .border_1()
            .border_color(theme.border)
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        Text::new(text.remote_players)
                            .size(TextSize::Sm)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(div().flex_1())
                    .child(self.render_scan_sotf_qr_button(theme, d, cx))
                    .child(
                        Button::new(
                            "discover-sotf-remotes",
                            if discovery_running {
                                "Scanning..."
                            } else {
                                "Refresh"
                            },
                        )
                        .variant(if discovery_running {
                            ButtonVariant::Ghost
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Xs)
                        .theme(theme.to_button_theme())
                        .on_click_event(cx.listener(
                            move |view, _: &ClickEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.start_remote_server_discovery();
                                });
                                cx.notify();
                            },
                        )),
                    ),
            )
            .child(Divider::new().color(theme.border))
            .child(
                div()
                    .text_size(d.text_xs)
                    .text_color(theme.text_secondary)
                    .child(format!(
                        "{server_count} saved, {discovered_count} found in the latest LAN scan."
                    )),
            )
            .child(
                div()
                    .text_size(d.text_xs)
                    .text_color(theme.text_muted)
                    .child(text.api_address_help),
            )
            .child(
                div()
                    .text_size(d.text_xs)
                    .text_color(theme.text_muted)
                    .child(text.api_token_help),
            )
            .when(discovery_error.is_some(), {
                let theme = theme.clone();
                move |el| {
                    el.child(
                        div()
                            .p(d.pad_y)
                            .bg(theme.background)
                            .rounded(d.r_sm)
                            .text_size(d.text_xs)
                            .text_color(theme.warning)
                            .child(discovery_error.unwrap_or_default()),
                    )
                }
            })
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(server_editable_field(
                        "remote-sotf-name",
                        "Name",
                        &manual_name,
                        "Listening Room",
                        theme,
                        d,
                        move |val, _window, cx| {
                            let value = val.to_string();
                            state_for_name.update(cx, |state, _cx| {
                                state.app.update_manual_remote_server_name(value);
                            });
                        },
                    ))
                    .child(server_editable_field(
                        "remote-sotf-url",
                        "API URL",
                        &manual_url,
                        "http://host.local:8732",
                        theme,
                        d,
                        move |val, _window, cx| {
                            let value = val.to_string();
                            state_for_url.update(cx, |state, _cx| {
                                state.app.update_manual_remote_server_url(value);
                            });
                        },
                    ))
                    .child(server_secret_field(
                        "remote-sotf-token",
                        "API Token",
                        &manual_token,
                        "Paste SOTF API token",
                        theme,
                        d,
                        show_manual_remote_token,
                        move |val, _window, cx| {
                            let value = val.to_string();
                            state_for_token.update(cx, |state, _cx| {
                                state.app.update_manual_remote_server_token(value);
                            });
                        },
                        move |cx| {
                            state_for_token_reveal.update(cx, |state, _| {
                                state.app.settings.show_manual_remote_token =
                                    !state.app.settings.show_manual_remote_token;
                            });
                            view_for_token_reveal.update(cx, |_, cx| cx.notify());
                        },
                    ))
                    .child(div().flex().justify_end().child(dev_track!(
                            Button::new("add-manual-sotf-remote", text.add_server)
                                .variant(ButtonVariant::Primary)
                                .size(ButtonSize::Xs)
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(
                                    move |view, _: &ClickEvent, _window, cx| {
                                        view.state.update(cx, |state, _cx| {
                                            match state.app.add_manual_remote_server_from_inputs() {
                                                Ok(server_id) => {
                                                    state.app.start_remote_server_probe(&server_id);
                                                }
                                                Err(err) => {
                                                    state.app.ui_state.toast_message =
                                                        Some(crate::app::ToastMessage::error(err));
                                                }
                                            }
                                        });
                                        cx.notify();
                                    },
                                )),
                            "settings.servers.remote.add",
                        )))
                    .build(),
            );

        if store.servers.is_empty() {
            section = section.child(
                div()
                    .p(d.pad_y)
                    .bg(theme.background)
                    .rounded(d.r_sm)
                    .text_size(d.text_xs)
                    .text_color(theme.text_muted)
                    .child(text.no_remote_players),
            );
        } else {
            for server in store.servers {
                let is_selected = selected_id.as_deref() == Some(server.id.as_str());
                let probe_status = probe_statuses.get(&server.id).cloned();
                let has_token = server_tokens
                    .get(&server.id)
                    .is_some_and(|token| !token.trim().is_empty());
                section = section.child(self.render_remote_sotf_server_row(
                    server,
                    is_selected,
                    has_token,
                    probe_status,
                    theme,
                    d,
                    cx,
                ));
            }
        }

        section
    }

    pub(crate) fn render_scan_sotf_qr_button(
        &self,
        theme: &crate::app::theme::Theme,
        _d: &Ds,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let text =
            ServerSettingsTranslations::for_language(self.state.read(cx).app.ui_state.language);
        #[cfg(target_os = "ios")]
        {
            Button::new("scan-sotf-remote-qr", text.scan_qr)
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Xs)
                .theme(theme.to_button_theme())
                .on_click_event(cx.listener(|_view, _: &ClickEvent, _window, _cx| {
                    unsafe extern "C" {
                        fn sotf_ios_show_qr_scanner();
                    }
                    // SAFETY: implemented by app-ios in the final iOS binary.
                    // It presents UIKit UI on the main queue and retains no
                    // Rust references across the FFI boundary.
                    unsafe { sotf_ios_show_qr_scanner() };
                }))
                .into_any_element()
        }
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            Button::new("scan-sotf-remote-qr", text.scan_qr)
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Xs)
                .theme(theme.to_button_theme())
                .on_click_event(cx.listener(|view, _: &ClickEvent, _window, cx| {
                    let weak_state = view.state.downgrade();
                    cx.spawn(async move |_, cx| {
                        let file = rfd::AsyncFileDialog::new()
                            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "bmp", "webp"])
                            .set_title("Select SOTF API QR Code")
                            .pick_file()
                            .await;
                        let Some(file) = file else {
                            return;
                        };
                        let Some(state_entity) = weak_state.upgrade() else {
                            return;
                        };
                        let path = file.path().to_path_buf();
                        state_entity.update(&mut cx.clone(), |state, cx| {
                            if let Err(err) = state.app.add_remote_server_from_qr_image_file(&path)
                            {
                                state.app.ui_state.toast_message =
                                    Some(crate::app::ToastMessage::error(err));
                            }
                            cx.notify();
                        });
                    })
                    .detach();
                }))
                .into_any_element()
        }
        #[cfg(target_os = "tvos")]
        {
            let _ = (theme, _d, cx);
            div().into_any_element()
        }
    }

    pub(crate) fn render_remote_sotf_server_row(
        &self,
        server: sotf_audio_player::SotfRemoteServer,
        is_selected: bool,
        has_token: bool,
        probe_status: Option<crate::app::state::app::RemoteServerProbeStatus>,
        theme: &crate::app::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let text =
            ServerSettingsTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let server_id_for_select = server.id.clone();
        let server_id_for_remove = server.id.clone();
        let server_id_for_test = server.id.clone();
        let address = server
            .address
            .clone()
            .or(server.host_name.clone())
            .unwrap_or_else(|| server.origin_url.clone());
        let endpoint = format!(
            "{}://{}:{}{}",
            server.protocol, address, server.port, server.api_path
        );
        let auth = server.auth.clone();
        let friendly_name = server.friendly_name.clone();
        let (status_label, status_color) = match &probe_status {
            Some(crate::app::state::app::RemoteServerProbeStatus::Testing) => {
                ("testing".to_string(), theme.warning)
            }
            Some(status @ crate::app::state::app::RemoteServerProbeStatus::Reachable { .. }) => {
                (status.label(), theme.success)
            }
            Some(status @ crate::app::state::app::RemoteServerProbeStatus::Failed(_)) => {
                (status.label(), theme.error)
            }
            None => ("untested".to_string(), theme.text_muted),
        };

        div()
            .flex()
            .items_center()
            .gap(d.gap_md)
            .p(d.pad_y)
            .bg(if is_selected {
                theme.surface_selected
            } else {
                theme.background
            })
            .rounded(d.r_sm)
            .border_1()
            .border_color(if is_selected {
                theme.accent
            } else {
                theme.border
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.grid)
                    .flex_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(d.gap)
                            .child(Text::section_header(friendly_name).color(theme.text_primary))
                            .when(is_selected, |row| {
                                row.child(
                                    div()
                                        .px(d.pad_y)
                                        .py(d.half_grid)
                                        .rounded(d.r_sm)
                                        .bg(theme.accent)
                                        .text_size(d.text_xs)
                                        .text_color(theme.text_on_accent)
                                        .child(text.selected),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .child(endpoint),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(format!(
                                "Auth: {auth} ({})",
                                if has_token {
                                    "token cached"
                                } else {
                                    "token missing"
                                }
                            )),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(status_color)
                            .child(format!("Status: {status_label}")),
                    ),
            )
            .child(
                Button::new(
                    SharedString::from(format!("test-sotf-remote-{server_id_for_test}")),
                    text.test,
                )
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Xs)
                .theme(theme.to_button_theme())
                .on_click_event(cx.listener(
                    move |view, _: &ClickEvent, _window, cx| {
                        let server_id = server_id_for_test.clone();
                        view.state.update(cx, |state, _cx| {
                            state.app.start_remote_server_probe(&server_id);
                        });
                        cx.notify();
                    },
                )),
            )
            .child(
                Button::new(
                    SharedString::from(format!("select-sotf-remote-{server_id_for_select}")),
                    text.select,
                )
                .variant(if is_selected {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Secondary
                })
                .size(ButtonSize::Xs)
                .theme(theme.to_button_theme())
                .on_click_event(cx.listener(
                    move |view, _: &ClickEvent, _window, cx| {
                        let server_id = server_id_for_select.clone();
                        view.state.update(cx, |state, _cx| {
                            state.app.select_remote_server(&server_id);
                        });
                        cx.notify();
                    },
                )),
            )
            .child(
                Button::new(
                    SharedString::from(format!("remove-sotf-remote-{server_id_for_remove}")),
                    text.remove,
                )
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Xs)
                .theme(theme.to_button_theme())
                .on_click_event(cx.listener(
                    move |view, _: &ClickEvent, _window, cx| {
                        let server_id = server_id_for_remove.clone();
                        view.state.update(cx, |state, _cx| {
                            state.app.remove_remote_server(&server_id);
                        });
                        cx.notify();
                    },
                )),
            )
    }
}

pub(crate) fn settings_section_label(
    label: &'static str,
    theme: &crate::app::theme::Theme,
    d: &Ds,
) -> impl IntoElement {
    div()
        .pt(d.pad_y)
        .text_size(d.text_xs)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text_secondary)
        .child(label)
}

pub(crate) fn server_editable_field(
    id: &str,
    label: &str,
    value: &str,
    placeholder: &str,
    theme: &crate::app::theme::Theme,
    d: &Ds,
    on_change: impl Fn(&str, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    server_editable_field_with_mode(id, label, value, placeholder, false, theme, d, on_change)
}

/// Render a server credential field. Its rendered and accessibility values are
/// masked by the toolkit; the value is only passed to the configured callback.
pub(crate) fn server_secret_field(
    id: &str,
    label: &str,
    value: &str,
    placeholder: &str,
    theme: &crate::app::theme::Theme,
    d: &Ds,
    revealed: bool,
    on_change: impl Fn(&str, &mut Window, &mut gpui::App) + 'static,
    on_toggle_reveal: impl Fn(&mut gpui::App) + 'static,
) -> impl IntoElement {
    let on_change = Rc::new(on_change);
    let on_confirm = on_change.clone();
    let on_text_change = on_change.clone();
    let reveal_label = if revealed {
        format!("Hide {label}")
    } else {
        format!("Show {label}")
    };

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
        .child(div().flex_1().child(dev_track!(
                Input::new(SharedString::from(id.to_string()))
                    .value(SharedString::from(value.to_string()))
                    .placeholder(SharedString::from(placeholder.to_string()))
                    .size(InputSize::Sm)
                    .password(!revealed)
                    .aria_label(SharedString::from(label.to_string()))
                    .on_text_change(move |value, window, cx| {
                        on_text_change(&value, window, cx);
                    })
                    .on_change(move |value, window, cx| {
                        on_confirm(value, window, cx);
                    }),
                format!("settings.servers.{id}"),
            )))
        .child(dev_track!(
            Button::new(
                SharedString::from(format!("{id}-reveal")),
                if revealed { "Hide" } else { "Show" },
            )
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::Xs)
            .theme(theme.to_button_theme())
            .aria_label(reveal_label)
            .on_click(move |_, cx| {
                // `Button` owns focus and keyboard activation, so this is
                // deliberately a real control rather than an input icon.
                on_toggle_reveal(cx);
            }),
            format!("settings.servers.{id}.reveal")
        ))
}

fn server_editable_field_with_mode(
    id: &str,
    label: &str,
    value: &str,
    placeholder: &str,
    secret: bool,
    theme: &crate::app::theme::Theme,
    d: &Ds,
    on_change: impl Fn(&str, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let on_change = Rc::new(on_change);
    let on_confirm = on_change.clone();
    let on_text_change = on_change.clone();

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
        .child(div().flex_1().child(dev_track!(
                Input::new(SharedString::from(id.to_string()))
                    .value(SharedString::from(value.to_string()))
                    .placeholder(SharedString::from(placeholder.to_string()))
                    .size(InputSize::Sm)
                    .password(secret)
                    .aria_label(SharedString::from(label.to_string()))
                    .on_text_change(move |value, window, cx| {
                        on_text_change(&value, window, cx);
                    })
                    .on_change(move |value, window, cx| {
                        on_confirm(value, window, cx);
                    }),
                format!("settings.servers.{id}"),
            )))
}
