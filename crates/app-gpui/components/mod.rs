// Screen rendering modules
//

pub mod dialogs;
pub mod graphs;
pub mod headphone_eq;
pub mod home;
pub mod icons;
pub mod migration;
pub mod plugins;
pub mod recording;
mod room_eq;
mod settings;
mod spinorama_eq;
pub use plugins::{
    LevelMeterElement, MeterColors, MeterData, SpectrumColors, SpectrumElement, get_param_count,
    render_plugin_content,
};

use crate::app::Screen;
use crate::app::types::PluginUpdateType;
use crate::components::icons::{Icon, IconName};
use crate::components::plugins::editing::PluginEditingManager;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonTheme, ButtonVariant, Card, HStack, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};

impl PlayerView {
    pub(crate) fn render_settings_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let active_tab = state.app.ui_state.active_settings_tab;
        let translations = state.app.ui_state.translations.clone();

        // Content area based on active tab
        let content = match active_tab {
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
            crate::app::SettingsTab::AudioDevice => self
                .render_audio_device_settings_content(cx)
                .into_any_element(),
            crate::app::SettingsTab::Plugins => {
                self.render_plugins_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::ReleaseChannel => self
                .render_release_channel_settings_content(cx)
                .into_any_element(),
        };

        // Tabs are now custom-rendered to avoid context issues

        // Clone state for home button click handler
        let state_for_home = self.state.clone();
        let text_muted = theme.text_muted;
        let text_secondary = theme.text_secondary;

        div()
            .id("settings-screen")
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .child(
                // Tab Header with Home button on left, tabs centered
                div()
                    .w_full()
                    .bg(theme.surface)
                    .px_4()
                    .pt_2()
                    .flex()
                    .items_center()
                    // Home button on the left
                    .child(
                        div()
                            .id("settings-home-button")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(rems(2.5))
                            .h(rems(2.0))
                            .cursor_pointer()
                            .rounded_md()
                            .hover(move |s| s.bg(theme.surface_hover))
                            .child(Icon::new(IconName::Home).color(text_muted))
                            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                state_for_home.update(cx, |state, _cx| {
                                    state.app.ui_state.current_screen = Screen::Library;
                                });
                            }),
                    )
                    // Centered tabs (flex-1 with centered content)
                    .child(div().flex_1().flex().justify_center().child({
                        // Custom tab rendering to avoid context issues
                        let state_entity = self.state.clone();
                        let tab_data = [
                            (
                                translations.settings_tab_library,
                                crate::app::SettingsTab::Library,
                            ),
                            (
                                translations.settings_tab_theme,
                                crate::app::SettingsTab::Theme,
                            ),
                            (
                                translations.settings_tab_language,
                                crate::app::SettingsTab::Language,
                            ),
                            (
                                translations.settings_tab_keybindings,
                                crate::app::SettingsTab::Keybindings,
                            ),
                            (
                                translations.settings_tab_audio_device,
                                crate::app::SettingsTab::AudioDevice,
                            ),
                            (
                                translations.settings_tab_plugins,
                                crate::app::SettingsTab::Plugins,
                            ),
                            (
                                translations.settings_tab_release_channel,
                                crate::app::SettingsTab::ReleaseChannel,
                            ),
                        ];

                        let mut tabs_container = div().flex().items_center();

                        for (label, tab_variant) in tab_data {
                            let is_selected = active_tab == tab_variant;
                            let entity_clone = state_entity.clone();
                            let accent = theme.accent;
                            let border = theme.border;
                            let text_selected = theme.text_primary;
                            let text_unselected = theme.text_muted;
                            let text_hover = text_secondary;

                            let tab = div()
                                .id(SharedString::from(format!(
                                    "settings-tab-{:?}",
                                    tab_variant
                                )))
                                .flex()
                                .flex_col()
                                .cursor_pointer()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .px_4()
                                        .py_2()
                                        .text_sm()
                                        .text_color(if is_selected {
                                            text_selected
                                        } else {
                                            text_unselected
                                        })
                                        .when(!is_selected, |d| {
                                            d.hover(move |s| s.text_color(text_hover))
                                        })
                                        .when(is_selected, |d| d.font_weight(FontWeight::SEMIBOLD))
                                        .child(label),
                                )
                                .child(
                                    div()
                                        .h(if is_selected { px(2.0) } else { px(1.0) })
                                        .w_full()
                                        .bg(if is_selected { accent } else { border }),
                                )
                                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                    entity_clone.update(cx, |state, _cx| {
                                        state.app.ui_state.active_settings_tab = tab_variant;
                                    });
                                });

                            tabs_container = tabs_container.child(tab);
                        }

                        tabs_container
                    }))
                    // Spacer on the right to balance the home button
                    .child(div().w(rems(2.5))),
            )
            // Content with vertical scroll
            .child(
                div()
                    .id("settings-content-scroll")
                    .overflow_y_scroll()
                    .flex_1()
                    .p_4()
                    .child(content),
            )
    }

    /// Clear all EQ plugins from the playback chain.
    /// Shared by spinorama_eq and headphone_eq workflows.
    pub fn clear_eq_from_playback(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            let plugins = state.app.plugin_state.chain.plugins();
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
                state.app.plugin_state.chain.remove_plugin(idx);
            }

            state.app.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
            state.app.sync_spectrum_visible();
            state.app.ui_state.toast_message = Some(crate::app::ToastMessage::success(
                "Cleared EQ from playback",
            ));
        });
        cx.notify();
    }

    /// Render the "Apply to Playback" card used in export steps.
    /// `apply_fn` and `clear_fn` are method pointers for applying/clearing EQ.
    pub(crate) fn render_apply_to_playback_card(
        &self,
        cx: &mut Context<Self>,
        id_prefix: &str,
        theme: &crate::theme::Theme,
        button_theme: &ButtonTheme,
        apply_fn: fn(&mut Self, &mut Context<Self>),
        clear_fn: fn(&mut Self, &mut Context<Self>),
    ) -> Card {
        let apply_id = SharedString::from(format!("apply-{}-eq", id_prefix));
        let clear_id = SharedString::from(format!("clear-{}-eq", id_prefix));

        Card::new()
            .background(theme.surface)
            .header_background(theme.background_secondary)
            .border(theme.border)
            .header(
                Text::new("Apply to Playback")
                    .color(theme.text_primary)
                    .weight(TextWeight::Semibold),
            )
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new(
                            "Apply the EQ to your current playback to hear the difference.",
                        )
                        .size(TextSize::Xs)
                        .color(theme.text_secondary),
                    )
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Xs)
                            .child(
                                Button::new(apply_id, "Apply to Playback")
                                    .variant(ButtonVariant::Primary)
                                    .size(ButtonSize::Sm)
                                    .theme(button_theme.clone())
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(move |view, _, _, cx| {
                                            apply_fn(view, cx);
                                        }),
                                    ),
                            )
                            .child(
                                Button::new(clear_id, "Clear EQ")
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Sm)
                                    .theme(button_theme.clone())
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(move |view, _, _, cx| {
                                            clear_fn(view, cx);
                                        }),
                                    ),
                            ),
                    ),
            )
    }
}
