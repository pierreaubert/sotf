use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_metadata_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = SettingsSurfaceTranslations::for_language(state.app.ui_state.language);
        let config = sotf_audio_player::config::load_metadata_services_config()
            .unwrap_or_else(|_| sotf_audio_player::MetadataServicesConfig::default());
        let provider = config.providers.first().cloned().unwrap_or_default();
        let account = provider
            .username
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("Anonymous");
        let auth_status = if provider.has_stored_credentials {
            "Credentials saved"
        } else {
            "Anonymous search enabled"
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
                    ),
            )
    }
}
use crate::app::i18n::SettingsSurfaceTranslations;
