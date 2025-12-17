// Screen rendering modules
//

pub mod dialogs;
pub mod graphs;
pub mod headphone_eq;
pub mod home;
pub mod icons;
pub mod recording;
mod room_eq;
mod settings;
mod speaker_diy;
mod spinorama_eq;

// Level meter and spectrum types are now in crate::plugins module
pub use crate::plugins::{
    LevelMeterElement, MeterColors, MeterData, SpectrumColors, SpectrumElement, get_param_count,
    render_plugin_content,
};

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{HStack, StackJustify, StackSpacing};

impl PlayerView {
    pub(crate) fn render_settings_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let active_tab = state.app.active_settings_tab;
        let translations = state.app.translations.clone();

        // Content area based on active tab
        let content = match active_tab {
            crate::app::SettingsTab::Library => {
                self.render_library_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::Appearance => self
                .render_appearance_settings_content(cx)
                .into_any_element(),
            crate::app::SettingsTab::AudioDevice => self
                .render_audio_device_settings_content(cx)
                .into_any_element(),
        };

        // Tabs are now custom-rendered to avoid context issues

        div()
            .id("settings-screen")
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .child(
                // Tab Header
                div().w_full().bg(theme.surface).px_4().pt_2().child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .justify(StackJustify::Center)
                        .child({
                            // Custom tab rendering to avoid context issues
                            let state_entity = self.state.clone();
                            let tab_data = [
                                (
                                    translations.settings_tab_library,
                                    crate::app::SettingsTab::Library,
                                ),
                                (
                                    translations.settings_tab_appearance,
                                    crate::app::SettingsTab::Appearance,
                                ),
                                (
                                    translations.settings_tab_audio_device,
                                    crate::app::SettingsTab::AudioDevice,
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
                                let text_hover = theme.text_secondary;

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
                                            .when(is_selected, |d| {
                                                d.font_weight(FontWeight::SEMIBOLD)
                                            })
                                            .child(label),
                                    )
                                    .child(
                                        div()
                                            .h(if is_selected { px(2.0) } else { px(1.0) })
                                            .w_full()
                                            .bg(if is_selected { accent } else { border }),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        move |_event, _window, cx| {
                                            entity_clone.update(cx, |state, _cx| {
                                                state.app.active_settings_tab = tab_variant;
                                            });
                                        },
                                    );

                                tabs_container = tabs_container.child(tab);
                            }

                            tabs_container
                        }),
                ),
            )
            // Content
            .child(
                div()
                    // .overflow_y_scroll()
                    .flex_1()
                    .p_4()
                    .child(content),
            )
    }
}
