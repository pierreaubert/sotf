#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::app::i18n::SettingsSurfaceTranslations;
use crate::app::types::PreferencesSetting;
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    AccessibilityExt, AccessibilityNode, AriaProps, AriaRole, AriaState, Button, ButtonSize,
    ButtonVariant, Spinner,
};

macro_rules! dev_track {
    ($element:expr, $selector:expr) => {{
        #[cfg(feature = "dev-api")]
        {
            $element.dev_track($selector)
        }
        #[cfg(not(feature = "dev-api"))]
        {
            $element
        }
    }};
}

impl PlayerView {
    fn update_metadata_preferences(&self, enabled: Option<bool>, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        if state.app.settings.metadata_loading {
            return;
        }
        let text = SettingsSurfaceTranslations::for_language(state.app.ui_state.language);
        let error_label = if enabled.is_some() {
            text.metadata_save_failed
        } else {
            text.metadata_services
        };
        self.state
            .update(cx, |state, _cx| state.app.settings.metadata_loading = true);
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut config = sotf_audio_player::config::load_metadata_services_config()
                        .map_err(|error| error.to_string())?;
                    if let Some(enabled) = enabled {
                        if config.providers.is_empty() {
                            config.providers.push(Default::default());
                        }
                        config.providers[0].enabled = enabled;
                        sotf_audio_player::config::save_metadata_services_config(&config)
                            .map_err(|error| error.to_string())?;
                    }
                    Ok::<_, String>(config)
                })
                .await;
            let _ = view.update(cx, |view, cx| {
                view.state.update(cx, |state, _cx| {
                    state.app.settings.metadata_loading = false;
                    match result {
                        Ok(config) => {
                            state.app.settings.metadata_config = Some(config);
                            state.app.settings.metadata_error = None;
                        }
                        Err(error) => {
                            state
                                .app
                                .settings
                                .metadata_config
                                .get_or_insert_with(Default::default);
                            state.app.settings.metadata_error =
                                Some(format!("{error_label}: {error}"));
                        }
                    }
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn render_metadata_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        if self.state.read(cx).app.settings.metadata_config.is_none() {
            self.update_metadata_preferences(None, cx);
        }
        let (theme, text, retained_error, config, loading) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                SettingsSurfaceTranslations::for_language(state.app.ui_state.language),
                state.app.settings.metadata_error.clone(),
                state
                    .app
                    .settings
                    .metadata_config
                    .clone()
                    .unwrap_or_default(),
                state.app.settings.metadata_loading,
            )
        };
        let provider = config.providers.first().cloned().unwrap_or_default();
        let enabled = provider.enabled;
        let account = provider
            .username
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(text.anonymous);
        let auth_status = if provider.has_stored_credentials {
            text.credentials_saved
        } else {
            text.anonymous_search_enabled
        };
        let toggle_label = if enabled {
            text.disable_metadata_search
        } else {
            text.enable_metadata_search
        };

        cx.register_accessible(AccessibilityNode {
            element_id: "metadata-search-toggle".into(),
            label: toggle_label.into(),
            props: AriaProps::with_role(AriaRole::Button).maybe_state(loading, AriaState::Disabled),
        });
        div()
            .flex()
            .flex_col()
            .gap(d.section_lg)
            .child(
                div()
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_primary)
                    .child(text.metadata_services),
            )
            .child(
                div()
                    .text_size(d.text_xs)
                    .text_color(theme.text_secondary)
                    .child(text.musicbrainz_description),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.grid)
                    .p(d.card)
                    .border_1()
                    .border_color(theme.border)
                    .rounded(d.r_md)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(text.musicbrainz),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .child(if enabled {
                                text.metadata_search_enabled
                            } else {
                                text.metadata_search_disabled
                            }),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .child(format!("Endpoint: {}", provider.endpoint)),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .child(format!("Account: {account}")),
                    )
                    .child(dev_track!(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .child(auth_status),
                        "settings.metadata.auth-status"
                    ))
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .child(format!("User-Agent: {}", config.user_agent)),
                    )
                    .child(dev_track!(
                        Button::new("metadata-search-toggle", toggle_label)
                            .variant(if enabled {
                                ButtonVariant::Secondary
                            } else {
                                ButtonVariant::Primary
                            })
                            .size(ButtonSize::Sm)
                            .theme(theme.to_button_theme())
                            .disabled(loading)
                            .build()
                            .text_size(d.text_sm)
                            .px(d.pad_x)
                            .py(d.pad_y_half)
                            .focusable()
                            .when_some(
                                self.preference_focus_handle(
                                    PreferencesSetting::MetadataSearch,
                                    cx
                                ),
                                |button, focus| {
                                    button.track_focus(&focus).track_focus_element(&focus)
                                }
                            )
                            .on_click(cx.listener(move |view, _, _window, cx| {
                                view.update_metadata_preferences(Some(!enabled), cx);
                            }))
                            .on_key_down(cx.listener(
                                move |view, event: &KeyDownEvent, _window, cx| {
                                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                        view.update_metadata_preferences(Some(!enabled), cx);
                                        cx.stop_propagation();
                                    }
                                }
                            ))
                            .map(|button| self.preference_control(
                                PreferencesSetting::MetadataSearch,
                                button,
                                cx
                            )),
                        "settings.metadata.search-toggle"
                    )),
            )
            .when(loading, |content| content.child(Spinner::new()))
            .when_some(retained_error, |content, error| {
                let alert = div()
                    .p(d.pad_x)
                    .rounded(d.r_md)
                    .border_1()
                    .border_color(theme.error)
                    .text_size(d.text_xs)
                    .text_color(theme.error)
                    .child(error);
                content.child(dev_track!(alert, "settings.metadata.error"))
            })
    }
}
