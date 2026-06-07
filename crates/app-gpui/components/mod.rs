// Screen rendering modules
//

pub mod autoeq;
pub mod design;
pub mod dialogs;
pub mod graphs;
pub mod headphone_eq;
pub mod home;
pub mod icons;
pub mod migration;
pub mod plugins;
pub mod recording;
pub mod room_eq;
mod settings;
mod spinorama_eq;
pub mod streams;
pub use plugins::{
    LevelMeterElement, MeterColors, MeterData, SpectrumColors, SpectrumElement, get_param_count,
    render_plugin_content,
};

use crate::app::types::PluginUpdateType;
use crate::app::{Screen, SettingsTab};
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName, IconSize};
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
        SettingsTab::Servers => IconName::Plug,
        SettingsTab::ReleaseChannel => IconName::AudioWaveform,
    }
}

pub fn settings_tab_label(tab: SettingsTab, translations: &Translations) -> &'static str {
    match tab {
        SettingsTab::Library => translations.settings_tab_library,
        SettingsTab::Theme => "Appearance",
        SettingsTab::Language => translations.settings_tab_language,
        SettingsTab::Keybindings => translations.settings_tab_keybindings,
        SettingsTab::AudioDevice => translations.settings_tab_audio_device,
        SettingsTab::Misc => "Resources",
        SettingsTab::Federation => translations.settings_tab_federation,
        SettingsTab::Servers => translations.settings_tab_servers,
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
pub(crate) fn themed_tooltip(text: &'static str, theme: &Theme, cx: &mut App) -> AnyView {
    cx.new(|_| ThemedTooltip {
        text: text.into(),
        bg: theme.surface,
        border: theme.border,
        text_color: theme.text_primary,
    })
    .into()
}

impl PlayerView {
    pub(crate) fn render_settings_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
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
            crate::app::SettingsTab::Misc => {
                self.render_plugins_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::Federation => self
                .render_federation_settings_content(cx)
                .into_any_element(),
            crate::app::SettingsTab::Servers => {
                self.render_servers_settings_content(cx).into_any_element()
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
            .text_color(theme.text_primary)
            .child(
                // Tab Header with Home button on left, tabs centered
                div()
                    .w_full()
                    .bg(theme.surface)
                    .border_b_1()
                    .border_color(theme.border)
                    .px(d.card)
                    .py(d.pad_y)
                    .flex()
                    .items_center()
                    .gap(d.gap)
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
                            .rounded(d.r_md)
                            .border_1()
                            .border_color(theme.border)
                            .hover(move |s| s.bg(theme.surface_hover))
                            .child(
                                Icon::new(IconName::Home)
                                    .size(IconSize::Sm)
                                    .color(text_muted),
                            )
                            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                state_for_home.update(cx, |state, _cx| {
                                    state.app.set_screen(Screen::NowPlaying, "SettingsHome");
                                });
                            }),
                    )
                    // Centered tabs (flex-1 with centered content)
                    .child(div().flex_1().min_w_0().child({
                        // Custom tab rendering to avoid context issues
                        let state_entity = self.state.clone();
                        let tab_data = [
                            SettingsTab::Library,
                            SettingsTab::Theme,
                            SettingsTab::Language,
                            SettingsTab::Keybindings,
                            SettingsTab::AudioDevice,
                            SettingsTab::Misc,
                            SettingsTab::Federation,
                            SettingsTab::Servers,
                            SettingsTab::ReleaseChannel,
                        ];

                        let mut tabs_container = div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .justify_center()
                            .gap(d.grid);

                        for tab_variant in tab_data {
                            let label = settings_tab_label(tab_variant, &translations);
                            let is_selected = active_tab == tab_variant;
                            let entity_clone = state_entity.clone();
                            let accent = theme.accent;
                            let border = theme.border;
                            let surface = theme.surface;
                            let surface_hover = theme.surface_hover;
                            let surface_selected = theme.surface_selected;
                            let text_selected = theme.text_primary;
                            let text_unselected = theme.text_muted;
                            let text_hover = text_secondary;
                            let icon_color = if is_selected {
                                theme.accent
                            } else {
                                text_unselected
                            };
                            let icon_name = settings_tab_icon_name(tab_variant);

                            let tab = div()
                                .id(SharedString::from(format!(
                                    "settings-tab-{:?}",
                                    tab_variant
                                )))
                                .flex()
                                .items_center()
                                .gap(d.grid)
                                .flex_shrink_0()
                                .px(d.pad_x)
                                .py(d.pad_y_half)
                                .rounded(d.r_md)
                                .border_1()
                                .border_color(if is_selected { accent } else { border })
                                .bg(if is_selected {
                                    surface_selected
                                } else {
                                    surface
                                })
                                .text_size(d.text_sm)
                                .text_color(if is_selected {
                                    text_selected
                                } else {
                                    text_unselected
                                })
                                .whitespace_nowrap()
                                .cursor_pointer()
                                .child(Icon::new(icon_name).size(IconSize::Xs).color(icon_color))
                                .child(label)
                                .when(!is_selected, |el| {
                                    el.hover(move |s| s.bg(surface_hover).text_color(text_hover))
                                })
                                .when(is_selected, |el| el.font_weight(FontWeight::SEMIBOLD))
                                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                    entity_clone.update(cx, |state, _cx| {
                                        state.app.ui_state.active_settings_tab = tab_variant;
                                    });
                                });

                            #[cfg(feature = "dev-api")]
                            let tab = {
                                use crate::app::dev_api::DevTrackExt;
                                tab.dev_track(format!("settings.tab.{:?}", tab_variant))
                            };

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
                    .p(d.card)
                    .child(div().w_full().max_w(rems(78.0)).child(content)),
            )
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
                        Text::new("Apply the EQ to your current playback to hear the difference.")
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
                                    .on_click_event(cx.listener(move |view, _, _, cx| {
                                        apply_fn(view, cx);
                                    })),
                            )
                            .child(
                                Button::new(clear_id, "Clear EQ")
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
