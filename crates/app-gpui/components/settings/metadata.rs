#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::app::i18n::SettingsSurfaceTranslations;
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant};

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
    pub(crate) fn render_metadata_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let (theme, text, retained_error) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                SettingsSurfaceTranslations::for_language(state.app.ui_state.language),
                state.app.settings.metadata_error.clone(),
            )
        };
        let (config, load_error) = match sotf_audio_player::config::load_metadata_services_config()
        {
            Ok(config) => (config, None),
            Err(error) => (
                sotf_audio_player::MetadataServicesConfig::default(),
                Some(format!("{}: {error}", text.metadata_save_failed)),
            ),
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
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .child(auth_status),
                    )
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
                            .on_click_event(cx.listener(
                                move |view, _: &ClickEvent, _window, cx| {
                                    view.state.update(cx, |state, cx| {
                                        let result = (|| {
                                            let mut config = sotf_audio_player::config::load_metadata_services_config()?;
                                            if config.providers.is_empty() {
                                                config.providers.push(Default::default());
                                            }
                                            config.providers[0].enabled = !enabled;
                                            sotf_audio_player::config::save_metadata_services_config(&config)
                                        })();
                                        state.app.settings.metadata_error = result
                                            .err()
                                            .map(|error| format!("{}: {error}", text.metadata_save_failed));
                                        cx.notify();
                                    });
                                },
                            )),
                        "settings.metadata.search-toggle"
                    )),
            )
            .when_some(retained_error.or(load_error), |content, error| {
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
