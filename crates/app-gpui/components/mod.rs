// Screen rendering modules
//

pub mod autoeq;
pub mod design;
pub mod dialogs;
pub mod graphs;
pub mod headphone_eq;
pub mod home;
pub mod icons;
pub mod listening_test;
pub mod migration;
pub mod playlists;
pub mod plugins;
pub mod recording;
pub mod room_eq;
mod settings;
mod spinorama_eq;
pub mod streams;
mod workflow;
pub use plugins::{
    LevelMeterElement, MeterColors, MeterData, SpectrumColors, SpectrumElement, get_param_count,
    render_plugin_content,
};

use crate::app::SettingsTab;
use crate::app::i18n::{Language, PlaybackApplyTranslations, WizardNavigationTranslations};
use crate::app::types::PluginUpdateType;
use crate::components::design::Ds;
use crate::components::icons::IconName;
use crate::components::plugins::editing::PluginEditingManager;
use crate::i18n::Translations;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonTheme, ButtonVariant, Card, HStack, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};

pub fn settings_tab_icon_name(tab: SettingsTab) -> IconName {
    match tab {
        SettingsTab::Library => IconName::Library,
        SettingsTab::Theme => IconName::PenTool,
        SettingsTab::Language => IconName::User,
        SettingsTab::Keybindings => IconName::Settings,
        SettingsTab::AudioDevice => IconName::Speaker,
        SettingsTab::Misc => IconName::SlidersHorizontal,
        SettingsTab::Federation => IconName::Plug,
        SettingsTab::Servers => IconName::Cog,
        SettingsTab::Metadata => IconName::Album,
        SettingsTab::ReleaseChannel => IconName::AudioWaveform,
    }
}

pub fn settings_tab_label(tab: SettingsTab, translations: &Translations) -> &'static str {
    match tab {
        SettingsTab::Library => translations.settings_tab_library,
        SettingsTab::Theme => translations.settings_tab_theme,
        SettingsTab::Language => translations.settings_tab_language,
        SettingsTab::Keybindings => translations.settings_tab_keybindings,
        SettingsTab::AudioDevice => translations.settings_tab_audio_device,
        SettingsTab::Misc => translations.settings_tab_misc,
        SettingsTab::Federation => translations.settings_tab_federation,
        SettingsTab::Servers => translations.settings_tab_servers,
        SettingsTab::Metadata => translations.settings_tab_metadata,
        SettingsTab::ReleaseChannel => translations.settings_tab_release_channel,
    }
}

/// Themed tooltip view for GPUI's native tooltip system.
/// Used by rack buttons, footer controls, and other interactive elements.
pub(crate) struct ThemedTooltip {
    text: SharedString,
    bg: Rgba,
    border: Rgba,
    text_color: Rgba,
}

impl Render for ThemedTooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        div()
            .px(d.pad_y)
            .py(d.pad_y_half)
            .bg(self.bg)
            .border_1()
            .border_color(self.border)
            .rounded(d.r_md)
            .shadow_md()
            .text_size(d.text_xs)
            .text_color(self.text_color)
            .whitespace_nowrap()
            .child(self.text.clone())
    }
}

/// Create a themed tooltip AnyView for GPUI's native `.tooltip()` method.
pub(crate) fn themed_tooltip(
    text: impl Into<SharedString>,
    theme: &Theme,
    cx: &mut App,
) -> AnyView {
    let text = text.into();
    cx.new(move |_| ThemedTooltip {
        text,
        bg: theme.surface,
        border: theme.border,
        text_color: theme.text_primary,
    })
    .into()
}

/// Consistent primary-action copy for every guided workflow.
pub fn wizard_continue_label(language: Language, next_step: Option<&str>) -> String {
    WizardNavigationTranslations::for_language(language).primary_action(next_step)
}

impl PlayerView {
    pub(crate) fn render_settings_tab_content(
        &self,
        active_tab: SettingsTab,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match active_tab {
            crate::app::SettingsTab::Library => {
                self.render_library_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::Theme => {
                self.render_theme_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::Language => {
                self.render_language_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::Keybindings => self
                .render_keybindings_settings_content(cx)
                .into_any_element(),
            crate::app::SettingsTab::AudioDevice => div()
                .flex()
                .flex_col()
                .child(self.render_audio_device_settings_content(cx))
                .child(self.render_audio_output_draft(cx))
                .child(self.render_replay_gain_settings(true, cx))
                .into_any_element(),
            crate::app::SettingsTab::Misc => {
                self.render_plugins_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::Federation => self
                .render_federation_settings_content(cx)
                .into_any_element(),
            crate::app::SettingsTab::Servers => {
                self.render_servers_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::Metadata => {
                self.render_metadata_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::ReleaseChannel => self
                .render_release_channel_settings_content(cx)
                .into_any_element(),
        }
    }

    /// Clear all EQ plugins from the playback chain.
    /// Shared by spinorama_eq and headphone_eq workflows.
    pub fn clear_eq_from_playback(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            let plugins = state.app.plugin_state.graph.plugins();
            let eq_indices: Vec<_> = plugins
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    if matches!(p.plugin_type(), sotf_audio_player::PluginType::EQ) {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect();

            for idx in eq_indices.into_iter().rev() {
                state
                    .app
                    .plugin_state
                    .graph
                    .remove_plugin_by_index(idx)
                    .ok();
            }

            state.app.plugin_state.update_state.pending_plugin_update =
                Some(PluginUpdateType::Structural);
            state.app.sync_spectrum_visible();
            state.app.ui_state.toast_message = Some(crate::app::ToastMessage::success(
                "Cleared EQ from playback",
            ));
        });
        cx.notify();
    }

    /// Render the "Apply to Playback" card used in export steps.
    /// `apply_fn` and `clear_fn` are method pointers for applying/clearing EQ.
    pub(crate) fn render_correction_export_status(
        &self,
        cx: &Context<Self>,
        delivery: &sotf_audio_player::ui_models::correction_delivery::CorrectionDelivery,
        result_is_current: bool,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = &state.app.ui_state.theme;
        let text = crate::app::i18n::CorrectionDeliveryTranslations::for_language(
            state.app.ui_state.language,
        );
        let current = delivery.current_result_exported(result_is_current);
        let description = match delivery.last_export() {
            None => text.not_exported,
            Some(_) if current => text.current_export,
            Some(_) => text.previous_export,
        };
        use sotf_audio_player::ui_models::correction_delivery::CorrectionApplicationStatus;
        let application_text = crate::app::i18n::CorrectionApplicationTranslations::for_language(
            state.app.ui_state.language,
        );
        let application = state
            .app
            .correction_application_status(delivery, result_is_current);
        let application_label = match application {
            CorrectionApplicationStatus::NotApplied => application_text.not_applied,
            CorrectionApplicationStatus::Pending => application_text.pending,
            CorrectionApplicationStatus::Applied => application_text.applied,
            CorrectionApplicationStatus::PreviousResult => application_text.previous,
            CorrectionApplicationStatus::ChangedGraph => application_text.changed,
            CorrectionApplicationStatus::Failed => application_text.failed,
        };
        let application_status = VStack::new()
            .spacing(StackSpacing::Xs)
            .child(Text::section_header(application_text.title))
            .child(Text::body(application_label).color(
                if application == CorrectionApplicationStatus::Applied {
                    theme.success
                } else if application == CorrectionApplicationStatus::Failed {
                    theme.error
                } else {
                    theme.text_secondary
                },
            ));
        #[cfg(feature = "dev-api")]
        let application_status = {
            use crate::app::dev_api::DevTrackExt;
            application_status.dev_track("correction.application_status")
        };
        let status = VStack::new()
            .spacing(StackSpacing::Sm)
            .child(Text::section_header(text.title))
            .child(Text::body(description).color(if current {
                theme.success
            } else {
                theme.text_secondary
            }))
            .when_some(delivery.last_export(), |stack, export| {
                stack.child(
                    div().w_full().min_w_0().whitespace_normal().child(
                        Text::caption(format!("{} · {}", export.format, export.path.display()))
                            .color(theme.text_secondary),
                    ),
                )
            });
        #[cfg(feature = "dev-api")]
        let status = {
            use crate::app::dev_api::DevTrackExt;
            status.dev_track("correction.export_status")
        };
        VStack::new()
            .spacing(StackSpacing::Sm)
            .child(application_status)
            .child(status)
    }

    pub(crate) fn render_apply_to_playback_card(
        &self,
        cx: &mut Context<Self>,
        id_prefix: &str,
        result_is_current: bool,
        theme: &crate::theme::Theme,
        button_theme: &ButtonTheme,
        apply_fn: fn(&mut Self, &mut Context<Self>),
        clear_fn: fn(&mut Self, &mut Context<Self>),
    ) -> Card {
        let text =
            PlaybackApplyTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let apply_id = SharedString::from(format!("apply-{}-eq", id_prefix));
        let clear_id = SharedString::from(format!("clear-{}-eq", id_prefix));

        let apply_button = Button::new(apply_id, text.title)
            .disabled(!result_is_current)
            .variant(ButtonVariant::Primary)
            .size(ButtonSize::Sm)
            .theme(button_theme.clone())
            .on_click_event(cx.listener(move |view, _, _, cx| {
                apply_fn(view, cx);
            }));
        #[cfg(feature = "dev-api")]
        let apply_button = {
            use crate::app::dev_api::DevTrackExt;
            apply_button.dev_track(format!("{id_prefix}.apply"))
        };

        Card::new()
            .background(theme.surface)
            .header_background(theme.background_secondary)
            .border(theme.border)
            .header(
                Text::new(text.title)
                    .color(theme.text_primary)
                    .weight(TextWeight::Semibold),
            )
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new(text.description)
                            .size(TextSize::Xs)
                            .color(theme.text_secondary),
                    )
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Xs)
                            .child(apply_button)
                            .child(
                                Button::new(clear_id, text.clear_eq)
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Sm)
                                    .theme(button_theme.clone())
                                    .on_click_event(cx.listener(move |view, _, _, cx| {
                                        clear_fn(view, cx);
                                    })),
                            ),
                    ),
            )
    }
}
